//! Timing for the paths that run every sim tick or every frame, to find what
//! slows matches down by measurement rather than by reading code.
//!
//! Turn [`ENABLED`] off before a release. While it is off, every [`Probe`] and
//! [`Timed`] item compiles down to the call it wraps.
//!
//! While it is on, one block is appended to `riot-items.log` next to the DLL
//! every [`REPORT_SECONDS`] of wall time in which a match was simulating: for
//! each section, how often it ran, how much time it took per wall-clock second,
//! and its average and slowest call; then the items that cost the most. Times
//! are inclusive: an item hook that deals damage also contains every hook that
//! damage sets off, and `frame` contains the `frame:` rows under it. The one
//! row that counts nothing twice is `sim: mod total`, which times only the
//! outermost of the mod's calls on a sim thread ([`Probe::sim`]).
//!
//! `sim: whole tick` is the time from one match tick hook to the next for the
//! same match on the same thread, so the engine's tick with every mod's hooks
//! in it. A match the player watches is paced by the frame rate, and those
//! waits are kept apart as `sim: tick gap` ([`TICK_GAP_NANOS`]).
//!
//! Counts are gathered per thread and folded into the shared totals every
//! [`FLUSH_CALLS`] calls, at once for a call slower than [`FLUSH_NANOS`], and
//! from the match hook every [`FLUSH_TICKS`] ticks. The buy detour runs on
//! every rayon worker at tens of thousands of calls a second, and a shared
//! atomic per call would contend on one cache line and slow down the thing
//! being measured.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use mod_api_stable::*;

pub const ENABLED: bool = false;

const REPORT_SECONDS: f64 = 5.0;
const FLUSH_CALLS: u64 = 256;
const FLUSH_NANOS: u64 = 1_000_000;
const FLUSH_TICKS: usize = 64;
/// A wait between two ticks of one match longer than this is the frame rate
/// pacing a watched match (or a stall), not the tick itself.
const TICK_GAP_NANOS: u64 = 2_000_000;
/// Items given rows of their own; any past this are timed by hook only.
const MAX_ITEMS: usize = 320;
/// Items listed by cost at the end of each block.
const TOP_ITEMS: usize = 20;

#[derive(Clone, Copy)]
pub enum Section {
    Frame,
    FrameTactics,
    FrameSoloRank,
    FrameItemStats,
    FrameChampionTraits,
    FrameItemStatsUi,
    FrameToolboxTab,
    FrameDraftWatch,
    FrameEditor,
    RecordIds,
    RecordRead,
    CaptureQueued,
    NativeBuildHook,
    NativeBuildGame,
    StableBuildHook,
    ScoreItem,
    SimTick,
    SimTickGap,
    SimTotal,
    MatchTick,
    MatchImmolate,
    MatchRiches,
    MatchCapture,
    BuyEarlyExit,
    BuyMemoHit,
    BuyFullPass,
    BuyMemoStored,
    BuyUnmemoizable,
    SpawnDetour,
    ItemUpdate,
    ItemOnAttack,
    ItemOnDamaged,
    ItemOnSkillHit,
    ItemOnKillAssist,
    ItemOther,
}

/// Row labels, in [`Section`] order.
const NAMES: [&str; SECTIONS] = [
    "frame (client, total)",
    "frame: tactics",
    "frame: solo rank ui",
    "frame: item stats sync",
    "frame: champion traits",
    "frame: item stats ui",
    "frame: toolbox tab",
    "frame: draft watch",
    "frame: strategy editor",
    "stats: record_ids",
    "stats: record read (full)",
    "stats: match captured (count)",
    "build hook (native)",
    "build hook: game's own fn",
    "build hook (stable)",
    "build hook: score_item",
    "sim: whole tick (engine+mods)",
    "sim: tick gap (paced/stalled)",
    "sim: mod total (outermost)",
    "match tick hook",
    "match tick: immolate",
    "match tick: shared riches",
    "match tick: item capture",
    "buy: early exit",
    "buy: memo hit",
    "buy: full pass",
    "buy: memo stored (count)",
    "buy: unmemoizable (count)",
    "spawn detour",
    "item update",
    "item on_attack",
    "item on_damaged",
    "item on_skill_hit",
    "item on_kill/assist",
    "item other hooks",
];

const SECTIONS: usize = Section::ItemOther as usize + 1;
/// Two per item after the sections: its `update`, then its other hooks.
const SLOTS: usize = SECTIONS + 2 * MAX_ITEMS;

#[derive(Clone, Copy)]
struct Acc {
    calls: u64,
    nanos: u64,
    max: u64,
}

impl Acc {
    const ZERO: Self = Self {
        calls: 0,
        nanos: 0,
        max: 0,
    };

    fn avg_us(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        self.nanos as f64 / self.calls as f64 / 1e3
    }
}

struct Total {
    calls: AtomicU64,
    nanos: AtomicU64,
    max: AtomicU64,
}

impl Total {
    const ZERO: Self = Self {
        calls: AtomicU64::new(0),
        nanos: AtomicU64::new(0),
        max: AtomicU64::new(0),
    };

    fn add(&self, acc: &Acc) {
        self.calls.fetch_add(acc.calls, Ordering::Relaxed);
        self.nanos.fetch_add(acc.nanos, Ordering::Relaxed);
        self.max.fetch_max(acc.max, Ordering::Relaxed);
    }

    fn take(&self) -> Acc {
        Acc {
            calls: self.calls.swap(0, Ordering::Relaxed),
            nanos: self.nanos.swap(0, Ordering::Relaxed),
            max: self.max.swap(0, Ordering::Relaxed),
        }
    }
}

static TOTALS: [Total; SLOTS] = [const { Total::ZERO }; SLOTS];

thread_local! {
    // `const` and no `Drop`, like `BUY_MEMO`: the buy and spawn detours run
    // this on the game's worker threads, where lazy TLS init and destructor
    // registration are best avoided.
    static LOCAL: RefCell<[Acc; SLOTS]> = const { RefCell::new([Acc::ZERO; SLOTS]) };
    /// How many [`Probe::sim`] probes are open on this thread.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// The last match tick this thread saw: (seed, tick, when).
    static LAST_TICK: Cell<Option<(u64, usize, Instant)>> = const { Cell::new(None) };
}

/// Item keys by registration order; item `i` is counted in slots
/// `SECTIONS + 2 * i` (its `update`) and the one after (its other hooks).
static ITEM_KEYS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// Adds one call of `nanos` to each of `slots`.
fn record(slots: [Option<usize>; 3], nanos: u64) {
    let _ = LOCAL.try_with(|local| {
        let Ok(mut local) = local.try_borrow_mut() else {
            return;
        };
        for slot in slots.into_iter().flatten() {
            let acc = &mut local[slot];
            acc.calls += 1;
            acc.nanos += nanos;
            acc.max = acc.max.max(nanos);
            if acc.calls >= FLUSH_CALLS || acc.nanos >= FLUSH_NANOS {
                TOTALS[slot].add(acc);
                *acc = Acc::ZERO;
            }
        }
    });
}

/// Folds this thread's unflushed counts into the totals.
fn flush_local() {
    let _ = LOCAL.try_with(|local| {
        let Ok(mut local) = local.try_borrow_mut() else {
            return;
        };
        for (slot, acc) in local.iter_mut().enumerate() {
            if acc.calls > 0 {
                TOTALS[slot].add(acc);
                *acc = Acc::ZERO;
            }
        }
    });
}

/// Counts one occurrence of `section`, with no time attached.
///
/// Straight into the totals rather than through the per-thread batch: counted
/// events are far rarer than timed calls.
pub fn count(section: Section) {
    if ENABLED {
        TOTALS[section as usize]
            .calls
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Called first thing in the match tick hook: times the tick that has just
/// ended, when this thread's previous call was the tick before of the same
/// match, and flushes this thread's counts now and then.
pub fn sim_tick(seed: u64, tick: usize, ended: bool) {
    if !ENABLED {
        return;
    }
    let now = Instant::now();
    let last = LAST_TICK
        .try_with(|last| last.replace(Some((seed, tick, now))))
        .ok()
        .flatten();
    if let Some((last_seed, last_tick, at)) = last {
        if last_seed == seed && last_tick + 1 == tick {
            let nanos = now.duration_since(at).as_nanos() as u64;
            let section = if nanos <= TICK_GAP_NANOS {
                Section::SimTick
            } else {
                Section::SimTickGap
            };
            record([Some(section as usize), None, None], nanos);
        }
    }
    if ended || tick % FLUSH_TICKS == 0 {
        flush_local();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    /// Not sim-thread work, or timing is off.
    Free,
    Nested,
    Outermost,
}

/// Times from its start until it is dropped, under whichever section it was
/// last [`set`](Probe::set) to, so a function with many exits can say which
/// kind of exit it took.
pub struct Probe {
    slot: usize,
    also: Option<usize>,
    scope: Scope,
    start: Option<Instant>,
}

impl Probe {
    #[inline]
    pub fn start(section: Section) -> Self {
        Self {
            slot: section as usize,
            also: None,
            scope: Scope::Free,
            start: ENABLED.then(Instant::now),
        }
    }

    /// [`start`](Probe::start) for work on a sim thread: the outermost such
    /// probe on a thread is counted under [`Section::SimTotal`] as well.
    #[inline]
    pub fn sim(section: Section) -> Self {
        let mut probe = Self::start(section);
        if ENABLED {
            probe.scope = DEPTH
                .try_with(|depth| {
                    let open = depth.get();
                    depth.set(open + 1);
                    if open == 0 {
                        Scope::Outermost
                    } else {
                        Scope::Nested
                    }
                })
                .unwrap_or(Scope::Free);
        }
        probe
    }

    #[inline]
    pub fn set(&mut self, section: Section) {
        self.slot = section as usize;
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let Some(start) = self.start else {
            return;
        };
        let nanos = start.elapsed().as_nanos() as u64;
        if self.scope != Scope::Free {
            let _ = DEPTH.try_with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
        let total = (self.scope == Scope::Outermost).then_some(Section::SimTotal as usize);
        record([Some(self.slot), self.also, total], nanos);
    }
}

/// Runs `f`, timed under `section`.
#[inline]
pub fn time<R>(section: Section, f: impl FnOnce() -> R) -> R {
    let _probe = Probe::start(section);
    f()
}

/// Writes a report if [`REPORT_SECONDS`] have passed since the last one. Called
/// once a frame from the client's `post_update`.
pub fn report_if_due() {
    if !ENABLED {
        return;
    }
    static WINDOW: Mutex<Option<(Instant, Instant)>> = Mutex::new(None);
    let now = Instant::now();
    let (session, seconds) = {
        let Ok(mut window) = WINDOW.lock() else {
            return;
        };
        let (session, started) = *window.get_or_insert((now, now));
        let seconds = now.duration_since(started).as_secs_f64();
        if seconds < REPORT_SECONDS {
            return;
        }
        *window = Some((session, now));
        (session, seconds)
    };
    flush_local();
    let totals: Vec<Acc> = TOTALS.iter().map(Total::take).collect();
    // Menus and the management screen are not what this is for.
    let simulating = totals[Section::MatchTick as usize].calls > 0
        || totals[Section::ItemUpdate as usize].calls > 0;
    if simulating {
        write_report(now.duration_since(session).as_secs_f64(), seconds, &totals);
    }
}

/// The log, opened (and truncated) on first use, so it only holds this game
/// launch.
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn with_file(write: impl FnOnce(&mut std::fs::File)) {
    let Ok(mut file) = FILE.lock() else {
        return;
    };
    if file.is_none() {
        *file = std::fs::File::create(crate::config::mod_dir().join("riot-items.log")).ok();
        if let Some(file) = file.as_mut() {
            let _ = writeln!(
                file,
                "riot_items_tfm2 {} perf probe, {REPORT_SECONDS}s windows",
                env!("CARGO_PKG_VERSION")
            );
        }
    }
    if let Some(file) = file.as_mut() {
        write(file);
    }
}

fn write_report(at: f64, seconds: f64, totals: &[Acc]) {
    let per_second = |value: u64| value as f64 / seconds;
    let of = |section: Section| &totals[section as usize];
    let ticks = of(Section::MatchTick).calls;
    let mut out = String::new();
    out.push_str(&format!(
        "[{at:8.1}s] {seconds:.1}s window | {:.1} frames/s | {:.0} match ticks/s (every simulating match) | {} matches captured\n",
        per_second(of(Section::Frame).calls),
        per_second(ticks),
        of(Section::CaptureQueued).calls,
    ));
    if ticks > 0 {
        let mod_us = of(Section::SimTotal).nanos as f64 / ticks as f64 / 1e3;
        let whole = of(Section::SimTick);
        if whole.calls > 0 {
            let tick_us = whole.avg_us();
            out.push_str(&format!(
                "  per match tick: mod {mod_us:.1} us, whole tick {tick_us:.1} us ({:.1}%)\n",
                mod_us / tick_us * 100.0
            ));
        } else {
            out.push_str(&format!(
                "  per match tick: mod {mod_us:.1} us, no unpaced tick to hold it against\n"
            ));
        }
    }
    out.push_str(&format!(
        "  {:<30} {:>10} {:>9} {:>9} {:>9}\n",
        "section", "calls/s", "ms/s", "avg us", "max us"
    ));
    for (name, acc) in NAMES.iter().zip(totals) {
        if acc.calls == 0 {
            continue;
        }
        out.push_str(&format!(
            "  {:<30} {:>10.1} {:>9.2} {:>9.1} {:>9.1}\n",
            name,
            per_second(acc.calls),
            per_second(acc.nanos) / 1e6,
            acc.avg_us(),
            acc.max as f64 / 1e3,
        ));
    }

    let keys = ITEM_KEYS
        .lock()
        .map(|keys| keys.clone())
        .unwrap_or_default();
    // (key, its `update`, its other hooks)
    let mut items: Vec<(&str, &Acc, &Acc)> = keys
        .iter()
        .zip(totals[SECTIONS..].chunks_exact(2))
        .filter(|(_, pair)| pair[0].calls + pair[1].calls > 0)
        .map(|(key, pair)| (*key, &pair[0], &pair[1]))
        .collect();
    items.sort_unstable_by_key(|(_, update, other)| std::cmp::Reverse(update.nanos + other.nanos));
    if !items.is_empty() {
        out.push_str(&format!(
            "  {:<34} {:>7} | {:>7} {:>9} {:>7} | {:>7} {:>9} {:>7} | {:>8}\n",
            "items by time",
            "ms/s",
            "update",
            "calls/s",
            "avg us",
            "hooks",
            "calls/s",
            "avg us",
            "max us"
        ));
        for (key, update, other) in items.into_iter().take(TOP_ITEMS) {
            out.push_str(&format!(
                "    {:<32} {:>7.2} | {:>7.2} {:>9.1} {:>7.2} | {:>7.2} {:>9.1} {:>7.2} | {:>8.1}\n",
                key,
                per_second(update.nanos + other.nanos) / 1e6,
                per_second(update.nanos) / 1e6,
                per_second(update.calls),
                update.avg_us(),
                per_second(other.nanos) / 1e6,
                per_second(other.calls),
                other.avg_us(),
                update.max.max(other.max) as f64 / 1e3,
            ));
        }
    }
    with_file(|file| {
        let _ = file.write_all(out.as_bytes());
        let _ = file.flush();
    });
}

/// An item with its hooks timed, both by hook kind and under its own key.
/// Derefs to the item, so what registration asks of it still reaches it.
///
/// It is also where an item balance patch reaches the item (`crate::patches`),
/// being the one thing every item registers in. The game copies the
/// registered item for each purchase; the first time any hook of a copy
/// runs, the copy is built again from its config with the patch in it. Once,
/// before the copy has done anything, so it holds no state to lose, and it
/// keeps those numbers for as long as it lives: a patch that lands does not
/// change an item somebody is holding.
#[derive(Clone)]
pub struct Timed<T> {
    inner: T,
    slot: Option<usize>,
    key: &'static str,
    /// The item's constructor from a config, tier and all.
    build: fn(&crate::config::ItemConfig) -> T,
    /// This copy has looked for its patch. Never set on the registered item,
    /// which no hook runs on, so every copy of it starts unset.
    patched: bool,
}

impl<T> Deref for Timed<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

/// Wraps `item` for timing under `key`. `build` is the constructor it was
/// made with, for a copy to be made again with a patched config.
pub fn timed<T: StableItem + Clone>(
    key: &'static str,
    item: T,
    build: fn(&crate::config::ItemConfig) -> T,
) -> Timed<T> {
    let slot = ENABLED
        .then(|| {
            let mut keys = ITEM_KEYS.lock().ok()?;
            (keys.len() < MAX_ITEMS).then(|| {
                keys.push(key);
                SECTIONS + 2 * (keys.len() - 1)
            })
        })
        .flatten();
    Timed {
        inner: item,
        slot,
        key,
        build,
        patched: false,
    }
}

impl<T> Timed<T> {
    /// Takes on the save's item balance patch, where it has one for this
    /// item. Out of line: it runs once in a copy's life.
    #[cold]
    fn adopt_patch(&mut self) {
        self.patched = true;
        if let Some(config) = crate::patches::live::config_for(self.key) {
            self.inner = (self.build)(&config);
        }
    }

    #[inline]
    fn hook<R>(&mut self, section: Section, f: impl FnOnce(&mut T) -> R) -> R {
        if !self.patched {
            self.adopt_patch();
        }
        let mut probe = Probe::sim(section);
        let other = !matches!(section, Section::ItemUpdate) as usize;
        probe.also = self.slot.map(|slot| slot + other);
        f(&mut self.inner)
    }
}

impl<T: StableItem + Clone> StableItem for Timed<T> {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.inner.key()
    }

    fn icon(&self) -> String {
        self.inner.icon()
    }

    fn price(&self) -> usize {
        self.inner.price()
    }

    fn tier(&self) -> usize {
        self.inner.tier()
    }

    fn stat(&self) -> BuffV1 {
        self.inner.stat()
    }

    fn next_tier(&self) -> Vec<String> {
        self.inner.next_tier()
    }

    fn previous_tier(&self) -> Vec<String> {
        self.inner.previous_tier()
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        self.inner.tags()
    }

    fn category(&self) -> ItemCategoryV1 {
        self.inner.category()
    }

    fn on_attack(
        &mut self,
        sim: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        self.hook(Section::ItemOnAttack, |item| {
            item.on_attack(
                sim,
                caster,
                target,
                damage,
                damage_type,
                attack_type,
                is_crit,
            )
        })
    }

    fn on_base_attack(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        player: usize,
        entity: usize,
    ) {
        self.hook(Section::ItemOther, |item| {
            item.on_base_attack(sim, rng_seed, player, entity)
        })
    }

    fn update(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize) {
        self.hook(Section::ItemUpdate, |item| {
            item.update(sim, rng_seed, player)
        })
    }

    fn on_spawn(&mut self, sim: &mut StableSim<'_>, player: usize) {
        self.hook(Section::ItemOther, |item| item.on_spawn(sim, player))
    }

    fn on_healed(
        &mut self,
        sim: &mut StableSim<'_>,
        caster: Option<usize>,
        entity: usize,
        heal: usize,
    ) {
        self.hook(Section::ItemOther, |item| {
            item.on_healed(sim, caster, entity, heal)
        })
    }

    fn on_damaged(
        &mut self,
        sim: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        self.hook(Section::ItemOnDamaged, |item| {
            item.on_damaged(
                sim,
                player,
                entity,
                attacker,
                damage,
                damage_type,
                attack_type,
                is_crit,
            )
        })
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        player: usize,
        entity: usize,
        victim: usize,
    ) {
        self.hook(Section::ItemOnKillAssist, |item| {
            item.on_kill(sim, rng_seed, player, entity, victim)
        })
    }

    fn on_assist(&mut self, sim: &mut StableSim<'_>, player: usize, entity: usize) {
        self.hook(Section::ItemOnKillAssist, |item| {
            item.on_assist(sim, player, entity)
        })
    }

    fn on_dead(&mut self, sim: &mut StableSim<'_>, player: usize) {
        self.hook(Section::ItemOther, |item| item.on_dead(sim, player))
    }

    fn on_cc(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize, caster: usize) {
        self.hook(Section::ItemOther, |item| {
            item.on_cc(sim, rng_seed, player, caster)
        })
    }

    fn on_skill_hit(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        self.hook(Section::ItemOnSkillHit, |item| {
            item.on_skill_hit(sim, rng_seed, caster, target, is_ally)
        })
    }

    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        self.inner.on_upgrade(next_key)
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        self.inner.on_upgraded_from(prev_key, carry)
    }
}
