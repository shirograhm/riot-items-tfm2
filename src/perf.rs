//! Timing for the paths that run every sim tick or every frame, to find what
//! slows matches down by measurement. Three rounds of lag fixes were chosen by
//! reading code (see the buy memo in `tactics`), and the slowdown is still
//! reported.
//!
//! Turn [`ENABLED`] off before a release. While it is off, every [`Probe`] and
//! [`Timed`] item compiles down to the call it wraps.
//!
//! While it is on, one block is appended to `riot-items.log` next to the DLL
//! every [`REPORT_SECONDS`] of wall time in which a match was simulating: for
//! each section, how often it ran, how much time it took per wall-clock second,
//! and its average and slowest call; then the items that cost the most. Times
//! are inclusive: an item hook that deals damage also contains every hook that
//! damage sets off, and `frame` contains the `frame:` rows under it.
//!
//! Counts are gathered per thread and folded into the shared totals every
//! [`FLUSH_CALLS`] calls, or at once for a call slower than [`FLUSH_NANOS`].
//! The buy detour runs on every rayon worker at tens of thousands of calls a
//! second, and a shared atomic per call would contend on one cache line and
//! slow down the thing being measured.

use std::cell::RefCell;
use std::io::Write;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use mod_api_stable::*;

pub const ENABLED: bool = true;

const REPORT_SECONDS: f64 = 5.0;
const FLUSH_CALLS: u64 = 256;
const FLUSH_NANOS: u64 = 1_000_000;
/// Items given a row of their own; any past this are timed by hook only.
const MAX_ITEMS: usize = 256;
/// Items listed by cost at the end of each block.
const TOP_ITEMS: usize = 12;

#[derive(Clone, Copy)]
pub enum Section {
    Frame,
    FrameTactics,
    FrameSoloRank,
    FrameItemStats,
    FrameChampionTraits,
    FrameItemStatsUi,
    FrameEditor,
    MatchTick,
    BuyEarlyExit,
    BuyMemoHit,
    BuyFullPass,
    BuyMemoStored,
    BuyUnmemoizable,
    BuyMissNoEntry,
    BuyMissChanged,
    SpawnDetour,
    NativeBuildHook,
    NativeBuildGame,
    StableBuildHook,
    ScoreItem,
    RecordIds,
    RecordRead,
    CaptureQueued,
    CaptureMatched,
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
    "frame: strategy editor",
    "match tick hook",
    "buy: early exit",
    "buy: memo hit",
    "buy: full pass",
    "buy: memo stored (count)",
    "buy: unmemoizable (count)",
    "buy: miss, none on thread",
    "buy: miss, inputs changed",
    "spawn detour",
    "build hook (native)",
    "build hook: game's own fn",
    "build hook (stable)",
    "build hook: score_item",
    "stats: record_ids",
    "stats: record read (full)",
    "stats: capture queued (count)",
    "stats: capture matched (count)",
    "item update",
    "item on_attack",
    "item on_damaged",
    "item on_skill_hit",
    "item on_kill/assist",
    "item other hooks",
];

const SECTIONS: usize = Section::ItemOther as usize + 1;
const SLOTS: usize = SECTIONS + MAX_ITEMS;

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
}

/// Item keys by registration order; item `i` is counted in slot `SECTIONS + i`.
static ITEM_KEYS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn record(slot: usize, nanos: u64) {
    let _ = LOCAL.try_with(|local| {
        let Ok(mut local) = local.try_borrow_mut() else {
            return;
        };
        let acc = &mut local[slot];
        acc.calls += 1;
        acc.nanos += nanos;
        acc.max = acc.max.max(nanos);
        if acc.calls >= FLUSH_CALLS || acc.nanos >= FLUSH_NANOS {
            TOTALS[slot].add(acc);
            *acc = Acc::ZERO;
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
pub fn count(section: Section) {
    count_n(section, 1);
}

/// Counts `n` occurrences of `section`, with no time attached.
///
/// Straight into the totals rather than through the per-thread batch: counted
/// events are far rarer than timed calls, and a thread that counts a few and
/// then goes quiet would otherwise hold them back from every report.
pub fn count_n(section: Section, n: u64) {
    if ENABLED && n > 0 {
        TOTALS[section as usize]
            .calls
            .fetch_add(n, Ordering::Relaxed);
    }
}

/// Times from [`Probe::start`] until the probe is dropped, under whichever
/// section it was last [`set`](Probe::set) to, so a function with many exits
/// can say which kind of exit it took.
pub struct Probe {
    slot: usize,
    start: Option<Instant>,
}

impl Probe {
    #[inline]
    pub fn start(section: Section) -> Self {
        Self {
            slot: section as usize,
            start: ENABLED.then(Instant::now),
        }
    }

    #[inline]
    pub fn set(&mut self, section: Section) {
        self.slot = section as usize;
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            record(self.slot, start.elapsed().as_nanos() as u64);
        }
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
    }
    if let Some(file) = file.as_mut() {
        write(file);
    }
}

/// Notes kept per key, so a note written from a hot path cannot flood the log.
const NOTES_PER_KEY: u32 = 50;

/// Writes one diagnostic line under `key`, at most [`NOTES_PER_KEY`] times per
/// key per launch. `text` is only built for lines that are written.
pub fn note(key: &'static str, text: impl FnOnce() -> String) {
    if !ENABLED {
        return;
    }
    static WRITTEN: Mutex<Vec<(&str, u32)>> = Mutex::new(Vec::new());
    {
        let Ok(mut written) = WRITTEN.lock() else {
            return;
        };
        let index = match written.iter().position(|(known, _)| *known == key) {
            Some(index) => index,
            None => {
                written.push((key, 0));
                written.len() - 1
            }
        };
        if written[index].1 >= NOTES_PER_KEY {
            return;
        }
        written[index].1 += 1;
    }
    let line = format!("  note {key}: {}\n", text());
    with_file(|file| {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    });
}

fn write_report(at: f64, seconds: f64, totals: &[Acc]) {
    let per_second = |value: u64| value as f64 / seconds;
    let mut out = String::new();
    out.push_str(&format!(
        "[{at:8.1}s] {seconds:.1}s window | {:.1} frames/s | {:.0} match ticks/s (every simulating match)\n",
        per_second(totals[Section::Frame as usize].calls),
        per_second(totals[Section::MatchTick as usize].calls),
    ));
    out.push_str(&format!(
        "  {:<28} {:>10} {:>9} {:>9} {:>9}\n",
        "section", "calls/s", "ms/s", "avg us", "max us"
    ));
    for (name, acc) in NAMES.iter().zip(totals) {
        if acc.calls == 0 {
            continue;
        }
        out.push_str(&format!(
            "  {:<28} {:>10.1} {:>9.2} {:>9.1} {:>9.1}\n",
            name,
            per_second(acc.calls),
            per_second(acc.nanos) / 1e6,
            acc.nanos as f64 / acc.calls as f64 / 1e3,
            acc.max as f64 / 1e3,
        ));
    }

    let keys = ITEM_KEYS.lock().map(|keys| keys.clone()).unwrap_or_default();
    let mut items: Vec<(&str, &Acc)> = keys
        .iter()
        .zip(&totals[SECTIONS..])
        .filter(|(_, acc)| acc.calls > 0)
        .map(|(key, acc)| (*key, acc))
        .collect();
    items.sort_unstable_by(|a, b| b.1.nanos.cmp(&a.1.nanos));
    if !items.is_empty() {
        out.push_str("  items by time (ms/s, calls/s, max us):\n");
        for (key, acc) in items.into_iter().take(TOP_ITEMS) {
            out.push_str(&format!(
                "    {:<34} {:>7.2} {:>9.1} {:>9.1}\n",
                key,
                per_second(acc.nanos) / 1e6,
                per_second(acc.calls),
                acc.max as f64 / 1e3,
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
#[derive(Clone)]
pub struct Timed<T> {
    inner: T,
    slot: Option<usize>,
}

impl<T> Deref for Timed<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

/// Wraps `item` for timing under `key`.
pub fn timed<T: StableItem + Clone>(key: &'static str, item: T) -> Timed<T> {
    let slot = ENABLED
        .then(|| {
            let mut keys = ITEM_KEYS.lock().ok()?;
            (keys.len() < MAX_ITEMS).then(|| {
                keys.push(key);
                SECTIONS + keys.len() - 1
            })
        })
        .flatten();
    Timed { inner: item, slot }
}

impl<T> Timed<T> {
    #[inline]
    fn hook<R>(&mut self, section: Section, f: impl FnOnce(&mut T) -> R) -> R {
        if !ENABLED {
            return f(&mut self.inner);
        }
        let start = Instant::now();
        let result = f(&mut self.inner);
        let nanos = start.elapsed().as_nanos() as u64;
        record(section as usize, nanos);
        if let Some(slot) = self.slot {
            record(slot, nanos);
        }
        result
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
            item.on_attack(sim, caster, target, damage, damage_type, attack_type, is_crit)
        })
    }

    fn on_base_attack(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize, entity: usize) {
        self.hook(Section::ItemOther, |item| {
            item.on_base_attack(sim, rng_seed, player, entity)
        })
    }

    fn update(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize) {
        self.hook(Section::ItemUpdate, |item| item.update(sim, rng_seed, player))
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
        self.hook(Section::ItemOther, |item| item.on_healed(sim, caster, entity, heal))
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
        self.hook(Section::ItemOnKillAssist, |item| item.on_assist(sim, player, entity))
    }

    fn on_dead(&mut self, sim: &mut StableSim<'_>, player: usize) {
        self.hook(Section::ItemOther, |item| item.on_dead(sim, player))
    }

    fn on_cc(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize, caster: usize) {
        self.hook(Section::ItemOther, |item| item.on_cc(sim, rng_seed, player, caster))
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
