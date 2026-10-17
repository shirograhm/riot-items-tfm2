//! In-game Item Build Editor: the **Builds** tab on the strategy screen, and
//! the composition test's build setup. One row per champion (and role) with its
//! item slots, a category-grouped item list and per-row buttons; it reads and
//! writes `item-builds.json` beside the DLL, which the hooks apply from the
//! next match.
//!
//! The `strategy.ui` override puts `#builds` in place of the game's Personal
//! tab, whose per-athlete rule the hook cannot act on. The vanilla panels are
//! hidden while the tab is up (their portrait tooltips are drawn outside the
//! subtree and cannot be covered), [`open_editor`] and [`close_editor`]
//! arbitrate with Team, and the right-hand Matchup card stays showing.
//!
//! Builds are keyed by champion because the item-build hook builds one team per
//! call with nothing saying whose, so a per-position rule reached both teams.
//! The panel chrome (`ui/layout/build_editor.ui`) is compiled in and spawned
//! from source, since `override_info` cannot declare a standalone asset.
//!
//! There are no real dropdowns (game code fills their options), so each combo
//! is a button opening a floating `selectable` list. Nothing carries `z`, which
//! is not inherited and makes a drawing node paint over its own children: the
//! lists win by being the window root's last children, and child images take
//! `ignore_event: true` so the button gets the click.
//!
//! The search box spawns only matching rows, each named for its index in the
//! saved list, so paths keep their meaning under a filter. Every path is
//! registered once through `REGISTERED` ([`super::Registered`]), forgotten when
//! the screen goes away and only then: a set kept for the process left the
//! editor dead on every later screen.

use std::sync::{Arc, Mutex, OnceLock};

use mod_api_stable::*;

use super::draft_watch;
use crate::build_config::{self, picker_slots, ChampionRow, Role};
use crate::item_catalog;
use crate::tactics;

// The editor's text as `#asset/…?key` references the game resolves at draw time
// in the active locale, since `ctx.i18n()` always answers in English.
// LabelRunner substitutes only whole-string labels, so every key is a complete
// label and anything concatenated here stays English; `text/ui.i18n` is merged
// into the base document for this ([`UI_TEXT_DOC`]).
#[derive(Clone)]
struct Strings {
    tab: String,
    add: String,
    filter: String,
    hint: String,
    col_champion: String,
    col_role: String,
    // One complete label per column — LabelRunner cannot compose `"ITEM " + n`.
    col_items: [String; 6],
    // The two cells of each footer toggle. Both are on screen at once, so these
    // name the choice a cell makes rather than describing the current state.
    unique_on: String,
    unique_off: String,
    scope_all: String,
    scope_own: String,
    // The two cells under the Matchup card. Each names the side whose drafted
    // champions it puts in the filter box.
    side_blue: String,
    side_red: String,
    save: String,
    ai_slot: String,
    no_champion: String,
}

impl Default for Strings {
    fn default() -> Self {
        Self {
            tab: "Builds".into(),
            add: "+ Add Champion".into(),
            filter: "filter by champion(s)...".into(),
            hint: "Builds are per champion. A blank slot is filled by the game, in the AI's own pick order.".into(),
            col_champion: "CHAMPION".into(),
            col_role: "ROLE".into(),
            col_items: [
                "ITEM 1".into(),
                "ITEM 2".into(),
                "ITEM 3".into(),
                "ITEM 4".into(),
                "ITEM 5".into(),
                "ITEM 6".into(),
            ],
            unique_on: "Enforce Smart Builds".into(),
            unique_off: "Allow Any Builds".into(),
            scope_all: "Apply To All Players".into(),
            scope_own: "Apply To Your Players Only".into(),
            side_blue: "Blue Team".into(),
            side_red: "Red Team".into(),
            save: "Save Item Builds".into(),
            ai_slot: AI_SLOT_LABEL_FALLBACK.into(),
            no_champion: NO_CHAMPION_LABEL_FALLBACK.into(),
        }
    }
}

// Where the editor's labels live. `text/ui.i18n` is merged into this base
// document by `mod.override_info`; LabelRunner resolves references against the
// base documents, so the merge is what makes the references work.
const UI_TEXT_DOC: &str = "#asset/base/text/ui";

static STRINGS: Mutex<Option<Arc<Strings>>> = Mutex::new(None);

// The resolved text, or the English defaults until [`load_strings`] has run.
fn strings() -> Arc<Strings> {
    STRINGS
        .lock()
        .ok()
        .and_then(|strings| strings.clone())
        .unwrap_or_else(|| Arc::new(Strings::default()))
}

// Resolves the editor's text for the game's current language.
fn load_strings(ctx: &StableClient<'_>) {
    let fallback = Strings::default();
    // A reference only once the key is known to resolve, or it would draw as
    // literal `#asset/...`; `ctx.i18n` answers in English whatever the locale,
    // which makes it an existence check.
    let reference = |key: &str, fallback: &str| {
        let path = format!("{UI_TEXT_DOC}?builds.{key}");
        match ctx.i18n(&path) {
            Some(text) if !text.is_empty() && !text.starts_with('#') => path,
            _ => fallback.to_string(),
        }
    };
    // For text this mod concatenates: LabelRunner never sees it as a whole
    // label, so it has to be resolved here and stays `en`.
    let resolved = |key: &str, fallback: &str| {
        ctx.i18n(&format!("{UI_TEXT_DOC}?builds.{key}"))
            .filter(|text| !text.is_empty() && !text.starts_with('#'))
            .unwrap_or_else(|| fallback.to_string())
    };
    let resolved_strings = Strings {
        tab: reference("tab", &fallback.tab),
        add: reference("add_champion", &fallback.add),
        hint: reference("hint", &fallback.hint),
        col_champion: reference("col_champion", &fallback.col_champion),
        col_role: reference("col_role", &fallback.col_role),
        col_items: std::array::from_fn(|i| {
            reference(&format!("col_item{}", i + 1), &fallback.col_items[i])
        }),
        unique_on: reference("unique_on", &fallback.unique_on),
        unique_off: reference("unique_off", &fallback.unique_off),
        scope_all: reference("scope_all", &fallback.scope_all),
        scope_own: reference("scope_own", &fallback.scope_own),
        side_blue: reference("side_blue", &fallback.side_blue),
        side_red: reference("side_red", &fallback.side_red),
        save: reference("save", &fallback.save),
        // A placeholder is a whole label, so it takes a reference like the rest.
        // Confirmed against the bundle: all 21 `placeholder:` values the game
        // ships are `#asset/...` references, so the engine resolves them there.
        filter: reference("filter_placeholder", &fallback.filter),
        ai_slot: resolved("ai_slot", &fallback.ai_slot),
        no_champion: resolved("no_champion", &fallback.no_champion),
    };
    if let Ok(mut strings) = STRINGS.lock() {
        *strings = Some(Arc::new(resolved_strings));
    }
}

// Writes the resolved text onto the nodes that carry it literally in the
// layout, whose English stands if this never runs. Buttons take
// `ui_set_properties`, since `ui_set_text` does not reach their nested `text`
// block.
fn apply_strings(ctx: &mut StableClient<'_>) {
    let strings = strings();
    let escape = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");

    for (path, text) in [(add_path(), &strings.add), (save_path(), &strings.save)] {
        ctx.ui_set_properties(path, &format!("text: {{ text: \"{}\"; }}", escape(text)));
    }
    ctx.ui_set_properties(
        search_path(),
        &format!("placeholder: \"{}\";", escape(&strings.filter)),
    );
    ctx.ui_set_text(hint_path(), &strings.hint);
    ctx.ui_set_text(
        &format!("{}.c_champion", colheader_path()),
        &strings.col_champion,
    );
    ctx.ui_set_text(&format!("{}.c_role", colheader_path()), &strings.col_role);
    // The tab and toggle cells carry `text` directly, so they take the plain
    // form. Labelled once here: a cell's label is what it is, and only which
    // one is lit changes.
    for (path, text) in [
        (BUILDS_TAB, &strings.tab),
        (unique_on_path(), &strings.unique_on),
        (unique_off_path(), &strings.unique_off),
        (scope_all_path(), &strings.scope_all),
        (scope_own_path(), &strings.scope_own),
        (side_blue_path(), &strings.side_blue),
        (side_red_path(), &strings.side_red),
    ] {
        ctx.ui_set_properties(path, &format!("text: \"{}\";", escape(text)));
    }
}

// Shown for a slot left to the game's own AI, and on the list row that clears a
// slot: the vanilla screen's own wording (`strategy.i18n`'s `build_auto`).
const AI_SLOT_LABEL_FALLBACK: &str = "Let Player Decide";

// What an unpinned slot button shows, in every language, padded so the dash
// sits where a pinned slot's icon is centred.
const EMPTY_SLOT_LABEL: &str = "     -";

// Shown for a row whose champion has not been chosen yet. Such a row is kept in
// the editor but never written.
const NO_CHAMPION_LABEL_FALLBACK: &str = "(champion)";

// The strategy screen's container and the editor's host there. Authored inside
// `strategy.ui` (root `main`), so it exists only while that screen is loaded;
// `main.contents.<other screen>` is wrong (see [`resolve_comp_tactics`]).
const UI_ROOT: &str = "main.contents";

// The Builds tab, added by the `strategy.ui` override beside Team. It exists
// only in the patched screen, so its presence doubles as the probe for "our
// layout is loaded" — rename it here and in `strategy.ui` together.
const BUILDS_TAB: &str = "main.contents.strategy.mode_toggle.builds";

// The game's remaining tab, watched so clicking it puts the editor away; game
// code then restores the Team panels itself.
const TEAM_TAB: &str = "main.contents.strategy.mode_toggle.team";

// The vanilla Personal panel, hidden on entry since game code may pick it when
// building the screen.
const PERSONAL_PATH: &str = "main.contents.strategy.personal";

// The screen's "Item Info" button, which game code keeps hiding on the Team
// tab; re-shown every frame the Builds tab is up.
const ITEM_INFO_BTN: &str = "main.contents.strategy.item_info_btn";

// The content panels hidden while the Builds tab is up: the Personal rows'
// portrait tooltips are drawn outside this subtree and cannot be covered, and
// `#sub1`-`#sub3` would show around the edges. `#sub4` stays (see
// [`MATCHUP_PATH`]).
const CONTENT_PANELS: [&str; 4] = [
    PERSONAL_PATH,
    "main.contents.strategy.sub1",
    "main.contents.strategy.sub2",
    "main.contents.strategy.sub3",
];

// The composition test's "Tactics & Item Build Setup" panel, relative to the
// training screen's root — the editor's second host, and one that shares none
// of the strategy screen's paths.
const COMP_TACTICS_REL: &str = "comp_test_popup.tactics";

// The composition test's vanilla per-test build block, hidden in favour of this
// editor, since two controls for one slot would disagree. Also the marker
// [`find_comp_tactics`] walks for.
const COMP_BUILDS_REL: &str = "comp_test_popup.tactics.builds";

// The root `training.ui` hangs the composition test off; a candidate, not the
// answer (see [`resolve_comp_tactics`]).
const COMP_ROOT: &str = "training";

// Frames between failed sweeps for the composition test panel (a hit is cached
// in [`EditorState::comp_tactics`]); an unthrottled sweep makes the mod lag.
const COMP_PROBE_EVERY: u32 = 30;

// Depth and node budget for that sweep, mirroring `item_stats::ui::find_screen`.
// A depth limit alone does not bound the walk — one wide level can be hundreds
// of nodes, and this runs on the UI thread.
const COMP_PROBE_DEPTH: u32 = 4;
const COMP_PROBE_NODES: u32 = 600;

// The editor popup's authored origin, restored when it goes back to the
// strategy screen after the composition test has moved it.
const EDITOR_RECT_STRATEGY: &str = "x: 47px; y: 182px;";

// Air left between the editor and the dialog furniture above and below it,
// matching the 10px the vanilla block leaves under its own header.
const COMP_SLOT_GAP: f32 = 10.0;

// The Team tab's fourth column, left showing (the panel is narrowed to 1364px
// for it) so the Matchup card, which only game code can populate, stays on
// screen.
const SUB4_PATH: &str = "main.contents.strategy.sub4";

// The Matchup card and the "Closing Out" block above it; hiding the latter
// floats Matchup to the column's top. Both are re-asserted every frame, since
// game code's tab is never Builds.
const MATCHUP_PATH: &str = "main.contents.strategy.sub4.matchup";
const GAME_FINISH_PATH: &str = "main.contents.strategy.sub4.game_finish";

// Tab highlight colours, lifted from `strategy_option` in
// `asset/base/style/main.style` so the painted state matches the vanilla one
// exactly. See [`paint_tabs`] for why they are painted rather than selected.
const TAB_SELECTED_FILL: &str = "#ecfbf8ff";
const TAB_SELECTED_TEXT: &str = "#0f5b4dff";
const TAB_IDLE_FILL: &str = "#00000000";
const TAB_IDLE_TEXT: &str = "#a3a9b6ff";
// Hover on a dim tab: `image.hover.color` and `label.hover.color` in the style.
// Equal to [`TAB_IDLE_TEXT`] by coincidence, not by meaning.
const TAB_HOVER_LINE: &str = "#a3a9b6ff";
const TAB_HOVER_TEXT: &str = "#e0e2e7ff";

// A footer toggle that does nothing on this screen ([`scope_applies`]): greyed
// cells, the saved setting still filled. This mod's colours, since
// `strategy_option` has no disabled state.
const TOGGLE_OFF_SELECTED_FILL: &str = "#3a3c44ff";
const TOGGLE_OFF_SELECTED_TEXT: &str = "#7d828dff";
const TOGGLE_OFF_IDLE_TEXT: &str = "#5a5e68ff";

// The editor's layout source, and its root's node name. The editor has two
// hosts in different scenes, so its paths are resolved per host
// ([`EditorPaths`]).
const EDITOR_SOURCE: &str = include_str!("../../ui/layout/build_editor.ui");
const EDITOR_NODE: &str = "build_editor";

// Sheet the item icons come from. The mod overrides this asset with its own
// 640x640 sheet (see `mod.override_info`), so frame names are the mod's.
pub(crate) const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";

// The panel size `build_editor.ui` authors, used on the strategy screen. The
// composition test's 1280-wide dialog cannot fit it, so the geometry below is
// insets from the panel's edges, re-laid on a host change
// ([`want_panel_size`]).
const PANEL_W: u32 = 1364;
const PANEL_H: u32 = 645;

// Insets from the panel's right edge, from the authored 1364 layout (rows 1306,
// delete button 1250, item band 1234, scrolling strips 1334), so the panel can
// resize without re-authoring.
const ROW_INSET: u32 = PANEL_W - 1306;
const DELETE_INSET: u32 = PANEL_W - 1250;
const COLUMNS_INSET: u32 = PANEL_W - 1234;
const STRIP_INSET: u32 = PANEL_W - 1334;
const ROWS_INSET: u32 = PANEL_W - 1314;

// Panel height not available to the row list: the toolbar and column header
// above it (92), the bottom-anchored footer (60), and the 7px of air the
// authored 645/486 pair leaves between them.
const CHROME_H: u32 = 92 + 60 + 7;

// The size the editor is currently laid out at. Not a constant because the two
// hosts are different sizes; see [`PANEL_W`].
static PANEL_SIZE: Mutex<(u32, u32)> = Mutex::new((PANEL_W, PANEL_H));

fn panel_size() -> (u32, u32) {
    PANEL_SIZE
        .lock()
        .map(|size| *size)
        .unwrap_or((PANEL_W, PANEL_H))
}

fn row_width() -> u32 {
    panel_size().0 - ROW_INSET
}

fn delete_x() -> u32 {
    panel_size().0 - DELETE_INSET
}

fn columns_right() -> u32 {
    panel_size().0 - COLUMNS_INSET
}

// Records the size to lay the editor out at, and whether it moved. Settled
// before `ensure_editor`, since rows carry their width in their source; `true`
// means rows were built for the old width and must be rebuilt.
fn want_panel_size(w: u32, h: u32) -> bool {
    match PANEL_SIZE.lock() {
        Ok(mut size) => {
            let changed = *size != (w, h);
            *size = (w, h);
            changed
        }
        Err(_) => false,
    }
}

// Writes the current size onto the panel's five sized nodes; everything else is
// anchored and follows.
fn apply_panel_size(ctx: &mut StableClient<'_>) {
    let (w, h) = panel_size();
    let strip = w.saturating_sub(STRIP_INSET);
    let list_h = h.saturating_sub(CHROME_H);
    let paths = paths();
    ctx.ui_set_properties(paths.popup, &format!("width: {w}px; height: {h}px;"));
    ctx.ui_set_properties(paths.toolbar, &format!("width: {w}px;"));
    ctx.ui_set_properties(paths.footer, &format!("width: {w}px;"));
    ctx.ui_set_properties(paths.colheader, &format!("width: {strip}px;"));
    ctx.ui_set_properties(
        paths.rowscroll,
        &format!("width: {strip}px; height: {list_h}px;"),
    );
    ctx.ui_set_properties(
        paths.rows,
        &format!("width: {}px;", w.saturating_sub(ROWS_INSET)),
    );
    // The item columns are derived from the panel width, so the headers move
    // with it. Cheap and idempotent, and the only other caller is `ensure_editor`
    // — which does not run when the panel merely changes size.
    sync_column_headers(ctx);
}

// Row height and gap are the vanilla Personal panel's own (50px rows, 10px
// spacing), so the tab reads as one of the game's own rather than a graft.
const ROW_HEIGHT: u32 = 50;
const ROW_SPACING: i32 = 10;
// Combos are 40px tall like the vanilla dropdowns, centred in the row.
const COMBO_Y: u32 = 5;
const COMBO_H: u32 = 40;
// Square buttons (clear, swap glyph target, delete), centred in the row.
const MINI_Y: u32 = 14;
const CHAMP_X: u32 = 8;
const CHAMP_W: u32 = 224;
// The Role column, between the champion button and the item band. Narrow: it
// holds one of six short words, and every px here comes off the item columns.
const ROLE_X: u32 = 240;
const ROLE_W: u32 = 100;

// The band the item columns share, between the champion button and the delete
// button, and the gap left between two columns for a swap button (34px wide,
// plus a few px of air on each side).
const COLUMNS_LEFT: u32 = 348;
const COLUMN_GAP: u32 = 44;

// Widest an item column gets: the icon at 8px, the clear button at `combo_w -
// 52` and the drop arrow at `combo_w - 24` fit in 88px.
const COMBO_MAX_W: u32 = 88;

// Width of one item column: [`COMBO_MAX_W`], or less where the shared band is
// narrower; the unused band is left empty.
fn combo_w() -> u32 {
    let slots = picker_slots() as u32;
    let split = (columns_right() - COLUMNS_LEFT - (slots - 1) * COLUMN_GAP) / slots;
    split.min(COMBO_MAX_W)
}

// Left edge of an item column, after the Role column; `sync_column_headers`
// places the headers from here.
fn combo_x(slot: usize) -> u32 {
    COLUMNS_LEFT + slot as u32 * (combo_w() + COLUMN_GAP)
}

// Left edge of the swap button between `slot` and `slot + 1`: tucked into the
// gap right after its left-hand column.
fn swap_x(slot: usize) -> u32 {
    combo_x(slot) + combo_w() + 4
}

// Sizes of the two floating lists, mirroring `build_editor.ui`. Kept here
// because the open code has to decide whether a list fits below the control
// that opened it.
const LIST_W: i32 = 320;
const LIST_H: i32 = 430;
const CHAMP_LIST_W: i32 = 260;
const ROLE_LIST_W: i32 = 160;

// Canvas the layouts are authored against; the clamps that keep a list on
// screen measure against this.
const CANVAS_W: i32 = 1920;
const CANVAS_H: i32 = 1080;

// Fill behind a category header in the item list, one step up from the panel's
// `#0f1016ff`; then the heights of the two kinds of list node
// ([`entry_source`], [`entry_height`]).
const LIST_HEADER_FILL: &str = "#1d1f2cff";
const ENTRY_HEADER_H: i32 = 26;
const ENTRY_ITEM_H: i32 = 30;

// One pickable item: the key the hook resolves, its display name, and the
// grouping the item list sorts and headers it by.
#[derive(Clone)]
struct ItemChoice {
    key: String,
    name: String,
    // Sprite-sheet frame, or `None` for an item with no art in the sheet.
    frame: Option<String>,
    category: &'static str,
}

// One node in the item list. Every variant occupies an index, so a click path
// resolves straight into this list without a second mapping.
#[derive(Clone)]
enum ListEntry {
    // Always first: puts the slot back to the AI's choice. Clicking the pinned
    // item again does the same thing, but only if a slot is already pinned and
    // only if you know to try it — this is the discoverable way.
    Clear,
    // A category heading. Inert: no handler is registered for it.
    Header(&'static str),
    Item(ItemChoice),
}

// One pickable champion.
#[derive(Clone)]
struct ChampionChoice {
    id: String,
    name: String,
}

// Which floating list is open, and what it is editing.
#[derive(Clone, Copy, PartialEq)]
enum OpenList {
    Item { row: usize, slot: usize },
    Champion { row: usize },
    Role { row: usize },
}

#[derive(Default)]
struct EditorState {
    // Whether the three tabs have been wired for the current screen.
    wired: bool,
    // Whether the editor subtree is spawned and its controls registered.
    modal_ready: bool,
    // Whether the Builds tab is the one currently showing.
    showing: bool,
    // The floating list currently showing, if any.
    open_list: Option<OpenList>,
    // The rows being edited, in display order. Loaded from `item-builds.json`
    // when the window is built and written back on every change.
    rows: Vec<ChampionRow>,
    // Which rows have a node spawned, as indices into `rows` (not screen
    // positions, which differ under a filter), so a rebuild removes exactly
    // those.
    spawned_rows: Vec<usize>,
    // The filter box's text, as of the last frame that read it. Rows whose
    // champion does not match are left unspawned.
    filter: String,
    // Which of the Blue Team / Red Team cells are lit. While either is, the
    // filter box holds the names those cells wrote, and [`visible_rows`]
    // matches them whole.
    side_blue: bool,
    side_red: bool,
    // Each side's drafted champions as the editor names them, blue then red,
    // and the [`draft_watch::revision`] they were worked out from. See
    // [`sync_sides`].
    side_names: [Vec<String>; 2],
    // The ids of those champions, for the ones the editor lists: what a side
    // cell gives a row to when it is lit and the champion has none. A pick
    // the editor could only name, not tie to a champion, has no id here.
    side_ids: [Vec<String>; 2],
    sides_revision: u64,
    // Frames for which the filter box may still read back its old text after
    // a side cell wrote to it. See [`sync_filter`].
    side_settle: u8,
    // The filter box's width as last written, and whether that was for text
    // too long for one line. `None` for a subtree fresh from source. See
    // [`fit_search`].
    search_fit: Option<(u32, bool)>,
    // The item list, headers included. Cached for the process lifetime — the
    // item pool cannot change without a restart.
    entries: Vec<ListEntry>,
    // The champion list. Cached for the same reason.
    champions: Vec<ChampionChoice>,
    // Where the vanilla Item Info popup was found on this screen, and frames
    // since it was looked for; the search is a tree walk, so it is throttled
    // ([`find_info_popup`]).
    info_popup: Option<String>,
    info_probe_tick: u32,
    // Whether that popup was up as of the last frame, so the scroll views are
    // only rewritten when it opens or closes.
    info_showing: bool,
    // Where the composition test's tactics panel was found, and frames since it
    // was looked for; cleared when the path stops resolving
    // ([`resolve_comp_tactics`]).
    comp_tactics: Option<String>,
    comp_probe_tick: u32,
}

static STATE: Mutex<Option<EditorState>> = Mutex::new(None);

// Runs `f` against the editor state, initializing it on first use. Returns
// `None` if the lock is poisoned, which disables the editor rather than
// panicking across the FFI boundary.
fn with_state<T>(f: impl FnOnce(&mut EditorState) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(EditorState::default)))
}

// Final items the mod registers, recorded in `init`: the game's settings
// document lists only vanilla finals and `StableMod` does not expose registered
// items. Membership also says whether an item has art in the icon sheet.
static MOD_FINALS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// Records one of the mod's final (radiant) items. Called from the
// registration macros in `lib.rs`.
pub(crate) fn note_final_item(key: &str) {
    if let Ok(mut finals) = MOD_FINALS.lock() {
        finals.push(key.to_string());
    }
}

// Paths that already have a handler, so none is registered twice (one click
// deleted rows 2 and 3 that way). Forgotten when the strategy screen goes away
// and never while it is up; see the module notes.
static REGISTERED: super::Registered = super::Registered::new(handle_event);

fn is_mod_item(key: &str) -> bool {
    MOD_FINALS
        .lock()
        .map(|finals| finals.iter().any(|final_key| final_key == key))
        .unwrap_or(false)
}

// [`MOD_FINALS`] as a set, built once: it never changes after `init`, and the
// item-build hook asks about every final of every player on parallel sim
// workers.
static MOD_FINAL_SET: OnceLock<std::collections::HashSet<String>> = OnceLock::new();

// [`is_mod_item`] for the simulation side. Reads empty — so promotes nothing —
// if it were ever called before registration, which cannot happen: items are
// registered in `init`, matches run later.
pub(crate) fn is_mod_final_item(key: &str) -> bool {
    MOD_FINAL_SET
        .get_or_init(|| {
            MOD_FINALS
                .lock()
                .map(|finals| finals.iter().cloned().collect())
                .unwrap_or_default()
        })
        .contains(key)
}

// -- paths --------------------------------------------------------------

// Every path inside the editor, resolved against one of its two hosts. Leaked
// as `&'static str` so the ~95 call sites stay simple; bounded by the two
// hosts, about 52 small strings.
struct EditorPaths {
    // The node the window is spawned under. Never addressed except to spawn.
    parent: &'static str,
    editor: &'static str,
    // The editor panel.
    popup: &'static str,
    // The two full-width bars. Only ever addressed to resize them.
    toolbar: &'static str,
    footer: &'static str,
    rows: &'static str,
    // The three scroll views in the editor: the row list and the two floating
    // lists. See [`focus_scroll`] — only one may take the wheel at a time.
    rowscroll: &'static str,
    // The column-header strip, whose labels have to track the same geometry the
    // rows are laid out with.
    colheader: &'static str,
    // The toolbar hint, the one label in the editor long enough to matter.
    hint: &'static str,
    // In the toolbar, above the column headers: adding a row acts on the list
    // below it.
    add: &'static str,
    // The filter box, styled like the ban/pick screen's champion search. Polled
    // every frame by [`sync_filter`], since `TextEditComplete` fires only on
    // commit; its X is a registered button.
    search: &'static str,
    search_clear: &'static str,
    // The Blue Team / Red Team cells under the Matchup card, each its own
    // switch; editor nodes, so they come and go with the tab. Placed from
    // `strategy.ui`'s numbers (the card spans 1446..1852 and ends at y 505,
    // then 15px), not its rect, which flips as "Closing Out" is hidden and
    // re-shown.
    sides: &'static str,
    side_blue: &'static str,
    side_red: &'static str,
    // In the footer rather than the toolbar: it is the panel's "done" button,
    // and bottom-right is where one is looked for.
    save: &'static str,
    // The confirmation tick left of Save. Hidden until a save succeeds; it is
    // the only thing that reports one now that the button stays on the tab.
    saved: &'static str,
    // Footer cells, matched by exact path (a button moved in `build_editor.ui`
    // must be moved here too). Each footer setting is a segmented control of
    // one `color_selectable` per choice, like `#mode_toggle`; the containers
    // are never addressed.
    unique_on: &'static str,
    unique_off: &'static str,
    // Beside the unique toggle, in the same footer bar: both are match-wide
    // rules about the builds rather than edits to one, so they read as a pair.
    scope_all: &'static str,
    scope_own: &'static str,
    // Full-screen transparent button shown while a floating list is open, so a
    // click elsewhere dismisses it. Declared between `#popup` and the lists, so
    // it sits above the window and below them.
    listcatch: &'static str,
    itemlist: &'static str,
    champlist: &'static str,
    rolelist: &'static str,
    itemlist_scroll: &'static str,
    champlist_scroll: &'static str,
    rolelist_scroll: &'static str,
}

// Leaks one path. See [`EditorPaths`] for why this is bounded.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

impl EditorPaths {
    fn build(parent: &str) -> &'static Self {
        let editor = leak(format!("{parent}.{EDITOR_NODE}"));
        let popup = leak(format!("{editor}.popup"));
        let toolbar = leak(format!("{popup}.toolbar"));
        let footer = leak(format!("{popup}.footer"));
        Box::leak(Box::new(Self {
            parent: leak(parent.to_string()),
            editor,
            popup,
            toolbar,
            footer,
            rows: leak(format!("{popup}.rowscroll.rows")),
            rowscroll: leak(format!("{popup}.rowscroll")),
            colheader: leak(format!("{popup}.colheader")),
            hint: leak(format!("{toolbar}.hint")),
            add: leak(format!("{toolbar}.add")),
            search: leak(format!("{toolbar}.search")),
            search_clear: leak(format!("{toolbar}.searchclear")),
            sides: leak(format!("{editor}.sides")),
            side_blue: leak(format!("{editor}.sides.blue")),
            side_red: leak(format!("{editor}.sides.red")),
            save: leak(format!("{footer}.save")),
            saved: leak(format!("{footer}.saved")),
            unique_on: leak(format!("{footer}.unique.on")),
            unique_off: leak(format!("{footer}.unique.off")),
            scope_all: leak(format!("{footer}.scope.all")),
            scope_own: leak(format!("{footer}.scope.own")),
            listcatch: leak(format!("{editor}.listcatch")),
            itemlist: leak(format!("{editor}.itemlist")),
            champlist: leak(format!("{editor}.champlist")),
            rolelist: leak(format!("{editor}.rolelist")),
            itemlist_scroll: leak(format!("{editor}.itemlist.list")),
            champlist_scroll: leak(format!("{editor}.champlist.list")),
            rolelist_scroll: leak(format!("{editor}.rolelist.list")),
        }))
    }
}

// Path sets built so far, one per host. Looked up by parent so a host revisited
// later reuses its set rather than leaking a second copy.
static HOSTS: Mutex<Vec<&'static EditorPaths>> = Mutex::new(Vec::new());

// The host the editor is currently addressed under.
static ACTIVE_HOST: Mutex<Option<&'static EditorPaths>> = Mutex::new(None);

fn host_paths(parent: &str) -> &'static EditorPaths {
    // A poisoned lock falls through to an unshared set rather than disabling the
    // editor: the paths are values, so a duplicate is merely wasteful.
    let Ok(mut hosts) = HOSTS.lock() else {
        return EditorPaths::build(parent);
    };
    if let Some(found) = hosts.iter().find(|paths| paths.parent == parent) {
        return *found;
    }
    let built = EditorPaths::build(parent);
    hosts.push(built);
    built
}

// Points every editor path at `parent`. Callers need not react:
// [`ensure_editor`] notices the old host's subtree is gone and respawns.
fn set_host(parent: &str) {
    let built = host_paths(parent);
    if let Ok(mut active) = ACTIVE_HOST.lock() {
        *active = Some(built);
    }
}

// The active host's paths, defaulting to the strategy screen's.
fn paths() -> &'static EditorPaths {
    if let Ok(active) = ACTIVE_HOST.lock() {
        if let Some(found) = *active {
            return found;
        }
    }
    host_paths(UI_ROOT)
}

fn editor_path() -> &'static str {
    paths().editor
}
fn popup_path() -> &'static str {
    paths().popup
}
fn rows_path() -> &'static str {
    paths().rows
}
fn rowscroll_path() -> &'static str {
    paths().rowscroll
}
fn colheader_path() -> &'static str {
    paths().colheader
}
fn hint_path() -> &'static str {
    paths().hint
}
fn add_path() -> &'static str {
    paths().add
}
fn search_path() -> &'static str {
    paths().search
}
fn search_clear_path() -> &'static str {
    paths().search_clear
}
fn sides_path() -> &'static str {
    paths().sides
}
fn side_blue_path() -> &'static str {
    paths().side_blue
}
fn side_red_path() -> &'static str {
    paths().side_red
}
fn save_path() -> &'static str {
    paths().save
}
fn saved_path() -> &'static str {
    paths().saved
}
fn unique_on_path() -> &'static str {
    paths().unique_on
}
fn unique_off_path() -> &'static str {
    paths().unique_off
}
fn scope_all_path() -> &'static str {
    paths().scope_all
}
fn scope_own_path() -> &'static str {
    paths().scope_own
}
fn listcatch_path() -> &'static str {
    paths().listcatch
}
fn itemlist_path() -> &'static str {
    paths().itemlist
}
fn champlist_path() -> &'static str {
    paths().champlist
}
fn rolelist_path() -> &'static str {
    paths().rolelist
}
fn itemlist_scroll() -> &'static str {
    paths().itemlist_scroll
}
fn champlist_scroll() -> &'static str {
    paths().champlist_scroll
}
fn rolelist_scroll() -> &'static str {
    paths().rolelist_scroll
}
// Resting `speed` of all three, restored to whichever one is live. Must match
// the value the three views are authored with in `build_editor.ui`.
const SCROLL_SPEED: i32 = 100;

fn editor_row_path(row: usize) -> String {
    format!("{}.row{row}", rows_path())
}

fn champ_path(row: usize) -> String {
    format!("{}.champ", editor_row_path(row))
}

fn role_path(row: usize) -> String {
    format!("{}.role", editor_row_path(row))
}

fn combo_path(row: usize, slot: usize) -> String {
    format!("{}.slot{slot}", editor_row_path(row))
}

fn combo_icon_path(row: usize, slot: usize) -> String {
    format!("{}.icon", combo_path(row, slot))
}

fn clear_path(row: usize, slot: usize) -> String {
    format!("{}.clear{slot}", editor_row_path(row))
}

fn delete_path(row: usize) -> String {
    format!("{}.delete", editor_row_path(row))
}

fn list_contents_path() -> String {
    format!("{}.itemlist.list.contents", editor_path())
}

fn entry_path(index: usize) -> String {
    format!("{}.e{index}", list_contents_path())
}

fn champ_contents_path() -> String {
    format!("{}.champlist.list.contents", editor_path())
}

fn champ_entry_path(index: usize) -> String {
    format!("{}.c{index}", champ_contents_path())
}

fn role_contents_path() -> String {
    format!("{}.rolelist.list.contents", editor_path())
}

fn role_entry_path(index: usize) -> String {
    format!("{}.r{index}", role_contents_path())
}

// Strips characters that would end a `.ui` string literal or be read as markup.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '<' | '>' | '{' | '}' | ';'))
        .collect()
}

// -- item list ----------------------------------------------------------

// Every final item the game knows, grouped into [`item_catalog`]'s categories
// and sorted by name, read from the item settings document so new items need no
// wiring.
fn load_entries(ctx: &StableClient<'_>) -> Vec<ListEntry> {
    let mut choices = Vec::new();
    if let Some(json) = ctx.setting_get_json(SettingTargetV1::ItemSetting, "") {
        if let Ok(serde_json::Value::Object(root)) =
            serde_json::from_str::<serde_json::Value>(&json)
        {
            collect_items(ctx, &root, 0, &mut choices);
        }
    }
    merge_mod_finals(ctx, &mut choices);

    // Category first, then name, so the headers come out in CATEGORY_ORDER.
    choices.sort_by(|a, b| {
        item_catalog::category_rank(a.category)
            .cmp(&item_catalog::category_rank(b.category))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut entries = Vec::with_capacity(choices.len() + item_catalog::CATEGORY_ORDER.len() + 2);
    entries.push(ListEntry::Clear);
    let mut current = "";
    for choice in choices {
        if choice.category != current {
            current = choice.category;
            entries.push(ListEntry::Header(current));
        }
        entries.push(ListEntry::Item(choice));
    }
    entries
}

// One [`ItemChoice`] from an item key; `None` for an item with no category,
// which keeps it out of the list.
fn make_choice(ctx: &StableClient<'_>, key: &str) -> Option<ItemChoice> {
    let slug = build_config::base_slug(key);
    Some(ItemChoice {
        frame: item_catalog::icon_frame(slug, is_mod_item(key)).map(sanitize),
        category: item_catalog::category_of(slug)?,
        name: sanitize(&item_display_name(ctx, key, slug)),
        key: sanitize(key),
    })
}

// Display name for an item without the "Radiant" tier word, which every pool
// item has: the base item's translated name. The six reskinned vanilla finals
// have no base, so their radiant name is trimmed, in English only.
fn item_display_name(ctx: &StableClient<'_>, key: &str, slug: &str) -> String {
    // The `#` prefix is part of the key: the probe found this spelling resolves
    // ("Radiant Bloodthirster") while the bare path returns nothing. Same form
    // the `.ui` text properties use.
    let lookup = |name: &str| {
        ctx.i18n(&format!("#asset/base/text/item?{name}.name"))
            .filter(|text| !text.is_empty())
    };
    if let Some(base) = lookup(slug) {
        return base;
    }
    lookup(key)
        .map(|name| name.strip_prefix("Radiant ").unwrap_or(&name).to_string())
        .unwrap_or_else(|| key.to_string())
}

// Adds the mod's own final items to `choices`, skipping keys the settings
// document already gave (renamed vanilla finals overlap).
fn merge_mod_finals(ctx: &StableClient<'_>, choices: &mut Vec<ItemChoice>) {
    let Ok(finals) = MOD_FINALS.lock() else {
        return;
    };
    let keys: Vec<String> = finals.clone();
    drop(finals); // `make_choice` -> `is_mod_item` takes the same lock.

    for key in keys {
        if choices.iter().any(|choice| choice.key == key) {
            continue;
        }
        choices.extend(make_choice(ctx, &key));
    }
}

// An object is an item when it has any field every item has; the settings
// document mixes items with group objects such as `mod_items`.
fn is_item(value: &serde_json::Value) -> bool {
    value.get("next_tier").is_some() || value.get("tier").is_some() || value.get("price").is_some()
}

// Walks the settings document collecting final items, descending into group
// objects. Depth is capped so an unexpected document shape cannot turn this
// into a deep traversal on the UI thread.
fn collect_items(
    ctx: &StableClient<'_>,
    map: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
    out: &mut Vec<ItemChoice>,
) {
    for (key, value) in map {
        let Some(object) = value.as_object() else {
            continue;
        };
        if !is_item(value) {
            // Two levels, not one: mod items sit under a per-mod bucket
            // (`mod_items.riot_items_tfm2.collector`).
            if depth < 2 {
                collect_items(ctx, object, depth + 1, out);
            }
            continue;
        }
        // `next_tier` empty: nothing upgrades from this item. The hook cannot
        // test this (`next_tier` does not cross the stable boundary), but the
        // JSON can.
        let is_final = value
            .get("next_tier")
            .and_then(|next| next.as_array())
            .is_none_or(|next| next.is_empty());
        if !is_final {
            continue;
        }
        out.extend(make_choice(ctx, key));
    }
}

// The item list, loading it on first use. Empty is a valid (if useless)
// answer — it means the client cannot read the item settings document — and is
// not cached, so a later frame can retry.
fn cached_entries(ctx: &StableClient<'_>) -> Vec<ListEntry> {
    let cached = with_state(|state| state.entries.clone()).unwrap_or_default();
    if !cached.is_empty() {
        return cached;
    }

    let loaded = load_entries(ctx);
    let items = loaded
        .iter()
        .filter(|entry| matches!(entry, ListEntry::Item(_)))
        .count();
    if items == 0 {
        // Not `loaded.is_empty()`: the Clear row is always there, so the list
        // being non-empty says nothing about whether any item was found.
    } else {
        let _ = with_state(|state| state.entries = loaded.clone());
    }
    loaded
}

// Copy of the cached list, for use outside the state lock.
fn snapshot_entries() -> Vec<ListEntry> {
    with_state(|state| state.entries.clone()).unwrap_or_default()
}

fn choice_of<'a>(entries: &'a [ListEntry], key: &str) -> Option<&'a ItemChoice> {
    entries.iter().find_map(|entry| match entry {
        ListEntry::Item(item) if item.key == key => Some(item),
        _ => None,
    })
}

// Rewrites every pinned key to the spelling of the list entry it means.
// `item-builds.json` accepts any spelling the hook resolves, but the editor
// compares exactly, so such a slot would look pinned with no icon and could not
// be unpinned.
fn canonicalize_rows(entries: &[ListEntry], rows: &mut [ChampionRow]) {
    for key in rows.iter_mut().flat_map(|row| row.slots.iter_mut().flatten()) {
        if let Some(item) = build_config::resolve_key(key, &|k: &str| choice_of(entries, k)) {
            if item.key != *key {
                *key = item.key.clone();
            }
        }
    }
}

// Champion list, in display order, cached. From the hook's roster, since the
// client's `champion_names()` returns nothing (still tried first); taken whole,
// other mods' champions and a stray dummy included, because a missing modded
// champion is worse than a wart.
fn load_champions(ctx: &StableClient<'_>) -> Vec<ChampionChoice> {
    let mut ids = ctx.champion_names();
    if ids.is_empty() {
        ids = build_config::champion_roster();
    }

    let mut out: Vec<ChampionChoice> = ids
        .iter()
        .map(|id| ChampionChoice {
            name: sanitize(&champion_display_name(ctx, id)),
            id: sanitize(id),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// Prefixes some modded champion ids carry. Cosmetic noise in the dropdown, so
// they are dropped from the shown name only — the id itself is what the hook
// matches on and is never rewritten.
const MOD_ID_PREFIXES: [&str; 2] = ["test_mod_", "cf_"];

// Drops leading mod prefixes, stacked ones too (`test_mod_cf_x`), unless
// nothing would be left.
fn strip_mod_prefixes(id: &str) -> &str {
    let mut id = id;
    'outer: loop {
        for prefix in MOD_ID_PREFIXES {
            if let Some(rest) = id.strip_prefix(prefix) {
                if !rest.is_empty() {
                    id = rest;
                    continue 'outer;
                }
            }
        }
        return id;
    }
}

// Readable name for a champion id: the game's translation where it has one (two
// spellings tried), else the id prettified (`snake_case` -> `Snake Case`).
fn champion_display_name(ctx: &StableClient<'_>, id: &str) -> String {
    for key in [
        format!("#asset/base/text/champion?{id}.name"),
        format!("#asset/base/text/champion?{id}"),
    ] {
        if let Some(name) = ctx
            .i18n(&key)
            .filter(|name| !name.is_empty() && !name.starts_with('#'))
        {
            return name;
        }
    }
    if id == "soldier" {
        // The one id whose prettified form is not what the game calls it.
        return "Soldier (Sniper)".to_string();
    }
    strip_mod_prefixes(id)
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn cached_champions(ctx: &StableClient<'_>) -> Vec<ChampionChoice> {
    let cached = with_state(|state| state.champions.clone()).unwrap_or_default();
    if !cached.is_empty() {
        return cached;
    }
    let loaded = load_champions(ctx);
    if loaded.is_empty() {
    } else {
        let _ = with_state(|state| state.champions = loaded.clone());
    }
    loaded
}

fn snapshot_champions() -> Vec<ChampionChoice> {
    with_state(|state| state.champions.clone()).unwrap_or_default()
}

// Display name for a champion id, falling back to the raw id for one the
// roster does not cover.
fn champion_label(champions: &[ChampionChoice], id: Option<&str>) -> String {
    let Some(id) = id else {
        return strings().no_champion.clone();
    };
    champions
        .iter()
        .find(|choice| choice.id == id)
        .map_or_else(|| id.to_string(), |choice| choice.name.clone())
}

// -- editing ------------------------------------------------------------

// Copy of the rows being edited.
fn snapshot_rows() -> Vec<ChampionRow> {
    with_state(|state| state.rows.clone()).unwrap_or_default()
}

// Applies `edit` to one row and autosaves; returns whether the file was
// written, so a failure can be reported.
fn edit_row(row: usize, edit: impl FnOnce(&mut ChampionRow)) -> bool {
    let rows = with_state(|state| {
        if let Some(entry) = state.rows.get_mut(row) {
            // Grow to the editable width only; never truncate a longer build.
            if entry.slots.len() < picker_slots() {
                entry.slots.resize(picker_slots(), None);
            }
            edit(entry);
        }
        state.rows.clone()
    });
    match rows {
        Some(rows) => build_config::save_champion_rows(&rows),
        None => false,
    }
}

// -- painting -----------------------------------------------------------

// Leading padding for a combo's label: `color_icon_button` has no text inset,
// so the string is padded. [`ICON_PAD`] clears the 24px icon at `x: 8px`;
// [`PLAIN_PAD`] is the smaller inset for a combo with no icon.
const ICON_PAD: &str = "          ";
const PLAIN_PAD: &str = "  ";

// The pinned key for one slot, or `None` when the AI owns it.
fn pinned_key(rows: &[ChampionRow], row: usize, slot: usize) -> Option<String> {
    rows.get(row)
        .and_then(|entry| entry.slots.get(slot))
        .and_then(Option::as_ref)
        .cloned()
}

// Repaints one slot's button: its label, its icon, and whether its clear button
// is offered at all — an unpinned slot has nothing to clear, so its X is
// hidden.
fn refresh_combo(
    ctx: &mut StableClient<'_>,
    entries: &[ListEntry],
    rows: &[ChampionRow],
    row: usize,
    slot: usize,
) {
    let pinned = pinned_key(rows, row, slot);
    // A pinned slot is its icon alone: button text is not clipped and would
    // spill over the swap button on a narrow host, which is also why the empty
    // slot is a dash.
    let (label, color) = match &pinned {
        Some(_) => (String::new(), "#e8e8e8ff"),
        None => (EMPTY_SLOT_LABEL.to_string(), "#a5a5abff"),
    };
    ctx.ui_set_properties(
        &combo_path(row, slot),
        &format!(
            "text: {{ text: \"{}\"; color: {color}; }}",
            sanitize(&label)
        ),
    );

    let frame = pinned
        .as_deref()
        .and_then(|key| choice_of(entries, key))
        .and_then(|item| item.frame.as_deref());
    let icon = match frame {
        Some(frame) => format!("visible: true; rect_tag: \"{frame}\";"),
        None => "visible: false;".to_string(),
    };
    ctx.ui_set_properties(&combo_icon_path(row, slot), &icon);

    ctx.ui_set_visible(&clear_path(row, slot), pinned.is_some());
}

// Repaints one row's champion button. A row with no champion is shown muted,
// the same way an unpinned slot reads, because it is not yet a build.
fn refresh_champ(
    ctx: &mut StableClient<'_>,
    champions: &[ChampionChoice],
    rows: &[ChampionRow],
    row: usize,
) {
    let champion = rows.get(row).and_then(|entry| entry.champion.clone());
    let color = if champion.is_some() {
        "#e8e8e8ff"
    } else {
        "#a5a5abff"
    };
    let label = champion_label(champions, champion.as_deref());
    ctx.ui_set_properties(
        &champ_path(row),
        &format!(
            "text: {{ text: \"{PLAIN_PAD}{}\"; color: {color}; }}",
            sanitize(&label)
        ),
    );
}

// Paints a row's Role button; `Any` is dimmed like an unassigned champion,
// being the default rather than a choice.
fn refresh_role(ctx: &mut StableClient<'_>, rows: &[ChampionRow], row: usize) {
    let role = rows.get(row).map(|entry| entry.role).unwrap_or_default();
    let color = if role == Role::Any {
        "#a5a5abff"
    } else {
        "#e8e8e8ff"
    };
    ctx.ui_set_properties(
        &role_path(row),
        &format!(
            "text: {{ text: \"{PLAIN_PAD}{}\"; color: {color}; }}",
            role.label()
        ),
    );
}

// Repaints one row's buttons.
fn refresh_row(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize) {
    let rows = snapshot_rows();
    refresh_champ(ctx, &snapshot_champions(), &rows, row);
    refresh_role(ctx, &rows, row);
    for slot in 0..picker_slots() {
        refresh_combo(ctx, entries, &rows, row, slot);
    }
}

// Lights the chosen cell of a two-option footer toggle and dims the other.
// Painted, not set (`ui_set_selectable_selected` rejects `color_selectable`;
// see [`paint_tabs`]), with both property pairs written alike so either state
// renders right.
fn paint_toggle(ctx: &mut StableClient<'_>, lit_path: &str, dim_path: &str) {
    ctx.ui_set_properties(lit_path, &toggle_style(true));
    ctx.ui_set_properties(dim_path, &toggle_style(false));
}

// Paints a two-option toggle greyed out, for a screen its setting does nothing
// on: the saved cell still filled, neither answering the cursor (the click is
// refused where it is handled).
fn paint_toggle_off(ctx: &mut StableClient<'_>, lit_path: &str, dim_path: &str) {
    ctx.ui_set_properties(lit_path, &toggle_off_style(true));
    ctx.ui_set_properties(dim_path, &toggle_off_style(false));
}

// One toggle cell's appearance, in the vanilla `strategy_option` colours the
// tabs and list rows already use.
fn toggle_style(lit: bool) -> String {
    toggle_style_sized(lit, 13)
}

// [`toggle_style`] with the label at `size`: the footer's cells are 13, the
// game's own option rows 16 (`crate::options_row`).
pub(crate) fn toggle_style_sized(lit: bool, size: u32) -> String {
    let (fill, text, stroke) = if lit {
        (TAB_SELECTED_FILL, TAB_SELECTED_TEXT, 0)
    } else {
        (TAB_IDLE_FILL, TAB_IDLE_TEXT, 1)
    };
    let (hover_line, hover_text) = if lit {
        (fill, text)
    } else {
        (TAB_HOVER_LINE, TAB_HOVER_TEXT)
    };
    toggle_cell_style(fill, text, stroke, hover_line, hover_text, size)
}

// One cell of a greyed-out toggle (see [`paint_toggle_off`]). Its hover
// colours are its resting ones, so it does not light up under the cursor.
fn toggle_off_style(lit: bool) -> String {
    toggle_off_style_sized(lit, 13)
}

// [`toggle_off_style`] with the label at `size`, for the game's own option
// rows (`crate::options_row`).
pub(crate) fn toggle_off_style_sized(lit: bool, size: u32) -> String {
    let (fill, text, stroke) = if lit {
        (TOGGLE_OFF_SELECTED_FILL, TOGGLE_OFF_SELECTED_TEXT, 0)
    } else {
        (TAB_IDLE_FILL, TOGGLE_OFF_IDLE_TEXT, 1)
    };
    toggle_cell_style(fill, text, stroke, fill, text, size)
}

// The property blocks of one toggle cell in the given colours.
fn toggle_cell_style(
    fill: &str,
    text: &str,
    stroke: u32,
    hover_line: &str,
    hover_text: &str,
    size: u32,
) -> String {
    // `size` and `align_x` are restated rather than left to the `.ui`, because a
    // property write replaces the block it names: a `label` carrying only a
    // colour would drop the centring the layout gives these cells.
    let pair = |image_key: &str, label_key: &str| {
        format!(
            "{image_key}: {{ color: {fill}; back_color: {fill}; stroke: {stroke}; \
             rounding: Uniform {{ rounding: 6; }} \
             hover: {{ color: {hover_line}; back_color: {fill}; }} }} \
             {label_key}: {{ size: {size}; align_x: Center; color: {text}; \
             hover: {{ color: {hover_text}; }} }}"
        )
    };
    format!(
        "{} {}",
        pair("image", "label"),
        pair("selected_image", "selected_label")
    )
}

// Paints which half of the unique-items toggle is the live setting.
fn refresh_unique(ctx: &mut StableClient<'_>) {
    if build_config::smart_builds_enabled() {
        paint_toggle(ctx, unique_on_path(), unique_off_path());
    } else {
        paint_toggle(ctx, unique_off_path(), unique_on_path());
    }
}

// Takes the save tick down. Called from every path that changes a row and none
// that only changes the view, so the tick stands only while nothing has been
// touched since a save.
fn clear_saved(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(saved_path(), false);
}

// Paints the build-scope toggle ([`build_config::own_team_only_enabled`]),
// greyed out where it does nothing ([`scope_applies`]) with the saved setting
// still marked.
fn refresh_scope(ctx: &mut StableClient<'_>) {
    let (lit, dim) = if build_config::own_team_only_enabled() {
        (scope_own_path(), scope_all_path())
    } else {
        (scope_all_path(), scope_own_path())
    };
    if scope_applies() {
        paint_toggle(ctx, lit, dim);
    } else {
        paint_toggle_off(ctx, lit, dim);
    }
}

// Whether the build-scope toggle does anything here: not on the 5v5 and lane
// tests, where both sides are the player's and get the pins regardless
// (`is_training` in `tactics::buy_replace_ctx`).
fn scope_applies() -> bool {
    paths().parent == UI_ROOT
}

// -- side cells -----------------------------------------------------------

// How long the filter box is given to read back what a side cell wrote into
// it, in frames. See [`sync_filter`].
const SIDE_SETTLE_FRAMES: u8 = 3;

// The filter box's left edge and the width it has for a line that fits, as
// `build_editor.ui` authors them.
const SEARCH_X: u32 = 205;
const SEARCH_LINE_W: u32 = 450;

// The gap between the filter box and its X button, and what that gap, the
// button and the toolbar's right margin take off the room the box may grow
// into.
const SEARCH_CLEAR_GAP: u32 = 10;
const SEARCH_RIGHT_INSET: u32 = SEARCH_CLEAR_GAP + 22 + 15;

// The filter box's left and right padding, which its text cannot use.
const SEARCH_PADDING: u32 = 44 + 15;

// Whether the Blue Team / Red Team cells are up: on the strategy screen after a
// draft [`draft_watch`] saw, never on the composition test (no draft, no
// Matchup card).
fn sides_shown() -> bool {
    with_state(|state| state.side_names.iter().any(|side| !side.is_empty())).unwrap_or(false)
        && paths().parent == UI_ROOT
}

// Shows the side cells when there is a lineup for them to write, and paints
// them.
fn refresh_sides(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(sides_path(), sides_shown());
    paint_sides(ctx);
}

// Sizes the filter box for the text in it. A `text_edit` draws every wrapped
// line outside its own rect, with no property to stop it, so text that would
// wrap gets the rest of the toolbar while the hint steps aside; only a change
// of geometry is written, since this runs on every keystroke.
fn fit_search(ctx: &mut StableClient<'_>) {
    // A host too narrow to give more than the line keeps the line, and wraps.
    let room = panel_size()
        .0
        .saturating_sub(SEARCH_X + SEARCH_RIGHT_INSET)
        .max(SEARCH_LINE_W);
    let Some((width, long)) = with_state(|state| {
        let long = filter_text_width(&state.filter) + SEARCH_PADDING > SEARCH_LINE_W;
        let fit = (if long { room } else { SEARCH_LINE_W }, long);
        (state.search_fit != Some(fit)).then(|| {
            state.search_fit = Some(fit);
            fit
        })
    })
    .flatten() else {
        return;
    };
    ctx.ui_set_properties(search_path(), &format!("width: {width}px;"));
    ctx.ui_set_properties(
        search_clear_path(),
        &format!("x: {}px;", SEARCH_X + width + SEARCH_CLEAR_GAP),
    );
    ctx.ui_set_visible(hint_path(), !long);
}

// A guess at how wide `text` draws in the filter box, in px: 7 a character
// (Latin measured 6.0-6.3), non-ASCII counted full-width, so the box widens a
// little early rather than late.
fn filter_text_width(text: &str) -> u32 {
    text.chars()
        .map(|c| if c.is_ascii() { 7 } else { 14 })
        .sum()
}

// Paints which side cells are lit, each its own switch (both property pairs
// written, as in [`paint_toggle`]).
fn paint_sides(ctx: &mut StableClient<'_>) {
    let (blue, red) =
        with_state(|state| (state.side_blue, state.side_red)).unwrap_or((false, false));
    ctx.ui_set_properties(side_blue_path(), &toggle_style(blue));
    ctx.ui_set_properties(side_red_path(), &toggle_style(red));
}

// What the lit side cells put in the filter box: their champions, blue's
// first, separated the way [`filter_terms`] splits them.
fn sides_text(state: &EditorState) -> String {
    state
        .side_names
        .iter()
        .zip([state.side_blue, state.side_red])
        .filter(|(_, lit)| *lit)
        .flat_map(|(names, _)| names.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(", ")
}

// Works out which of the editor's champions each side drafted, whenever
// [`draft_watch`]'s picks change. From `post_update`, since host calls in a
// click handler have come back empty.
fn sync_sides(ctx: &mut StableClient<'_>) {
    let revision = draft_watch::revision();
    if with_state(|state| state.sides_revision == revision).unwrap_or(true) {
        return;
    }
    let champions = cached_champions(ctx);
    let game_names: Vec<String> = champions
        .iter()
        .map(|choice| game_champion_name(ctx, &choice.id))
        .collect();
    let picks = [false, true].map(draft_watch::picks);
    let names = [0, 1].map(|side| {
        picks[side]
            .iter()
            .filter_map(|pick| drafted_label(&champions, &game_names, pick))
            .collect::<Vec<_>>()
    });
    let ids = [0, 1].map(|side| {
        picks[side]
            .iter()
            .filter_map(|pick| drafted_champion(&champions, &game_names, pick))
            .map(|choice| choice.id.clone())
            .collect::<Vec<_>>()
    });
    let _ = with_state(|state| {
        state.side_names = names;
        state.side_ids = ids;
        state.sides_revision = revision;
    });
    refresh_sides(ctx);
}

// The game's own name for a champion id (`description.<id>.name`, in English),
// or nothing. Not always the prettified id: `priest` is "Priestess",
// `cavalry_knight` "Cavalry".
fn game_champion_name(ctx: &StableClient<'_>, id: &str) -> String {
    ctx.i18n(&format!("#asset/base/text/champion?description.{id}.name"))
        .filter(|name| !name.starts_with('#'))
        .unwrap_or_default()
}

// What the filter box should say for one drafted champion: the editor's label
// where the pick ties to a listed champion ([`drafted_champion`]), else what
// the grid showed, else nothing.
fn drafted_label(
    champions: &[ChampionChoice],
    game_names: &[String],
    pick: &draft_watch::Pick,
) -> Option<String> {
    if let Some(choice) = drafted_champion(champions, game_names, pick) {
        return Some(choice.name.clone());
    }
    // Not a champion the editor lists. What the grid showed is still the best
    // term there is, unless it is a reference: nothing in the list could match
    // one and nobody could read it. A comma would split the term in two.
    let reference = pick.name.starts_with('#') && pick.name.contains('?');
    let raw = sanitize(&pick.name).replace(',', " ");
    (!reference && !raw.trim().is_empty()).then_some(raw)
}

// The listed champion one drafted pick is, if any: by id where one can be read
// (a `#asset/…?description.<id>.name` reference, else the slot's node name),
// and in between by name through [`fold`] against the game's names, the
// editor's labels and the ids. The name route only works with the game in
// English.
fn drafted_champion<'a>(
    champions: &'a [ChampionChoice],
    game_names: &[String],
    pick: &draft_watch::Pick,
) -> Option<&'a ChampionChoice> {
    let by_id = |id: &str| champions.iter().find(|choice| choice.id == id);
    let reference = pick
        .name
        .strip_prefix('#')
        .and_then(|rest| rest.split_once('?'))
        .map(|(_, key)| key);
    let shown = fold(&pick.name);
    let by_name = || {
        if reference.is_some() || shown.is_empty() {
            return None;
        }
        champions
            .iter()
            .zip(game_names)
            .find(|(_, name)| shown == fold(name))
            .map(|(choice, _)| choice)
            .or_else(|| {
                champions
                    .iter()
                    .find(|choice| shown == fold(&choice.name) || shown == fold(&choice.id))
            })
    };
    reference
        .and_then(|key| key.split('.').find_map(by_id))
        .or_else(by_name)
        .or_else(|| by_id(&pick.node))
}

// A name with everything but its letters and digits dropped, lowercased, so
// that `Poison Dart Hunter` and `poison_dart_hunter` compare equal.
fn fold(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

// -- spawning -----------------------------------------------------------

// `.ui` source for one champion row.
fn row_source(row: usize) -> String {
    // Bound once: the spawned `.ui` source carries the text literally, so
    // this is where the editor's language reaches a freshly built row.
    let strings = strings();
    let no_champion = &strings.no_champion;
    // Both follow the panel width, so a row built for one host is the wrong
    // shape for the other; see [`want_panel_size`].
    let row_w = row_width();
    let delete_x = delete_x();
    let mut source = format!(
        "row{row}:color {{\n\
         width: {row_w}px;\n\
         height: {ROW_HEIGHT}px;\n\
         color: #161721ff;\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         #champ:color_icon_button {{\n\
         @\"asset/base/style/main#tertiary_button\";\n\
         x: {CHAMP_X}px;\n\
         y: {COMBO_Y}px;\n\
         width: {CHAMP_W}px;\n\
         height: {COMBO_H}px;\n\
         \n\
         text: {{\n\
         text: \"{no_champion}\";\n\
         align_x: Left;\n\
         align_y: Center;\n\
         size: 14;\n\
         color: #a5a5abff;\n\
         }}\n\
         \n\
         #arrow:image {{\n\
         ignore_event: true;\n\
         anchor_x: 1;\n\
         pivot_x: 1;\n\
         x: -12px;\n\
         anchor_y: 0.5;\n\
         pivot_y: 0.5;\n\
         width: 12px;\n\
         height: 12px;\n\
         source: \"asset/base/ui/icons/dropdown\";\n\
         color: #a5a5abff;\n\
         }}\n\
         }}\n\
         \n\
         #role:color_icon_button {{\n\
         @\"asset/base/style/main#tertiary_button\";\n\
         x: {ROLE_X}px;\n\
         y: {COMBO_Y}px;\n\
         width: {ROLE_W}px;\n\
         height: {COMBO_H}px;\n\
         \n\
         text: {{\n\
         text: \"{PLAIN_PAD}Any\";\n\
         align_x: Left;\n\
         align_y: Center;\n\
         size: 14;\n\
         color: #a5a5abff;\n\
         }}\n\
         \n\
         #arrow:image {{\n\
         ignore_event: true;\n\
         anchor_x: 1;\n\
         pivot_x: 1;\n\
         x: -12px;\n\
         anchor_y: 0.5;\n\
         pivot_y: 0.5;\n\
         width: 12px;\n\
         height: 12px;\n\
         source: \"asset/base/ui/icons/dropdown\";\n\
         color: #a5a5abff;\n\
         }}\n\
         }}\n"
    );

    let combo_w = combo_w();
    for slot in 0..picker_slots() {
        let x = combo_x(slot);
        // The clear button sits inside the right end of its slot, left of the
        // drop arrow, so clearing a slot does not need the list opened first.
        let clear_x = x + combo_w - 52;
        source.push_str(&format!(
            "#slot{slot}:color_icon_button {{\n\
             @\"asset/base/style/main#tertiary_button\";\n\
             x: {x}px;\n\
             y: {COMBO_Y}px;\n\
             width: {combo_w}px;\n\
             height: {COMBO_H}px;\n\
             \n\
             text: {{\n\
             text: \"{EMPTY_SLOT_LABEL}\";\n\
             align_x: Left;\n\
             align_y: Center;\n\
             size: 14;\n\
             color: #a5a5abff;\n\
             }}\n\
             \n\
             #icon:image {{\n\
             ignore_event: true;\n\
             x: 8px;\n\
             anchor_y: 0.5;\n\
             pivot_y: 0.5;\n\
             width: 24px;\n\
             height: 24px;\n\
             visible: false;\n\
             source: \"{ICON_SHEET}\";\n\
             }}\n\
             \n\
             #arrow:image {{\n\
             ignore_event: true;\n\
             anchor_x: 1;\n\
             pivot_x: 1;\n\
             x: -12px;\n\
             anchor_y: 0.5;\n\
             pivot_y: 0.5;\n\
             width: 12px;\n\
             height: 12px;\n\
             source: \"asset/base/ui/icons/dropdown\";\n\
             color: #a5a5abff;\n\
             }}\n\
             }}\n\
             \n\
             #clear{slot}:color_icon_button {{\n\
             x: {clear_x}px;\n\
             y: {MINI_Y}px;\n\
             width: 22px;\n\
             height: 22px;\n\
             visible: false;\n\
             \n\
             btn: {{ color: #00000000; }}\n\
             \n\
             icon: {{\n\
             source: \"asset/base/ui/icons/cross\";\n\
             rect: {{ x: 6px; y: 6px; w: 10px; h: 10px; }}\n\
             }}\n\
             \n\
             #glyph:image {{\n\
             ignore_event: true;\n\
             x: 6px;\n\
             y: 6px;\n\
             width: 10px;\n\
             height: 10px;\n\
             source: \"asset/base/ui/icons/cross\";\n\
             color: #60ddc2ff;\n\
             }}\n\
             }}\n"
        ));
    }

    for slot in 0..picker_slots().saturating_sub(1) {
        let x = swap_x(slot);
        source.push_str(&format!(
            "#swap{slot}:color_icon_button {{\n\
             @\"asset/base/style/main#tertiary_button\";\n\
             x: {x}px;\n\
             y: {COMBO_Y}px;\n\
             width: 34px;\n\
             height: {COMBO_H}px;\n\
             \n\
             icon: {{\n\
             source: \"asset/base/ui/icons/swap\";\n\
             rect: {{ x: 9px; y: 12px; w: 16px; h: 16px; }}\n\
             }}\n\
             \n\
             #glyph:image {{\n\
             ignore_event: true;\n\
             x: 9px;\n\
             y: 12px;\n\
             width: 16px;\n\
             height: 16px;\n\
             source: \"asset/base/ui/icons/swap\";\n\
             color: #60ddc2ff;\n\
             }}\n\
             }}\n"
        ));
    }

    source.push_str(&format!(
        "#delete:color_icon_button {{\n\
         x: {delete_x}px;\n\
         y: {MINI_Y}px;\n\
         width: 22px;\n\
         height: 22px;\n\
         \n\
         btn: {{ color: #00000000; }}\n\
         \n\
         icon: {{\n\
         source: \"asset/base/ui/icons/delete\";\n\
         rect: {{ x: 4px; y: 4px; w: 14px; h: 14px; }}\n\
         }}\n\
         \n\
         #glyph:image {{\n\
         ignore_event: true;\n\
         x: 4px;\n\
         y: 4px;\n\
         width: 14px;\n\
         height: 14px;\n\
         source: \"asset/base/ui/icons/delete\";\n\
         color: #e8645aff;\n\
         }}\n\
         }}\n"
    ));

    source.push_str("}\n");
    source
}

// Height of one item-list node. Paired with [`entry_source`], which lays the
// node out at exactly this height; the two are read together to give
// `#contents` an explicit height.
fn entry_height(entry: &ListEntry) -> i32 {
    match entry {
        ListEntry::Header(_) => ENTRY_HEADER_H,
        ListEntry::Clear | ListEntry::Item(_) => ENTRY_ITEM_H,
    }
}

// `.ui` source for one item-list node: a category header, or a pickable item
// with its sheet icon. Kept to the proven picker shape plus the icon and label
// colours.
fn entry_source(index: usize, entry: &ListEntry) -> String {
    let strings = strings();
    let ai_slot = &strings.ai_slot;
    match entry {
        ListEntry::Clear => format!(
            "e{index}:color_selectable {{\n\
             @\"asset/base/style/main#strategy_option\";\n\
             width: 304px;\n\
             height: {ENTRY_ITEM_H}px;\n\
             label: {{ size: 14; align_x: Left; color: {LIST_CLEAR_TEXT}; }}\n\
             text: \"{PLAIN_PAD}{ai_slot}\";\n\
             }}"
        ),
        // A `color` band with the label as its child, since a label has no
        // fill; both ignore events so a click on a header falls through to the
        // catcher and closes the list.
        ListEntry::Header(name) => format!(
            "e{index}:color {{\n\
             width: 304px;\n\
             height: {ENTRY_HEADER_H}px;\n\
             color: {LIST_HEADER_FILL};\n\
             rounding: Uniform {{ rounding: 4; }}\n\
             ignore_event: true;\n\
             \n\
             #text:label {{\n\
             @\"asset/base/style/main#bold_label\";\n\
             width: 304px;\n\
             height: {ENTRY_HEADER_H}px;\n\
             align_x: Center;\n\
             align_y: Center;\n\
             size: 13;\n\
             color: #a5a5abff;\n\
             ignore_event: true;\n\
             text: \"{name}\";\n\
             }}\n\
             }}"
        ),
        ListEntry::Item(item) => {
            let icon = match &item.frame {
                Some(frame) => format!(
                    "#icon:image {{\n\
                     ignore_event: true;\n\
                     x: 6px;\n\
                     anchor_y: 0.5;\n\
                     pivot_y: 0.5;\n\
                     width: 22px;\n\
                     height: 22px;\n\
                     source: \"{ICON_SHEET}\";\n\
                     rect_tag: \"{frame}\";\n\
                     }}\n"
                ),
                None => String::new(),
            };
            // Padded for the same reason the row combos are: the style centres
            // its label, and left-aligning it puts the name under the icon.
            let pad = if item.frame.is_some() {
                ICON_PAD
            } else {
                PLAIN_PAD
            };
            format!(
                "e{index}:color_selectable {{\n\
                 @\"asset/base/style/main#strategy_option\";\n\
                 width: 304px;\n\
                 height: {ENTRY_ITEM_H}px;\n\
                 label: {{ size: 14; align_x: Left; color: {LIST_ROW_TEXT}; }}\n\
                 text: \"{pad}{}\";\n\
                 {icon}\
                 }}",
                item.name
            )
        }
    }
}

// `.ui` source for one champion-list node. Plain rows, no icons: there is no
// champion portrait in a sheet the mod can address by frame name.
fn champ_entry_source(index: usize, choice: &ChampionChoice) -> String {
    format!(
        "c{index}:color_selectable {{\n\
         @\"asset/base/style/main#strategy_option\";\n\
         width: 244px;\n\
         height: {ENTRY_ITEM_H}px;\n\
         label: {{ size: 14; align_x: Left; color: {LIST_ROW_TEXT}; }}\n\
         text: \"{PLAIN_PAD}{}\";\n\
         }}",
        choice.name
    )
}

// `.ui` source for one entry of the role list. Label only, like the champion
// list: a role is a word, and there is no art for one.
fn role_entry_source(index: usize, role: Role) -> String {
    format!(
        "r{index}:color_selectable {{\n\
         @\"asset/base/style/main#strategy_option\";\n\
         width: 144px;\n\
         height: {ENTRY_ITEM_H}px;\n\
         label: {{ size: 14; align_x: Left; color: {LIST_ROW_TEXT}; }}\n\
         text: \"{PLAIN_PAD}{}\";\n\
         }}",
        role.label()
    )
}

// Removes the spawned row nodes and spawns one per row passing the filter,
// registering every control; rebuilt wholesale, since deleting a row renumbers
// everything after it anyway.
fn rebuild_rows(ctx: &mut StableClient<'_>, entries: &[ListEntry]) {
    let previous = with_state(|state| std::mem::take(&mut state.spawned_rows)).unwrap_or_default();
    for row in previous {
        ctx.ui_remove_node(&editor_row_path(row));
    }

    let mut spawned = Vec::new();
    for row in visible_rows() {
        if !ctx.ui_spawn_source(rows_path(), &row_source(row)) {
            break;
        }
        spawned.push(row);
        REGISTERED.register(ctx, &champ_path(row));
        REGISTERED.register(ctx, &role_path(row));
        REGISTERED.register(ctx, &delete_path(row));
        for slot in 0..picker_slots() {
            REGISTERED.register(ctx, &combo_path(row, slot));
            REGISTERED.register(ctx, &clear_path(row, slot));
        }
        for slot in 0..picker_slots().saturating_sub(1) {
            REGISTERED.register(ctx, &format!("{}.swap{slot}", editor_row_path(row)));
        }
    }
    let count = spawned.len();
    let _ = with_state(|state| state.spawned_rows = spawned.clone());

    // `#rows` is `height: auto`, which measures zero on an unlaid subtree and
    // shows nothing, so the exact height is stated.
    let height = count as i32 * (ROW_HEIGHT as i32 + ROW_SPACING);
    ctx.ui_set_properties(rows_path(), &format!("height: {height}px;"));

    for row in spawned {
        refresh_row(ctx, entries, row);
    }
}

// `.ui` source for one column header, matching the `#c_item*` labels
// `build_editor.ui` authors for the first three.
fn header_source(slot: usize) -> String {
    let x = combo_x(slot);
    let w = combo_w();
    let number = slot + 1;
    let label = strings().col_items[slot.min(5)].clone();
    format!(
        "c_item{number}:label {{\n\
         @\"asset/base/style/main#label\";\n\
         x: {x}px;\n\
         width: {w}px;\n\
         height: 32px;\n\
         align_x: Left;\n\
         align_y: Center;\n\
         size: 13;\n\
         color: #a5a5abff;\n\
         text: \"{label}\";\n\
         }}\n"
    )
}

// Lines the column headers up with the columns, re-placing the authored three
// and spawning the rest; the subtree is rebuilt on every [`ensure_editor`], so
// no stale header is left.
fn sync_column_headers(ctx: &mut StableClient<'_>) {
    for slot in 0..picker_slots() {
        let path = format!("{}.c_item{}", colheader_path(), slot + 1);
        if ctx.ui_exists(&path) {
            // Text as well as geometry: the first three are authored in
            // `build_editor.ui` with their English labels, so this is the only
            // thing that translates them.
            let label = strings().col_items[slot.min(5)].clone();
            ctx.ui_set_properties(
                &path,
                &format!(
                    "x: {}px; width: {}px; text: \"{label}\";",
                    combo_x(slot),
                    combo_w()
                ),
            );
        } else {
            ctx.ui_spawn_source(colheader_path(), &header_source(slot));
        }
    }
}

// Rows that pass the current filter, as indices into `state.rows`; filtered by
// not spawning (see the module notes). A row with no champion always passes, so
// `+ Add Champion` never adds an invisible row.
fn visible_rows() -> Vec<usize> {
    // Taken before the lock: `with_state` is a plain `Mutex`, so reading the
    // champion list from inside the closure would deadlock on itself.
    let champions = snapshot_champions();
    with_state(|state| {
        // Owned, so the borrow of `state.filter` ends before `state.rows` is
        // iterated below.
        let terms = filter_terms(&state.filter);
        // Terms a side cell wrote are whole names, and are matched whole:
        // "Knight" off the draft must not bring up Magic Knight's row as well.
        // What the player types still matches anywhere in a name.
        let whole = state.side_blue || state.side_red;
        state
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| match &row.champion {
                None => true,
                // The id as well as the shown name, so a build authored against
                // a champion this install does not have — whose label falls back
                // to the raw id — is still reachable by typing it.
                Some(id) => {
                    terms.is_empty() || {
                        let label = champion_label(&champions, Some(id)).to_lowercase();
                        let id = id.to_lowercase();
                        // Any term, not all: commas read as "or", so a row needs
                        // to answer only one of the names typed.
                        if whole {
                            terms.iter().any(|term| *term == label || *term == id)
                        } else {
                            let haystack = format!("{label} {id}");
                            terms.iter().any(|term| haystack.contains(term))
                        }
                    }
                }
            })
            .map(|(index, _)| index)
            .collect()
    })
    .unwrap_or_default()
}

// The search box's text as the terms a row may match: comma-separated
// alternatives, lowercased, blanks dropped so a half-typed `ashe,` matches
// neither everything nor nothing.
fn filter_terms(filter: &str) -> Vec<String> {
    filter
        .split(',')
        .map(|term| term.trim().to_lowercase())
        .filter(|term| !term.is_empty())
        .collect()
}

// Whether a filter is narrowing the list, through [`filter_terms`] so that a
// lone `,` does not count.
fn filter_active() -> bool {
    with_state(|state| !filter_terms(&state.filter).is_empty()).unwrap_or(false)
}

// Reads the filter box every frame (its only event fires on commit) and
// rebuilds the rows when the text changed. Typing puts out the side cells;
// their own writes are waited out for [`SIDE_SETTLE_FRAMES`] in case the box
// reads back late.
fn sync_filter(ctx: &mut StableClient<'_>) {
    let Some(text) = ctx.ui_text_edit_text(search_path()) else {
        return;
    };
    let Some(unlit) = with_state(|state| {
        if state.filter == text {
            state.side_settle = 0;
            return None;
        }
        if state.side_settle > 0 {
            state.side_settle -= 1;
            return None;
        }
        state.filter = text.clone();
        let unlit = state.side_blue || state.side_red;
        state.side_blue = false;
        state.side_red = false;
        Some(unlit)
    })
    .flatten() else {
        return;
    };
    if unlit {
        paint_sides(ctx);
    }
    fit_search(ctx);

    ctx.ui_set_visible(search_clear_path(), !text.trim().is_empty());
    // A floating list is positioned against the row that opened it, and that row
    // is about to be removed and respawned.
    close_list(ctx);
    let entries = snapshot_entries();
    rebuild_rows(ctx, &entries);
}

// The composition test's tactics panel, or `None` when it is not up. The
// training layout's root is `training` and `contents` exists only in
// `strategy.ui`, so [`COMP_ROOT`] is a one-lookup candidate, backed by a
// throttled walk for the `comp_test_popup.tactics.builds` shape.
fn resolve_comp_tactics(ctx: &StableClient<'_>) -> Option<String> {
    // Cached from an earlier frame, and still resolving. This is the usual path
    // and it costs one call.
    if let Some(found) = with_state(|state| state.comp_tactics.clone()).flatten() {
        if ctx.ui_exists(&found) {
            return Some(found);
        }
        let _ = with_state(|state| state.comp_tactics = None);
    }

    let candidate = format!("{COMP_ROOT}.{COMP_TACTICS_REL}");
    if ctx.ui_exists(&candidate) {
        let _ = with_state(|state| state.comp_tactics = Some(candidate.clone()));
        return Some(candidate);
    }

    // Everything above is one `ui_exists`; the sweep runs only on the Training
    // tab, or every other screen would pay a failed 600-node walk twice a
    // second.
    if !on_training_tab(ctx) {
        return None;
    }

    let due = with_state(|state| {
        state.comp_probe_tick = state.comp_probe_tick.wrapping_add(1);
        state.comp_probe_tick % COMP_PROBE_EVERY == 0
    })
    .unwrap_or(false);
    if !due {
        return None;
    }

    let found = find_comp_tactics(ctx)?;
    let _ = with_state(|state| state.comp_tactics = Some(found.clone()));
    Some(found)
}

// Whether the client says the Training tab is up, gating the sweep like
// `item_stats::ui`'s `on_statistics_tab`. `None` allows the sweep (the throttle
// still bounds it), so a broken call does not kill the fallback.
fn on_training_tab(ctx: &StableClient<'_>) -> bool {
    let Some(tab) = ctx.client_main_tab() else {
        return true;
    };
    tab.to_ascii_lowercase().contains("train")
}

// Breadth-first from the UI root for the node holding the composition test's
// build block. Bounded by both depth and node count — one wide level can be
// hundreds of nodes, and this runs on the UI thread.
fn find_comp_tactics(ctx: &StableClient<'_>) -> Option<String> {
    let mut level: Vec<String> = ctx.ui_child_names("");
    if level.is_empty() {
        // A root that enumerates nothing would make this inert forever, so the
        // scene roots seen in the layouts are tried as a seed rather than
        // trusting one call.
        level = [COMP_ROOT, "main"].iter().map(|s| s.to_string()).collect();
    }

    let mut budget = COMP_PROBE_NODES;
    for _ in 0..COMP_PROBE_DEPTH {
        if level.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for path in level {
            if ctx.ui_exists(&format!("{path}.{COMP_BUILDS_REL}")) {
                return Some(format!("{path}.{COMP_TACTICS_REL}"));
            }
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

// Puts the editor in the composition test's build section in place of the
// vanilla rows: opened on arrival, there being no tab, and torn down with the
// screen like the strategy screen's.
fn comp_test_host(ctx: &mut StableClient<'_>, tactics: &str) {
    let builds = format!("{tactics}.builds");

    shrink_team_tactics(ctx, tactics);

    // Measured against the header and the Back button, which stay visible, not
    // the hidden block it replaces. The dialog is resized at runtime and the
    // lane test lays out differently, so nothing comes from the layout file.
    let slot = comp_test_slot(ctx, tactics);

    // Point every editor path at this screen before one is read. The strategy
    // screen's host is a node inside `strategy.ui`, which is not loaded here, so
    // until this runs `editor_path()` names something that cannot exist.
    set_host(tactics);

    // Re-asserted every frame: game code rebuilds this block whenever the
    // champion selection changes, and `post_update` runs after it.
    ctx.ui_set_visible(&builds, false);

    // Whether the panel is already up. It survives the dialog closing — same
    // scene, same subtree — so this is true again the *next* time the dialog is
    // opened, in whichever mode.
    let already_up =
        with_state(|state| state.showing).unwrap_or(false) && ctx.ui_exists(editor_path());

    // Settled before any spawn, because rows carry the panel width in the source
    // they are built from.
    let resized = slot.map(|s| want_panel_size(s.2, s.3)).unwrap_or(false);

    if !already_up {
        // Read here, in `post_update`, for the reason given on the strategy
        // screen's copy: `setting_get_json` returns None inside a click handler.
        cached_entries(ctx);
        cached_champions(ctx);
        if !ensure_editor(ctx) {
            return;
        }
    }

    // Geometry is re-asserted every frame too: the 5v5 and lane tests lay out
    // the same panel differently, and the editor outlives the dialog.
    apply_panel_size(ctx);
    if resized {
        let entries = snapshot_entries();
        rebuild_rows(ctx, &entries);
    }

    // Rects are absolute and the popup's x/y host-relative, so the host origin
    // comes off ([`host_origin`]). Left in place when the measurement fails.
    if let Some((x, y, _, _)) = slot {
        let (origin_x, origin_y) = host_origin(ctx);
        let (left, top) = (x as i32 - origin_x, y as i32 - origin_y);
        ctx.ui_set_properties(popup_path(), &format!("x: {left}px; y: {top}px;"));
    }

    if !already_up {
        let entries = snapshot_entries();
        open_editor(ctx, &entries);
    }
}

// Whether this screen shows team tactics, which tells the 5v5 from the lane
// test. By visibility of the "Team Tactics" label and block, since hidden rows
// still report a rect; a missing node counts as hidden.
fn team_tactics_shown(ctx: &StableClient<'_>, tactics: &str) -> bool {
    ctx.ui_visible(&format!("{tactics}.strategy_header")) == Some(true)
        && ctx.ui_visible(&format!("{tactics}.strategy")) == Some(true)
}

// Height the team-tactics block is cut to on the 5v5 screen: eight of its
// twelve rows, on the 38px pitch they are authored at. The rest scroll.
const TEAM_TACTICS_HEIGHT: i32 = 296;

// The block's authored height, restored on any screen that is not the 5v5.
// Keep in step with `#strategy` in `ui/layout/training.ui`, which this mod
// ships - it is the same twelve rows at the same pitch.
const TEAM_TACTICS_FULL_HEIGHT: i32 = 456;

// Gap the exe leaves between the bottom of that block and the "Personal
// Tactics" header. Measured, not guessed: with the block at its authored
// `(y 160, h 456)` the exe puts the header at `y 662`, and 662 - 616 is this.
const BUILD_HEADER_GAP: i32 = 46;

// Shortens the team-tactics block on the 5v5 screen and pulls the header under
// it, giving the height to the editor. `training.ui` shares the block between
// the 5v5 and the lane test, so the shrink is done here, every frame, since
// game code rebuilds the panel on reselect.
fn shrink_team_tactics(ctx: &mut StableClient<'_>, tactics: &str) {
    // Lane test: put the block back, since the panel outlives the dialog and a
    // 5v5 before it would leave the block shrunk. The header needs nothing; the
    // exe places it itself.
    if !team_tactics_shown(ctx, tactics) {
        ctx.ui_set_properties(
            &format!("{tactics}.strategy"),
            &format!("height: {TEAM_TACTICS_FULL_HEIGHT}px;"),
        );
        return;
    }

    let Some(panel) = ctx.ui_node_rect(tactics) else {
        return;
    };
    let Some(strategy) = ctx.ui_node_rect(&format!("{tactics}.strategy")) else {
        return;
    };

    ctx.ui_set_properties(
        &format!("{tactics}.strategy"),
        &format!("height: {TEAM_TACTICS_HEIGHT}px;"),
    );

    // Absolute rect to parent-relative property, as the editor popup does; the
    // block's top is read live.
    let top = (strategy.1 - panel.1).round() as i32 + TEAM_TACTICS_HEIGHT + BUILD_HEADER_GAP;
    ctx.ui_set_properties(
        &format!("{tactics}.build_header"),
        &format!("y: {top}px;"),
    );
}

// The rectangle for the editor inside the composition test, `(x, y, w, h)` in
// `ui_node_rect` space: from under the "Personal Tactics" header to above Back,
// at the header's x and width.
fn comp_test_slot(ctx: &StableClient<'_>, tactics: &str) -> Option<(f32, f32, u32, u32)> {
    let header = ctx.ui_node_rect(&format!("{tactics}.build_header"))?;
    let back = ctx.ui_node_rect(&format!("{tactics}.back"))?;
    if header.2 <= 0.0 || header.3 <= 0.0 {
        return None;
    }

    let top = header.1 + header.3 + COMP_SLOT_GAP;
    let bottom = back.1 - COMP_SLOT_GAP;
    if bottom <= top {
        return None;
    }
    Some((header.0, top, header.2 as u32, (bottom - top) as u32))
}

// Spawns the window, its rows and the floating lists and registers every
// control, on first need and retried on failure. `modal_ready` survives a
// screen change, so the `ui_exists` check is what notices the subtree is gone
// and respawns into the current host.
fn ensure_editor(ctx: &mut StableClient<'_>) -> bool {
    if with_state(|state| state.modal_ready).unwrap_or(false) && ctx.ui_exists(editor_path()) {
        return true;
    }

    // Drop any half-built subtree from a previous attempt so this is idempotent.
    ctx.ui_remove_node(editor_path());
    if !ctx.ui_spawn_source(paths().parent, EDITOR_SOURCE) {
        return false;
    }
    if !ctx.ui_exists(editor_path()) {
        return false;
    }
    // Before anything reads a label. The subtree was just respawned from source,
    // so its English literals are back and everything below — the headers, the
    // rows, the buttons — has to be told the game's language again.
    load_strings(ctx);
    apply_strings(ctx);
    sync_column_headers(ctx);

    let entries = cached_entries(ctx);
    let champions = cached_champions(ctx);

    let contents = list_contents_path();
    for (index, entry) in entries.iter().enumerate() {
        if !ctx.ui_spawn_source(&contents, &entry_source(index, entry)) {
            break;
        }
        if !matches!(entry, ListEntry::Header(_)) {
            REGISTERED.register(ctx, &entry_path(index));
        }
    }
    let content_height: i32 = entries.iter().map(entry_height).sum();
    ctx.ui_set_properties(&contents, &format!("height: {content_height}px;"));

    let champ_contents = champ_contents_path();
    for (index, choice) in champions.iter().enumerate() {
        if !ctx.ui_spawn_source(&champ_contents, &champ_entry_source(index, choice)) {
            break;
        }
        REGISTERED.register(ctx, &champ_entry_path(index));
    }
    ctx.ui_set_properties(
        &champ_contents,
        &format!("height: {}px;", champions.len() as i32 * ENTRY_ITEM_H),
    );

    // The six roles are fixed, so unlike the champion list this one is spawned
    // complete and never rebuilt.
    let role_contents = role_contents_path();
    for (index, role) in Role::ALL.iter().enumerate() {
        if !ctx.ui_spawn_source(&role_contents, &role_entry_source(index, *role)) {
            break;
        }
        REGISTERED.register(ctx, &role_entry_path(index));
    }
    ctx.ui_set_properties(
        &role_contents,
        &format!("height: {}px;", Role::ALL.len() as i32 * ENTRY_ITEM_H),
    );

    // Read from disk here rather than at every open, so a file edited by hand
    // while the game sits on this screen is picked up.
    let _ = with_state(|state| {
        state.rows = build_config::load_champion_rows();
        canonicalize_rows(&entries, &mut state.rows);
        state.spawned_rows.clear();
        // The subtree was just spawned, so its filter box is empty whatever the
        // last screen was left filtered by, and no side cell is lit.
        state.filter.clear();
        state.side_blue = false;
        state.side_red = false;
        state.side_settle = 0;
        state.search_fit = None;
    });
    rebuild_rows(ctx, &entries);

    for path in [
        save_path(),
        add_path(),
        unique_on_path(),
        unique_off_path(),
        scope_all_path(),
        scope_own_path(),
        listcatch_path(),
        search_clear_path(),
        side_blue_path(),
        side_red_path(),
    ] {
        REGISTERED.register(ctx, path);
    }
    // Authored hidden, so this is what puts them up: the lineup was worked out
    // before the subtree it is shown in existed.
    refresh_sides(ctx);

    // Both lists start hidden but nothing in the source stops them taking the
    // wheel, so without this a scroll of the rows also scrolls two panels that
    // are not on screen — invisible until one is opened and found part-way down.
    focus_scroll(ctx, rowscroll_path());

    let _ = with_state(|state| state.modal_ready = true);
    true
}

// -- interaction --------------------------------------------------------

// Enters the Builds tab: selects it, hides what the other tabs show, and puts
// the editor in their place. Nothing arbitrates between the `color_selectable`
// tabs and game code has never heard of ours, so the deselecting is done here.
fn open_editor(ctx: &mut StableClient<'_>, entries: &[ListEntry]) {
    paint_tabs(ctx, true);

    // The panel is authored visible and only ever hidden with the editor around
    // it, so this is belt and braces rather than a mode reset.
    ctx.ui_set_visible(popup_path(), true);

    for panel in CONTENT_PANELS {
        ctx.ui_set_visible(panel, false);
    }
    ctx.ui_set_visible(ITEM_INFO_BTN, true);
    keep_matchup(ctx);

    // A tick from a previous visit is not a statement about this one: the
    // subtree survives closing the panel, and a tick already showing on arrival
    // acknowledges a click the player has since forgotten making.
    clear_saved(ctx);

    refresh_unique(ctx);
    refresh_scope(ctx);
    // The spawned set, not every row: with a filter up the others have no nodes
    // to repaint.
    for row in with_state(|state| state.spawned_rows.clone()).unwrap_or_default() {
        refresh_row(ctx, entries, row);
    }
    ctx.ui_set_visible(editor_path(), true);
    let _ = with_state(|state| state.showing = true);
}

// Leaves the Builds tab. It restores no panel: every exit is a click on Team,
// whose own handler shows the Team columns.
fn close_editor(ctx: &mut StableClient<'_>) {
    close_list(ctx);
    paint_tabs(ctx, false);
    ctx.ui_set_visible(editor_path(), false);
    // "Closing Out" is the one thing put back, since game code may not re-show
    // it; a second `true` is harmless.
    ctx.ui_set_visible(GAME_FINISH_PATH, true);
    let _ = with_state(|state| state.showing = false);
}

// Paints which tab looks selected. `ui_set_selectable_selected` rejects
// `color_selectable`, so the highlight is drawn: through `image`/`label` for
// ours (never selected) and `selected_image`/`selected_label` for Team (always
// selected underneath).
//
// With `stroke` non-zero, `color` is the outline and `back_color` the fill, so
// lighting our tab drops the stroke to 0. Hover is written on both states: a
// lit tab's matches its resting look, and a dim one greys its outline and
// brightens its text.
fn paint_tabs(ctx: &mut StableClient<'_>, builds_active: bool) {
    if !ctx.ui_set_properties(BUILDS_TAB, &tab_style("image", "label", builds_active)) {}
    if !ctx.ui_set_properties(
        TEAM_TAB,
        &tab_style("selected_image", "selected_label", !builds_active),
    ) {}
}

// Idle label colour of a pickable list row, and of the "Let Player Decide" row
// above them, which is greyer because it is the absence of a pick.
const LIST_ROW_TEXT: &str = "#d7dbe4ff";
const LIST_CLEAR_TEXT: &str = "#a5a5abff";

// One list row's appearance, lit the way the Builds tab is: the rows are
// `color_selectable` for the fill, so which is picked is painted, not set (see
// [`paint_tabs`]). A plain `selectable` could be set but draws no fill.
fn list_entry_style(lit: bool, idle_text: &str) -> String {
    let (fill, text, stroke) = if lit {
        (TAB_SELECTED_FILL, TAB_SELECTED_TEXT, 0)
    } else {
        (TAB_IDLE_FILL, idle_text, 1)
    };
    let (hover_line, hover_text) = if lit {
        (fill, text)
    } else {
        (TAB_HOVER_LINE, TAB_HOVER_TEXT)
    };
    format!(
        "image: {{ color: {fill}; back_color: {fill}; stroke: {stroke}; \
         rounding: Uniform {{ rounding: 4; }} \
         hover: {{ color: {hover_line}; back_color: {fill}; }} }} \
         label: {{ size: 14; align_x: Left; color: {text}; \
         hover: {{ color: {hover_text}; }} }}"
    )
}

// One tab's appearance through the property pair that renders for it:
// `image`/`label` for a tab this mod added, `selected_image`/`selected_label`
// for a vanilla one. Shared with [`crate::item_stats::ui`]'s statistics tab.
pub(crate) fn tab_style(image_key: &str, label_key: &str, lit: bool) -> String {
    let (fill, text, stroke) = if lit {
        (TAB_SELECTED_FILL, TAB_SELECTED_TEXT, 0)
    } else {
        (TAB_IDLE_FILL, TAB_IDLE_TEXT, 1)
    };
    let (hover_line, hover_text) = if lit {
        (fill, text)
    } else {
        (TAB_HOVER_LINE, TAB_HOVER_TEXT)
    };
    format!(
        "{image_key}: {{ color: {fill}; back_color: {fill}; stroke: {stroke}; \
         rounding: Uniform {{ rounding: 8; }} \
         hover: {{ color: {hover_line}; back_color: {fill}; }} }} \
         {label_key}: {{ color: {text}; hover: {{ color: {hover_text}; }} }}"
    )
}

// Holds the Matchup card on screen and "Closing Out" off it, on entry and every
// frame, since game code re-asserts both from its own tab.
fn keep_matchup(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(SUB4_PATH, true);
    ctx.ui_set_visible(GAME_FINISH_PATH, false);
    ctx.ui_set_visible(MATCHUP_PATH, true);
}

// Places a floating list under the control that opened it, or above where there
// is no room below. The anchor comes from `ui_node_rect` (layout design space),
// since rows scroll.
fn place_list(ctx: &mut StableClient<'_>, panel: &str, anchor: &str, width: i32) -> (i32, i32) {
    let Some((x, y, _, h)) = ctx.ui_node_rect(anchor).filter(|rect| rect.3 > 0.0) else {
        return (0, 0);
    };
    let (x, y, h) = (x.round() as i32, y.round() as i32, h.round() as i32);

    let below = y + h + 4;
    let top = if below + LIST_H <= CANVAS_H - 8 {
        below
    } else {
        (y - LIST_H - 4).max(8)
    };
    // Nudged left to keep the whole list on screen, clamped in screen space,
    // then moved into the host's frame.
    let left = x.min(CANVAS_W - width - 8).max(8);

    let (origin_x, origin_y) = host_origin(ctx);
    let (local_left, local_top) = (left - origin_x, top - origin_y);
    if !ctx.ui_set_properties(panel, &format!("x: {local_left}px; y: {local_top}px;")) {}
    (left, top)
}

// Origin of the editor's host in `ui_node_rect`'s absolute space, to take off
// measured positions (a node's x/y are parent-relative). (0, 0) on the strategy
// screen, and on a failed measurement, since a list in the wrong place beats
// none.
fn host_origin(ctx: &StableClient<'_>) -> (i32, i32) {
    ctx.ui_node_rect(paths().parent)
        .map(|(x, y, _, _)| (x.round() as i32, y.round() as i32))
        .unwrap_or((0, 0))
}

// Shows the item list under the slot that was clicked, with the slot's current
// pick ticked.
fn open_item_list(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize, slot: usize) {
    place_list(ctx, itemlist_path(), &combo_path(row, slot), LIST_W);

    let pinned = pinned_key(&snapshot_rows(), row, slot);
    for (index, entry) in entries.iter().enumerate() {
        let selected = match entry {
            ListEntry::Clear => pinned.is_none(),
            ListEntry::Item(item) => pinned.as_deref() == Some(item.key.as_str()),
            ListEntry::Header(_) => continue,
        };
        let idle = match entry {
            ListEntry::Clear => LIST_CLEAR_TEXT,
            _ => LIST_ROW_TEXT,
        };
        ctx.ui_set_properties(&entry_path(index), &list_entry_style(selected, idle));
    }

    ctx.ui_set_visible(listcatch_path(), true);
    ctx.ui_set_visible(itemlist_path(), true);
    focus_scroll(ctx, itemlist_scroll());
    let _ = with_state(|state| state.open_list = Some(OpenList::Item { row, slot }));
}

// Shows the champion list under the row's champion button, with the row's
// current champion ticked.
fn open_champ_list(ctx: &mut StableClient<'_>, champions: &[ChampionChoice], row: usize) {
    place_list(ctx, champlist_path(), &champ_path(row), CHAMP_LIST_W);

    let current = snapshot_rows()
        .get(row)
        .and_then(|entry| entry.champion.clone());
    for (index, choice) in champions.iter().enumerate() {
        let selected = current.as_deref() == Some(choice.id.as_str());
        ctx.ui_set_properties(
            &champ_entry_path(index),
            &list_entry_style(selected, LIST_ROW_TEXT),
        );
    }

    ctx.ui_set_visible(listcatch_path(), true);
    ctx.ui_set_visible(champlist_path(), true);
    focus_scroll(ctx, champlist_scroll());
    let _ = with_state(|state| state.open_list = Some(OpenList::Champion { row }));
}

// Opens the role list under a row's Role button.
fn open_role_list(ctx: &mut StableClient<'_>, row: usize) {
    place_list(ctx, rolelist_path(), &role_path(row), ROLE_LIST_W);

    let current = snapshot_rows()
        .get(row)
        .map(|entry| entry.role)
        .unwrap_or_default();
    for (index, role) in Role::ALL.iter().enumerate() {
        ctx.ui_set_properties(
            &role_entry_path(index),
            &list_entry_style(*role == current, LIST_ROW_TEXT),
        );
    }

    ctx.ui_set_visible(listcatch_path(), true);
    ctx.ui_set_visible(rolelist_path(), true);
    focus_scroll(ctx, rolelist_scroll());
    let _ = with_state(|state| state.open_list = Some(OpenList::Role { row }));
}

// Hides whichever floating list is showing. All are hidden unconditionally:
// it costs one call and cannot leave a stale panel behind.
fn close_list(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(itemlist_path(), false);
    ctx.ui_set_visible(champlist_path(), false);
    ctx.ui_set_visible(rolelist_path(), false);
    ctx.ui_set_visible(listcatch_path(), false);
    focus_scroll(ctx, rowscroll_path());
    let _ = with_state(|state| state.open_list = None);
}

// Node name of the vanilla Item Info popup, a full-screen `z: 200` panel game
// code spawns, so its path has to be found ([`find_info_popup`]).
const INFO_POPUP_NAME: &str = "item_info_popup";

// The scroll view that is live when nothing may scroll at all. Not a path
// anything has, which is the point: [`focus_scroll`] mutes every view that is
// not the one named.
const SCROLL_NONE: &str = "";

// Looks for the Item Info popup breadth-first from `main`, two levels deep:
// game code spawns it as a sibling of the screen, so its path cannot be written
// down.
fn find_info_popup(ctx: &StableClient<'_>) -> Option<String> {
    let mut level = vec!["main".to_string()];
    for _ in 0..2 {
        let mut next = Vec::new();
        for parent in level {
            for name in ctx.ui_child_names(&parent) {
                let path = format!("{parent}.{name}");
                if name == INFO_POPUP_NAME {
                    return Some(path);
                }
                next.push(path);
            }
        }
        level = next;
    }
    None
}

// Mutes the editor's scroll views while the Item Info popup is up (game code's
// node, which cannot be made to consume the wheel), and hands the wheel back
// when it closes, to an open list if there is one. Written only on the
// transition.
fn sync_info_popup(ctx: &mut StableClient<'_>) {
    // Searched for once a second until found, in case game code spawns the
    // popup on first use rather than with the screen.
    let known = with_state(|state| {
        let due = state.info_popup.is_none() && state.info_probe_tick % 60 == 0;
        state.info_probe_tick = state.info_probe_tick.wrapping_add(1);
        (state.info_popup.clone(), due)
    })
    .unwrap_or((None, false));
    let path = match known {
        (Some(path), _) => path,
        (None, false) => return,
        (None, true) => {
            let found = find_info_popup(ctx);
            let _ = with_state(|state| state.info_popup = found.clone());
            match found {
                Some(path) => path,
                None => return,
            }
        }
    };

    let showing = ctx.ui_visible(&path).unwrap_or(false);
    let changed = with_state(|state| {
        let changed = state.info_showing != showing;
        state.info_showing = showing;
        changed
    })
    .unwrap_or(false);
    if !changed {
        return;
    }

    if showing {
        focus_scroll(ctx, SCROLL_NONE);
        return;
    }
    let active = match with_state(|state| state.open_list).flatten() {
        Some(OpenList::Item { .. }) => itemlist_scroll(),
        Some(OpenList::Champion { .. }) => champlist_scroll(),
        Some(OpenList::Role { .. }) => rolelist_scroll(),
        None => rowscroll_path(),
    };
    focus_scroll(ctx, active);
}

// Gives the wheel to one scroll view and takes it from the others, since an
// open list and the rows under it would both scroll. Done with `speed: 0`, as
// `ignore_event` governs hit-testing and the wheel follows hover; written on
// all three every time, and [`close_list`] restores the rows.
fn focus_scroll(ctx: &mut StableClient<'_>, active: &str) {
    for path in [
        rowscroll_path(),
        itemlist_scroll(),
        champlist_scroll(),
        rolelist_scroll(),
    ] {
        let (ignore, speed) = if path == active {
            ("false", SCROLL_SPEED)
        } else {
            ("true", 0)
        };
        ctx.ui_set_properties(path, &format!("ignore_event: {ignore}; speed: {speed};"));
    }
}

// Row index from an event path (`….rows.row2.slot1` -> `2`).
fn row_from_path(path: &str) -> Option<usize> {
    path.rsplit_once(".row")
        .and_then(|(_, rest)| rest.split('.').next())
        .and_then(|digits| digits.parse::<usize>().ok())
}

// Trailing index of a `slotN` / `clearN` / `swapN` / `eN` / `cN` node name.
fn index_after(path: &str, prefix: &str) -> Option<usize> {
    path.rsplit_once(prefix)?.1.parse().ok()
}

// Single handler for every registered control; dispatches on the firing path.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // `Remove` fires as nodes are torn down, every match start, and must not
    // reach the dispatch as if clicked.
    if matches!(event.kind, Some(UiEventKindV1::Remove)) {
        return;
    }
    let path = event.path.clone();

    if path == BUILDS_TAB {
        // This tab only exists on the strategy screen, so its host is settled.
        // `post_update` has almost certainly said so already; saying it here too
        // is what keeps the handler correct on its own rather than by ordering.
        set_host(UI_ROOT);
        // Back to the authored size. Settled before the spawn for the reason
        // given on `want_panel_size`: rows carry the width they were built at,
        // and the composition test builds them narrower.
        let resized = want_panel_size(PANEL_W, PANEL_H);
        // Built on first entry rather than up front, so a strategy screen whose
        // Builds tab is never opened pays nothing for it.
        if !ensure_editor(ctx) {
            return;
        }
        apply_panel_size(ctx);
        if resized {
            let entries = snapshot_entries();
            rebuild_rows(ctx, &entries);
        }
        // Back to the authored rect; belt and braces, since the composition
        // test moves its own subtree.
        ctx.ui_set_properties(popup_path(), EDITOR_RECT_STRATEGY);
        let entries = snapshot_entries();
        open_editor(ctx, &entries);
        return;
    }

    // Every event on the Team tab lands here, hover included, so this must be
    // idempotent and cheap — it is only allowed to do anything when the Builds
    // tab is the one currently up.
    if path == TEAM_TAB {
        if with_state(|state| state.showing).unwrap_or(false) {
            close_editor(ctx);
        }
        return;
    }

    if path == listcatch_path() {
        close_list(ctx);
        return;
    }

    let entries = snapshot_entries();

    if path == save_path() {
        // Save stays on the tab and reports through the tick, which shows only
        // when the write succeeds and goes down on the next edit
        // (`clear_saved`).
        let rows = snapshot_rows();
        ctx.ui_set_visible(saved_path(), build_config::save_champion_rows(&rows));
        return;
    }

    if path == add_path() {
        let _ = with_state(|state| state.rows.push(ChampionRow::default()));
        clear_saved(ctx);
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    if path == search_clear_path() {
        ctx.ui_set_text_edit_text(search_path(), "");
        ctx.ui_set_visible(search_clear_path(), false);
        let _ = with_state(|state| {
            state.filter.clear();
            // The names a side cell wrote go with the rest of the box.
            state.side_blue = false;
            state.side_red = false;
        });
        paint_sides(ctx);
        fit_search(ctx);
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    // Each side cell is its own switch, so a click on a lit one puts it out.
    // Only a click counts (an unreported kind too, the `item_stats::ui` gate),
    // or other events would undo it.
    if path == side_blue_path() || path == side_red_path() {
        if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
            return;
        }
        let red = path == side_red_path();
        let (text, added) = with_state(|state| {
            let lit = if red {
                state.side_red = !state.side_red;
                state.side_red
            } else {
                state.side_blue = !state.side_blue;
                state.side_blue
            };
            // Lighting a cell also gives every champion of its side a row (an
            // empty one at the end where it has none), never written until
            // something is pinned.
            let mut added = false;
            if lit {
                for id in state.side_ids[usize::from(red)].clone() {
                    let listed = state
                        .rows
                        .iter()
                        .any(|row| row.champion.as_deref() == Some(id.as_str()));
                    if !listed {
                        state.rows.push(ChampionRow {
                            champion: Some(id),
                            ..ChampionRow::default()
                        });
                        added = true;
                    }
                }
            }
            // Whatever was in the box is replaced, not added to: the cells say
            // what it holds. Recorded as read here, so `sync_filter` does not
            // find the box changed and take that for typing.
            state.filter = sides_text(state);
            state.side_settle = SIDE_SETTLE_FRAMES;
            (state.filter.clone(), added)
        })
        .unwrap_or_default();
        if added {
            clear_saved(ctx);
        }
        paint_sides(ctx);
        // Sized before it is filled, so the names arrive in a box that holds
        // them on one line.
        fit_search(ctx);
        ctx.ui_set_text_edit_text(search_path(), &text);
        ctx.ui_set_visible(search_clear_path(), !text.is_empty());
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    // A segmented control's cells assert a value, so clicking the lit one is a
    // no-op. Only the setting is written; the saved builds are untouched.
    if path == unique_on_path() || path == unique_off_path() {
        if build_config::set_smart_builds(path == unique_on_path()) {
            refresh_unique(ctx);
        }
        return;
    }

    if path == scope_all_path() || path == scope_own_path() {
        // Greyed out on the tests (see `scope_applies`): a click there must
        // not change a setting the screen says it has nothing to do with.
        if scope_applies() && build_config::set_own_team_only(path == scope_own_path()) {
            refresh_scope(ctx);
        }
        return;
    }

    if path.starts_with(&list_contents_path()) {
        if let Some(index) = index_after(&path, ".e") {
            pick_item(ctx, &entries, index);
        }
        return;
    }

    if path.starts_with(&champ_contents_path()) {
        if let Some(index) = index_after(&path, ".c") {
            pick_champion(ctx, &entries, index);
        }
        return;
    }

    if path.starts_with(&role_contents_path()) {
        if let Some(index) = index_after(&path, ".r") {
            pick_role(ctx, &entries, index);
        }
        return;
    }

    let Some(row) = row_from_path(&path) else {
        return;
    };

    if path.ends_with(".delete") {
        let _ =
            with_state(|state| (row < state.rows.len()).then(|| state.rows.remove(row).champion))
                .flatten();
        let rows = snapshot_rows();
        build_config::save_champion_rows(&rows);
        clear_saved(ctx);
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    if let Some(slot) = index_after(&path, ".clear") {
        if slot < picker_slots() {
            edit_row(row, |entry| entry.slots[slot] = None);
            clear_saved(ctx);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".swap") {
        if slot + 1 < picker_slots() {
            edit_row(row, |entry| entry.slots.swap(slot, slot + 1));
            clear_saved(ctx);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
        }
        return;
    }

    if path.ends_with(".role") {
        if with_state(|state| state.open_list).flatten() == Some(OpenList::Role { row }) {
            close_list(ctx);
        } else {
            open_role_list(ctx, row);
        }
        return;
    }

    if path.ends_with(".champ") {
        // Clicking the open control again closes its list, so both toggle.
        if with_state(|state| state.open_list).flatten() == Some(OpenList::Champion { row }) {
            close_list(ctx);
        } else {
            open_champ_list(ctx, &snapshot_champions(), row);
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".slot") {
        if slot >= picker_slots() {
            return;
        }
        if with_state(|state| state.open_list).flatten() == Some(OpenList::Item { row, slot }) {
            close_list(ctx);
        } else {
            open_item_list(ctx, &entries, row, slot);
        }
    }
}

// Commits a clicked item row: clicking the pinned item again clears the slot,
// so a slot can be returned to AI choice without reaching for the X.
fn pick_item(ctx: &mut StableClient<'_>, entries: &[ListEntry], index: usize) {
    let Some(Some(OpenList::Item { row, slot })) = with_state(|state| state.open_list) else {
        return;
    };
    // Clicking the pinned item again unpins it, which is the same outcome as
    // picking the Clear row — kept because it is the quicker gesture once known.
    let picked = match entries.get(index) {
        Some(ListEntry::Clear) => None,
        Some(ListEntry::Item(item)) => {
            let already =
                pinned_key(&snapshot_rows(), row, slot).as_deref() == Some(item.key.as_str());
            (!already).then(|| item.key.clone())
        }
        _ => return,
    };
    edit_row(row, |entry| entry.slots[slot] = picked.clone());
    clear_saved(ctx);

    close_list(ctx);
    refresh_row(ctx, entries, row);
}

// Commits a clicked role to the open row; `Any` is an entry, so clearing is
// picking it.
fn pick_role(ctx: &mut StableClient<'_>, entries: &[ListEntry], index: usize) {
    let Some(Some(OpenList::Role { row })) = with_state(|state| state.open_list) else {
        return;
    };
    let Some(role) = Role::ALL.get(index).copied() else {
        return;
    };

    edit_row(row, |entry| entry.role = role);
    clear_saved(ctx);

    close_list(ctx);
    refresh_row(ctx, entries, row);
}

// Commits a clicked champion to the open row; clicking the assigned one clears
// it, taking the row out of the file but not the editor.
fn pick_champion(ctx: &mut StableClient<'_>, entries: &[ListEntry], index: usize) {
    let Some(Some(OpenList::Champion { row })) = with_state(|state| state.open_list) else {
        return;
    };
    let champions = snapshot_champions();
    let Some(choice) = champions.get(index) else {
        return;
    };

    let current = snapshot_rows()
        .get(row)
        .and_then(|entry| entry.champion.clone());
    let already = current.as_deref() == Some(choice.id.as_str());
    let picked = (!already).then(|| choice.id.clone());
    edit_row(row, |entry| entry.champion = picked.clone());
    clear_saved(ctx);

    close_list(ctx);
    refresh_row(ctx, entries, row);
    // The row's champion is what the filter tests, so assigning one can take the
    // row out of (or a clear can bring it back into) the filtered list.
    if filter_active() {
        rebuild_rows(ctx, entries);
    }
}

pub struct StrategyPicker;

impl StableExtension for StrategyPicker {
    fn on_init(&self, _ctx: &mut StableClient<'_>) {
        // Proves two things at once when it lands in the log: the client
        // extension is registered and being called, and the log file is
        // writable at the path the DLL resolves.
    }

    fn post_update(&self, ctx: &mut StableClient<'_>, _dt_micros: u64) {
        use crate::perf::{self, Section};
        // This is the one per-frame hook, so it is also where timing reports
        // are written from. Both no-ops unless `perf::ENABLED`.
        perf::report_if_due();
        let _frame = perf::Probe::start(Section::Frame);

        // The native tactics half, first and unconditional: this is the mod's
        // only per-frame client hook, and its hooks install and self-heal on
        // every frame, everywhere.
        perf::time(Section::FrameTactics, || tactics::driver::post_update(ctx));

        // Unconditional too, since its screen is not this one; inert unless the
        // slot count is three.
        perf::time(Section::FrameSoloRank, || crate::solo_rank_ui::sync(ctx));

        // Which save's item totals are loaded, settled before anything reads
        // them. Unconditional, since the statistics screen and the management
        // tick are both past the early return.
        perf::time(Section::FrameItemStats, || crate::item_stats::sync(ctx));

        // The editor's two toggles, which are the loaded save's. After the
        // line above, whose word it takes that the save has been read.
        build_config::sync_settings(ctx);

        // The save's item balance patches: state, due checks, and patched
        // numbers in any item tooltip up, after the save is read. One atomic
        // read a frame while nothing is patched.
        crate::patches::sync(ctx);

        // Item Rules: the save's setting, then its Options row, ahead of the
        // patcher row that goes under it.
        crate::item_tiers::sync(ctx);
        crate::options_row::sync(ctx);

        // The Item Patcher Settings row and window; one lookup a frame while
        // the dialog is closed.
        crate::ui::patch_settings::sync(ctx);

        // Champion facts for Smart Builds, which only the client can ask for;
        // returns at once with nothing new to ask.
        perf::time(Section::FrameChampionTraits, || {
            crate::champion_traits::learn(ctx)
        });

        // And the game's own items' stats, for the same rules. Unconditional
        // for the same reason; one atomic read once it has them.
        crate::item_stats::prime_item_traits(ctx);

        // Same again, for the statistics screen and its Item Stats tab. Inert
        // anywhere else: it returns on its first line unless that screen is up.
        perf::time(Section::FrameItemStatsUi, || {
            crate::item_stats::ui::sync(ctx)
        });

        // Drafter's Toolbox's Advanced Stats tab, when that mod is enabled too.
        // After the line above, not merely near it: it hides the Item Stats
        // dropdowns that line has just shown while Advanced is open over them.
        perf::time(Section::FrameToolboxTab, || {
            crate::item_stats::toolbox_tab::sync(ctx)
        });

        // The draft screen's picks, for the Blue Team / Red Team cells: read on
        // their own screen, one failed lookup elsewhere.
        perf::time(Section::FrameDraftWatch, || draft_watch::sync(ctx));

        // The player's builds in the in-match Check Tactics panel. Unconditional
        // because that screen is not this one; outside a match it is one failed
        // lookup.
        crate::match_builds::sync(ctx);

        // What the item under the cursor has cost its athlete, in the game's
        // own item tooltip of a match. Unconditional for the same reason,
        // and as cheap outside one.
        crate::item_spend::sync(ctx);

        // What an item has built up over the match, on its icon in the
        // player rows. Unconditional for the same reason; outside a match it
        // looks nothing up.
        crate::item_counts::sync(ctx);

        // Everything from here to whichever return this frame takes.
        let _editor = perf::Probe::start(Section::FrameEditor);

        // The composition test hosts the editor too, handled before the gate
        // below, which tears the editor down. Gated on visibility, since the
        // panel exists while the Training screen is loaded.
        if let Some(tactics) = resolve_comp_tactics(ctx) {
            if ctx.ui_visible(&tactics).unwrap_or(false) {
                comp_test_host(ctx, &tactics);
                // The editor's own upkeep, minus the strategy-screen half
                // (`ITEM_INFO_BTN`, `keep_matchup`): those paths do not exist
                // here.
                if with_state(|state| state.showing).unwrap_or(false) {
                    sync_filter(ctx);
                    sync_info_popup(ctx);
                }
                return;
            }
        }

        if !ctx.ui_exists(BUILDS_TAB) {
            // Not on the (patched) strategy screen: forget the spawned panel so
            // the next match reinstalls it into the fresh screen.
            let (stale, drafted) = with_state(|state| {
                let stale = state.wired;
                let drafted = state.side_ids.clone();
                state.wired = false;
                state.modal_ready = false;
                state.spawned_rows.clear();
                state.filter.clear();
                state.side_blue = false;
                state.side_red = false;
                state.side_settle = 0;
                state.search_fit = None;
                state.open_list = None;
                // The popup found on this screen dies with it, so the next one
                // searches again rather than writing to a stale path.
                state.info_popup = None;
                state.info_probe_tick = 0;
                state.info_showing = false;
                state.showing = false;
                (stale, drafted)
            })
            .unwrap_or_default();
            if stale {
                // The screen and everything registered on it is gone, so the
                // next one has to wire itself from scratch.
                REGISTERED.forget();
                // The lineup goes to its match, where the Check Tactics panel
                // uses it; the next strategy screen gets its own draft or no
                // cells.
                crate::match_builds::note_lineup(drafted);
                draft_watch::forget();
            }

            return;
        }

        // Repointed here every frame, since the composition test points the
        // paths at itself.
        set_host(UI_ROOT);

        if !with_state(|state| state.wired).unwrap_or(true) {
            // Loaded here, in `post_update`, not from a click handler:
            // `setting_get_json` returned None when called inside one, matching
            // the trait's note that only ui/asset calls are live there.
            cached_entries(ctx);
            cached_champions(ctx);
            // Team is registered too, observation only, so switching back puts
            // the editor away.
            for path in [BUILDS_TAB, TEAM_TAB] {
                REGISTERED.register(ctx, path);
            }
            let _ = with_state(|state| state.wired = true);
        }

        // Before the `showing` gate below: the lineup has to be worked out by
        // the time the tab is first opened, and once it is this is a single
        // comparison a frame.
        sync_sides(ctx);

        // Re-asserted every frame: game code drives these from its own tab,
        // which is never Builds, and `post_update` runs after it.
        if with_state(|state| state.showing).unwrap_or(false) {
            ctx.ui_set_visible(ITEM_INFO_BTN, true);
            keep_matchup(ctx);
            sync_filter(ctx);
            sync_info_popup(ctx);
        }
    }

    // Nothing is owed a write at process end: the totals live in the save,
    // which `item_stats::sync` keeps current every frame.
    fn on_end(&self) {}
}
