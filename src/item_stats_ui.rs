//! Item Stats: a fourth tab on the statistics screen, beside Champ/Player/Team.
//!
//! One row per item, ordered by win rate, over every match the game still holds
//! a record of. [`crate::item_stats`] owns the numbers; this file owns the tab,
//! the panel and the bookkeeping that keeps the two sides of the screen from
//! showing at once.
//!
//! # Why the tab lives in a layout override
//!
//! `ui/layout/statistics.ui` is an asset override (see `mod.override_info`): a
//! full copy of the game's layout with a fourth `#item` selectable in `#tabs`,
//! `#tabs` widened 704 -> 936 to fit it, and an `#item_stats` panel beside the
//! three vanilla ones in `#data`. Only the rows are built at runtime.
//!
//! The first version spawned all of it with `ui_spawn_source` instead, on the
//! reasoning that a bad override kills the whole screen —
//!
//! ```text
//! [main_ui] failed to create main tab: Statistics
//! ```
//!
//! — the way a missing fourth item slot killed Solorank, whereas a failed spawn
//! leaves the vanilla screen standing. That reasoning was sound and the
//! conclusion was still wrong, because it valued the wrong risk: **game code
//! rebuilds this screen's subtrees on its own tab switches**, and a spawned node
//! is not in the layout it rebuilds from. So the tab disappeared on the first
//! switch to Champ/Player/Team, and re-spawning it from [`heal`] traded that for
//! a visible flicker on every switch.
//!
//! A declared node has no such problem: the rebuild puts it back, because it is
//! part of what "rebuild" means. The exe's string table is what makes this safe
//! to declare — the runner addresses its tabs and panels by literal name
//! (`tabs.champion`, `tabs.athlete`, `tabs.team`, and a single
//! `data.athlete`/`data.champion`/`data.team` lookup blob) and never enumerates
//! `#tabs`, so a fourth child is one it does not look at.
//!
//! What survives from the spawning version is [`heal`], now narrowed to the one
//! thing the layout cannot restore: the rows.
//!
//! # The three filters are hidden, not ignored
//!
//! `#position`, `#patch`, `#year_filter` and `#league_filter` drive the vanilla
//! tables and know nothing about this one. Leaving them up while this tab is
//! open would offer four controls that look like they filter what is on screen
//! and do not — the same reason the Builds tab removed Personal rather than
//! leaving it beside an editor that supersedes it. They are hidden on entry and
//! restored on the way out.
//!
//! Making them work is a real feature and a much larger one: it needs a season,
//! league and lane recorded against every sample rather than a single running
//! total, which is a different shape of aggregate than [`crate::item_stats`]
//! keeps.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use mod_api_stable::*;

use crate::item_stats::{self, ItemInfo, Totals};
use crate::strategy_ui::{tab_style, ICON_SHEET};

/// How often to walk the tree looking for the statistics screen, in frames.
///
/// [`find_screen`] is a breadth-first `ui_child_names` sweep and running one per
/// frame is a documented way to make this mod lag — but [`on_statistics_tab`]
/// gates it to the one screen that needs it, so the throttle only has to bound
/// repeated *failed* sweeps rather than pay for every other screen in the game.
///
/// It was a full second, which is what "the tab takes a moment to appear" was:
/// the screen is up and the walk has not run yet. A twelfth of that is not
/// noticeable and still collapses to a single successful sweep.
const PROBE_EVERY: u32 = 5;

/// How deep below the UI root [`find_screen`] will look.
///
/// The screen sits at depth 2 in the management scene (`main` -> `contents` ->
/// the tab), so this has room to spare without turning a missed match into a
/// sweep of the entire tree.
const SCREEN_DEPTH: u32 = 4;

/// Nodes [`find_screen`] will visit before giving up for this sweep.
const SCREEN_NODES: u32 = 600;

/// Frames between repaints while records are still being folded.
///
/// A repaint rewrites every visible row, and the order changes as samples land,
/// so it cannot be skipped entirely during the scan — but it also does not need
/// to happen at 60Hz for a table nobody is reading yet.
const REPAINT_EVERY: u32 = 12;

/// Frames between checks that our spawned nodes are still there.
const HEAL_EVERY: u32 = 10;

/// Frames between patch-backfill passes while the screen is up.
///
/// Thirty seconds. `wire` runs one on arrival, which is the pass that matters;
/// this is the safety net for a screen the game hides rather than destroys, and
/// for records that appear while it is open.
///
/// Slower than it was, because a pass now re-reads *every* record rather than
/// only ids it has not seen — record ids are recycled, so "already scanned" is
/// not a durable fact. Nothing is folded from records any more, so the only cost
/// is the reads, and there is no reason to pay it every five seconds.
const SWEEP_EVERY: u32 = 1800;

/// Rows spawned per repaint.
///
/// The full table is one row per item in the pool, which is around 150 nodes'
/// worth of subtree. Spawning them in one frame is a visible hitch on arrival;
/// spread over a few repaints it is not.
const SPAWN_PER_REPAINT: usize = 60;

/// Most rows the table will ever show.
const MAX_ROWS: usize = 300;

/// Row pitch: 56px of data, a 1px separator, and the scroll view's 4px spacing.
const ROW_PITCH: usize = 61;

/// Column widths, left to right. They sum to less than the panel's 1600px on
/// purpose — the vanilla champion table fills its width with three damage
/// columns this table has no equivalent of, and stretching six columns across
/// the gap would leave the numbers floating far from their headings.
const COL_RANK: u32 = 68;
const COL_NAME: u32 = 420;
const COL_GAMES: u32 = 140;
const COL_WIN: u32 = 130;
const COL_LOSE: u32 = 130;
const COL_RATE: u32 = 150;
/// Wider than the other percentage column because its heading is the longest on
/// the table — "Primeiro Item" and "Первый предмет" both run past 130px, and a
/// heading that reaches the sort arrow looks like a rendering fault.
const COL_FIRST: u32 = 160;
const COL_CHAMPS: u32 = 160;

/// The three vanilla tabs and the panel each one shows, paired so the two can
/// never drift apart. Named relative to the screen root, resolved at runtime.
const VANILLA: [(&str, &str); 3] = [
    ("tabs.champion", "data.champion"),
    ("tabs.athlete", "data.athlete"),
    ("tabs.team", "data.team"),
];

/// The header controls that filter the vanilla tables and not this one.
const FILTERS: [&str; 4] = ["position", "patch", "year_filter", "league_filter"];

/// Fills the patch list from the patches the scan has actually seen.
///
/// Rewritten whenever the set changes, which during a scan is often at first and
/// then never. Rows past the end are hidden rather than removed, because the
/// list is declared in the layout: a rebuilt panel brings all of them back and
/// nothing here has to re-spawn anything.
fn refresh_patch_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let patches = item_stats::patches();

    // Written every time rather than only when the set changes. Caching on
    // `patch_rows` looked free and was the bug behind "the patch dropdown breaks
    // when I tab back and forth": game code rebuilds these nodes on a tab
    // switch, so they come back from the layout blank and hidden while the
    // cached set still matched — and the list opened empty, for good. Twelve
    // property writes on open is not worth outsmarting.

    // Resolved before the writes below: `label` reads the ctx and
    // `ui_set_properties` wants it mutably, so the two cannot share a statement.
    let all = escape(&label(ctx, "item_stats.cat_all", "   All"));

    for row in 0..PATCH_ROWS {
        let path = format!("{screen}.item_patch_list.pat{row}");
        // Row 0 is All and is always present; the rest follow the patch list and
        // run out before the row pool does.
        let text = match row.checked_sub(1) {
            None => Some(all.clone()),
            // Patch names are version strings, not translated text, so the
            // indent the category rows get from their i18n entry is prepended
            // here instead.
            Some(index) => patches
                .get(index)
                .map(|patch| escape(&format!("{ROW_INDENT}{patch}"))),
        };
        match text {
            Some(text) => {
                ctx.ui_set_properties(&path, &format!("visible: true; text: \"{text}\";"));
            }
            None => {
                ctx.ui_set_visible(&path, false);
            }
        }
    }

    // Sized to what is actually on offer. The layout declares a pool of
    // [`PATCH_ROWS`], and a save one patch into its first season would otherwise
    // get a panel eleven empty rows tall.
    let shown = 1 + patches.len().min(PATCH_ROWS - 1);
    ctx.ui_set_properties(
        &format!("{screen}.item_patch_list"),
        &format!(
            "height: {}px;",
            shown * PATCH_ROW_HEIGHT + PATCH_LIST_PADDING
        ),
    );

    // A patch that has scrolled off the end of the list cannot stay selected, or
    // the table would be filtered by something with no row to unselect it from.
    let _ = with_state(|state| {
        if state
            .patch
            .as_ref()
            .is_some_and(|current| !patches.contains(current))
        {
            state.patch = None;
            state.dirty = true;
        }
        state.patch_rows = patches.clone();
    });
}

/// The item categories the filter offers, in the order the list declares them.
/// Row 0 is "All"; row `i` is `CATEGORIES[i - 1]`.
const CATEGORIES: [&str; 6] = crate::item_catalog::CATEGORY_ORDER;

/// Row height and the list's own padding, both as the layout declares them —
/// needed here because the list is resized to the patches on offer.
const PATCH_ROW_HEIGHT: usize = 36;
const PATCH_LIST_PADDING: usize = 8;

/// Leading whitespace that indents a dropdown row's text.
///
/// The indent is in the text rather than in the node's `x`, because insetting
/// the node moves its highlight in too and the row stops looking full-width.
/// A `label:` block accepts nothing positional (size/font/align/colour/hover
/// only), so this is the one place the offset can live.
const ROW_INDENT: &str = "   ";

/// Rows the patch list declares. One is "All"; the rest hold the patches found
/// in the records, newest first, and any beyond this are not offered — a save
/// with more than this many patches shows the most recent.
const PATCH_ROWS: usize = 12;

/// i18n keys for the tier rows, All first — parallel to the `#tier{i}` nodes.
/// Row `i` filters to tier `i - 1`, which is the item's own `tier` field.
const TIER_KEYS: [&str; 6] = [
    "item_stats.cat_all",
    "item_stats.tier_starter",
    "item_stats.tier_basic",
    "item_stats.tier_epic",
    "item_stats.tier_legendary",
    "item_stats.tier_radiant",
];

/// i18n keys for the list rows, All first — parallel to the `#cat{i}` nodes.
const CATEGORY_KEYS: [&str; 7] = [
    "item_stats.cat_all",
    "item_stats.cat_assassin",
    "item_stats.cat_fighter",
    "item_stats.cat_marksman",
    "item_stats.cat_mage",
    "item_stats.cat_tank",
    "item_stats.cat_support",
];

/// Which column the table is ordered by.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SortBy {
    /// Display name, so it follows the game's language rather than the key.
    Item,
    Games,
    Wins,
    Losses,
    #[default]
    WinRate,
    /// Share of this item's buys where it went in the first slot.
    FirstRate,
}

impl SortBy {
    /// Header node under `#header`, and the click path that selects this column.
    fn node(self) -> &'static str {
        match self {
            SortBy::Item => "item_name",
            SortBy::Games => "games",
            SortBy::Wins => "win",
            SortBy::Losses => "lose",
            SortBy::WinRate => "win_rate",
            SortBy::FirstRate => "first_rate",
        }
    }

    /// Which way round to sort when this column is first clicked.
    ///
    /// Descending for the numbers, because "most" is the question being asked of
    /// every one of them. Ascending for the name, because that is what
    /// alphabetical means to a reader.
    fn starts_descending(self) -> bool {
        self != SortBy::Item
    }

    const ALL: [SortBy; 6] = [
        SortBy::Item,
        SortBy::Games,
        SortBy::Wins,
        SortBy::Losses,
        SortBy::WinRate,
        SortBy::FirstRate,
    ];
}

#[derive(Default)]
struct State {
    /// Statistics screen path, once found. Cleared when the screen goes away so
    /// a re-entered screen is searched for again rather than written to at a
    /// stale path.
    screen: Option<String>,
    /// Whether every handler is registered for the current screen.
    wired: bool,
    /// Whether the "still building" note has been logged for this screen.
    wire_warned: bool,
    /// Whether this tab is the one currently showing.
    showing: bool,
    /// Row nodes spawned so far.
    spawned: usize,
    /// Rows currently visible, so a shrinking table hides its tail.
    shown: usize,
    /// Set when new records land; cleared by a repaint.
    dirty: bool,
    /// Category filter, as an index into [`CATEGORIES`]. `None` is "All".
    category: Option<usize>,
    /// Tier filter — the item's own `tier`, 0..=4. `None` is "All".
    tier: Option<usize>,
    /// Whether the tier list is dropped down.
    tier_open: bool,
    /// Patch filter. `None` is "All".
    patch: Option<String>,
    /// Patches currently offered by the list, in row order.
    patch_rows: Vec<String>,
    /// Whether the category list is dropped down.
    list_open: bool,
    /// Whether the patch list is dropped down. Only ever one of the two.
    patch_open: bool,
    /// Column the table is ordered by.
    sort: SortBy,
    /// Which way round. Named for the *non*-default so that `State::default()`
    /// gives the intended opening view — win rate, highest first — without
    /// hand-writing a `Default` impl for the whole struct.
    ascending: bool,
    tick: u32,
    /// Counts every frame the screen is up, which `tick` does not — it only
    /// advances while probing for the screen or while this tab is showing.
    sweep_tick: u32,
    /// The last event acted on, as `(path, frame)`. See [`already_handled`].
    last_event: Option<(String, u32)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(State::default)))
}

/// Paths this module has registered a handler for.
///
/// Same hazard and same fix as the build editor's set: a handler outlives the
/// node it was registered for, so registering twice on one live path runs the
/// handler twice, and a process-lifetime set would leave the *second* visit to
/// this screen unwired. Cleared only on teardown.
static REGISTERED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn register_once(ctx: &mut StableClient<'_>, path: &str) {
    let fresh = REGISTERED
        .lock()
        .map(|mut set| set.insert(path.to_string()))
        .unwrap_or(false);
    if !fresh {
        return;
    }
    if !ctx.ui_register_path_events(path, handle_event) {
        if let Ok(mut set) = REGISTERED.lock() {
            set.remove(path);
        }
    }
}

fn forget_registrations() {
    if let Ok(mut set) = REGISTERED.lock() {
        set.clear();
    }
}

/// Per-frame entry point, called from the mod's one client hook.
///
/// Ordered before that hook's strategy-screen early return, since this screen is
/// not that one.
pub fn sync(ctx: &mut StableClient<'_>) {
    let Some(screen) = resolve_screen(ctx) else {
        return;
    };

    // Frames spent on this screen. Drives the two housekeeping cadences below,
    // and is separate from `tick` because that one only advances while probing
    // for the screen or while this tab is showing.
    let frame = with_state(|state| {
        state.sweep_tick = state.sweep_tick.wrapping_add(1);
        state.sweep_tick
    })
    .unwrap_or(0);

    if frame % HEAL_EVERY == 0 {
        heal(ctx, &screen);
    }

    if !with_state(|state| state.wired).unwrap_or(true) {
        wire(ctx, &screen);
    }

    // Both of these run whether or not the tab is open, and both have to run
    // from here rather than from the click that opens it: `setting_get_json`
    // and `record_get_json` are not live inside a UI handler. Folding early
    // also means the scan is finished by the time the player reaches for the
    // tab, rather than starting when they do.
    item_stats::prime_catalog(ctx);
    if item_stats::pump(ctx) {
        let _ = with_state(|state| state.dirty = true);
    }

    if frame % SWEEP_EVERY == 0 {
        item_stats::sweep(ctx);
    }

    if !with_state(|state| state.showing).unwrap_or(false) {
        return;
    }

    // Game code drives its own panels and header controls from its own tab
    // state, which never becomes this one, so a one-shot hide on entry does not
    // survive anything that makes it re-assert them — which is why the position
    // dropdown was still on screen over this tab.
    for (_, panel) in VANILLA {
        ctx.ui_set_visible(&format!("{screen}.{panel}"), false);
    }
    for filter in FILTERS {
        ctx.ui_set_visible(&format!("{screen}.{filter}"), false);
    }
    // The panel and both dropdowns are authored `visible: false`, so a rebuild
    // hands them back hidden. Asserted here rather than only on entry, for the
    // same reason the vanilla panels are.
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), true);
    ctx.ui_set_visible(&format!("{screen}.item_category"), true);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), true);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), true);

    // The catcher is a full-screen transparent button, so if it is ever left up
    // it eats every click on this screen — which is the difference between "a
    // dropdown is open" and "the tab is dead". Driving it from state every frame
    // means it cannot be stranded by a rebuild.
    let (list_open, patch_open, tier_open) =
        with_state(|state| (state.list_open, state.patch_open, state.tier_open))
            .unwrap_or((false, false, false));
    ctx.ui_set_visible(&format!("{screen}.item_category_list"), list_open);
    ctx.ui_set_visible(&format!("{screen}.item_patch_list"), patch_open);
    ctx.ui_set_visible(&format!("{screen}.item_tier_list"), tier_open);
    ctx.ui_set_visible(
        &format!("{screen}.item_category_catch"),
        list_open || patch_open || tier_open,
    );

    let due = with_state(|state| {
        state.tick = state.tick.wrapping_add(1);
        let due = state.dirty && state.tick % REPAINT_EVERY == 0;
        if due {
            state.dirty = false;
        }
        due
    })
    .unwrap_or(false);

    if due {
        repaint(ctx, &screen);
    }
}

/// Re-syncs to a screen subtree game code has rebuilt underneath us.
///
/// Game code rebuilds parts of this screen on its own tab switches. The tab and
/// the panel come back on their own now that the layout declares them — that is
/// the whole reason they moved into the override, and what stopped them
/// flickering — but the **rows** do not: those are spawned per session from the
/// current totals, and a rebuilt panel comes back with an empty `#contents` and
/// its authored `visible: false`.
///
/// Missing rows are therefore the signal that a rebuild happened. Without this,
/// `spawned` still claims rows exist, every repaint writes text at paths that no
/// longer resolve, and the table stays blank for the rest of the visit.
///
/// Rate-limited rather than run every frame: if game code ever rebuilt
/// continuously, an unthrottled version would respawn a subtree per frame. At
/// [`HEAL_EVERY`] the worst case is a few per second, and the normal case — one
/// rebuild, caught within a sixth of a second — is imperceptible.
fn heal(ctx: &mut StableClient<'_>, screen: &str) {
    if !with_state(|state| state.wired).unwrap_or(false) {
        return;
    }

    // The layout's own nodes. Gone means the override stopped applying, which is
    // not something to paper over silently.
    if !ctx.ui_exists(&format!("{screen}.tabs.item"))
        || !ctx.ui_exists(&format!("{screen}.data.item_stats"))
    {
        diag("layout nodes vanished; re-adopting");
        let _ = with_state(|state| *state = State::default());
        return;
    }

    let spawned = with_state(|state| state.spawned).unwrap_or(0);
    if spawned == 0 {
        return;
    }
    let rows_alive = ctx.ui_exists(&format!("{screen}.data.item_stats.data.contents.row0"));
    if rows_alive {
        return;
    }

    diag("panel was rebuilt; respawning rows");
    let _ = with_state(|state| {
        // The handlers are keyed by path and outlive the nodes, so `REGISTERED`
        // is deliberately untouched: re-registering a live path is what makes
        // one click run twice.
        state.spawned = 0;
        state.shown = 0;
        // `showing` is deliberately NOT cleared. A rebuild is game code
        // replacing nodes underneath us, not the player leaving the tab — and
        // clearing it stopped `sync` re-asserting anything, which left the tab
        // frozen: sort clicks did nothing, the dropdowns went dead, and only
        // leaving and coming back fixed it. Staying "showing" is what lets the
        // per-frame block put the rebuilt panel back the way it was.
        //
        // The lists do not survive a rebuild in any useful state, so they close.
        state.list_open = false;
        state.patch_open = false;
        state.tier_open = false;
        state.dirty = true;
    });

    // Everything the rebuild reverted to a layout default and that no per-frame
    // assert covers: the two dropdown faces and the tab highlight all carry
    // state that only lives in this module.
    if with_state(|state| state.showing).unwrap_or(false) {
        paint_category_button(ctx, screen);
        paint_patch_button(ctx, screen);
        paint_tier_button(ctx, screen);
        paint_headers(ctx, screen);
        paint_tabs(ctx, screen, true);
    }
}

/// The statistics screen path, or `None` when it is not up.
///
/// # Why this searches instead of naming a path
///
/// The first version of this anchored on `main.contents.statistics`, reasoning
/// from `main.contents.strategy`, which is a path the build editor uses and
/// which works. That reasoning was wrong, and the tab silently never appeared.
///
/// A path's root is the root **node name of the scene's own layout**, and that
/// name is not `main` everywhere:
///
/// ```text
/// main.ui        main:main_ui          <- the management scene
/// strategy.ui    main:strategy_ui      <- also `main`, hence main.contents.strategy
/// lineup.ui      main:lineup_ui
/// ingame.ui      ingame:ingame_ui      <- NOT main; see the in-match note
/// solo_rank.ui   solo_rank:solo_rank_ui
/// statistics.ui  statistics:statistics_ui
/// ```
///
/// So "prefix with `main.`" is not a rule, it is a coincidence of the three
/// layouts that happen to declare that root, and the statistics screen is not
/// one of them. Rather than swap one guess for another, this walks down from the
/// real UI root — `ui_child_names("")` — until it finds a node with a
/// `tabs.champion` under it, which is a shape no other screen has.
///
/// The walk is throttled to [`PROBE_EVERY`] because an unthrottled breadth-first
/// `ui_child_names` sweep is a known way to make this mod lag, and it stops as
/// soon as the screen is cached.
fn resolve_screen(ctx: &mut StableClient<'_>) -> Option<String> {
    if let Some(screen) = with_state(|state| state.screen.clone()).flatten() {
        if ctx.ui_exists(&format!("{screen}.tabs.champion")) {
            return Some(screen);
        }
        // Left the screen. Everything spawned into it died with it, so the next
        // visit rebuilds rather than writing to paths that no longer resolve.
        let _ = with_state(|state| {
            *state = State::default();
        });
        forget_registrations();
        return None;
    }

    let due = with_state(|state| {
        state.tick = state.tick.wrapping_add(1);
        state.tick % PROBE_EVERY == 0
    })
    .unwrap_or(false);
    if !due {
        return None;
    }

    if !on_statistics_tab(ctx) {
        return None;
    }

    let found = find_screen(ctx)?;
    diag(&format!("screen found at {found}"));
    let _ = with_state(|state| state.screen = Some(found.clone()));
    Some(found)
}

/// Whether the client is on the Statistics main tab.
///
/// This is the gate that keeps [`find_screen`]'s sweep off every other screen in
/// the game, which is the difference between a walk that runs once on arrival
/// and one that runs once a second forever. `Statistics` is a real main-tab
/// variant name in the exe's string table, beside `Solorank` and `Recruitment`.
///
/// Matched loosely and failing open: `None` means the host would not say (an
/// older ABI, or not on the Main screen at all), and being inert is a worse
/// answer than searching. The substring test is because the engine has form for
/// spelling these inconsistently — `solo_rank` vs `solorank` cost the
/// solo-rank module the same question.
fn on_statistics_tab(ctx: &StableClient<'_>) -> bool {
    let Some(tab) = ctx.client_main_tab() else {
        return true;
    };

    // Logged on change rather than per probe: it names the tab the player is
    // actually on, which is both the confirmation that this gate works and the
    // exact spelling to match if it ever stops working.
    static LAST: Mutex<Option<String>> = Mutex::new(None);
    if let Ok(mut last) = LAST.lock() {
        if last.as_deref() != Some(tab.as_str()) {
            diag(&format!("main tab = {tab:?}"));
            *last = Some(tab.clone());
        }
    }

    tab.to_ascii_lowercase().contains("statistic")
}

/// Breadth-first from the UI root for the node holding the statistics tabs.
///
/// `tabs.champion` is the marker rather than the screen's name, for the same
/// reason `solo_rank_ui` matches on an `item_slot3` child: the name is game
/// code's business and the engine has been inconsistent about it before
/// (`solo_rank` vs `solorank`), while the shape is what this module actually
/// needs to be true.
fn find_screen(ctx: &StableClient<'_>) -> Option<String> {
    let mut level: Vec<String> = ctx.ui_child_names("");

    // Logged on the first probe rather than only on failure. A search that never
    // matches would otherwise write nothing at all, which is indistinguishable
    // from the mod not running — and "what are the root's children" is the one
    // fact that turns a wrong assumption about the tree into a correct one.
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(false);
        if !ONCE.swap(true, Ordering::Relaxed) {
            diag(&format!("ui root children: {level:?}"));
        }
    }

    if level.is_empty() {
        // A root that enumerates nothing would make this inert forever, so the
        // scene roots seen in the layouts are tried as a seed rather than
        // trusting one call.
        level = ["main", "statistics"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    }

    let mut budget = SCREEN_NODES;
    for _ in 0..SCREEN_DEPTH {
        if level.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for path in level {
            if ctx.ui_exists(&format!("{path}.tabs.champion")) {
                return Some(path);
            }
            // A depth limit alone does not bound this: one wide level can be
            // hundreds of nodes, and this runs on the UI thread.
            budget = budget.saturating_sub(1);
            if budget == 0 {
                diag("screen search hit its node budget");
                return None;
            }
            for name in ctx.ui_child_names(&path) {
                next.push(format!("{path}.{name}"));
            }
        }
        level = next;
    }
    None
}

/// Spawns the tab and the panel, and registers all four tabs.
///
/// Registering the vanilla three is observation only — a handler on them is what
/// puts this panel away when the player switches back, and game code's own
/// handling for those clicks is untouched.
fn wire(ctx: &mut StableClient<'_>, screen: &str) {
    let paths = handler_paths(screen);

    // Every one of them, before registering any of them. `resolve_screen` finds
    // this screen the moment `tabs.champion` resolves, which is not the moment
    // the whole subtree exists — the dropdowns are declared after `#data` and
    // arrive later. Registering what happened to be ready and then claiming
    // `wired` left the rest without handlers for the life of the screen: on the
    // first visit the dropdowns did nothing and only some sort headers
    // responded, and only leaving and coming back fixed it.
    // The panel carries no handler of its own, but nothing here works without
    // it, so it joins the readiness check.
    let panel = format!("{screen}.data.item_stats");
    if let Some(missing) = paths
        .iter()
        .chain(std::iter::once(&panel))
        .find(|path| !ctx.ui_exists(path))
    {
        // Once per screen, not once per frame: this is the normal state for the
        // frame or two a screen takes to build, and it must not burn the log.
        if !with_state(|state| std::mem::replace(&mut state.wire_warned, true)).unwrap_or(true) {
            diag(&format!("waiting for {missing}"));
        }
        return;
    }

    for path in &paths {
        register_once(ctx, path);
    }

    // The id list is refreshed per screen entry rather than per frame: it copies
    // the whole set across the ABI twice, and no match is simmed while the
    // player is standing here.
    item_stats::sweep(ctx);
    diag(&format!(
        "wired {} handlers; match replay records = {}",
        paths.len(),
        ctx.record_ids(RecordKindV1::MatchReplay).len()
    ));

    let _ = with_state(|state| {
        state.wired = true;
        state.dirty = true;
    });
}

/// Every node this module puts a click handler on.
///
/// One list so that "is the screen ready" and "what gets registered" cannot
/// disagree — the bug above was exactly that disagreement.
fn handler_paths(screen: &str) -> Vec<String> {
    let mut paths = Vec::new();
    // Registering the vanilla tabs is observation only: a handler on them is
    // what puts this panel away when the player switches back, and game code's
    // own handling for those clicks is untouched.
    for (tab, _) in VANILLA {
        paths.push(format!("{screen}.{tab}"));
    }
    paths.push(format!("{screen}.tabs.item"));
    for column in SortBy::ALL {
        paths.push(header_path(screen, column));
    }
    paths.push(format!("{screen}.item_category"));
    paths.push(format!("{screen}.item_category_catch"));
    for row in 0..CATEGORY_KEYS.len() {
        paths.push(format!("{screen}.item_category_list.cat{row}"));
    }
    paths.push(format!("{screen}.item_patch"));
    for row in 0..PATCH_ROWS {
        paths.push(format!("{screen}.item_patch_list.pat{row}"));
    }
    paths.push(format!("{screen}.item_tier"));
    for row in 0..TIER_KEYS.len() {
        paths.push(format!("{screen}.item_tier_list.tier{row}"));
    }
    paths
}

/// Tags a line as this half's and hands it to the shared, capped writer.
///
/// Every failure mode here is silent — a wrong path, a rejected spawn and a
/// screen that was never found all look identical from the outside, which is a
/// whole build spent guessing.
fn diag(line: &str) {
    crate::item_stats::diag(&format!("[ui] {line}"));
}

/// Whether this exact event has already been acted on this frame.
///
/// # Why one click arrives twice
///
/// `ui_register_path_events` registers a handler for **every** event whose path
/// equals the one given, and the closure it takes is leaked — "handlers live
/// until process exit". A registration is therefore permanent and keyed by the
/// *path*, not by the node that happens to sit at it. [`forget_registrations`]
/// clears only this module's own bookkeeping when the screen goes away, so the
/// next visit registers the same paths again and leaves **two** live handlers on
/// each of them.
///
/// Two is not harmless, because nearly everything this handler does is a
/// toggle. A dropdown opens and instantly closes again; a sort header reverses
/// and reverses back. That is the whole of "the dropdowns do nothing and the
/// sort only ever goes one way" — and it is why leaving and re-entering appeared
/// to fix it: a third registration restores the parity a fourth breaks again, so
/// the tab works on odd visits and is dead on even ones. Which is also why it
/// looked intermittent rather than broken.
///
/// # Why it is suppressed here
///
/// Rather than by not re-registering, which would be the other obvious fix.
/// This one is correct whether or not the host *also* drops a registration when
/// the screen it belongs to is destroyed, and that is not a question this can
/// answer from the outside. Guessing wrong in the other direction gives a tab
/// that is dead on every visit but the first, which is worse than a handler that
/// occasionally does nothing.
fn already_handled(path: &str) -> bool {
    let duplicate = with_state(|state| {
        // `sweep_tick` advances once per frame for as long as the screen is up,
        // and a person cannot click the same node twice inside one frame — so a
        // repeat within a frame is a second delivery, not a second click.
        let stamp = (path.to_string(), state.sweep_tick);
        let duplicate = state.last_event.as_ref() == Some(&stamp);
        state.last_event = Some(stamp);
        duplicate
    })
    .unwrap_or(false);

    if duplicate {
        // Once, not per click: it confirms the diagnosis from the log rather
        // than from reasoning about it, and after the first there is one of
        // these for every click for the rest of the session.
        use std::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(false);
        if !ONCE.swap(true, Ordering::Relaxed) {
            diag(&format!("suppressed a duplicate event on {path}"));
        }
    }
    duplicate
}

fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // Only clicks drive this tab. `Remove` in particular fires as a node is torn
    // down, and this screen is torn down every time the player leaves it — so
    // the tab's own destruction would otherwise arrive here as a click and open
    // a panel into a dying tree.
    //
    // An unreported kind is treated as a click: on a host that does not fill the
    // field in, a tab that does nothing at all is the worse failure.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    if already_handled(&event.path) {
        return;
    }
    let Some(screen) = with_state(|state| state.screen.clone()).flatten() else {
        return;
    };

    if event.path == format!("{screen}.tabs.item") {
        open(ctx, &screen);
        return;
    }

    if let Some(column) = SortBy::ALL
        .into_iter()
        .find(|column| event.path == header_path(&screen, *column))
    {
        sort_by(ctx, &screen, column);
        return;
    }

    if event.path == format!("{screen}.item_category") {
        let open = with_state(|state| {
            state.list_open = !state.list_open;
            state.patch_open = false;
            state.tier_open = false;
            state.list_open
        })
        .unwrap_or(false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_category_list(ctx, &screen, open);
        return;
    }

    if event.path == format!("{screen}.item_patch") {
        let open = with_state(|state| {
            state.patch_open = !state.patch_open;
            state.list_open = false;
            state.tier_open = false;
            state.patch_open
        })
        .unwrap_or(false);
        show_category_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, open);
        return;
    }

    for row in 0..PATCH_ROWS {
        if event.path == format!("{screen}.item_patch_list.pat{row}") {
            pick_patch(ctx, &screen, row);
            return;
        }
    }

    if event.path == format!("{screen}.item_tier") {
        let open = with_state(|state| {
            state.tier_open = !state.tier_open;
            state.list_open = false;
            state.patch_open = false;
            state.tier_open
        })
        .unwrap_or(false);
        show_category_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, open);
        return;
    }

    for row in 0..TIER_KEYS.len() {
        if event.path == format!("{screen}.item_tier_list.tier{row}") {
            pick_tier(ctx, &screen, row);
            return;
        }
    }

    if event.path == format!("{screen}.item_category_catch") {
        let _ = with_state(|state| {
            state.list_open = false;
            state.patch_open = false;
            state.tier_open = false;
        });
        show_category_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        return;
    }

    for row in 0..CATEGORY_KEYS.len() {
        if event.path == format!("{screen}.item_category_list.cat{row}") {
            pick_category(ctx, &screen, row);
            return;
        }
    }

    // Every event on a vanilla tab lands here, hover included, so this has to be
    // idempotent and cheap — it may only act when this tab is the one up.
    let clicked = VANILLA
        .iter()
        .find(|(tab, _)| event.path == format!("{screen}.{tab}"));
    if let Some((_, panel)) = clicked {
        if with_state(|state| state.showing).unwrap_or(false) {
            close(ctx, &screen, panel);
        }
    }
}

fn open(ctx: &mut StableClient<'_>, screen: &str) {
    // Ahead of the repaint below, not left to the next `sync`: a click that
    // lands between a rebuild and the throttled heal would otherwise paint text
    // onto row nodes that no longer exist and show an empty table until the
    // heal caught up.
    heal(ctx, screen);

    for (_, panel) in VANILLA {
        ctx.ui_set_visible(&format!("{screen}.{panel}"), false);
    }
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), true);
    ctx.ui_set_visible(&format!("{screen}.item_category"), true);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), true);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), true);
    paint_category_button(ctx, screen);
    refresh_patch_rows(ctx, screen);
    paint_patch_button(ctx, screen);
    paint_tier_button(ctx, screen);
    paint_tabs(ctx, screen, true);
    // The panel comes back from the layout with every arrow transparent, so the
    // opening view would otherwise be sorted by a column that does not say so.
    paint_headers(ctx, screen);

    let _ = with_state(|state| state.showing = true);
    repaint(ctx, screen);
}

/// Leaves the tab for `panel`, the vanilla panel belonging to the tab that was
/// clicked.
///
/// The panel is shown here rather than left to game code, which is the opposite
/// of what the Builds tab does on the strategy screen. The difference is what is
/// selected underneath: game code never stopped considering one of these three
/// tabs selected while this one was up, so a click on *that* tab is a click on
/// the already-selected tab, and a runner that short-circuits it would leave the
/// screen with nothing showing at all. Showing it here cannot disagree with game
/// code — it is the panel belonging to the tab just clicked, so the worst case
/// is that both of us show the same one.
fn close(ctx: &mut StableClient<'_>, screen: &str, panel: &str) {
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), false);
    ctx.ui_set_visible(&format!("{screen}.item_category"), false);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), false);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), false);
    let _ = with_state(|state| {
        state.list_open = false;
        state.patch_open = false;
        state.tier_open = false;
    });
    show_category_list(ctx, screen, false);
    show_patch_list(ctx, screen, false);
    show_tier_list(ctx, screen, false);
    ctx.ui_set_visible(&format!("{screen}.{panel}"), true);
    for filter in FILTERS {
        ctx.ui_set_visible(&format!("{screen}.{filter}"), true);
    }
    paint_tabs(ctx, screen, false);
    let _ = with_state(|state| state.showing = false);
}

/// Shows or hides the category list, and the full-screen catcher behind it.
///
/// The catcher is what makes a click anywhere else dismiss the list. Without one
/// the list can only be closed by choosing something or by clicking the button
/// again, which is not what a dropdown does — the build editor learned the same
/// thing and for the same reason.
fn show_category_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_category_list"), open);
    if open {
        paint_category_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

/// Shows or hides the patch list, sharing the category list's catcher.
fn show_patch_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_patch_list"), open);
    if open {
        // Kept in step here rather than per frame: the set only changes while
        // the scan is running, and this is the moment it is about to be read.
        refresh_patch_rows(ctx, screen);
        paint_patch_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

/// Applies the patch on row `row` and closes the list.
fn pick_patch(ctx: &mut StableClient<'_>, screen: &str, row: usize) {
    let _ = with_state(|state| {
        // Row 0 is All, which is the absence of a filter.
        state.patch = row
            .checked_sub(1)
            .and_then(|index| state.patch_rows.get(index).cloned());
        state.patch_open = false;
        state.dirty = true;
    });

    show_patch_list(ctx, screen, false);
    paint_patch_button(ctx, screen);
    repaint(ctx, screen);
}

/// Writes the selected patch onto the button face.
///
/// A version string is not translated text, so this goes on as a literal —
/// unlike the category button, which hands over a document reference so the
/// label follows the game's language.
fn paint_patch_button(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.patch.clone()).unwrap_or(None);
    let text = match &selected {
        Some(patch) => escape(&format!("{ROW_INDENT}{patch}")),
        None => escape(&label(ctx, "item_stats.cat_all", "   All")),
    };
    ctx.ui_set_properties(
        &format!("{screen}.item_patch.text"),
        &format!("text: \"{text}\";"),
    );
}

/// Lights the row of the patch currently in force.
fn paint_patch_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let (selected, rows) =
        with_state(|state| (state.patch.clone(), state.patch_rows.clone())).unwrap_or_default();
    let lit_row = selected
        .and_then(|patch| rows.iter().position(|row| *row == patch))
        .map_or(0, |index| index + 1);
    for row in 0..PATCH_ROWS {
        ctx.ui_set_properties(
            &format!("{screen}.item_patch_list.pat{row}"),
            &tab_style("image", "label", row == lit_row),
        );
    }
}

/// Shows or hides the tier list, sharing the one catcher.
fn show_tier_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_tier_list"), open);
    if open {
        paint_tier_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

/// Applies the tier on row `row` and closes the list.
fn pick_tier(ctx: &mut StableClient<'_>, screen: &str, row: usize) {
    let _ = with_state(|state| {
        // Row 0 is All; row `i` is tier `i - 1`.
        state.tier = row.checked_sub(1);
        state.tier_open = false;
        state.dirty = true;
    });
    show_tier_list(ctx, screen, false);
    paint_tier_button(ctx, screen);
    repaint(ctx, screen);
}

/// Writes the selected tier onto the button face.
fn paint_tier_button(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.tier).unwrap_or(None);
    let key = TIER_KEYS[selected.map_or(0, |tier| tier + 1)];
    let text = label(ctx, key, "   All");
    ctx.ui_set_text(&format!("{screen}.item_tier.text"), &text);
}

/// Lights the row of the tier currently in force.
fn paint_tier_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.tier).unwrap_or(None);
    let lit_row = selected.map_or(0, |tier| tier + 1);
    for row in 0..TIER_KEYS.len() {
        ctx.ui_set_properties(
            &format!("{screen}.item_tier_list.tier{row}"),
            &tab_style("image", "label", row == lit_row),
        );
    }
}

/// Puts the catcher up while any list is down, and takes it away otherwise.
///
/// One place rather than three, so a new dropdown cannot forget to consider the
/// other two and strand a full-screen button over the whole screen.
fn sync_catch(ctx: &mut StableClient<'_>, screen: &str) {
    let any =
        with_state(|state| state.list_open || state.patch_open || state.tier_open).unwrap_or(false);
    ctx.ui_set_visible(&format!("{screen}.item_category_catch"), any);
}

/// Applies the category on row `row` and closes the list.
fn pick_category(ctx: &mut StableClient<'_>, screen: &str, row: usize) {
    let _ = with_state(|state| {
        // Row 0 is All, which is the absence of a filter rather than one of the
        // categories, hence the `Option` rather than a sentinel index.
        state.category = row.checked_sub(1);
        state.list_open = false;
        state.dirty = true;
    });
    show_category_list(ctx, screen, false);
    paint_category_button(ctx, screen);
    repaint(ctx, screen);
}

/// Writes the selected category onto the button face.
///
/// The text is handed over as a document reference, not as resolved text, so the
/// button follows the game's language the way the layout's own labels do.
fn paint_category_button(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.category).unwrap_or(None);
    let key = CATEGORY_KEYS[selected.map_or(0, |index| index + 1)];
    let fallback = match selected {
        Some(index) => format!("{ROW_INDENT}{}", CATEGORIES[index]),
        None => format!("{ROW_INDENT}All"),
    };
    let text = label(ctx, key, &fallback);
    ctx.ui_set_text(&format!("{screen}.item_category.text"), &text);
}

/// Lights the row of the category currently in force.
fn paint_category_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.category).unwrap_or(None);
    let lit_row = selected.map_or(0, |index| index + 1);
    for row in 0..CATEGORY_KEYS.len() {
        ctx.ui_set_properties(
            &format!("{screen}.item_category_list.cat{row}"),
            &tab_style("image", "label", row == lit_row),
        );
    }
}

/// The category an item belongs to, or `None` for one with no role.
///
/// Components and the game's own lower tiers have no role — a BF Sword goes into
/// half the builds in the game — so they answer `None` and appear only under
/// "All". That is the honest answer rather than filing them somewhere.
fn category_of(key: &str) -> Option<&'static str> {
    crate::item_catalog::category_of(crate::build_config::base_slug(key))
}

fn header_path(screen: &str, column: SortBy) -> String {
    format!("{screen}.data.item_stats.header.{}", column.node())
}

/// Re-orders the table by `column`.
///
/// Clicking the column already in use flips the direction; clicking a new one
/// starts it at [`SortBy::starts_descending`]. That is the behaviour every table
/// with clickable headings has, and getting it wrong is immediately obvious.
fn sort_by(ctx: &mut StableClient<'_>, screen: &str, column: SortBy) {
    // Same reason `open` does it: a click landing between a rebuild and the
    // throttled heal would otherwise repaint onto rows that no longer exist.
    heal(ctx, screen);

    let _ = with_state(|state| {
        if state.sort == column {
            state.ascending = !state.ascending;
        } else {
            state.sort = column;
            state.ascending = !column.starts_descending();
        }
        state.dirty = true;
    });
    paint_headers(ctx, screen);
    repaint(ctx, screen);
}

/// Puts the arrow on the column being sorted by, pointing the way it sorts.
///
/// The layout authors every arrow transparent, so "inactive" needs no repaint of
/// its own — only the active one is coloured in, and the previous active one is
/// cleared by the same loop. `dropdown`/`dropdown_up` are the game's own matched
/// pair, which is why the direction can be shown by swapping `source` rather
/// than by rotating anything.
fn paint_headers(ctx: &mut StableClient<'_>, screen: &str) {
    let (sort, ascending) =
        with_state(|state| (state.sort, state.ascending)).unwrap_or((SortBy::default(), false));

    for column in SortBy::ALL {
        let active = column == sort;
        let source = if active && ascending {
            "asset/base/ui/icons/dropdown_up"
        } else {
            "asset/base/ui/icons/dropdown"
        };
        let color = if active { "#ecfbf8ff" } else { "#00000000" };
        ctx.ui_set_properties(
            &header_path(screen, column),
            &format!("icon: {{ source: \"{source}\"; color: {color}; }}"),
        );
    }
}

/// Paints which tab looks selected.
///
/// The selection cannot be *set*: `ui_set_selectable_selected` is
/// `state_set_json` with `{"selected": …}`, which the host accepts only for the
/// `checkbox`, `text_edit`, `slider` and `selectable` runner kinds — these tabs
/// are `color_selectable`, so the write is rejected. The highlight is drawn on
/// instead, exactly as the Builds tab does it.
///
/// The two sides go through different property pairs because they are in
/// different states underneath. This tab is never `selected` as far as game code
/// is concerned, so its plain `image`/`label` are what render. Whichever vanilla
/// tab game code thinks is selected renders `selected_image`/`selected_label`
/// instead, and those are the ones that have to be dulled — writing all three is
/// how this avoids having to know which one it is, since the other two are
/// rendering `image`/`label` and ignore it.
fn paint_tabs(ctx: &mut StableClient<'_>, screen: &str, ours_active: bool) {
    ctx.ui_set_properties(
        &format!("{screen}.tabs.item"),
        &tab_style("image", "label", ours_active),
    );
    for (tab, _) in VANILLA {
        ctx.ui_set_properties(
            &format!("{screen}.{tab}"),
            &tab_style("selected_image", "selected_label", !ours_active),
        );
    }
}

// -- rendering --------------------------------------------------------------

/// Orders the table by the chosen column, on the column's value and nothing else.
///
/// Every comparison ends on the item key, so the order is total: without that,
/// rows that tie — and on a small sample most of them tie — would swap places
/// between repaints while the scan is still folding records, which reads as the
/// table flickering.
///
/// Win rate used to demote thin samples below everything else, on the grounds
/// that one game at 100% is not the best item in the game. That is true of the
/// number and not this function's call to make: the pick count is right there in
/// the next column, so the reader can see the sample for themselves.
fn order_rows(
    rows: &mut [(String, item_stats::Totals)],
    catalog: &BTreeMap<String, item_stats::ItemInfo>,
    sort: SortBy,
    ascending: bool,
) {
    let name_of = |key: &String| {
        catalog
            .get(key)
            .map(|info| info.name.to_lowercase())
            .unwrap_or_else(|| key.to_lowercase())
    };

    rows.sort_by(|(a_key, a), (b_key, b)| {
        let ordering = match sort {
            SortBy::Item => name_of(a_key).cmp(&name_of(b_key)),
            SortBy::Games => a.games.cmp(&b.games),
            SortBy::Wins => a.wins.cmp(&b.wins),
            SortBy::Losses => a.losses().cmp(&b.losses()),
            SortBy::WinRate => a
                .win_rate()
                .partial_cmp(&b.win_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortBy::FirstRate => a
                .first_rate()
                .partial_cmp(&b.first_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        let ordering = if ascending {
            ordering
        } else {
            ordering.reverse()
        };
        ordering.then_with(|| a_key.cmp(b_key))
    });
}

fn repaint(ctx: &mut StableClient<'_>, screen: &str) {
    let patch = with_state(|state| state.patch.clone()).unwrap_or(None);
    let mut snapshot = item_stats::snapshot(patch.as_deref());
    let catalog = item_stats::catalog();
    let contents = format!("{screen}.data.item_stats.data.contents");

    let (sort, ascending, category, tier) =
        with_state(|state| (state.sort, state.ascending, state.category, state.tier)).unwrap_or((
            SortBy::default(),
            false,
            None,
            None,
        ));
    if let Some(index) = category {
        let wanted = CATEGORIES[index];
        snapshot
            .rows
            .retain(|(key, _)| category_of(key) == Some(wanted));
    }
    if let Some(wanted) = tier {
        // An item the catalog cannot place has no tier, so it answers no tier
        // filter — the same rule the category filter follows for an item with
        // no role.
        snapshot
            .rows
            .retain(|(key, _)| catalog.get(key).and_then(|info| info.tier) == Some(wanted));
    }
    order_rows(&mut snapshot.rows, &catalog, sort, ascending);

    let wanted = snapshot.rows.len().min(MAX_ROWS);

    // Spawned in bounded batches, so the first repaint after arrival is not a
    // 150-node stall. A short table this frame is a table that fills in over the
    // next few, which is what the scan is doing anyway.
    let spawned = with_state(|state| state.spawned).unwrap_or(0);
    if spawned < wanted {
        let target = wanted.min(spawned + SPAWN_PER_REPAINT);
        for index in spawned..target {
            ctx.ui_spawn_source(&contents, &row_source(index));
        }
        let _ = with_state(|state| state.spawned = target);
    }

    let spawned = with_state(|state| state.spawned).unwrap_or(0);
    let visible = wanted.min(spawned);

    for (index, (key, totals)) in snapshot.rows.iter().take(visible).enumerate() {
        let info = catalog.get(key).cloned().unwrap_or_else(|| ItemInfo {
            // An item nothing describes: a key from a save written with another
            // item set. Showing the raw key is the honest answer, and no tier
            // means it answers no tier filter rather than being filed under a
            // guess.
            name: key.clone(),
            frame: None,
            tier: None,
        });
        let champions = snapshot
            .champions
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default();
        write_row(ctx, &contents, index, index + 1, &info, totals, champions);
    }

    // A shrinking table (a fresh sweep after a save change) has to put its tail
    // away; the nodes stay spawned for the next one.
    let previously = with_state(|state| state.shown).unwrap_or(0);
    for index in visible..previously.min(spawned) {
        ctx.ui_set_visible(&format!("{contents}.row{index}"), false);
    }
    let _ = with_state(|state| state.shown = visible);

    ctx.ui_set_properties(&contents, &format!("height: {}px;", visible * ROW_PITCH));
    ctx.ui_set_text(
        &format!("{screen}.data.item_stats.status"),
        &status_text(&snapshot),
    );

    // A repaint that could not spawn its whole batch has to ask for another
    // one. Without this the table stalls at whatever `SPAWN_PER_REPAINT`
    // reached when the scan finished, because `dirty` is otherwise only ever
    // set by new records arriving and there are none left to arrive.
    let _ = with_state(|state| {
        if state.spawned < wanted {
            state.dirty = true;
        }
    });
}

fn write_row(
    ctx: &mut StableClient<'_>,
    contents: &str,
    index: usize,
    rank: usize,
    info: &ItemInfo,
    totals: &Totals,
    champions: &[String],
) {
    let row = format!("{contents}.row{index}");
    ctx.ui_set_visible(&row, true);

    let cell = |name: &str| format!("{row}.data.{name}.text");
    ctx.ui_set_text(&cell("rank"), &rank.to_string());
    ctx.ui_set_text(&cell("item_name"), &info.name);
    ctx.ui_set_text(&cell("games"), &totals.games.to_string());
    ctx.ui_set_text(&cell("win"), &totals.wins.to_string());
    ctx.ui_set_text(&cell("lose"), &totals.losses().to_string());
    ctx.ui_set_text(
        &cell("win_rate"),
        &totals
            .win_rate()
            .map(|rate| format!("{rate:.1}%"))
            .unwrap_or_else(|| "-".into()),
    );
    // Of this item's buys, how often it was the one rushed. A dash rather than
    // 0.0% for an item with no games, for the same reason the win rate uses one.
    ctx.ui_set_text(
        &cell("first_rate"),
        &totals
            .first_rate()
            .map(|rate| format!("{rate:.1}%"))
            .unwrap_or_else(|| "-".into()),
    );

    // Two properties, never `sheet#tag`: `ui_set_properties` feeds the `.ui`
    // parser, which has no `#` form. Passing one returns true and renders
    // nothing.
    // The champions that bought it most. Slots are fixed and hidden rather than
    // spawned, so an item with one buyer shows one portrait, not one portrait
    // and two empty boxes.
    let strip = format!("{row}.data.most_used.container");
    for slot in 0..item_stats::TOP_CHAMPIONS {
        let path = format!("{strip}.slot{slot}");
        match champions.get(slot).filter(|name| !name.is_empty()) {
            Some(champion) => {
                // 36x36 at scale 2.0 is what the game passes for its own
                // portraits of this size; the call fits the sprite into that box
                // rather than stretching the node to the sheet.
                //
                // Shown only if it lands: a champion from a mod that is no
                // longer enabled is still in the records but has no sheet, and
                // an empty rounded box reads as a bug rather than as absence.
                let drawn =
                    ctx.ui_set_champion_icon(&format!("{path}.icon"), champion, 36.0, 36.0, 2.0);
                ctx.ui_set_visible(&path, drawn);
            }
            None => {
                ctx.ui_set_visible(&path, false);
            }
        }
    }

    let icon = format!("{row}.data.item_name.icon_slot.icon");
    match &info.frame {
        Some(frame) => {
            ctx.ui_set_properties(
                &icon,
                &format!(
                    "visible: true; source: \"{ICON_SHEET}\"; rect_tag: \"{}\";",
                    escape(frame)
                ),
            );
        }
        None => {
            ctx.ui_set_visible(&icon, false);
        }
    }
}

fn status_text(snapshot: &item_stats::Snapshot) -> String {
    if snapshot.matches == 0 && snapshot.pending > 0 {
        return "Reading match records…".to_string();
    }
    if snapshot.matches == 0 {
        return "No matches with end-of-game items yet".into();
    }

    let mut text = format!("{} matches simmed", snapshot.matches);
    if snapshot.uncaptured > 0 {
        // Matches that predate loadout capture. Named rather than folded in:
        // they are real matches this table deliberately does not count, and a
        // total that quietly omitted them would look like data loss.
        text.push_str(&format!(" · {} before item tracking", snapshot.uncaptured));
    }
    text
}

// -- layout sources ---------------------------------------------------------

/// One table row, mirroring `statistics_component/champion.ui`: a `LeftToRight`
/// data band over a 1px separator, with the item's icon in the name cell the way
/// the champion table carries a portrait.
fn row_source(index: usize) -> String {
    let value_cell = |name: &str, width: u32| {
        format!(
            "#{name}:empty {{\n\
             width: {width}px;\n\
             height: 56px;\n\
             #text:label {{\n\
             @\"asset/base/style/main#label\";\n\
             x: 21px;\n\
             y: 18px;\n\
             height: 20px;\n\
             align_y: Center;\n\
             size: 18;\n\
             text: \"\";\n\
             }}\n\
             }}\n"
        )
    };

    format!(
        "row{index}:empty {{\n\
         width: 1600px;\n\
         height: 57px;\n\
         child_type: TopToBottom {{ spacing: 0px; }}\n\
         \n\
         #data:empty {{\n\
         width: 1600px;\n\
         height: 56px;\n\
         child_type: LeftToRight {{ spacing: 0px; }}\n\
         \n\
         {rank}\
         \n\
         #item_name:empty {{\n\
         width: {COL_NAME}px;\n\
         height: 56px;\n\
         \n\
         #icon_slot:color {{\n\
         x: 21px;\n\
         anchor_y: 0.5;\n\
         pivot_y: 0.5;\n\
         width: 40px;\n\
         height: 40px;\n\
         color: #1d1f2cff;\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         #icon:image {{\n\
         width: 36px;\n\
         height: 36px;\n\
         anchor_x: 0.5;\n\
         anchor_y: 0.5;\n\
         pivot_x: 0.5;\n\
         pivot_y: 0.5;\n\
         }}\n\
         }}\n\
         \n\
         #text:label {{\n\
         @\"asset/base/style/main#label\";\n\
         x: 66px;\n\
         y: 18px;\n\
         width: {name_w}px;\n\
         height: 20px;\n\
         align_y: Center;\n\
         size: 18;\n\
         fit_width: true;\n\
         text: \"\";\n\
         }}\n\
         }}\n\
         \n\
         {games}{win}{lose}{rate}{first}\
         \n\
         #most_used:empty {{\n\
         width: {COL_CHAMPS}px;\n\
         height: 56px;\n\
         \n\
         #container:empty {{\n\
         x: 21px;\n\
         width: 132px;\n\
         height: 40px;\n\
         anchor_y: 0.5;\n\
         pivot_y: 0.5;\n\
         child_type: LeftToRight {{ spacing: 4px; }}\n\
         {champs}\
         }}\n\
         }}\n\
         }}\n\
         \n\
         #line:color {{\n\
         width: 100%;\n\
         height: 1px;\n\
         color: #1d1f2cff;\n\
         }}\n\
         }}",
        rank = value_cell("rank", COL_RANK),
        // The label starts 66px in, behind the icon, so it gets what is left of
        // the cell. `fit_width` shrinks the longest names ("Radiant Sword of
        // Blossoming Dawn") rather than letting them run into the next column.
        name_w = COL_NAME - 66 - 12,
        games = value_cell("games", COL_GAMES),
        win = value_cell("win", COL_WIN),
        lose = value_cell("lose", COL_LOSE),
        rate = value_cell("win_rate", COL_RATE),
        first = value_cell("first_rate", COL_FIRST),
        // Three fixed slots rather than one spawn per row: the count never
        // changes, and the vanilla cell this copies is 132px wide — three 40px
        // slots and two 4px gaps, exactly. A slot with no champion is hidden,
        // which is why they are authored invisible.
        champs = (0..item_stats::TOP_CHAMPIONS)
            .map(|slot| {
                format!(
                    "#slot{slot}:color {{\n\
                     width: 40px;\n\
                     height: 40px;\n\
                     visible: false;\n\
                     color: #1d1f2cff;\n\
                     rounding: Uniform {{ rounding: 8; }}\n\
                     \n\
                     #icon:image {{\n\
                     width: 36px;\n\
                     height: 36px;\n\
                     anchor_x: 0.5;\n\
                     anchor_y: 0.5;\n\
                     pivot_x: 0.5;\n\
                     pivot_y: 0.5;\n\
                     }}\n\
                     }}\n"
                )
            })
            .collect::<String>(),
    )
}

/// A `text/ui` key as a document reference, where one resolves.
///
/// Handed to the label as a reference rather than as resolved text, so
/// LabelRunner does the lookup and the category button follows the game's
/// language the way the layout's own headings do. `ctx.i18n` answers in `en`
/// whatever the locale is, which is useless as a translation and exactly right
/// as the existence check this needs — an unresolvable key would otherwise be
/// drawn as a literal `#asset/...`.
///
/// Safe from a click handler, unlike `setting_get_json`: `i18n` reads through
/// the asset table, and asset calls are live there.
fn label(ctx: &StableClient<'_>, key: &str, fallback: &str) -> String {
    let path = format!("#asset/base/text/ui?{key}");
    match ctx.i18n(&path) {
        Some(text) if !text.is_empty() && !text.starts_with('#') => path,
        _ => fallback.to_string(),
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
