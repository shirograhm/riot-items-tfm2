//! Per-item win/loss totals over the matches [`crate::item_stats_sim`] captured.
//!
//! # Where the numbers come from
//!
//! The loadouts come from the simulation, not from the match record. The
//! record's per-player `items` is the build the game *assigned*, not what the
//! champion finished holding — proven by its shape: every player carried exactly
//! 3 or 4 items, never fewer, and never a component. `StablePlayer::item_keys`
//! on the last tick is the real end state, and it hands back real item keys
//! rather than numbers indexing a table the API does not expose.
//!
//! So a match is counted once, when a record vouches for it, and the record
//! answers only what the simulation cannot: which patch it was played on, and
//! whether it was a league match at all.
//!
//! # What the record is used for
//!
//! Only two fields: `version`, which is the patch the match was played on and
//! what the patch filter groups by, and `seed`, which joins it to the loadout
//! [`crate::item_stats_sim`] captured from the simulation.
//!
//! Its per-player `items` is deliberately **not** read. That field holds the
//! build the game *assigned*, not what was finished with, and it is stored as
//! bare numbers indexing a table the API does not expose — which previously
//! meant inferring the whole id space from the order items are declared and
//! registered in. Taking the loadout from the sim instead makes both problems
//! disappear at once: real keys, and the real end state.
//!
//! # Why the totals are kept, and not the matches
//!
//! These counters **are** the stored history: a match is folded in once and its
//! loadouts are dropped. The alternative, keeping every match and re-folding them
//! whenever one was added, cost two passes over the whole save — the fold itself
//! and the file write that followed it — and both got slower the longer the save
//! was played, which is exactly backwards for a feature that only becomes useful
//! once a lot has been played. What is on disk is now proportional to the number
//! of items, patches and lanes, all of which are fixed.
//!
//! The trade is that nothing can be recomputed. A column added later starts
//! empty and fills from new matches, where a stored history could have answered
//! it at once — that is how the first-item rate was added retroactively. Worth
//! knowing before adding the next column.
//!
//! Records still cannot be trusted to persist or to keep their ids: the count was
//! seen going 126 -> 28 -> 77 inside one session, so the game prunes and recycles
//! them freely. Re-reading every record on every pass stays safe because a record
//! contributes no numbers, only a patch, and a match can be counted only once —
//! [`crate::item_stats_sim::take`] hands each one over exactly once and remembers
//! that it did.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use mod_api_stable::*;
use serde_json::Value;

/// Records read per [`pump`] call.
///
/// Two fields are wanted from each, but the whole record still crosses the ABI
/// and is parsed — ten players' match statistics included — and this runs on the
/// UI thread. A season is hundreds of matches, so reading the set in one frame
/// is a visible hitch; at 24 a frame a full pass costs a handful of frames.
const CHUNK: usize = 24;

#[derive(Clone, Copy, Default)]
pub(crate) struct Totals {
    pub games: u32,
    pub wins: u32,
    /// Games where this item was the one in the player's **first** item slot.
    ///
    /// "First" is slot order: `StablePlayer::item_keys` enumerates the player's
    /// items by index, and a champion's items are appended as they are
    /// completed, so slot 0 is the item they finished first. That is the closest
    /// thing to a purchase order the simulation exposes — there is no timestamp
    /// on an item — and it is the same order the assigned build is written in.
    pub firsts: u32,
}

impl Totals {
    pub fn losses(&self) -> u32 {
        self.games.saturating_sub(self.wins)
    }

    /// Win rate in percent, or `None` for an item with no games — which is not
    /// the same as 0% and must not print as it.
    pub fn win_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.wins as f64 * 100.0 / self.games as f64)
    }

    /// Share of this item's buys where it was bought first, in percent.
    ///
    /// Same `None`-for-no-games rule as [`Totals::win_rate`], and for the same
    /// reason: an item nobody has bought has no first-item rate, and printing
    /// 0.0% for it would claim it is never rushed.
    pub fn first_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.firsts as f64 * 100.0 / self.games as f64)
    }
}

#[derive(Default)]
struct Aggregate {
    /// Record ids still to read for their patch, newest first.
    pending: Vec<usize>,
    /// Whether the counters have been read back from disk yet.
    loaded: bool,
    /// Patch -> (lane, item) -> totals.
    ///
    /// Keyed by patch first so the filter is a map lookup rather than a re-scan:
    /// picking one reads its submap, and "All" merges them. The record's
    /// `version` is what a patch is here — it sits beside `seed` in the replay
    /// data, which is what a replay would need to reproduce the balance a match
    /// was played under.
    ///
    /// The lane rides in the inner key rather than adding a third level of map,
    /// so both filters are one pass over the same entries and "All" on either
    /// axis is the same merge with one term dropped.
    counts: BTreeMap<String, BTreeMap<(Option<usize>, String), Totals>>,
    /// Patch -> (lane, item) -> champion -> times that champion was holding it.
    /// Feeds the "purchased on" column, which is the top few of these by count.
    champions: BTreeMap<String, BTreeMap<(Option<usize>, String), BTreeMap<String, u32>>>,
    /// Patch -> captured matches.
    matches: BTreeMap<String, u32>,
}

static AGG: Mutex<Option<Aggregate>> = Mutex::new(None);

fn with_agg<T>(f: impl FnOnce(&mut Aggregate) -> T) -> Option<T> {
    let mut guard = AGG.lock().ok()?;
    let agg = guard.get_or_insert_with(Aggregate::default);
    if !agg.loaded {
        agg.loaded = true;
        load_into(agg);
    }
    Some(f(agg))
}

/// What the panel draws.
pub(crate) struct Snapshot {
    /// Items in display order — see [`rows`].
    pub rows: Vec<(String, Totals)>,
    pub matches: u32,
    /// Records still to read. Non-zero means a patch pass is in flight.
    pub pending: usize,
    /// Per item, the champions that bought it most, best first, at most
    /// [`TOP_CHAMPIONS`] of them.
    pub champions: BTreeMap<String, Vec<String>>,
}

/// How many champions the "purchased on" column shows.
///
/// Three, because that is what the vanilla "Most Used Champ" column shows and
/// the cell it borrows its shape from is 132px wide — three 40px slots and two
/// 4px gaps, with nothing left over.
pub(crate) const TOP_CHAMPIONS: usize = 3;

/// Queues every match record for a patch-backfill pass.
///
/// Every record, not just unseen ones. Record ids are **reused**: the count was
/// observed going 126 -> 28 -> 77 inside one session, so the game prunes and
/// recycles them, and "id 12 is already scanned" is not a fact that stays true.
/// Re-reading them all is what makes that harmless.
///
/// It is affordable because a record is now read for two fields and nothing is
/// folded from it — the totals come from the captures, which are deduplicated by
/// match seed and cannot be double counted however often a record is re-read.
pub(crate) fn sweep(ctx: &StableClient<'_>) {
    let ids = ctx.record_ids(RecordKindV1::MatchReplay);
    if ids.is_empty() {
        return;
    }
    let _ = with_agg(|agg| {
        agg.pending = ids.iter().rev().copied().collect();
    });
}

/// Reads a bounded batch of records to backfill patches, then re-folds the
/// totals if the captures have changed.
///
/// Records no longer contribute any numbers. They answer one question — which
/// patch was this match played on — and the answer is written onto the capture
/// so it survives the record being pruned.
pub(crate) fn pump(ctx: &StableClient<'_>) -> bool {
    let batch = with_agg(|agg| {
        let take = CHUNK.min(agg.pending.len());
        agg.pending.drain(..take).collect::<Vec<_>>()
    })
    .unwrap_or_default();

    let mut folded = false;
    for id in &batch {
        let Some((patch, seed)) = read_record(ctx, *id) else {
            continue;
        };
        // A seed with no capture waiting is either a match simmed before
        // capturing began — the loadout it would need does not exist, and the
        // record carries no usable substitute — or one already counted, which
        // `take` declines a second time. Both are nothing to do.
        let Some(players) = crate::item_stats_sim::take(seed) else {
            continue;
        };
        fold(&patch, &players);
        folded = true;
    }

    if folded {
        DIRTY.store(true, Ordering::Relaxed);
        return true;
    }

    let finished = with_agg(|agg| agg.pending.is_empty()).unwrap_or(false);
    !batch.is_empty() && finished
}

/// Folds one vouched match into the running totals.
///
/// Called once per match, ever. The counters it adds to are the stored history,
/// so nothing is recomputed and nothing is walked twice — which is the whole
/// point of keeping numbers rather than matches.
fn fold(patch: &str, players: &[crate::item_stats_sim::CapturedPlayer]) {
    let _ = with_agg(|agg| {
        *agg.matches.entry(patch.to_string()).or_default() += 1;

        let per_item = agg.counts.entry(patch.to_string()).or_default();
        for player in players {
            for (slot, key) in player.items.iter().enumerate() {
                let entry = per_item.entry((player.lane, key.clone())).or_default();
                entry.games += 1;
                entry.wins += u32::from(player.won);
                // Slot order is completion order, so the first slot is the item
                // this player rushed. There is no timestamp on an item; this is
                // the only purchase order the simulation exposes.
                entry.firsts += u32::from(slot == 0);
            }
        }

        let per_champion = agg.champions.entry(patch.to_string()).or_default();
        for player in players {
            // A player whose champion could not be read still counts toward the
            // item's games and wins — the loadout is real — but it must not be
            // tallied as a champion. It used to be, under the empty key, and
            // `top_champions` then ranked it like any other name: on an item
            // bought mostly by champions that were dead at the final tick, the
            // blank outranked every real name and took a column slot that then
            // drew nothing. That is the "played, but no portraits" case.
            if player.champion.is_empty() {
                continue;
            }
            for key in &player.items {
                *per_champion
                    .entry((player.lane, key.clone()))
                    .or_default()
                    .entry(player.champion.clone())
                    .or_default() += 1;
            }
        }
    });
}
/// The patches seen in the records, newest first.
///
/// Populated from the records themselves rather than from the game's own patch
/// list, which the stable API does not expose. That also makes it exactly the
/// right set: a patch nothing was played on has nothing to filter to.
pub(crate) fn patches() -> Vec<String> {
    with_agg(|agg| {
        let mut out: Vec<String> = agg.counts.keys().cloned().collect();
        out.sort_by(|a, b| b.cmp(a));
        out
    })
    .unwrap_or_default()
}

/// The current table, in key order, for one patch and lane or for all of them.
///
/// Deliberately *not* sorted for display: the column the player picked can be
/// the item's name, which lives in the catalog, so ordering is the UI's job.
/// Key order makes it a stable starting point, which is what keeps equal rows
/// from reshuffling between repaints mid-scan.
pub(crate) fn snapshot(patch: Option<&str>, lane: Option<usize>) -> Snapshot {
    with_agg(|agg| {
        // One patch reads its own submap; "All" merges them. Merging here rather
        // than keeping a second running total means there is one set of numbers
        // to be wrong, and it costs a walk of at most items x patches.
        let mut counts: BTreeMap<String, Totals> = BTreeMap::new();
        let mut champions: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut matches = 0;

        // A lane filter keeps only the entries recorded under it. A player the
        // host would not give a lane for is `None`, which no lane selection
        // matches — the same rule the category filter follows for an item with
        // no role, and the reason such a player still shows up under "All".
        let wanted_lane = |recorded: Option<usize>| lane.is_none_or(|want| recorded == Some(want));

        for (key, per_item) in &agg.counts {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for ((recorded, item), totals) in per_item {
                if !wanted_lane(*recorded) {
                    continue;
                }
                let entry = counts.entry(item.clone()).or_default();
                entry.games += totals.games;
                entry.wins += totals.wins;
                entry.firsts += totals.firsts;
            }
        }
        for (key, per_item) in &agg.champions {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for ((recorded, item), tally) in per_item {
                if !wanted_lane(*recorded) {
                    continue;
                }
                let entry = champions.entry(item.clone()).or_default();
                for (champion, count) in tally {
                    *entry.entry(champion.clone()).or_default() += count;
                }
            }
        }
        for (key, count) in &agg.matches {
            if patch.is_none_or(|wanted| wanted == key.as_str()) {
                matches += count;
            }
        }

        Snapshot {
            rows: rows(&counts),
            matches,
            pending: agg.pending.len(),
            champions: top_champions(&champions),
        }
    })
    .unwrap_or(Snapshot {
        rows: Vec::new(),
        matches: 0,
        pending: 0,
        champions: BTreeMap::new(),
    })
}

/// The most frequent champions per item, best first.
///
/// Ties break on the champion key so the three shown do not swap places between
/// repaints while the scan is still folding records.
fn top_champions(tally: &BTreeMap<String, BTreeMap<String, u32>>) -> BTreeMap<String, Vec<String>> {
    tally
        .iter()
        .map(|(item, champions)| {
            let mut ranked: Vec<(&String, &u32)> = champions.iter().collect();
            ranked.sort_by(|(a_key, a), (b_key, b)| b.cmp(a).then_with(|| a_key.cmp(b_key)));
            let top = ranked
                .into_iter()
                .take(TOP_CHAMPIONS)
                .map(|(champion, _)| champion.clone())
                .collect();
            (item.clone(), top)
        })
        .collect()
}

fn rows(counts: &BTreeMap<String, Totals>) -> Vec<(String, Totals)> {
    counts
        .iter()
        .map(|(key, totals)| (key.clone(), *totals))
        .collect()
}

/// One match as `(item keys, did that side win)`, one entry per side.
///
/// Two fields are wanted — the patch and the join key — but the whole record is
/// fetched in one call where the host allows it, because one round trip beats
/// two and the parse is the same either way.
fn read_record(ctx: &StableClient<'_>, id: usize) -> Option<(String, u64)> {
    let record = match ctx
        .record_get_json(RecordKindV1::MatchReplay, id, "")
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
    {
        Some(record) => record,
        // Older hosts, or a path grammar that does not accept the empty path for
        // this record kind. Named reads say the same thing, and are reassembled
        // into the same shape so that everything below has one case to handle
        // rather than two.
        None => {
            let field = |name: &str| {
                ctx.record_get_json(RecordKindV1::MatchReplay, id, name)
                    .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            };
            serde_json::json!({
                "version": field("version"),
                "seed": field("seed"),
            })
        }
    };

    // A league match always names its version, so one that does not is not a
    // record this table can place — skipped rather than filed under a catch-all
    // bucket that would only ever collect things that should not be there.
    let patch = record
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())?
        .to_string();

    // The seed joins this record to the loadout the simulation captured for the
    // same match. `serde_json` keeps integers as `u64`, so a match seed survives
    // the round trip that an `f64` would round off.
    let seed = record.get("seed").and_then(Value::as_u64)?;
    Some((patch, seed))
}

// -- item catalog -----------------------------------------------------------

/// Display name and sprite frame for one item key.
#[derive(Clone, Default)]
pub(crate) struct ItemInfo {
    pub name: String,
    /// `rect_tag` into the item sheet, or `None` for an item with no art.
    pub frame: Option<String>,
    /// 0..=4, which the tier filter reads as starter/basic/epic/legendary/
    /// radiant. `None` for an item neither the settings document nor this mod
    /// describes.
    pub tier: Option<usize>,
}

static CATALOG: Mutex<Option<BTreeMap<String, ItemInfo>>> = Mutex::new(None);

/// This mod's item keys, in the order `init` registered them — the second half
/// of the id space.
///
/// Recorded as they are registered rather than read back from anywhere, because
/// there is nowhere to read it from: `ItemSetting` contains only the 30 base
/// items (confirmed — the document parses to exactly 30 entries), so a mod's
/// items are absent from the one document that describes items at all.
static REGISTERED: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());

/// Notes one item key and its tier at registration time. Called from `init` for
/// every item the mod adds.
///
/// The tier has to come from here because the settings document describes only
/// the game's own items — a mod's are absent from the one place items are
/// described, so `StableItem::tier` at registration is the only source.
pub(crate) fn note_registered(key: &str, tier: usize) {
    if let Ok(mut keys) = REGISTERED.lock() {
        keys.push((key.to_string(), tier));
    }
}

/// Builds the item table from the settings document, once.
///
/// Unlike the build editor's list this keeps **every** item, not just finals: a
/// match record holds whatever a player was carrying when it ended, and a board
/// full of half-finished components is a normal way for a game to end. Filtering
/// them out here would silently drop games from the totals.
///
/// Split from [`catalog`] and called from `post_update` because
/// `setting_get_json` does **not** work inside a UI click handler — the trait
/// notes only ui/asset calls are live there, and the build editor already paid
/// for learning that. Building it lazily from the first repaint would mean the
/// first repaint is the one inside the click that opens the tab, which would
/// cache nothing, draw every row under its raw key, and then have no reason to
/// repaint again once the scan had finished.
pub(crate) fn prime_catalog(ctx: &StableClient<'_>) {
    if CATALOG
        .lock()
        .map(|cached| cached.is_some())
        .unwrap_or(false)
    {
        return;
    }

    let mut out = BTreeMap::new();
    if let Some(json) = ctx.setting_get_json(SettingTargetV1::ItemSetting, "") {
        if let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&json) {
            collect_items(ctx, &root, 0, &mut out);
        }
    }

    // An empty result is not cached: it means the document could not be read
    // this frame, which a later frame may well manage.
    if out.is_empty() {
        return;
    }

    // The settings document holds only the base items, so every one of this
    // mod's has to be described from what the mod itself knows: its name from
    // the merged item text, its sprite frame from its own key.
    //
    // The key *is* the frame — `StableItem::icon` returns it verbatim, and all
    // 153 registered keys have a byte-identical tag in the sheet. Resolving it
    // through `item_catalog::icon_frame` instead is wrong here, because that
    // takes a base slug: it strips `radiant_`, and the sheet stores the plain
    // and radiant art of an item as two separate tags, with the gold border
    // being what makes a radiant look radiant. So every one of the mod's 66
    // radiant items drew its own non-radiant twin.
    if let Ok(keys) = REGISTERED.lock() {
        for (key, tier) in keys.iter() {
            out.entry(key.clone()).or_insert_with(|| ItemInfo {
                name: display_name(ctx, key),
                frame: Some(key.clone()),
                tier: Some(*tier),
            });
        }
    }

    if let Ok(mut cached) = CATALOG.lock() {
        *cached = Some(out);
    }
}

/// The catalog as [`prime_catalog`] last left it, empty until it succeeds.
/// Takes no ctx, so it is safe to call from a click handler.
pub(crate) fn catalog() -> BTreeMap<String, ItemInfo> {
    CATALOG
        .lock()
        .ok()
        .and_then(|cached| cached.clone())
        .unwrap_or_default()
}

fn collect_items(
    ctx: &StableClient<'_>,
    map: &serde_json::Map<String, Value>,
    depth: usize,
    out: &mut BTreeMap<String, ItemInfo>,
) {
    for (key, value) in map {
        let Some(object) = value.as_object() else {
            continue;
        };
        let is_item = object.contains_key("next_tier")
            || object.contains_key("tier")
            || object.contains_key("price");
        if !is_item {
            // Two levels, not one: mod items sit under a per-mod bucket
            // (`mod_items.riot_items_tfm2.collector`).
            if depth < 2 {
                collect_items(ctx, object, depth + 1, out);
            }
            continue;
        }
        // Filed under the item's own `key` field, not the map key it sits
        // under. They are the same for all but one item — `iron_blade` calls
        // itself `ironsword` — and the inner one is the identity that matters:
        // it is what `StablePlayer::item_keys` returns and what the item text
        // document is keyed by. Using the map key left that row with a raw
        // `ironsword` and no icon.
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|inner| !inner.is_empty())
            .unwrap_or(key);
        out.insert(
            key.to_string(),
            ItemInfo {
                name: display_name(ctx, key),
                frame: icon_frame(object, key),
                tier: object
                    .get("tier")
                    .and_then(Value::as_u64)
                    .map(|tier| tier as usize),
            },
        );
    }
}

/// The item's own name, tier word included.
///
/// The build editor deliberately strips "Radiant" because every row in its list
/// is a final and the prefix distinguishes nothing. Here it distinguishes a
/// great deal: `infinity_edge` and `radiant_infinity_edge` are separate keys
/// with separate win rates, and two rows reading "Infinity Edge" would be a
/// table nobody could act on.
fn display_name(ctx: &StableClient<'_>, key: &str) -> String {
    ctx.i18n(&format!("#asset/base/text/item?{key}.name"))
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .unwrap_or_else(|| key.to_string())
}

/// The frame a base item draws from the (mod-overridden) item sheet.
///
/// The settings document's own `icon` is the authority: base items carry a
/// tier-slot name like `t5_0`, which the mod's sheet fills with its reskin of
/// that item — gold border included, since the game's tier-5 items are the ones
/// this mod presents as radiant.
///
/// The fallback is the key itself, never `base_slug`. Stripping `radiant_` picks
/// the plain twin of an item whose radiant art is a separate tag, which is
/// precisely the bug that made 66 items draw as non-radiant.
fn icon_frame(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get("icon")
        .and_then(Value::as_str)
        .filter(|icon| !icon.is_empty())
        .map(str::to_string)
        .or_else(|| Some(key.to_string()))
}

/// Set when the counters change, cleared when they reach disk.
static DIRTY: AtomicBool = AtomicBool::new(false);

/// Key the save's own id is kept under, inside this mod's save namespace.
///
/// The namespace belongs to the save file, so a value written here travels with
/// it and nothing else can see it. That is the whole trick: the id does not have
/// to be derived from anything the game exposes — there is no save name or handle
/// in the client API — because the save can simply be asked to hold one.
const SAVE_KEY: &str = "stats_save_id";

/// The save whose totals are loaded, or `None` before one has been adopted.
static SAVE: Mutex<Option<String>> = Mutex::new(None);

/// Folder holding every save's statistics, one subfolder each.
const STATS_DIR: &str = "item_stats";

/// This save's counters, inside its own folder.
const FILE: &str = "totals.json";

/// Where this save's statistics live: `item_stats/<save id>/`.
///
/// `None` until a save has been adopted, which is what keeps the main menu from
/// writing anything: there is nowhere to put it, and nothing to put there either,
/// since a match is only ever counted from the statistics screen.
///
/// Shared with [`crate::item_stats_sim`], which keeps its queue of uncounted
/// matches in the same folder — a save's captures belong to it as much as its
/// totals do, and keeping them together means one directory to copy or delete.
pub(crate) fn save_dir() -> Option<PathBuf> {
    let id = SAVE.lock().ok()?.clone()?;
    crate::config::dll_dir().map(|dir| dir.join(STATS_DIR).join(id))
}

/// The path to write one of this save's files to, folder created.
///
/// Reads go through [`save_dir`] instead: a missing folder is a save with nothing
/// recorded yet, which reads as an empty table rather than as an error, and there
/// is no reason for a read to leave a folder behind.
pub(crate) fn save_file(name: &str) -> Option<PathBuf> {
    let dir = save_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(name))
}

/// Points the totals at whichever save is loaded, and puts away the last one's.
///
/// Called every frame from the client's `post_update`, off the statistics screen
/// as well as on it — the numbers have to be pointed at the right save before
/// anything folds into them or flushes them, and neither of those happens here.
///
/// Cheap to call repeatedly: a save key read, and on the frame the save changes,
/// one write and one load.
///
/// **A new id is minted only when this process holds none.** The namespace is
/// not a reliable answer to "which save is this" on any given frame — it reads
/// empty mid-load, and the id it holds only reaches disk when the player saves —
/// so an empty read is treated as "ask again later", not as a new save. Only an
/// id that comes back *different* moves the totals to another folder.
pub(crate) fn adopt_save(ctx: &mut StableClient<'_>) {
    // The save namespace no-ops outside an active game, so this is also how the
    // main menu is recognised. The previous save's totals stay loaded there,
    // which is harmless: the tab that reads them cannot be reached.
    if !ctx.save_can_write() {
        return;
    }

    // Bound before the match rather than read in its scrutinee: a temporary there
    // lives to the end of the match, so the shared borrow of `ctx` would still be
    // alive when the arms below need it mutably to write the id back.
    let existing = ctx.save_get_string(SAVE_KEY).filter(|id| !id.is_empty());
    let adopted = SAVE.lock().ok().and_then(|current| current.clone());

    let id = match (existing, adopted) {
        // The save names itself and it is the one already loaded.
        (Some(id), Some(adopted)) if id == adopted => return,
        // The save names itself and it is a different one. Only an id that came
        // out of the namespace is proof of a switch, which is why this is the
        // one case that moves to another folder.
        (Some(id), _) => id,
        // Nothing in the namespace, but this process has already adopted an id.
        //
        // An empty namespace is NOT proof of a different save. It reads empty
        // while a save is still being loaded, and an id only reaches disk when
        // the player saves, so a restart before saving loses it too. Minting
        // here treated both as new saves and forked one save's history across
        // folders — three of them in one evening, with no match in common,
        // 44 minutes and then 20 seconds apart. Re-assert what is held instead:
        // if the namespace is real the id sticks, and if it is not, the write
        // goes nowhere and costs nothing.
        (None, Some(adopted)) => {
            let _ = ctx.save_set_string(SAVE_KEY, &adopted);
            return;
        }
        // Nothing anywhere: the first save this process has seen. The id is
        // arbitrary — it only has to differ from every other save's and be
        // usable in a file name.
        (None, None) => {
            let id = new_save_id();
            if !ctx.save_set_string(SAVE_KEY, &id) {
                return;
            }
            id
        }
    };

    // Order matters: both files are written into the outgoing save's folder, so
    // they have to go out before the id that names it is replaced. The queue is
    // forced rather than flushed, because its throttle may be sitting on a
    // deferred write and this is the last chance to take it.
    flush();
    crate::item_stats_sim::flush_now();
    let _ = with_agg(|agg| *agg = Aggregate::default());
    crate::item_stats_sim::forget();
    if let Ok(mut save) = SAVE.lock() {
        *save = Some(id);
    }
}

/// An id for a save that has none yet.
///
/// The clock rather than a random number: the mod has no RNG outside a
/// simulation, and two saves cannot be first seen in the same nanosecond.
fn new_save_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or(0);
    format!("{nanos:016x}")
}

/// The totals format this build writes and is willing to read.
///
/// A file that does not match is ignored rather than migrated, and that save's
/// table starts over. These counters cannot be recomputed from anything — the
/// matches behind them are long gone — so a shape change is the one case where
/// history is lost, and worth weighing before bumping this.
const FORMAT: u32 = 1;

/// Writes the counters out if they changed since the last call.
///
/// Driven from the management tick beside the queue's own flush. The file is a
/// few thousand rows whatever the save has been through, so unlike the history it
/// replaced this costs the same on the first match and the ten-thousandth.
pub(crate) fn flush() {
    if !DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    let Some(text) = with_agg(serialise) else {
        return;
    };
    let Some(path) = save_file(FILE) else {
        return;
    };
    // Written whole rather than appended: a partial append after a crash would
    // be a file that no longer parses, and these numbers cannot be re-derived.
    let _ = std::fs::write(path, text);
}

/// `{"v": 1, "t": {"<patch>": {"m": matches, "i": [entry, ...]}}}`, where an
/// entry is `{"n": lane, "k": item, "g": games, "w": wins, "f": firsts,
/// "c": {champion: count}}`.
///
/// The champion tally rides inside the entry rather than in a map of its own
/// because both are keyed by the same `(lane, item)` pair — every champion count
/// came from a player whose items were counted in the same pass, so the item map
/// is always a superset and there is no second key space to keep in step.
///
/// `n` is omitted for a player the host gave no lane for, and `c` when no champion
/// on the entry could be named.
fn serialise(agg: &mut Aggregate) -> String {
    let mut out = format!("{{\"v\":{FORMAT},\"t\":{{");
    for (slot, (patch, per_item)) in agg.counts.iter().enumerate() {
        if slot > 0 {
            out.push(',');
        }
        let matches = agg.matches.get(patch).copied().unwrap_or(0);
        out.push_str(&format!("{}:{{\"m\":{matches},\"i\":[", quote(patch)));
        let per_champion = agg.champions.get(patch);
        for (slot, ((lane, item), totals)) in per_item.iter().enumerate() {
            if slot > 0 {
                out.push(',');
            }
            out.push('{');
            if let Some(lane) = lane {
                out.push_str(&format!("\"n\":{lane},"));
            }
            out.push_str(&format!(
                "\"k\":{},\"g\":{},\"w\":{},\"f\":{}",
                quote(item),
                totals.games,
                totals.wins,
                totals.firsts
            ));
            let tally = per_champion.and_then(|per| per.get(&(*lane, item.clone())));
            if let Some(tally) = tally.filter(|tally| !tally.is_empty()) {
                out.push_str(",\"c\":{");
                for (slot, (champion, count)) in tally.iter().enumerate() {
                    if slot > 0 {
                        out.push(',');
                    }
                    out.push_str(&format!("{}:{count}", quote(champion)));
                }
                out.push('}');
            }
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("}}");
    out
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn load_into(agg: &mut Aggregate) {
    let Some(path) = save_dir().map(|dir| dir.join(FILE)) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(Value::Object(file)) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if file.get("v").and_then(Value::as_u64) != Some(FORMAT as u64) {
        return;
    }
    let Some(Value::Object(patches)) = file.get("t") else {
        return;
    };

    for (patch, body) in patches {
        let Some(body) = body.as_object() else {
            continue;
        };
        let matches = body.get("m").and_then(Value::as_u64).unwrap_or(0) as u32;
        let Some(Value::Array(entries)) = body.get("i") else {
            continue;
        };
        let per_item = agg.counts.entry(patch.clone()).or_default();
        let mut champions: BTreeMap<(Option<usize>, String), BTreeMap<String, u32>> =
            BTreeMap::new();
        for entry in entries {
            let Some(fields) = entry.as_object() else {
                continue;
            };
            let Some(item) = fields.get("k").and_then(Value::as_str) else {
                continue;
            };
            let lane = fields
                .get("n")
                .and_then(Value::as_u64)
                .map(|lane| lane as usize);
            let key = (lane, item.to_string());
            let read = |name: &str| fields.get(name).and_then(Value::as_u64).unwrap_or(0) as u32;
            per_item.insert(
                key.clone(),
                Totals {
                    games: read("g"),
                    wins: read("w"),
                    firsts: read("f"),
                },
            );
            if let Some(Value::Object(tally)) = fields.get("c") {
                let per_champion = champions.entry(key).or_default();
                for (champion, count) in tally {
                    let Some(count) = count.as_u64() else {
                        continue;
                    };
                    per_champion.insert(champion.clone(), count as u32);
                }
            }
        }
        agg.matches.insert(patch.clone(), matches);
        if !champions.is_empty() {
            agg.champions.insert(patch.clone(), champions);
        }
    }
}
