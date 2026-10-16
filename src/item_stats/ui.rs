//! Item Stats: a fourth tab on the statistics screen, beside Champ/Player/Team.
//! [`crate::item_stats`] owns the numbers; this file owns the tab and its
//! panel.
//!
//! The tab and panel are declared in the `ui/layout/statistics.ui` override
//! rather than spawned, because game code rebuilds this screen's subtrees on
//! tab switches and only declared nodes come back; only the rows are spawned,
//! and [`heal`] restores them. The vanilla filters are hidden while this tab is
//! open, since they do not filter it.

use std::collections::BTreeMap;
use std::sync::Mutex;

use mod_api_stable::*;

use crate::item_stats::{self, ItemInfo, Totals};
use crate::strategy_ui::{tab_style, ICON_SHEET};

// Frames between walks of the UI tree for the statistics screen;
// [`on_statistics_tab`] already keeps the walk to that screen.
const PROBE_EVERY: u32 = 5;

// How deep below the UI root [`find_screen`] looks; the screen sits at depth 2.
const SCREEN_DEPTH: u32 = 4;

// Nodes [`find_screen`] will visit before giving up for this sweep.
const SCREEN_NODES: u32 = 600;

// Frames between repaints while records are still being folded.
const REPAINT_EVERY: u32 = 12;

// Frames between checks that our spawned nodes are still there.
const HEAL_EVERY: u32 = 10;

// Frames between record sweeps while the screen is up (30 s). [`wire`] sweeps
// on arrival, so this only catches records that appear while it is open.
const SWEEP_EVERY: u32 = 1800;

// Rows spawned per repaint, so the ~150-row table does not spawn in one
// hitching frame.
const SPAWN_PER_REPAINT: usize = 60;

// Most rows the table will ever show.
const MAX_ROWS: usize = 300;

// Row pitch: 56px of data, a 1px separator, and the scroll view's 4px spacing.
const ROW_PITCH: usize = 61;

// Column widths, left to right. They leave part of the 1600px panel empty so
// the numbers stay near their headings.
const COL_RANK: u32 = 68;
const COL_NAME: u32 = 420;
const COL_GAMES: u32 = 140;
const COL_WIN: u32 = 130;
const COL_LOSE: u32 = 130;
const COL_RATE: u32 = 150;
const COL_PLAY: u32 = 150;
// Wider than the other rate columns: "Primeiro Item" and "Первый предмет" would
// run into the sort arrow at 130px.
const COL_FIRST: u32 = 160;
const COL_CHAMPS: u32 = 160;

// The three vanilla tabs and their panels, relative to the screen root.
const VANILLA: [(&str, &str); 3] = [
    ("tabs.champion", "data.champion"),
    ("tabs.athlete", "data.athlete"),
    ("tabs.team", "data.team"),
];

// The header controls that filter the vanilla tables and not this one.
const FILTERS: [&str; 4] = ["position", "patch", "year_filter", "league_filter"];

// Fills the patch list from the patches seen in the records. Unused rows are
// hidden rather than removed, since the list is declared in the layout.
fn refresh_patch_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let patches = item_stats::patches();

    // Written every time: a tab switch rebuilds these nodes blank, and caching
    // on `patch_rows` left the list empty for good.

    // Resolved first: `label` borrows the ctx and `ui_set_properties` wants it
    // mutably.
    let all = escape(&label(ctx, "item_stats.cat_all", "   All"));

    for row in 0..PATCH_ROWS {
        let path = format!("{screen}.item_patch_list.pat{row}");
        // Row 0 is All; the rest follow the patch list.
        let text = match row.checked_sub(1) {
            None => Some(all.clone()),
            // Patch names are not translated text, so the indent the category
            // rows get from i18n is added here.
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

    // Sized to the patches on offer rather than the whole row pool.
    let shown = 1 + patches.len().min(PATCH_ROWS - 1);
    ctx.ui_set_properties(
        &format!("{screen}.item_patch_list"),
        &format!(
            "height: {}px;",
            shown * PATCH_ROW_HEIGHT + PATCH_LIST_PADDING
        ),
    );

    // A patch no longer listed cannot stay selected, or nothing could unselect
    // it.
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

// The item categories the filter offers, in the order the list declares them.
// Row 0 is "All"; row `i` is `CATEGORIES[i - 1]`.
const CATEGORIES: [&str; 6] = crate::item_catalog::CATEGORY_ORDER;

// Row height and list padding as the layout declares them, for resizing the
// list.
const PATCH_ROW_HEIGHT: usize = 36;
const PATCH_LIST_PADDING: usize = 8;

// Indent for dropdown row text. It lives in the text because moving the node
// would move its highlight too, and a `label:` block takes no position.
const ROW_INDENT: &str = "   ";

// Rows the patch list declares: All, then up to eleven patches, newest first.
const PATCH_ROWS: usize = 12;

// Radiant, as the item documents number tiers: 0..=4 is
// starter/basic/epic/legendary/radiant.
const TIER_RADIANT: usize = 4;

// The tier the filter opens on: Radiant, since "All" is led by components like
// the BF Sword, which out-sample every finished item.
const DEFAULT_TIER: Option<usize> = Some(TIER_RADIANT);

// i18n keys for the tier rows, All first — parallel to the `#tier{i}` nodes.
// Row `i` filters to tier `i - 1`, which is the item's own `tier` field.
const TIER_KEYS: [&str; 6] = [
    "item_stats.cat_all",
    "item_stats.tier_starter",
    "item_stats.tier_basic",
    "item_stats.tier_epic",
    "item_stats.tier_legendary",
    "item_stats.tier_radiant",
];

// Lane icons for the button face, by `LaneV1` code: the game's own 18x18 SVGs,
// so `source` alone is the reference. [`LANE_KEYS`] holds the row labels, All
// first, row `i` filtering to lane `i - 1`.
const LANE_ICONS: [&str; 5] = [
    "asset/base/ui/icons/top",
    "asset/base/ui/icons/jungle",
    "asset/base/ui/icons/mid",
    "asset/base/ui/icons/bottom",
    "asset/base/ui/icons/support",
];

const LANE_KEYS: [&str; 6] = [
    "item_stats.cat_all",
    "item_stats.lane_top",
    "item_stats.lane_jungle",
    "item_stats.lane_mid",
    "item_stats.lane_bottom",
    "item_stats.lane_support",
];

// i18n keys for the list rows, All first — parallel to the `#cat{i}` nodes.
const CATEGORY_KEYS: [&str; 7] = [
    "item_stats.cat_all",
    "item_stats.cat_assassin",
    "item_stats.cat_fighter",
    "item_stats.cat_marksman",
    "item_stats.cat_mage",
    "item_stats.cat_tank",
    "item_stats.cat_support",
];

// Which column the table is ordered by.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SortBy {
    // Display name, so it follows the game's language rather than the key.
    Item,
    Games,
    Wins,
    Losses,
    #[default]
    WinRate,
    // Times built per match.
    PlayRate,
    // Share of this item's buys where it went in the first slot.
    FirstRate,
}

impl SortBy {
    // Header node under `#header`, and the click path that selects this column.
    fn node(self) -> &'static str {
        match self {
            SortBy::Item => "item_name",
            SortBy::Games => "games",
            SortBy::Wins => "win",
            SortBy::Losses => "lose",
            SortBy::WinRate => "win_rate",
            SortBy::PlayRate => "play_rate",
            SortBy::FirstRate => "first_rate",
        }
    }

    // Direction on first click: descending for the numbers, ascending for the
    // name.
    fn starts_descending(self) -> bool {
        self != SortBy::Item
    }

    const ALL: [SortBy; 7] = [
        SortBy::Item,
        SortBy::Games,
        SortBy::Wins,
        SortBy::Losses,
        SortBy::WinRate,
        SortBy::PlayRate,
        SortBy::FirstRate,
    ];
}

#[derive(Default)]
struct State {
    // Statistics screen path once found; cleared when the screen goes away so
    // the next visit searches again.
    screen: Option<String>,
    // Whether every handler is registered for the current screen.
    wired: bool,
    // Whether this tab is the one currently showing.
    showing: bool,
    // Row nodes spawned so far.
    spawned: usize,
    // Rows currently visible, so a shrinking table hides its tail.
    shown: usize,
    // Set when new records land; cleared by a repaint.
    dirty: bool,
    // Category filter, as an index into [`CATEGORIES`]. `None` is "All".
    category: Option<usize>,
    // Tier filter — the item's own `tier`, 0..=4. `None` is "All".
    tier: Option<usize>,
    tier_open: bool,
    // Lane filter — `LaneV1` as its code, 0..=4. `None` is "All".
    lane: Option<usize>,
    lane_open: bool,
    // Patch filter. `None` is "All".
    patch: Option<String>,
    // Patches currently offered by the list, in row order.
    patch_rows: Vec<String>,
    // Whether the category list is dropped down.
    list_open: bool,
    // Whether the patch list is dropped down. Only ever one of the two.
    patch_open: bool,
    sort: SortBy,
    // Named for the non-default direction, so the derived `Default` opens on
    // win rate, highest first.
    ascending: bool,
    tick: u32,
    // Counts every frame the screen is up; `tick` advances only while probing
    // or showing.
    sweep_tick: u32,
    // [`item_stats::folds`] when the table was last marked for a repaint.
    folds: u64,
    // The last event acted on, as `(path, frame)`. See [`already_handled`].
    last_event: Option<(String, u32)>,
}

impl State {
    // A fresh state with the filters at their opening positions.
    fn new() -> Self {
        Self {
            tier: DEFAULT_TIER,
            ..Self::default()
        }
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(State::new)))
}

// Paths with a registered handler. Handlers outlive their nodes, so registering
// a live path twice runs the handler twice; cleared only on teardown.
static REGISTERED: crate::ui::Registered = crate::ui::Registered::new(handle_event);

// Per-frame entry, called from the client hook before its strategy-screen early
// return.
pub fn sync(ctx: &mut StableClient<'_>) {
    let Some(screen) = resolve_screen(ctx) else {
        return;
    };

    // Frames on this screen, for the housekeeping cadences below.
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

    // Run whether or not the tab is open, and from here rather than its click
    // handler: `setting_get_json` and `record_get_json` do not work inside a UI
    // handler.
    item_stats::prime_catalog(ctx);
    if item_stats::pump(ctx) {
        let _ = with_state(|state| state.dirty = true);
    }
    // Repaint for matches folded by `item_stats::sync` earlier in the frame.
    let folds = item_stats::folds();
    let _ = with_state(|state| {
        if state.folds != folds {
            state.folds = folds;
            state.dirty = true;
        }
    });

    if frame % SWEEP_EVERY == 0 {
        item_stats::sweep(ctx);
    }

    if !with_state(|state| state.showing).unwrap_or(false) {
        return;
    }

    // Game code re-shows its own panels and filters from its own tab state, so
    // they are hidden every frame, not once.
    for (_, panel) in VANILLA {
        ctx.ui_set_visible(&format!("{screen}.{panel}"), false);
    }
    for filter in FILTERS {
        ctx.ui_set_visible(&format!("{screen}.{filter}"), false);
    }
    // The panel and dropdowns are authored hidden, so a rebuild hides them
    // again; they are asserted every frame too.
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), true);
    ctx.ui_set_visible(&format!("{screen}.item_category"), true);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), true);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), true);
    ctx.ui_set_visible(&format!("{screen}.item_lane"), true);

    // The catcher is a full-screen button that would eat every click if left
    // up, so it follows state every frame.
    let (list_open, patch_open, tier_open, lane_open) = with_state(|state| {
        (
            state.list_open,
            state.patch_open,
            state.tier_open,
            state.lane_open,
        )
    })
    .unwrap_or((false, false, false, false));
    ctx.ui_set_visible(&format!("{screen}.item_category_list"), list_open);
    ctx.ui_set_visible(&format!("{screen}.item_patch_list"), patch_open);
    ctx.ui_set_visible(&format!("{screen}.item_tier_list"), tier_open);
    ctx.ui_set_visible(&format!("{screen}.item_lane_list"), lane_open);
    ctx.ui_set_visible(
        &format!("{screen}.item_category_catch"),
        list_open || patch_open || tier_open || lane_open,
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

// Restores the rows after game code rebuilds the panel on a tab switch; missing
// rows are the signal. Rate-limited to [`HEAL_EVERY`] in case game code ever
// rebuilds continuously.
fn heal(ctx: &mut StableClient<'_>, screen: &str) {
    if !with_state(|state| state.wired).unwrap_or(false) {
        return;
    }

    // The layout's own nodes gone means the override stopped applying: start
    // over.
    if !ctx.ui_exists(&format!("{screen}.tabs.item"))
        || !ctx.ui_exists(&format!("{screen}.data.item_stats"))
    {
        let _ = with_state(|state| *state = State::new());
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

    let _ = with_state(|state| {
        // Handlers outlive their nodes, so `REGISTERED` is left alone:
        // re-registering would make one click run twice.
        state.spawned = 0;
        state.shown = 0;
        // `showing` stays set: a rebuild is not the player leaving, and the
        // per-frame block puts the panel back. Open lists close.
        state.list_open = false;
        state.patch_open = false;
        state.tier_open = false;
        state.lane_open = false;
        state.dirty = true;
    });

    // Repaint what the rebuild reset and no per-frame assert covers: the
    // dropdown faces, headers and tab highlight.
    if with_state(|state| state.showing).unwrap_or(false) {
        paint_category_button(ctx, screen);
        paint_patch_button(ctx, screen);
        paint_tier_button(ctx, screen);
        paint_lane_button(ctx, screen);
        paint_headers(ctx, screen);
        paint_tabs(ctx, screen, true);
    }
}

// The statistics screen path, or `None` when it is not up. It is searched for
// because this layout's root is `statistics`, not `main`: the walk looks for
// the node holding `tabs.champion`, throttled to [`PROBE_EVERY`].
fn resolve_screen(ctx: &mut StableClient<'_>) -> Option<String> {
    if let Some(screen) = with_state(|state| state.screen.clone()).flatten() {
        if ctx.ui_exists(&format!("{screen}.tabs.champion")) {
            return Some(screen);
        }
        // Left the screen: everything spawned in it is gone, so the next visit
        // starts over.
        let _ = with_state(|state| {
            *state = State::new();
        });
        REGISTERED.forget();
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
    let _ = with_state(|state| state.screen = Some(found.clone()));
    Some(found)
}

// Whether the client is on the Statistics main tab, which keeps [`find_screen`]
// off every other screen. Matched loosely, and `None` (the host would not say)
// counts as yes.
fn on_statistics_tab(ctx: &StableClient<'_>) -> bool {
    let Some(tab) = ctx.client_main_tab() else {
        return true;
    };

    tab.to_ascii_lowercase().contains("statistic")
}

// Breadth-first from the UI root for the node holding `tabs.champion`, a shape
// no other screen has.
fn find_screen(ctx: &StableClient<'_>) -> Option<String> {
    let mut level: Vec<String> = ctx.ui_child_names("");

    if level.is_empty() {
        // A root that enumerates nothing would make this inert forever, so seed
        // it with the known scene roots.
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
            // A depth limit alone does not bound one wide level, and this runs
            // on the UI thread.
            budget = budget.saturating_sub(1);
            if budget == 0 {
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

// Registers every handler once the screen is fully built, then sweeps the
// records.
fn wire(ctx: &mut StableClient<'_>, screen: &str) {
    let paths = handler_paths(screen);

    // Wait until every handled node and the panel exist: the screen is found
    // before its dropdowns arrive, and registering only part of it left those
    // dead for the whole visit.
    let panel = format!("{screen}.data.item_stats");
    if paths
        .iter()
        .chain(std::iter::once(&panel))
        .any(|path| !ctx.ui_exists(path))
    {
        // Normal for the frame or two a screen takes to build; `sync` retries
        // next frame.
        return;
    }

    for path in &paths {
        REGISTERED.register(ctx, path);
    }

    // Swept once per visit, not per frame: it copies every id across the ABI,
    // and no match is simmed while the player is here.
    item_stats::sweep(ctx);

    let _ = with_state(|state| {
        state.wired = true;
        state.dirty = true;
    });
}

// Every node this module handles clicks on, in one list so readiness and
// registration agree.
fn handler_paths(screen: &str) -> Vec<String> {
    let mut paths = Vec::new();
    // Handlers on the vanilla tabs only observe, to put this panel away; game
    // code's own handling is untouched.
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
    paths.push(format!("{screen}.item_lane"));
    for row in 0..LANE_KEYS.len() {
        paths.push(format!("{screen}.item_lane_list.lane{row}"));
    }
    paths
}

// Whether this exact event was already acted on this frame. Handlers are
// permanent and keyed by path, so a revisit leaves two on each path, and since
// most actions here toggle, a second delivery would undo the first.
fn already_handled(path: &str) -> bool {
    with_state(|state| {
        // `sweep_tick` advances once a frame and nobody clicks one node twice
        // in a frame, so a repeat is a second delivery.
        let stamp = (path.to_string(), state.sweep_tick);
        let duplicate = state.last_event.as_ref() == Some(&stamp);
        state.last_event = Some(stamp);
        duplicate
    })
    .unwrap_or(false)
}

fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // Only clicks: `Remove` fires as the screen is torn down and would
    // otherwise open a panel into a dying tree. An unreported kind counts as a
    // click.
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
            state.lane_open = false;
            state.list_open
        })
        .unwrap_or(false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_lane_list(ctx, &screen, false);
        show_category_list(ctx, &screen, open);
        return;
    }

    if event.path == format!("{screen}.item_patch") {
        let open = with_state(|state| {
            state.patch_open = !state.patch_open;
            state.list_open = false;
            state.tier_open = false;
            state.lane_open = false;
            state.patch_open
        })
        .unwrap_or(false);
        show_category_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_lane_list(ctx, &screen, false);
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
            state.lane_open = false;
            state.tier_open
        })
        .unwrap_or(false);
        show_category_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, false);
        show_lane_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, open);
        return;
    }

    for row in 0..TIER_KEYS.len() {
        if event.path == format!("{screen}.item_tier_list.tier{row}") {
            pick_tier(ctx, &screen, row);
            return;
        }
    }

    if event.path == format!("{screen}.item_lane") {
        let open = with_state(|state| {
            state.lane_open = !state.lane_open;
            state.list_open = false;
            state.patch_open = false;
            state.tier_open = false;
            state.lane_open
        })
        .unwrap_or(false);
        show_category_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_lane_list(ctx, &screen, open);
        return;
    }

    for row in 0..LANE_KEYS.len() {
        if event.path == format!("{screen}.item_lane_list.lane{row}") {
            pick_lane(ctx, &screen, row);
            return;
        }
    }

    if event.path == format!("{screen}.item_category_catch") {
        let _ = with_state(|state| {
            state.list_open = false;
            state.patch_open = false;
            state.tier_open = false;
            state.lane_open = false;
        });
        show_category_list(ctx, &screen, false);
        show_patch_list(ctx, &screen, false);
        show_tier_list(ctx, &screen, false);
        show_lane_list(ctx, &screen, false);
        return;
    }

    for row in 0..CATEGORY_KEYS.len() {
        if event.path == format!("{screen}.item_category_list.cat{row}") {
            pick_category(ctx, &screen, row);
            return;
        }
    }

    // Every event on a vanilla tab lands here, so act only when this tab is up.
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
    // Heal first, so a click between a rebuild and the throttled heal does not
    // paint onto missing rows.
    heal(ctx, screen);

    for (_, panel) in VANILLA {
        ctx.ui_set_visible(&format!("{screen}.{panel}"), false);
    }
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), true);
    ctx.ui_set_visible(&format!("{screen}.item_category"), true);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), true);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), true);
    ctx.ui_set_visible(&format!("{screen}.item_lane"), true);
    paint_category_button(ctx, screen);
    refresh_patch_rows(ctx, screen);
    paint_patch_button(ctx, screen);
    paint_tier_button(ctx, screen);
    paint_lane_button(ctx, screen);
    paint_tabs(ctx, screen, true);
    // The layout authors every arrow transparent, so show which column sorts.
    paint_headers(ctx, screen);

    let _ = with_state(|state| state.showing = true);
    repaint(ctx, screen);
}

// Leaves this tab for `panel`, the clicked vanilla tab's panel. Shown here
// rather than by game code: that tab never stopped being selected, so game code
// may treat the click as a no-op.
fn close(ctx: &mut StableClient<'_>, screen: &str, panel: &str) {
    ctx.ui_set_visible(&format!("{screen}.data.item_stats"), false);
    ctx.ui_set_visible(&format!("{screen}.item_category"), false);
    ctx.ui_set_visible(&format!("{screen}.item_patch"), false);
    ctx.ui_set_visible(&format!("{screen}.item_tier"), false);
    ctx.ui_set_visible(&format!("{screen}.item_lane"), false);
    let _ = with_state(|state| {
        state.list_open = false;
        state.patch_open = false;
        state.tier_open = false;
        state.lane_open = false;
    });
    show_category_list(ctx, screen, false);
    show_patch_list(ctx, screen, false);
    show_tier_list(ctx, screen, false);
    show_lane_list(ctx, screen, false);
    ctx.ui_set_visible(&format!("{screen}.{panel}"), true);
    for filter in FILTERS {
        ctx.ui_set_visible(&format!("{screen}.{filter}"), true);
    }
    paint_tabs(ctx, screen, false);
    let _ = with_state(|state| state.showing = false);
}

// Shows or hides the category list and the full-screen catcher that closes it
// on any other click.
fn show_category_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_category_list"), open);
    if open {
        paint_category_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

// Shows or hides the patch list, sharing the category list's catcher.
fn show_patch_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_patch_list"), open);
    if open {
        // Refreshed on open; the set changes only while the scan runs.
        refresh_patch_rows(ctx, screen);
        paint_patch_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

// Applies the patch on row `row` and closes the list.
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

// Writes the selected patch onto the button face, as a literal since a version
// string is not translated text.
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

// Lights the row of the patch currently in force.
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

// Shows or hides the tier list, sharing the one catcher.
fn show_tier_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_tier_list"), open);
    if open {
        paint_tier_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

// Applies the tier on row `row` and closes the list.
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

// Writes the selected tier onto the button face.
fn paint_tier_button(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.tier).unwrap_or(DEFAULT_TIER);
    let key = TIER_KEYS[selected.map_or(0, |tier| tier + 1)];
    let text = label(ctx, key, "   All");
    ctx.ui_set_text(&format!("{screen}.item_tier.text"), &text);
}

// Lights the row of the tier currently in force.
fn paint_tier_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.tier).unwrap_or(DEFAULT_TIER);
    let lit_row = selected.map_or(0, |tier| tier + 1);
    for row in 0..TIER_KEYS.len() {
        ctx.ui_set_properties(
            &format!("{screen}.item_tier_list.tier{row}"),
            &tab_style("image", "label", row == lit_row),
        );
    }
}

// Shows or hides the lane list, sharing the one catcher.
fn show_lane_list(ctx: &mut StableClient<'_>, screen: &str, open: bool) {
    ctx.ui_set_visible(&format!("{screen}.item_lane_list"), open);
    if open {
        paint_lane_rows(ctx, screen);
    }
    sync_catch(ctx, screen);
}

// Applies the lane on row `row` and closes the list.
fn pick_lane(ctx: &mut StableClient<'_>, screen: &str, row: usize) {
    let _ = with_state(|state| {
        // Row 0 is All; row `i` is lane `i - 1`.
        state.lane = row.checked_sub(1);
        state.lane_open = false;
        state.dirty = true;
    });
    show_lane_list(ctx, screen, false);
    paint_lane_button(ctx, screen);
    repaint(ctx, screen);
}

// Writes the selected lane onto the button face as its icon, which lets the
// button be 96px wide. "All" has no icon, so it shows the word.
fn paint_lane_button(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.lane).unwrap_or(None);
    let icon = format!("{screen}.item_lane.icon");
    let text = format!("{screen}.item_lane.text");

    match selected.and_then(|lane| LANE_ICONS.get(lane)) {
        Some(source) => {
            ctx.ui_set_properties(&icon, &format!("visible: true; source: \"{source}\";"));
            ctx.ui_set_visible(&text, false);
        }
        None => {
            // Resolved first: `label` borrows the ctx and `ui_set_text` wants
            // it mutably.
            let all = label(ctx, LANE_KEYS[0], "   All");
            ctx.ui_set_visible(&icon, false);
            ctx.ui_set_visible(&text, true);
            ctx.ui_set_text(&text, &all);
        }
    }
}

// Lights the row of the lane currently in force.
fn paint_lane_rows(ctx: &mut StableClient<'_>, screen: &str) {
    let selected = with_state(|state| state.lane).unwrap_or(None);
    let lit_row = selected.map_or(0, |lane| lane + 1);
    for row in 0..LANE_KEYS.len() {
        ctx.ui_set_properties(
            &format!("{screen}.item_lane_list.lane{row}"),
            &tab_style("image", "label", row == lit_row),
        );
    }
}

// Puts the catcher up while any list is down; one place, so a new dropdown
// cannot strand it.
fn sync_catch(ctx: &mut StableClient<'_>, screen: &str) {
    let any = with_state(|state| {
        state.list_open || state.patch_open || state.tier_open || state.lane_open
    })
    .unwrap_or(false);
    ctx.ui_set_visible(&format!("{screen}.item_category_catch"), any);
}

// Applies the category on row `row` and closes the list.
fn pick_category(ctx: &mut StableClient<'_>, screen: &str, row: usize) {
    let _ = with_state(|state| {
        // Row 0 is All, the absence of a filter.
        state.category = row.checked_sub(1);
        state.list_open = false;
        state.dirty = true;
    });
    show_category_list(ctx, screen, false);
    paint_category_button(ctx, screen);
    repaint(ctx, screen);
}

// Writes the selected category onto the button face as a document reference, so
// it follows the game's language.
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

// Lights the row of the category currently in force.
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

// The category an item belongs to; `None` for components and the game's lower
// tiers, which appear only under "All".
fn category_of(key: &str) -> Option<&'static str> {
    crate::item_catalog::category_of(crate::build_config::base_slug(key))
}

fn header_path(screen: &str, column: SortBy) -> String {
    format!("{screen}.data.item_stats.header.{}", column.node())
}

// Re-orders by `column`: the current column flips direction, a new one starts
// at [`SortBy::starts_descending`].
fn sort_by(ctx: &mut StableClient<'_>, screen: &str, column: SortBy) {
    // Heal first, as `open` does.
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

// Colours the sorted column's arrow, `dropdown` or `dropdown_up` for the
// direction; the layout authors the rest transparent.
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

// Paints which tab looks selected, since the host rejects `selected` writes on
// `color_selectable` tabs. This tab draws through `image`/`label`; the vanilla
// tab game code thinks is selected draws `selected_image`/`selected_label`, so
// all three are dulled through those.
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

// Orders the table by the chosen column, ending every comparison on the key so
// tied rows do not swap between repaints. Thin samples are not demoted; the
// games column shows the sample size.
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
            // Every row shares one match count, so play rate orders as games
            // do.
            SortBy::Games | SortBy::PlayRate => a.games.cmp(&b.games),
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
    let (patch, lane) =
        with_state(|state| (state.patch.clone(), state.lane)).unwrap_or((None, None));
    let mut snapshot = item_stats::snapshot(patch.as_deref(), lane);
    let catalog = item_stats::catalog();
    let contents = format!("{screen}.data.item_stats.data.contents");

    let (sort, ascending, category, tier) =
        with_state(|state| (state.sort, state.ascending, state.category, state.tier)).unwrap_or((
            SortBy::default(),
            false,
            None,
            DEFAULT_TIER,
        ));
    if let Some(index) = category {
        let wanted = CATEGORIES[index];
        snapshot
            .rows
            .retain(|(key, _)| category_of(key) == Some(wanted));
    }
    if let Some(wanted) = tier {
        // An item with no tier answers no tier filter.
        snapshot
            .rows
            .retain(|(key, _)| catalog.get(key).and_then(|info| info.tier) == Some(wanted));
    }
    order_rows(&mut snapshot.rows, &catalog, sort, ascending);

    let wanted = snapshot.rows.len().min(MAX_ROWS);

    // Spawned in bounded batches, so the table fills in over a few repaints
    // instead of stalling.
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
            // Nothing describes this key (a save from another item set): show
            // it raw, with no tier.
            name: key.clone(),
            frame: None,
            tier: None,
        });
        let champions = snapshot
            .champions
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default();
        write_row(
            ctx,
            &contents,
            index,
            index + 1,
            &info,
            totals,
            snapshot.matches,
            champions,
        );
    }

    // Hide the tail of a shrinking table; the nodes stay for reuse.
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

    // Ask for another repaint until every row is spawned; nothing else sets
    // `dirty` once the scan is done.
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
    matches: u32,
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
    // Times built per match, to one decimal like the other rates.
    ctx.ui_set_text(
        &cell("play_rate"),
        &totals
            .play_rate(matches)
            .map(|rate| format!("{rate:.1}%"))
            .unwrap_or_else(|| "-".into()),
    );
    // How often it was the item rushed; a dash for no games, like the win rate.
    ctx.ui_set_text(
        &cell("first_rate"),
        &totals
            .first_rate()
            .map(|rate| format!("{rate:.1}%"))
            .unwrap_or_else(|| "-".into()),
    );

    // The champions that bought it most, in fixed slots hidden when empty. The
    // item icon below is set as `source` plus `rect_tag`: the `.ui` parser has
    // no `sheet#tag` form.
    let strip = format!("{row}.data.most_used.container");
    for slot in 0..item_stats::TOP_CHAMPIONS {
        let path = format!("{strip}.slot{slot}");
        match champions.get(slot).filter(|name| !name.is_empty()) {
            Some(champion) => {
                // 36x36 at scale 2.0, as the game draws its own portraits this
                // size. Shown only if it draws: a disabled mod's champion has
                // no sheet.
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

    // Just the count: anything longer runs under the "Purchased On" heading.
    format!("{} matches simmed", snapshot.matches)
}

// -- layout sources ---------------------------------------------------------

// One table row, mirroring `statistics_component/champion.ui`: a `LeftToRight`
// data band over a 1px separator, with the item's icon in the name cell the way
// the champion table carries a portrait.
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
         {games}{win}{lose}{rate}{play}{first}\
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
        // The label starts 66px in, after the icon; `fit_width` shrinks the
        // longest names to fit.
        name_w = COL_NAME - 66 - 12,
        games = value_cell("games", COL_GAMES),
        win = value_cell("win", COL_WIN),
        lose = value_cell("lose", COL_LOSE),
        rate = value_cell("win_rate", COL_RATE),
        play = value_cell("play_rate", COL_PLAY),
        first = value_cell("first_rate", COL_FIRST),
        // Three fixed slots, authored hidden: 40px each with 4px gaps fill the
        // vanilla cell's 132px.
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

// A `text/ui` key as a document reference, so the label follows the game's
// language; `ctx.i18n` (English whatever the locale) only checks that it
// resolves. Safe in a click handler, since asset calls are live there.
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
