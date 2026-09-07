//! In-game Item Build Editor: the **Builds** tab on the strategy screen.
//!
//! One row per champion, three item slots per row, a category-grouped item list,
//! and swap/clear buttons per row. It reads and writes `item-builds.json` next
//! to the DLL, which is the same file the hook applies — so a build takes effect
//! on the next match with no restart.
//!
//! # A tab rather than a window
//!
//! The override replaces the game's **Personal** tab with `#builds`, leaving
//! Team beside it. The editor panel sits in the content band those tabs switch
//! between, at 1364x645 (canvas 47,182) — the band's full 1824px minus the
//! right-hand column, which is left uncovered so the vanilla Matchup card in
//! `#sub4` stays on screen beside the editor.
//!
//! Removing Personal is deliberate: this editor supersedes it. That tab's five
//! per-player category dropdowns are the same setting expressed per athlete
//! instead of per champion, and the hook cannot act on a per-athlete rule
//! anyway (see below) — so leaving both would have offered a control that looks
//! like it works and does not.
//!
//! Its `#personal` panel node is still in the layout and is hidden on entry,
//! since game code may pick that tab when the screen is built without knowing
//! its tab is gone. Nothing here shows it any more.
//!
//! Nothing arbitrates between the two tabs. They are independent
//! `color_selectable`s and game code has never heard of ours, so [`open_editor`]
//! and [`close_editor`] do that bookkeeping. Leaving deliberately restores no
//! panel: the click that leaves is a click on Team, and game code's handler for
//! it shows the Team columns already.
//!
//! The vanilla panels are hidden rather than covered while the tab is up. The
//! Personal rows' champion portraits have a tooltip drawn by game code *outside*
//! this subtree, so an opaque panel of ours cannot be drawn over it — only
//! removed.
//!
//! # Why champions and not positions
//!
//! An earlier design gave each of the five *positions* a row. That cannot work:
//! the hooked `get_item_builds_list` computes **one team per call** and is called
//! once for each side, with `team1` being whichever team it is building for.
//! Nothing in its arguments identifies the player's team, and the stable API has
//! no athlete-to-champion mapping the client could use to work it out — so a rule
//! keyed by route index fired for the enemy too, and a build pinned to "Top"
//! reached both top laners. Keying by champion is the only thing the hook can
//! actually discriminate on.
//!
//! Layout lives in two places by necessity:
//!
//! - `ui/layout/strategy.ui` is an asset override (see `mod.override_info`). It
//!   swaps `#personal` for `#builds` inside `#mode_toggle`, which keeps the
//!   toggle at its vanilla 244px (4px padding either side plus two 118px tabs)
//!   and leaves `#item_info_btn` at its vanilla x:480. Nothing else on that
//!   screen is touched.
//! - `ui/layout/build_editor.ui` is the panel chrome, compiled in with
//!   `include_str!` and spawned under [`UI_ROOT`]. The rows and both dropdown
//!   lists are spawned from source here, because their contents depend on the
//!   saved builds and on the loaded item pool.
//!
//! # Why the window is spawned from source rather than as a template asset
//!
//! `ui_spawn_template` resolves a *registered* asset. `strategy.ui` is
//! registered because `mod.override_info` remaps it over a base asset, but
//! `override_info` can only remap base to mod — it cannot declare a standalone
//! asset — so there is no supported way to make `build_editor` resolvable by
//! path. `ui_spawn_source` takes the same `.ui` grammar as literal text, so the
//! layout is compiled into the DLL and the asset system is left out of it.
//!
//! # Why there is no real dropdown
//!
//! A dropdown's option list is populated by game code: no `.ui` property
//! declares one (the runner parses only styling and layout keys), and the stable
//! ABI's `state_set_json` accepts `checkbox`, `text_edit`, `slider` and
//! `selectable` but not `dropdown`. So each combo is a `color_icon_button` that
//! opens a floating panel of `selectable` rows, moved under whichever control was
//! clicked — which is what a dropdown looks like anyway.
//!
//! # Do not put `z` on anything in here
//!
//! `z` is per-node and is *not* inherited by children, which makes it close to
//! useless for a floating panel and actively harmful on a node that draws.
//! Three symptoms, all the same cause:
//!
//! - `#itemlist:color { z: 400 }` rendered an empty panel. The fill was at 400,
//!   its children at 0, so the panel painted over its own contents.
//! - Moving the fill into a child of a `z: 400` `empty` fixed that but did not
//!   stop the rows showing through, because the children still sat at 0 while
//!   the row glyphs were at 220-250.
//! - `selectable` has no `z` in its property table, so the list rows cannot be
//!   lifted to match. There is no z value that puts a list above the rows.
//!
//! What does work is tree order at equal `z`: a child draws over its parent, and
//! a later sibling over an earlier one. Everything here is therefore at the
//! default `z`, and the floating lists win by being the last children of the
//! window root. The only nodes still carrying a `z` are `empty` containers,
//! which draw nothing, so their `z` cannot cover anything.
//!
//! `ignore_event: true` on the child images is unrelated and still required: it
//! is about hit-testing, not drawing, and without it a child swallows the click
//! meant for the button it sits on.
//!
//! # The filter narrows what is spawned, not what is visible
//!
//! The toolbar's search box hides rows whose champion does not match. It does
//! that by spawning only the matching rows, and it keeps naming each node for
//! the row's index in the saved list rather than its position on screen — so
//! `…rows.row4` is row 4 of the file whether or not rows 0-3 are showing, and
//! every path, handler and [`row_from_path`] keeps its meaning with a filter up.
//! Hiding the non-matching nodes instead would have depended on `TopToBottom`
//! skipping invisible children when it measures, and left 50px holes in the list
//! if it does not.
//!
//! # Register each path exactly once
//!
//! `ui_register_path_events` registers "a handler for EVERY UI event whose path
//! equals `path`", and the handler lives until process exit. It is keyed by the
//! path *string* and is not bound to a node, which means registering the same
//! path twice makes one click run the handler twice, and a handler outlives the
//! node it was registered for.
//!
//! That is a correctness problem, not just a leak. Row nodes are respawned on
//! every add and delete, and the whole screen is rebuilt every match, so the
//! obvious "register when you spawn" ends up stacking handlers: deleting row 2
//! removed row 2, then the new row 2 (which had been row 3), and so on to the
//! end of the list.
//!
//! Everything therefore registers through [`register_once`], which keeps a set
//! of the paths already registered.
//!
//! That set must *not* outlive the screen, which is what this originally got
//! wrong. The assumption was that a handler surviving its node meant the ones
//! registered on the first visit kept working on every later one — so the set
//! was kept for the whole process. In practice the editor worked on the first
//! strategy screen of a session and was completely inert on every screen after
//! it: nothing re-registered, because the set said everything already had a
//! handler. [`forget_registrations`] therefore clears it when the screen goes
//! away, and only then — while a screen is up, registering a live path twice is
//! still the double-fire hazard described above.

use std::sync::{Arc, Mutex, OnceLock};

use mod_api_stable::*;

use crate::build_config::{self, picker_slots, ChampionRow, Role};
use crate::item_catalog;
use crate::tactics;

/// Shown for a slot left to the game's own AI, and on the list row that puts a
/// slot back into that state. The vanilla strategy screen's own wording for it
/// (`strategy.i18n`'s `build_auto`), so the editor and the screen behind it call
/// the same thing by the same name.
/// The editor's own text, as `#asset/…?key` **references** the game resolves,
/// not as strings this mod resolves itself.
///
/// That distinction is the whole feature. `ctx.i18n()` returns the `en` value
/// regardless of the game's language, so resolving here and writing the result
/// produced an English editor inside a Korean game. Handing the *reference* to
/// the label instead lets the engine's LabelRunner substitute it at draw time,
/// against the active locale — the same mechanism `tactics::VANILLA_OPTS` uses
/// for the personal-tactics dropdown, which is verified working in game.
///
/// The catch, also recorded there: LabelRunner substitutes **whole-string**
/// labels only, with no inline composition. So every key here is a complete
/// label — `col_item1`..`col_item4` rather than one `"ITEM {n}"` template — and
/// anything this mod concatenates (padded rows, truncated item names) cannot use
/// this path and stays in `en`.
///
/// `ctx.i18n()` is still called once per key, but only to *validate* that the
/// key resolves at all; a key that does not gets its English literal instead, so
/// a raw `#asset/…` string can never reach the screen. `text/ui.i18n` is
/// hand-authored and merged into `asset/base/text/ui` by `mod.override_info` —
/// LabelRunner reads the base document, so the merge is required.
#[derive(Clone)]
struct Strings {
    tab: String,
    add: String,
    filter: String,
    hint: String,
    col_champion: String,
    col_role: String,
    /// One complete label per column — LabelRunner cannot compose `"ITEM " + n`.
    col_items: [String; 4],
    /// The two cells of each footer toggle. Both are on screen at once, so these
    /// name the choice a cell makes rather than describing the current state.
    unique_on: String,
    unique_off: String,
    scope_all: String,
    scope_own: String,
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
            col_items: ["ITEM 1".into(), "ITEM 2".into(), "ITEM 3".into(), "ITEM 4".into()],
            unique_on: "Unique Items Enforced".into(),
            unique_off: "Duplicates Allowed".into(),
            scope_all: "Apply To All Players".into(),
            scope_own: "Apply To Your Players Only".into(),
            save: "Save Item Builds".into(),
            ai_slot: AI_SLOT_LABEL_FALLBACK.into(),
            no_champion: NO_CHAMPION_LABEL_FALLBACK.into(),
        }
    }
}

/// Where the editor's labels live. `text/ui.i18n` is merged into this base
/// document by `mod.override_info`; LabelRunner resolves references against the
/// base documents, so the merge is what makes the references work.
const UI_TEXT_DOC: &str = "#asset/base/text/ui";

static STRINGS: Mutex<Option<Arc<Strings>>> = Mutex::new(None);

/// The resolved text, or the English defaults until [`load_strings`] has run.
fn strings() -> Arc<Strings> {
    STRINGS
        .lock()
        .ok()
        .and_then(|strings| strings.clone())
        .unwrap_or_else(|| Arc::new(Strings::default()))
}

/// Resolves the editor's text for the game's current language.
fn load_strings(ctx: &StableClient<'_>) {
    let fallback = Strings::default();
    // A reference the game will resolve — but only once it is known to resolve,
    // because an unknown key would otherwise be drawn as literal `#asset/...`.
    // `ctx.i18n` answers in `en` whatever the locale is, which is useless as a
    // translation and perfect as an existence check.
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

/// Writes the resolved text onto the nodes that carry it literally in the
/// layout. The `.ui` keeps the English so the editor reads correctly if this
/// never runs.
///
/// `ui_set_properties` rather than `ui_set_text` for the buttons: their label is
/// a nested `text` block, which `ui_set_text` does not reach into.
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
    // The tab and the footer toggle cells carry `text` as a direct property, not
    // a nested block — see `#builds` in `strategy.ui` — so they take the plain
    // form. The cells are labelled once here rather than on every repaint: in a
    // segmented control the label of a cell is what it *is*, and only which one
    // is lit changes with the setting.
    for (path, text) in [
        (BUILDS_TAB, &strings.tab),
        (unique_on_path(), &strings.unique_on),
        (unique_off_path(), &strings.unique_off),
        (scope_all_path(), &strings.scope_all),
        (scope_own_path(), &strings.scope_own),
    ] {
        ctx.ui_set_properties(path, &format!("text: \"{}\";", escape(text)));
    }
}

const AI_SLOT_LABEL_FALLBACK: &str = "Let Player Decide";

/// Shown for a row whose champion has not been chosen yet. Such a row is kept in
/// the editor but never written.
const NO_CHAMPION_LABEL_FALLBACK: &str = "(champion)";

/// The strategy screen's container, and the editor's host there.
///
/// **Not a universal root.** `main` is the name of `strategy.ui`'s root node
/// (`main:strategy_ui`) and `contents` is its first child, so this whole path is
/// authored inside that one layout and exists only while that screen is loaded —
/// `main.ui` declares no `contents` at all. Reasoning from it to
/// `main.contents.<other screen>` is how both the Item Stats tab and the
/// composition test host were first written wrong; see [`resolve_comp_tactics`].
///
/// The bare `contents.…` prefix that appears in the executable's string table is
/// what game code *builds* paths with, one level below the root the query API
/// expects — confirmed by the path probe, which reports `contents.*` absent and
/// `main.contents.*` present.
const UI_ROOT: &str = "main.contents";

/// The Builds tab, added by the `strategy.ui` override beside Team. It exists
/// only in the patched screen, so its presence doubles as the probe for "our
/// layout is loaded" — rename it here and in `strategy.ui` together.
const BUILDS_TAB: &str = "main.contents.strategy.mode_toggle.builds";

/// The game's own remaining tab. The mod does not drive it; it watches it, so
/// that clicking it puts the editor away. That click also makes game code
/// restore the Team panels, which is why leaving the Builds tab never has to
/// un-hide anything itself.
const TEAM_TAB: &str = "main.contents.strategy.mode_toggle.team";

/// The vanilla Personal panel. Its tab is gone and nothing here shows it, but
/// it is still hidden on entry: game code decides what is visible when the
/// screen is first built and may well pick Personal, having no idea its tab no
/// longer exists.
const PERSONAL_PATH: &str = "main.contents.strategy.personal";

/// The screen's own "Item Info" button. Game code hides it on the Team tab and
/// keeps re-asserting that from its own tab state, which never becomes Builds,
/// so `post_update` re-shows it every frame while the tab is up rather than
/// once on entry. Leaving does not hide it again: the tab click that leaves is
/// handled by game code, which sets it to whatever that tab wants.
const ITEM_INFO_BTN: &str = "main.contents.strategy.item_info_btn";

/// The content panels hidden while the Builds tab is up.
///
/// Covering them is not enough. The Personal rows carry champion portraits whose
/// tooltip is drawn by game code *outside* this subtree — `strategy.ui` has no
/// tooltip node at all — so an opaque panel of ours cannot be drawn over it and
/// nothing stops the hover that summons it. With the panel hidden there is
/// nothing left to hover. `#sub1`-`#sub3` are the Team tab's first three
/// columns; they occupy the same band and would otherwise show through around
/// our edges. `#sub4` is deliberately absent — see [`MATCHUP_PATH`].
const CONTENT_PANELS: [&str; 4] = [
    PERSONAL_PATH,
    "main.contents.strategy.sub1",
    "main.contents.strategy.sub2",
    "main.contents.strategy.sub3",
];

/// The composition test's "Tactics & Item Build Setup" panel, relative to the
/// training screen's root — the editor's second host, and one that shares none
/// of the strategy screen's paths.
const COMP_TACTICS_REL: &str = "comp_test_popup.tactics";

/// The vanilla block the editor replaces there, relative to the same root.
///
/// Despite the "Personal Tactics" heading above it (`tactics_builds`, while the
/// *team* tactics above are `tactics_strategy` — the two read backwards), this
/// is a build editor: ten rows of `blue0..blue4`/`red0..red4`, each three item
/// dropdowns. It sets a build for that one test, where this module's editor
/// writes the persistent per-champion builds in `item-builds.json`. Leaving both
/// up would be two controls for the same slot disagreeing, so it is hidden.
///
/// Doubles as the marker [`find_comp_tactics`] walks for: no other screen has a
/// `comp_test_popup.tactics.builds` under it.
const COMP_BUILDS_REL: &str = "comp_test_popup.tactics.builds";

/// The root `training.ui` says the composition test hangs off.
///
/// Its first line is `training:training_ui`, and `#comp_test_popup` is a direct
/// child of that root, so this is what the documented rule gives — see
/// [`resolve_comp_tactics`] for why it is a candidate rather than the answer.
const COMP_ROOT: &str = "training";

/// Frames between failed sweeps for the composition test panel.
///
/// Only failed ones: a hit is cached in [`EditorState::comp_tactics`] and
/// re-checked with a single `ui_exists`. The throttle exists because an
/// unthrottled breadth-first `ui_child_names` sweep is a documented way to make
/// this mod lag, and unlike the statistics tab there is no cheap "am I on the
/// right screen?" test to gate it with.
const COMP_PROBE_EVERY: u32 = 30;

/// Depth and node budget for that sweep, mirroring `item_stats_ui::find_screen`.
/// A depth limit alone does not bound the walk — one wide level can be hundreds
/// of nodes, and this runs on the UI thread.
const COMP_PROBE_DEPTH: u32 = 4;
const COMP_PROBE_NODES: u32 = 600;

/// The editor popup's authored origin, restored when it goes back to the
/// strategy screen after the composition test has moved it.
const EDITOR_RECT_STRATEGY: &str = "x: 47px; y: 182px;";

/// Air left between the editor and the dialog furniture above and below it,
/// matching the 10px the vanilla block leaves under its own header.
const COMP_SLOT_GAP: f32 = 10.0;

/// The Team tab's fourth column, left showing while the Builds tab is up so its
/// Matchup card stays on screen — the editor panel is narrowed to 1364px to
/// leave exactly this column uncovered.
///
/// The card cannot be rebuilt on our side: its portraits and names are written
/// by game code, and nothing in the stable API maps an athlete to a champion.
/// Reusing the game's own node is the only way to have one that is populated.
const SUB4_PATH: &str = "main.contents.strategy.sub4";

/// The Matchup card, and the "Closing Out" block above it. `#sub4` lays its
/// children out `TopToBottom`, so hiding `#game_finish` floats Matchup to the
/// top of the column — which is how the vanilla Personal tab showed it too.
///
/// Both are re-asserted every frame: game code drives them from its own idea of
/// the current tab, which never becomes Builds.
const MATCHUP_PATH: &str = "main.contents.strategy.sub4.matchup";
const GAME_FINISH_PATH: &str = "main.contents.strategy.sub4.game_finish";

// Tab highlight colours, lifted from `strategy_option` in
// `asset/base/style/main.style` so the painted state matches the vanilla one
// exactly. See [`paint_tabs`] for why they are painted rather than selected.
const TAB_SELECTED_FILL: &str = "#ecfbf8ff";
const TAB_SELECTED_TEXT: &str = "#0f5b4dff";
const TAB_IDLE_FILL: &str = "#00000000";
const TAB_IDLE_TEXT: &str = "#a3a9b6ff";
/// Hover on a dim tab: `image.hover.color` and `label.hover.color` in the style.
/// Equal to [`TAB_IDLE_TEXT`] by coincidence, not by meaning.
const TAB_HOVER_LINE: &str = "#a3a9b6ff";
const TAB_HOVER_TEXT: &str = "#e0e2e7ff";

/// The editor's layout source, and the node name its root declares.
///
/// There is no `EDITOR_PARENT` constant any more: the editor has two hosts and
/// they live in different scenes. `#contents` is authored inside `strategy.ui`,
/// so `main.contents` does not exist at all while the composition test is up —
/// see [`EditorPaths`].
const EDITOR_SOURCE: &str = include_str!("../ui/layout/build_editor.ui");
const EDITOR_NODE: &str = "build_editor";

/// Sheet the item icons come from. The mod overrides this asset with its own
/// 640x640 sheet (see `mod.override_info`), so frame names are the mod's.
pub(crate) const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";

// Row geometry, inside the 1314px band `#rows` gives its children. The x offsets
// match `build_editor.ui`'s column headers. The band is the panel minus the
// right-hand column the vanilla Matchup card is left sitting in.
/// The panel size `build_editor.ui` authors, and what the strategy screen uses.
///
/// The composition test cannot use it: its dialog is 1280 wide, so a 1364 panel
/// hangs 42px off each side. Everything below is therefore expressed as an inset
/// from the panel's edges rather than an absolute x, and [`set_panel_size`]
/// re-lays the chrome whenever the host changes.
const PANEL_W: u32 = 1364;
const PANEL_H: u32 = 645;

/// Insets from the panel's right edge, taken from the authored 1364 layout:
/// rows 1306, the delete button at 1250, the item band ending at 1234, and the
/// two scrolling strips 1334. Keeping them as insets is what makes the panel
/// resizable without re-authoring the layout.
const ROW_INSET: u32 = PANEL_W - 1306;
const DELETE_INSET: u32 = PANEL_W - 1250;
const COLUMNS_INSET: u32 = PANEL_W - 1234;
const STRIP_INSET: u32 = PANEL_W - 1334;
const ROWS_INSET: u32 = PANEL_W - 1314;

/// Panel height not available to the row list: the toolbar and column header
/// above it (92), the bottom-anchored footer (60), and the 7px of air the
/// authored 645/486 pair leaves between them.
const CHROME_H: u32 = 92 + 60 + 7;

/// The size the editor is currently laid out at. Not a constant because the two
/// hosts are different sizes; see [`PANEL_W`].
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

/// Records the size the editor should be laid out at, reporting whether it
/// moved.
///
/// Separate from [`apply_panel_size`] because the two have to happen either side
/// of a spawn: rows carry their width in the `.ui` source they are built from,
/// so the size has to be settled *before* `ensure_editor` runs, while the chrome
/// can only be written *after* it. A caller that gets `true` back has rows built
/// for the old width and must rebuild them.
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

/// Writes the current size onto the panel's five sized nodes.
///
/// Only five carry a size at all: the toolbar and footer span the panel, the
/// column header and row list are inset 15px each side, and everything else is
/// anchored — the footer to the bottom, Save and the toolbar hint to the right —
/// so they follow on their own.
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

/// Row height and gap are the vanilla Personal panel's own (50px rows, 10px
/// spacing), so the tab reads as one of the game's own rather than a graft.
const ROW_HEIGHT: u32 = 50;
const ROW_SPACING: i32 = 10;
/// Combos are 40px tall like the vanilla dropdowns, centred in the row.
const COMBO_Y: u32 = 5;
const COMBO_H: u32 = 40;
/// Square buttons (clear, swap glyph target, delete), centred in the row.
const MINI_Y: u32 = 14;
const CHAMP_X: u32 = 8;
const CHAMP_W: u32 = 224;
/// The Role column, between the champion button and the item band. Narrow: it
/// holds one of six short words, and every px here comes off the item columns.
const ROLE_X: u32 = 240;
const ROLE_W: u32 = 100;

/// The band the item columns share, between the champion button and the delete
/// button, and the gap left between two columns for a swap button (34px wide,
/// plus a few px of air on each side).
const COLUMNS_LEFT: u32 = 348;
const COLUMN_GAP: u32 = 44;

/// Width of one item column: the shared band split evenly, gaps removed. Three
/// slots give 266px, four give 188px. The champion and Role columns were
/// trimmed to hand the difference here, since item names are what truncate.
fn combo_w() -> u32 {
    let slots = picker_slots() as u32;
    (columns_right() - COLUMNS_LEFT - (slots - 1) * COLUMN_GAP) / slots
}

/// Left edge of an item column. The band starts after the Role column, so the
/// authored 306/630/954 no longer apply — `sync_column_headers` re-places the
/// headers from here, which is what keeps the two in step.
fn combo_x(slot: usize) -> u32 {
    COLUMNS_LEFT + slot as u32 * (combo_w() + COLUMN_GAP)
}

/// Left edge of the swap button between `slot` and `slot + 1`: tucked into the
/// gap right after its left-hand column.
fn swap_x(slot: usize) -> u32 {
    combo_x(slot) + combo_w() + 4
}

/// Sizes of the two floating lists, mirroring `build_editor.ui`. Kept here
/// because the open code has to decide whether a list fits below the control
/// that opened it.
const LIST_W: i32 = 320;
const LIST_H: i32 = 430;
const CHAMP_LIST_W: i32 = 260;
const ROLE_LIST_W: i32 = 160;

/// Canvas the layouts are authored against; the clamps that keep a list on
/// screen measure against this.
const CANVAS_W: i32 = 1920;
const CANVAS_H: i32 = 1080;

/// Heights of the two kinds of item-list node, shared by [`entry_source`] (which
/// lays them out) and [`entry_height`] (which adds them up).
/// Fill behind a category header in the item list, one step up from the list
/// panel's own `#0f1016ff` so the groups read as bands rather than loose text.
const LIST_HEADER_FILL: &str = "#1d1f2cff";
const ENTRY_HEADER_H: i32 = 26;
const ENTRY_ITEM_H: i32 = 30;

/// One pickable item: the key the hook resolves, its display name, and the
/// grouping the item list sorts and headers it by.
#[derive(Clone)]
struct ItemChoice {
    key: String,
    name: String,
    /// Sprite-sheet frame, or `None` for an item with no art in the sheet.
    frame: Option<String>,
    category: &'static str,
}

/// One node in the item list. Every variant occupies an index, so a click path
/// resolves straight into this list without a second mapping.
#[derive(Clone)]
enum ListEntry {
    /// Always first: puts the slot back to the AI's choice. Clicking the pinned
    /// item again does the same thing, but only if a slot is already pinned and
    /// only if you know to try it — this is the discoverable way.
    Clear,
    /// A category heading. Inert: no handler is registered for it.
    Header(&'static str),
    Item(ItemChoice),
}

/// One pickable champion.
#[derive(Clone)]
struct ChampionChoice {
    id: String,
    name: String,
}

/// Which floating list is open, and what it is editing.
#[derive(Clone, Copy, PartialEq)]
enum OpenList {
    Item { row: usize, slot: usize },
    Champion { row: usize },
    Role { row: usize },
}

#[derive(Default)]
struct EditorState {
    /// Whether the three tabs have been wired for the current screen.
    wired: bool,
    /// Whether the editor subtree is spawned and its controls registered.
    modal_ready: bool,
    /// Whether the Builds tab is the one currently showing.
    showing: bool,
    /// The floating list currently showing, if any.
    open_list: Option<OpenList>,
    /// The rows being edited, in display order. Loaded from `item-builds.json`
    /// when the window is built and written back on every change.
    rows: Vec<ChampionRow>,
    /// Which rows currently have a node spawned, so a rebuild removes exactly
    /// those and no others. Indices into `rows`, not positions on screen: with a
    /// filter active the two differ, and every path in the editor is built from
    /// the former.
    spawned_rows: Vec<usize>,
    /// The filter box's text, as of the last frame that read it. Rows whose
    /// champion does not match are left unspawned.
    filter: String,
    /// The item list, headers included. Cached for the process lifetime — the
    /// item pool cannot change without a restart.
    entries: Vec<ListEntry>,
    /// The champion list. Cached for the same reason.
    champions: Vec<ChampionChoice>,
    /// Where the vanilla Item Info popup was found on this screen, and how many
    /// frames have passed since it was last looked for. Both are per screen: the
    /// path found dies with the screen, and the search is a walk of the node
    /// tree, so it is throttled rather than run every frame. See
    /// [`find_info_popup`].
    info_popup: Option<String>,
    info_probe_tick: u32,
    /// Whether that popup was up as of the last frame, so the scroll views are
    /// only rewritten when it opens or closes.
    info_showing: bool,
    /// Where the composition test's tactics panel was found, and how many frames
    /// have passed since it was last looked for. Cleared as soon as the cached
    /// path stops resolving, so leaving the screen re-resolves on the next visit
    /// rather than writing to a node that no longer exists. See
    /// [`resolve_comp_tactics`].
    comp_tactics: Option<String>,
    comp_probe_tick: u32,
}

static STATE: Mutex<Option<EditorState>> = Mutex::new(None);

/// Runs `f` against the editor state, initializing it on first use. Returns
/// `None` if the lock is poisoned, which disables the editor rather than
/// panicking across the FFI boundary.
fn with_state<T>(f: impl FnOnce(&mut EditorState) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(EditorState::default)))
}

/// Final items the mod registers itself, recorded as they are added in `init`.
///
/// They are not in the game's item settings document — that lists only the six
/// vanilla finals — so a client that reads settings alone offers none of the
/// mod's items. `StableMod` does not expose its registered items, so the keys
/// are captured at the one place that already knows them. Membership also
/// decides whether an item has art in the icon sheet.
static MOD_FINALS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Engine category per entry of [`MOD_FINALS`], as an `ItemCategoryV1` code.
///
/// Kept beside the keys rather than in `item_catalog`, whose classes are the
/// mod's own finer grouping for the picker (`Marksman`, `Mage`, …) and not what
/// the engine sorts items by. `tactics` needs the engine's answer, because that
/// is the category the item-build hook de-duplicates within — matching it is
/// what stops a de-duplicated 4th item changing what kind of item it is.
static MOD_FINAL_CATEGORIES: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// Records one of the mod's final (radiant) items and its engine category.
/// Called from the registration macros in `lib.rs`.
pub(crate) fn note_final_item(key: &str, category: ItemCategoryV1) {
    if let Ok(mut finals) = MOD_FINALS.lock() {
        finals.push(key.to_string());
    }
    if let Ok(mut categories) = MOD_FINAL_CATEGORIES.lock() {
        categories.push((key.to_string(), category.code()));
    }
}

/// The engine category of one of the mod's final items, or `None` for a key the
/// mod did not register.
pub(crate) fn mod_item_category(key: &str) -> Option<u32> {
    MOD_FINAL_CATEGORIES
        .lock()
        .ok()?
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, category)| *category)
}

/// Paths that already have a handler, so none is ever registered twice.
///
/// `ui_register_path_events` registers "a handler for EVERY UI event whose path
/// equals `path`" and the handler lives until process exit — it is keyed by the
/// path *string*, not bound to a node. Two consequences, and the second one bit:
///
/// - Registering the same path twice means one click runs the handler twice.
///   That is invisible for something idempotent like opening a list, and
///   destructive for `+ Add Champion` and row delete: deleting row 2 removed
///   row 2, then the new row 2 (which had been row 3), and so on down the list.
/// - A handler outlives the node it was registered for and fires again for
///   whatever is at that path later. Re-registering when the strategy screen is
///   rebuilt each match is therefore not just wasteful, it is the same bug.
///
/// Registering once per path is what both facts ask for, and it also bounds
/// what used to be an ever-growing pile of leaked closures.
static REGISTERED: Mutex<std::collections::BTreeSet<String>> =
    Mutex::new(std::collections::BTreeSet::new());

/// Registers [`handle_event`] for `path`, unless it already has one.
fn register_once(ctx: &mut StableClient<'_>, path: &str) {
    let fresh = REGISTERED
        .lock()
        .map(|mut set| set.insert(path.to_string()))
        .unwrap_or(false);
    if !fresh {
        return;
    }
    if !ctx.ui_register_path_events(path, handle_event) {
        // Forget it again so a later attempt can retry rather than silently
        // leaving a dead control behind.
        if let Ok(mut set) = REGISTERED.lock() {
            set.remove(path);
        }
    }
}

/// Forgets every registration, so the next screen registers from scratch.
///
/// Called once when the strategy screen goes away. Every path this module
/// registers lives under `main.contents` — the tabs inside the screen itself,
/// and the spawned editor beside them — so all of them die with it, and a
/// process-lifetime set of "already registered" would keep the second visit's
/// controls from ever being wired. That is the shape of the bug this fixes: the
/// editor worked on the first strategy screen of a session and was inert on
/// every one after, because `register_once` had nothing left to do and the
/// handlers it was trusting had gone with the nodes.
///
/// This is the one thing the process-wide set must not outlive. Registering the
/// same *live* path twice is still the hazard the set exists to prevent (one
/// click running a handler twice, which is how deleting row 2 once deleted rows
/// 2 and 3), so this clears only on teardown, never while a screen is up.
fn forget_registrations() {
    if let Ok(mut set) = REGISTERED.lock() {
        set.clear();
    }
}

fn is_mod_item(key: &str) -> bool {
    MOD_FINALS
        .lock()
        .map(|finals| finals.iter().any(|final_key| final_key == key))
        .unwrap_or(false)
}

/// [`MOD_FINALS`] as a set, built once.
///
/// `MOD_FINALS` is written during `init` and never again, so a snapshot cannot
/// go stale — and the item-build hook asks this question for every selectable
/// final item of every player of every match, on parallel sim workers, where a
/// global mutex and a linear scan per candidate is contention for an answer that
/// cannot change.
static MOD_FINAL_SET: OnceLock<std::collections::HashSet<String>> = OnceLock::new();

/// [`is_mod_item`] for the simulation side. Reads empty — so promotes nothing —
/// if it were ever called before registration, which cannot happen: items are
/// registered in `init`, matches run later.
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

/// Every path inside the editor, resolved against one host.
///
/// # Why these are not constants any more
///
/// The editor is spawned under whichever node hosts it, and it has two hosts in
/// two different scenes, so nothing below its root has a fixed absolute path.
///
/// # Why they hand back `&'static str`
///
/// Because that is what keeps the change from reaching the ~95 places that use
/// them: a call site gains `()` and nothing else — no borrow, no temporary to
/// keep alive, and `==` on two of them still compares text, which several click
/// handlers depend on. Returning `String` would have touched every one of those
/// lines, on a refactor with no way to compile-check the result here.
///
/// The strings are leaked, which is sound because the set is bounded by the
/// number of distinct hosts — two — and each is built at most once. That is a
/// fixed ~46 small strings for the life of the process; it does not grow with
/// time, with screen changes, or with use.
struct EditorPaths {
    /// The node the window is spawned under. Never addressed except to spawn.
    parent: &'static str,
    editor: &'static str,
    /// The editor panel.
    popup: &'static str,
    /// The two full-width bars. Only ever addressed to resize them.
    toolbar: &'static str,
    footer: &'static str,
    rows: &'static str,
    /// The three scroll views in the editor: the row list and the two floating
    /// lists. See [`focus_scroll`] — only one may take the wheel at a time.
    rowscroll: &'static str,
    /// The column-header strip, whose labels have to track the same geometry the
    /// rows are laid out with.
    colheader: &'static str,
    /// The toolbar hint, the one label in the editor long enough to matter.
    hint: &'static str,
    /// In the toolbar, above the column headers: adding a row acts on the list
    /// below it.
    add: &'static str,
    /// The filter box, beside Add. A `text_edit` styled after the ban/pick
    /// screen's own champion search — same `main#text_edit` style, same
    /// magnifier child, same `placeholder`/`max_length` keys — so it reads as one
    /// of the game's controls.
    ///
    /// No handler is registered for it. `TextEditComplete` fires on commit (Enter
    /// or focus loss), not per keystroke, so a handler alone would give a filter
    /// that only updates when you leave the box; [`sync_filter`] polls its text
    /// every frame instead. Its X *is* a registered control, since a click is
    /// exactly the event a button reports.
    search: &'static str,
    search_clear: &'static str,
    /// In the footer rather than the toolbar: it is the panel's "done" button,
    /// and bottom-right is where one is looked for.
    save: &'static str,
    /// The confirmation tick left of Save. Hidden until a save succeeds; it is
    /// the only thing that reports one now that the button stays on the tab.
    saved: &'static str,
    /// In the footer beside Save. These paths are matched by exact string, so a
    /// button moved between the two bars in `build_editor.ui` must be moved here
    /// too — a stale path registers nothing and the control goes quietly dead.
    /// The two footer settings are segmented controls, not buttons whose label
    /// changes: each is a bordered box holding one `color_selectable` per choice,
    /// built exactly like the Team/Builds `#mode_toggle` at the top of the screen.
    /// Both options stay on screen, so the alternative is readable without
    /// clicking to find out what it is — which a single relabelling button cannot
    /// do.
    ///
    /// The container paths themselves are never addressed; only the cells are.
    unique_on: &'static str,
    unique_off: &'static str,
    /// Beside the unique toggle, in the same footer bar: both are match-wide
    /// rules about the builds rather than edits to one, so they read as a pair.
    scope_all: &'static str,
    scope_own: &'static str,
    /// Full-screen transparent button shown only while a floating list is open,
    /// so that a click anywhere other than the list dismisses it.
    ///
    /// Declared between `#popup` and the two lists, which at equal `z` puts it
    /// above the window and below them — a click on a list row reaches the row, a
    /// click anywhere else reaches this. Without it a list could only be
    /// dismissed by choosing something or by hitting the backdrop outside the
    /// window, which left clicks on the window itself doing nothing at all.
    listcatch: &'static str,
    itemlist: &'static str,
    champlist: &'static str,
    rolelist: &'static str,
    itemlist_scroll: &'static str,
    champlist_scroll: &'static str,
    rolelist_scroll: &'static str,
}

/// Leaks one path. See [`EditorPaths`] for why this is bounded.
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

/// Path sets built so far, one per host. Looked up by parent so a host revisited
/// later reuses its set rather than leaking a second copy.
static HOSTS: Mutex<Vec<&'static EditorPaths>> = Mutex::new(Vec::new());

/// The host the editor is currently addressed under.
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

/// Points every editor path at `parent`.
///
/// Callers do not have to react to a change of host. The previous host's subtree
/// is in a scene that is no longer loaded, and [`ensure_editor`] notices that on
/// its own: its `ui_exists` stops resolving and it respawns into the new host.
fn set_host(parent: &str) {
    let built = host_paths(parent);
    if let Ok(mut active) = ACTIVE_HOST.lock() {
        *active = Some(built);
    }
}

/// The active host's paths.
///
/// Defaults to the strategy screen, which is where the editor lived when its
/// paths were constants — so anything reading a path before a host is chosen
/// behaves exactly as it used to.
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
/// Resting `speed` of all three, restored to whichever one is live. Must match
/// the value the three views are authored with in `build_editor.ui`.
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

/// Strips characters that would end a `.ui` string literal or be read as markup.
/// Item and player names are plain text, so this only ever guards against a
/// future rename.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '<' | '>' | '{' | '}' | ';'))
        .collect()
}

/// Approximate width, in canvas px, of one character of the 14px label text the
/// rows use.
///
/// The UI layer exposes no text measurement, so a label that has to be made to
/// fit is estimated instead. Three buckets — narrow, wide, and everything else —
/// are enough for the job: item names are ordinary Latin words, and the cost of
/// being a few px out is one character more or less before the ellipsis.
///
/// Calibrated against names measured off a rendered 199px column ("Frozen
/// Mallet", "Zeke's Herald", "Infinity Edge", "Liandry's Torment"), and rounded
/// *up* from there on purpose: over-estimating costs a character before the
/// ellipsis, while under-estimating puts the name back under the clear button,
/// which is the bug this is here to fix.
fn char_width(character: char) -> f32 {
    match character {
        ' ' | '\'' | '.' | ',' | ':' | ';' | '!' | '|' | 'i' | 'j' | 'l' | 't' | 'f' | 'r'
        | 'I' => 4.0,
        'm' | 'w' | 'M' | 'W' | '@' => 12.0,
        _ => 8.0,
    }
}

fn text_width(text: &str) -> f32 {
    text.chars().map(char_width).sum()
}

/// Shortens `text` until it fits `max_width`, marking the cut with an ellipsis.
///
/// Returns `text` untouched when it already fits, which is every name in the
/// three-slot layout — the columns only get tight enough to need this when
/// 4-slot mode splits the same band four ways.
fn fit_text(text: &str, max_width: f32) -> String {
    const ELLIPSIS: &str = "...";
    if text_width(text) <= max_width {
        return text.to_string();
    }

    let budget = max_width - text_width(ELLIPSIS);
    let mut fitted = String::new();
    let mut used = 0.0;
    for character in text.chars() {
        let width = char_width(character);
        if used + width > budget {
            break;
        }
        fitted.push(character);
        used += width;
    }
    // "Locket of ..." reads better than "Locket of ...", so drop the space the
    // cut landed on rather than spacing the ellipsis off the last word.
    while fitted.ends_with(' ') {
        fitted.pop();
    }
    fitted.push_str(ELLIPSIS);
    fitted
}

/// Width a pinned item's name has inside its slot button.
///
/// The name starts after [`ICON_PAD`] clears the icon and has to stop before the
/// clear button that sits at `combo_w - 52`, with a few px of air so the last
/// glyph does not touch it.
fn slot_label_width() -> f32 {
    combo_w() as f32 - 52.0 - 4.0 - text_width(ICON_PAD)
}

// -- item list ----------------------------------------------------------

/// Every final item (one with no further upgrades) the game currently knows,
/// grouped into [`item_catalog`]'s categories and sorted by name within each.
/// Read from the item settings document rather than a hand-kept list, so items
/// the mod adds later show up with no extra wiring.
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

/// Builds one [`ItemChoice`] from an item key, resolving its display name and
/// its place in the catalog.
/// `None` for an item with no category, which keeps it out of the list
/// entirely — there is no catch-all group to put it in.
fn make_choice(ctx: &StableClient<'_>, key: &str) -> Option<ItemChoice> {
    let slug = build_config::base_slug(key);
    Some(ItemChoice {
        frame: item_catalog::icon_frame(slug, is_mod_item(key)).map(sanitize),
        category: item_catalog::category_of(slug)?,
        name: sanitize(&item_display_name(ctx, key, slug)),
        key: sanitize(key),
    })
}

/// Display name for an item, without the "Radiant" tier word.
///
/// Every final item in the pool is a radiant one, so the prefix is on every row
/// and distinguishes nothing — it just costs the width that tells two items
/// apart. The *base* item's name is used rather than trimming a prefix off the
/// radiant one, because the prefix is a translated word: trimming "Radiant "
/// would work in English and leave the tier word in place everywhere else.
///
/// Six items have no base tier to read: the vanilla finals the mod reskins are
/// renames of existing keys, not new items on top of a base (`radiant_
/// bloodthirster` *is* `warlords_final_judgement`; there is no `bloodthirster`).
/// For those the radiant name is trimmed instead, which only works in English —
/// but it is six rows, and the alternative is six rows out of sixty wearing a
/// prefix none of the others do.
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

/// Adds the mod's own final items to `choices`, skipping any the settings
/// document already yielded — the mod renames several vanilla finals (the
/// vanilla `warlords_final_judgement` is "Radiant Bloodthirster"), so the two
/// sources overlap by key.
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

/// An object is an item when it carries any of the fields every item has.
/// Checked structurally because the settings document mixes items with
/// container objects — mod-added items sit under a `mod_items` group rather
/// than at the top level.
fn is_item(value: &serde_json::Value) -> bool {
    value.get("next_tier").is_some() || value.get("tier").is_some() || value.get("price").is_some()
}

/// Walks the settings document collecting final items, descending into group
/// objects. Depth is capped so an unexpected document shape cannot turn this
/// into a deep traversal on the UI thread.
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
        // `next_tier` empty means nothing upgrades from this item. The
        // item-build hook cannot make this test — `next_tier` does not cross the
        // stable boundary — so it works from tier instead; this side reads the
        // catalog JSON and can still ask the question directly.
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

/// The item list, loading it on first use. Empty is a valid (if useless)
/// answer — it means the client cannot read the item settings document — and is
/// not cached, so a later frame can retry.
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

/// Copy of the cached list, for use outside the state lock.
fn snapshot_entries() -> Vec<ListEntry> {
    with_state(|state| state.entries.clone()).unwrap_or_default()
}

/// Display name for a pinned key, falling back to the raw key for an item the
/// current pool does not contain (a build authored against another mod set).
fn name_of(entries: &[ListEntry], key: &str) -> String {
    entries
        .iter()
        .find_map(|entry| match entry {
            ListEntry::Item(item) if item.key == key => Some(item.name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| key.to_string())
}

fn choice_of<'a>(entries: &'a [ListEntry], key: &str) -> Option<&'a ItemChoice> {
    entries.iter().find_map(|entry| match entry {
        ListEntry::Item(item) if item.key == key => Some(item),
        _ => None,
    })
}

/// Champion list, in display order. Cached on first use.
///
/// The ids come from the hook's roster file, because the client cannot
/// enumerate champions: `champion_names()` returns nothing here (the host does
/// not answer those vtable slots), and `SettingTargetV1` exposes only
/// `GameSetting` and `ItemSetting`. It is still tried first, so this starts
/// working on its own if the host ever answers them.
///
/// The roster is taken whole, deliberately. It is not filtered against
/// `champion.i18n`'s `description` map (which would look like a way to drop
/// non-champion entries) because that map holds exactly the 64 *base-game*
/// champions — filtering on it would strip every champion added by another mod,
/// which is the opposite of what is wanted here. On this install the roster is
/// 86 ids: 64 base, 21 from a champion mod, and one dummy. Whatever the hook can
/// be handed in `team1` is a key a build can legitimately use, so the roster is
/// the authoritative list by definition; a stray dummy id in the dropdown is a
/// cosmetic wart, a missing modded champion is a broken feature.
///
/// It is also why the list is not simply hardcoded from the base game's 64
/// champions: a hardcoded list cannot see modded champions at all, and would
/// need a free-text id box to reach them.
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

/// Prefixes some modded champion ids carry. Cosmetic noise in the dropdown, so
/// they are dropped from the shown name only — the id itself is what the hook
/// matches on and is never rewritten.
const MOD_ID_PREFIXES: [&str; 2] = ["test_mod_", "cf_"];

/// Drops any leading mod prefixes, including stacked ones (`test_mod_cf_x`).
///
/// A prefix is only removed when something is left after it, so an id that is
/// nothing but a prefix still shows as itself rather than as an empty button.
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

/// Readable name for a champion id, translated where the game has a name to
/// give and prettified from the id (`snake_case` -> `Snake Case`) where it does
/// not.
///
/// The lookup was long documented here as pointless — `champion.i18n` was found
/// to carry `skill_name` and `description` per champion but no display-name map,
/// and the game was assumed to derive the shown name from the id the same way.
/// It is attempted anyway rather than argued about: two spellings, each accepted
/// only if it answers, and the prettified id behind them. That costs one miss
/// per champion on a cached path and settles the question per locale instead of
/// per comment. A locale whose champions come back in the id's language means
/// the keys genuinely are not there.
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

/// Display name for a champion id, falling back to the raw id for one the
/// roster does not cover.
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

/// Copy of the rows being edited.
fn snapshot_rows() -> Vec<ChampionRow> {
    with_state(|state| state.rows.clone()).unwrap_or_default()
}

/// Applies `edit` to one row and autosaves: the file is written on every change
/// rather than only on Save.
///
/// Returns whether the file was written, so the caller can report a failure
/// instead of silently looking like it worked.
fn edit_row(row: usize, edit: impl FnOnce(&mut ChampionRow)) -> bool {
    let rows = with_state(|state| {
        if let Some(entry) = state.rows.get_mut(row) {
            // Grow to the editable width only. Truncating here would drop a
            // fourth item that is being kept for a 4-slot mode that is
            // currently off, the moment any other slot on the row was touched.
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

/// Leading padding for a combo's label.
///
/// `color_icon_button` has no property that insets its text — its only keys are
/// `btn`, `text`, `icon`, `hover`, `active`, `disabled` and the two sounds, and
/// `text` takes label *styling*, not geometry (the dropdown runner needs a
/// separate `text_layout` for exactly that reason). Left-aligned text therefore
/// starts hard against the button's rounded edge, and under the item icon.
///
/// Padding the string is the one lever here that needs no property the parser
/// might reject. [`ICON_PAD`] clears the 24px icon at `x: 8px`; [`PLAIN_PAD`] is
/// the smaller inset for a combo showing no icon, so both columns start at a
/// sensible margin.
const ICON_PAD: &str = "          ";
const PLAIN_PAD: &str = "  ";

/// The pinned key for one slot, or `None` when the AI owns it.
fn pinned_key(rows: &[ChampionRow], row: usize, slot: usize) -> Option<String> {
    rows.get(row)
        .and_then(|entry| entry.slots.get(slot))
        .and_then(Option::as_ref)
        .cloned()
}

/// Repaints one slot's button: its label, its icon, and whether its clear button
/// is offered at all — an unpinned slot has nothing to clear, so its X is
/// hidden.
fn refresh_combo(
    ctx: &mut StableClient<'_>,
    entries: &[ListEntry],
    rows: &[ChampionRow],
    row: usize,
    slot: usize,
) {
    let pinned = pinned_key(rows, row, slot);
    // A pinned slot shows an icon and needs the wider inset; an unpinned one has
    // no icon, so it takes the plain margin.
    let (label, color) = match &pinned {
        Some(key) => (
            format!(
                "{ICON_PAD}{}",
                fit_text(&name_of(entries, key), slot_label_width())
            ),
            "#e8e8e8ff",
        ),
        None => (format!("{PLAIN_PAD}{}", strings().ai_slot), "#a5a5abff"),
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

/// Repaints one row's champion button. A row with no champion is shown muted,
/// the same way an unpinned slot reads, because it is not yet a build.
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

/// Paints a row's Role button with the role it currently carries.
///
/// `Any` is dimmed the way an unassigned champion is: it is the default rather
/// than a choice the player made, and the column should read as empty until it
/// is set.
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

/// Repaints one row: its champion button and its three slot buttons.
fn refresh_row(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize) {
    let rows = snapshot_rows();
    refresh_champ(ctx, &snapshot_champions(), &rows, row);
    refresh_role(ctx, &rows, row);
    for slot in 0..picker_slots() {
        refresh_combo(ctx, entries, &rows, row, slot);
    }
}

/// Lights the chosen cell of a two-option footer toggle and dims the other.
///
/// The selection is painted, not set, for the reason spelled out at
/// [`paint_tabs`]: `ui_set_selectable_selected` is rejected for the
/// `color_selectable` kind. Both property pairs are written with the same
/// appearance — `image`/`label` and `selected_image`/`selected_label` — so the
/// cell looks right whichever pair the runner decides to render. The tabs can
/// afford to pick one pair because their underlying state is known and fixed;
/// these cells are clicked directly, and paying one extra write beats depending
/// on a click not flipping a flag we cannot read back.
fn paint_toggle(ctx: &mut StableClient<'_>, lit_path: &str, dim_path: &str) {
    ctx.ui_set_properties(lit_path, &toggle_style(true));
    ctx.ui_set_properties(dim_path, &toggle_style(false));
}

/// One toggle cell's appearance, in the vanilla `strategy_option` colours the
/// tabs and list rows already use.
fn toggle_style(lit: bool) -> String {
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
    // `size` and `align_x` are restated rather than left to the `.ui`, because a
    // property write replaces the block it names: a `label` carrying only a
    // colour would drop the centring the layout gives these cells.
    let pair = |image_key: &str, label_key: &str| {
        format!(
            "{image_key}: {{ color: {fill}; back_color: {fill}; stroke: {stroke}; \
             rounding: Uniform {{ rounding: 6; }} \
             hover: {{ color: {hover_line}; back_color: {fill}; }} }} \
             {label_key}: {{ size: 13; align_x: Center; color: {text}; \
             hover: {{ color: {hover_text}; }} }}"
        )
    };
    format!(
        "{} {}",
        pair("image", "label"),
        pair("selected_image", "selected_label")
    )
}

/// Paints which half of the unique-items toggle is the live setting.
fn refresh_unique(ctx: &mut StableClient<'_>) {
    if build_config::unique_items_enabled() {
        paint_toggle(ctx, unique_on_path(), unique_off_path());
    } else {
        paint_toggle(ctx, unique_off_path(), unique_on_path());
    }
}

/// Takes the save tick down.
///
/// Called from every path that changes a row, and from none that only changes
/// what is on screen: the filter box and the two footer toggles leave the builds
/// exactly as the tick found them.
///
/// The tick therefore acknowledges *a save*, and stands only while nothing has
/// been touched since — it is not a claim about the file, which every edit
/// rewrites anyway. Writing `false` when it is already hidden is what makes the
/// callers able to say "edited" without first asking whether it was showing.
fn clear_saved(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(saved_path(), false);
}

/// Paints which half of the build-scope toggle is the live setting.
fn refresh_scope(ctx: &mut StableClient<'_>) {
    if build_config::own_team_only_enabled() {
        paint_toggle(ctx, scope_own_path(), scope_all_path());
    } else {
        paint_toggle(ctx, scope_all_path(), scope_own_path());
    }
}

// -- spawning -----------------------------------------------------------

/// `.ui` source for one champion row.
fn row_source(row: usize) -> String {
    // Bound once: the spawned `.ui` source carries the text literally, so
    // this is where the editor's language reaches a freshly built row.
    let strings = strings();
    let no_champion = &strings.no_champion;
    let ai_slot = &strings.ai_slot;
    // Both follow the panel width, so a row built for one host is the wrong
    // shape for the other -- see `set_panel_size`, which rebuilds them.
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
             text: \"{ai_slot}\";\n\
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

/// Height of one item-list node. Paired with [`entry_source`], which lays the
/// node out at exactly this height; the two are read together to give
/// `#contents` an explicit height.
fn entry_height(entry: &ListEntry) -> i32 {
    match entry {
        ListEntry::Header(_) => ENTRY_HEADER_H,
        ListEntry::Clear | ListEntry::Item(_) => ENTRY_ITEM_H,
    }
}

/// `.ui` source for one item-list node: a category header, or a pickable item
/// with its sheet icon.
///
/// The item node is deliberately the shape the earlier picker used and proved
/// renders — style ref, size, `label`/`selected_label` overrides, `text` — with
/// nothing added but the icon child and explicit label colors. `text_offset`
/// (which would left-align the name past the icon) appears in no shipped layout
/// and is not worth carrying as an unknown, so the style's centred label stands.
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
        // A `color` band with the label as its child rather than a bare label,
        // because a label has no fill of its own. The child draws over the
        // parent at equal `z` (tree order), so the text lands on top of the
        // band. Both ignore events so a click on a header falls through to the
        // catcher and dismisses the list instead of being swallowed.
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

/// `.ui` source for one champion-list node. Plain rows, no icons: there is no
/// champion portrait in a sheet the mod can address by frame name.
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

/// `.ui` source for one entry of the role list. Label only, like the champion
/// list: a role is a word, and there is no art for one.
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

/// Removes the spawned row nodes and spawns one per row passing the filter,
/// registering every control. Called when the window is built, after any add or
/// delete, and whenever the filter text changes.
///
/// Rebuilding wholesale rather than splicing keeps row index and node name in
/// step: a row's identity is its position in the list, so deleting row 1 has to
/// renumber everything after it anyway.
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
        register_once(ctx, &champ_path(row));
        register_once(ctx, &role_path(row));
        register_once(ctx, &delete_path(row));
        for slot in 0..picker_slots() {
            register_once(ctx, &combo_path(row, slot));
            register_once(ctx, &clear_path(row, slot));
        }
        for slot in 0..picker_slots().saturating_sub(1) {
            register_once(ctx, &format!("{}.swap{slot}", editor_row_path(row)));
        }
    }
    let count = spawned.len();
    let _ = with_state(|state| state.spawned_rows = spawned.clone());

    // `#rows` is authored `height: auto`, but an auto height measured over a
    // subtree that has not been laid out is zero, and a scroll view whose
    // contents are zero tall shows nothing however many children it has. The
    // height is knowable exactly, so it is stated.
    let height = count as i32 * (ROW_HEIGHT as i32 + ROW_SPACING);
    ctx.ui_set_properties(rows_path(), &format!("height: {height}px;"));

    for row in spawned {
        refresh_row(ctx, entries, row);
    }
}

/// `.ui` source for one column header, matching the `#c_item*` labels
/// `build_editor.ui` authors for the first three.
fn header_source(slot: usize) -> String {
    let x = combo_x(slot);
    let w = combo_w();
    let number = slot + 1;
    let label = strings().col_items[slot.min(3)].clone();
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

/// Lines the column headers up with the columns.
///
/// `build_editor.ui` authors three headers at the three-slot geometry, which is
/// exactly what [`combo_x`] and [`combo_w`] produce for three slots — so this is
/// a no-op in the vanilla case and only earns its keep at four, where it
/// re-places the first three and spawns the fourth. The editor subtree is
/// rebuilt from source on every [`ensure_editor`], so there is never a stale
/// fourth header to remove.
fn sync_column_headers(ctx: &mut StableClient<'_>) {
    for slot in 0..picker_slots() {
        let path = format!("{}.c_item{}", colheader_path(), slot + 1);
        if ctx.ui_exists(&path) {
            // Text as well as geometry: the first three are authored in
            // `build_editor.ui` with their English labels, so this is the only
            // thing that translates them.
            let label = strings().col_items[slot.min(3)].clone();
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

/// Rows that pass the current filter, as indices into `state.rows`.
///
/// Filtering by *not spawning* rather than by hiding is what keeps the rest of
/// the module unchanged: a row's node is named for its index in `state.rows`
/// (`…rows.row4`), so every path, every handler and [`row_from_path`] go on
/// meaning the same thing with a filter up. Hiding would have worked too, but
/// only if `TopToBottom` skips invisible children when it measures — and a
/// filtered list with 50px gaps in it is exactly the bug that would cause.
///
/// A row with no champion always passes. It is the row that was just added or is
/// about to be assigned, and having `+ Add Champion` produce a row that is
/// immediately filtered out of sight is worse than showing one extra line. (The
/// old external editor solved the same problem by clearing the search box on
/// add, which loses the filter you were in the middle of using.)
fn visible_rows() -> Vec<usize> {
    // Taken before the lock: `with_state` is a plain `Mutex`, so reading the
    // champion list from inside the closure would deadlock on itself.
    let champions = snapshot_champions();
    with_state(|state| {
        // Owned, so the borrow of `state.filter` ends before `state.rows` is
        // iterated below.
        let terms = filter_terms(&state.filter);
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
                        let haystack = format!("{} {id}", champion_label(&champions, Some(id)))
                            .to_lowercase();
                        // Any term, not all: commas read as "or", so a row needs
                        // to answer only one of the names typed.
                        terms.iter().any(|term| haystack.contains(term))
                    }
                }
            })
            .map(|(index, _)| index)
            .collect()
    })
    .unwrap_or_default()
}

/// The search box's text as the terms a row may match.
///
/// Commas separate alternatives, so `ashe, garen` shows both. Blank terms are
/// dropped rather than kept as empty strings, which is what stops a lone comma
/// or a trailing separator — the state the box is in half way through typing a
/// second name — from either matching everything or matching nothing.
///
/// Lowercased here so the comparison against the label does not have to be.
fn filter_terms(filter: &str) -> Vec<String> {
    filter
        .split(',')
        .map(|term| term.trim().to_lowercase())
        .filter(|term| !term.is_empty())
        .collect()
}

/// Whether a filter is currently narrowing the list.
///
/// Asks [`filter_terms`] rather than testing the raw text, so that "the box has
/// something in it" and "the list is narrowed" cannot drift apart: `,` is the
/// case where they differ.
fn filter_active() -> bool {
    with_state(|state| !filter_terms(&state.filter).is_empty()).unwrap_or(false)
}

/// Reads the filter box and rebuilds the rows when its text has changed.
///
/// Polled from `post_update` rather than driven by an event: the only text
/// event the ABI defines is `TextEditComplete`, which fires on commit, so a
/// handler would leave the list stale until the box lost focus. Reading a string
/// once a frame is cheap, and the rebuild is gated on the text actually
/// differing.
fn sync_filter(ctx: &mut StableClient<'_>) {
    let Some(text) = ctx.ui_text_edit_text(search_path()) else {
        return;
    };
    let changed = with_state(|state| {
        let changed = state.filter != text;
        state.filter = text.clone();
        changed
    })
    .unwrap_or(false);
    if !changed {
        return;
    }

    ctx.ui_set_visible(search_clear_path(), !text.trim().is_empty());
    // A floating list is positioned against the row that opened it, and that row
    // is about to be removed and respawned.
    close_list(ctx);
    let entries = snapshot_entries();
    rebuild_rows(ctx, &entries);
}

/// The composition test's tactics panel, or `None` when it is not up.
///
/// # Why this searches instead of naming a path
///
/// The first version named `main.contents.training.comp_test_popup.tactics`,
/// reasoning from `main.contents.strategy`. That was wrong twice over. The
/// training layout's root node is `training` (`training:training_ui`), not
/// `main`; and `contents` is a node authored *inside* `strategy.ui`, so it does
/// not exist on any screen but that one. `ui_visible` returned `None` for the
/// whole path, the vanilla rows were never hidden and the editor never opened —
/// the same silent failure `item_stats_ui::resolve_screen` documents for the
/// statistics tab, which cost a build there.
///
/// So the documented path is a *candidate*, not an answer: [`COMP_ROOT`] is
/// tried first because it costs one `ui_exists`, and a throttled breadth-first
/// walk for the `comp_test_popup.tactics.builds` shape backs it up. The shape is
/// the marker rather than the screen's name for the reason `find_screen` gives:
/// the name is game code's business, the shape is what this module needs.
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

    // Everything above is one `ui_exists` and runs anywhere. The sweep is not:
    // without this gate a screen that simply has no composition test — which is
    // every screen but one — would pay for a failed 600-node walk twice a second,
    // forever.
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

/// Whether the client says the Training tab is the one up.
///
/// Gates the sweep in [`resolve_comp_tactics`] the way `item_stats_ui`'s
/// `on_statistics_tab` gates its own. `None` means the client could not answer,
/// and the sweep is allowed rather than blocked: the throttle still bounds it,
/// and refusing on a missing answer would make the fallback dead code on any
/// build where that call stops working.
fn on_training_tab(ctx: &StableClient<'_>) -> bool {
    let Some(tab) = ctx.client_main_tab() else {
        return true;
    };
    tab.to_ascii_lowercase().contains("train")
}

/// Breadth-first from the UI root for the node holding the composition test's
/// build block. Bounded by both depth and node count — one wide level can be
/// hundreds of nodes, and this runs on the UI thread.
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

/// Puts the editor in the composition test's build section, in place of the
/// vanilla rows.
///
/// There is no tab to enter here and none to leave by, so the editor is simply
/// what that section *is* while the panel is up — opened on arrival rather than
/// on a click, and torn down with the screen by the same teardown the strategy
/// screen uses.
fn comp_test_host(ctx: &mut StableClient<'_>, tactics: &str) {
    let builds = format!("{tactics}.builds");

    shrink_team_tactics(ctx, tactics);

    // Measured against the *header* and the Back button, not against the block
    // the editor replaces. `builds` is hidden two lines below, so from the second
    // frame on its rect is unreliable, and a panel that can only be placed on the
    // frame it first appeared is a panel that never recovers from a relayout.
    // These two stay visible for as long as the dialog does.
    //
    // Nothing here can come from the layout file: the dialog is authored 680px
    // tall and resized at runtime to fit this 1072px panel, so everything inside
    // it moves, and the lane test lays out differently again. `ui_node_rect`
    // reports in the same design space the layouts are authored in — see
    // `place_list`, which positions the floating lists the same way.
    let slot = comp_test_slot(ctx, tactics);

    // Point every editor path at this screen before one is read. The strategy
    // screen's host is a node inside `strategy.ui`, which is not loaded here, so
    // until this runs `editor_path()` names something that cannot exist.
    set_host(tactics);

    // Re-asserted every frame rather than once on entry: game code rebuilds
    // this block whenever the champion selection changes, and a one-shot hide
    // does not survive that. `post_update` runs after game code, so this is the
    // last word — the same reason the strategy screen re-asserts its writes.
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

    // Geometry is re-asserted every frame, like the hide above, and for a
    // sharper reason: the 5v5 and lane tests are the same `tactics` panel laid
    // out differently, and the editor outlives the dialog. Applying this only on
    // the frame the editor was built meant each mode kept whichever geometry the
    // *other* one had measured.
    apply_panel_size(ctx);
    if resized {
        let entries = snapshot_entries();
        rebuild_rows(ctx, &entries);
    }

    // Both rects are absolute and the popup's x/y are relative to the host, so
    // the host's own origin comes off — see [`host_origin`]. On the strategy
    // screen that subtraction is a no-op, which is why the first version of this
    // looked right there and only there.
    //
    // Left where it is when the measurement did not come back — an editor in the
    // wrong place still works, and the alternative is not showing one at all.
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

/// Whether this screen is showing team tactics at all, which is what separates
/// the 5v5 from the lane test.
///
/// **Visibility, not geometry.** The first version of this asked whether the
/// first tactics row had a rect, and it read `true` in both modes: the exe
/// leaves those nodes laid out and simply stops drawing them, so a hidden row
/// still reports its size. The "Team Tactics" label is the node that actually
/// disappears in the lane test - along with the Blue/Red labels beside it - and
/// `ui_visible` is what reports that.
///
/// Both the label and the block are checked, because either one being hidden
/// means there is nothing to scroll; `None` (no such node) is treated as hidden
/// for the same reason.
fn team_tactics_shown(ctx: &StableClient<'_>, tactics: &str) -> bool {
    ctx.ui_visible(&format!("{tactics}.strategy_header")) == Some(true)
        && ctx.ui_visible(&format!("{tactics}.strategy")) == Some(true)
}

/// Height the team-tactics block is cut to on the 5v5 screen: eight of its
/// twelve rows, on the 38px pitch they are authored at. The rest scroll.
const TEAM_TACTICS_HEIGHT: i32 = 296;

/// The block's authored height, restored on any screen that is not the 5v5.
/// Keep in step with `#strategy` in `ui/layout/training.ui`, which this mod
/// ships - it is the same twelve rows at the same pitch.
const TEAM_TACTICS_FULL_HEIGHT: i32 = 456;

/// Gap the exe leaves between the bottom of that block and the "Personal
/// Tactics" header. Measured, not guessed: with the block at its authored
/// `(y 160, h 456)` the exe puts the header at `y 662`, and 662 - 616 is this.
const BUILD_HEADER_GAP: i32 = 46;

/// Shortens the team-tactics block on the 5v5 screen and pulls the header under
/// it, handing the reclaimed height to the item-build editor below.
///
/// # Why the 5v5 only, and how that is decided
///
/// `training.ui` has one `comp_test_popup.tactics.strategy` block, shared by
/// the 5v5 and the lane test, so the layout cannot tell them apart - which is
/// why the `scroll_view` this needs is declared there at the block's *unchanged*
/// height, and the shrink happens here instead.
///
/// The test is whether the rows are on screen at all. The lane test has no team
/// tactics: the exe hides that whole section, header and Blue/Red labels
/// included, leaving the block occupying empty space. Asking `focused` - the
/// first row - for a rect answers that directly, and it answers it about the
/// thing being resized rather than about some proxy for the mode. A screen with
/// nothing to scroll keeps the vanilla block and never shows a bar.
///
/// # Why every frame
///
/// The same reason the rest of this screen's geometry is re-asserted: game code
/// rebuilds this panel when the champion selection changes, and `post_update`
/// runs after it, so a write here is the last word. A one-shot would survive
/// until the first reselect.
fn shrink_team_tactics(ctx: &mut StableClient<'_>, tactics: &str) {
    // Lane test. Put the block back rather than merely leaving it alone: this
    // write sticks to the node, and the panel outlives the dialog - same
    // subtree, same scene - so a 5v5 followed by a lane test would otherwise
    // hand the lane test the shrunk block it is not supposed to get.
    // `build_header` needs no counterpart: the exe positions that one itself
    // (authored at 252, it lands at 662), so not writing it is enough.
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

    // `ui_node_rect` is absolute and `ui_set_properties` is parent-relative, and
    // the header's parent is the panel - the same subtraction the editor popup
    // does against its own host. The block's own top is read live rather than
    // assumed, so this follows the panel wherever the exe puts it.
    let top = (strategy.1 - panel.1).round() as i32 + TEAM_TACTICS_HEIGHT + BUILD_HEADER_GAP;
    ctx.ui_set_properties(
        &format!("{tactics}.build_header"),
        &format!("y: {top}px;"),
    );
}

/// The rectangle the editor should occupy inside the composition test, in the
/// absolute space `ui_node_rect` reports: `(x, y, w, h)`.
///
/// Spans from just under the "Personal Tactics" header down to just above the
/// Back button, at the header's own x and width — which is the same 24px inset
/// and 1232px span the vanilla block uses, taken from the live layout rather
/// than copied out of `training.ui`.
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

/// Spawns the window, its rows and both dropdown lists, and registers every
/// control. Deferred until the first click so a failure costs nothing until the
/// player actually asks for the editor, and is retried on the next click.
///
/// Also the join between the two hosts: `modal_ready` is not enough on its own,
/// because the flag survives a screen change while the subtree it describes does
/// not. The `ui_exists` beside it is what notices that the editor's root now
/// names a node in a scene that has been unloaded, and respawns into the current
/// host instead.
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
            register_once(ctx, &entry_path(index));
        }
    }
    let content_height: i32 = entries.iter().map(entry_height).sum();
    ctx.ui_set_properties(&contents, &format!("height: {content_height}px;"));

    let champ_contents = champ_contents_path();
    for (index, choice) in champions.iter().enumerate() {
        if !ctx.ui_spawn_source(&champ_contents, &champ_entry_source(index, choice)) {
            break;
        }
        register_once(ctx, &champ_entry_path(index));
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
        register_once(ctx, &role_entry_path(index));
    }
    ctx.ui_set_properties(
        &role_contents,
        &format!("height: {}px;", Role::ALL.len() as i32 * ENTRY_ITEM_H),
    );

    // Read from disk here rather than at every open, so a file edited by hand
    // while the game sits on this screen is picked up.
    let _ = with_state(|state| {
        state.rows = build_config::load_champion_rows();
        state.spawned_rows.clear();
        // The subtree was just spawned, so its filter box is empty whatever the
        // last screen was left filtered by.
        state.filter.clear();
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
    ] {
        register_once(ctx, path);
    }

    // Both lists start hidden but nothing in the source stops them taking the
    // wheel, so without this a scroll of the rows also scrolls two panels that
    // are not on screen — invisible until one is opened and found part-way down.
    focus_scroll(ctx, rowscroll_path());

    let _ = with_state(|state| state.modal_ready = true);
    true
}

// -- interaction --------------------------------------------------------

/// Enters the Builds tab: selects it, hides what the other two tabs show, and
/// puts the editor in their place.
///
/// The three tabs are `color_selectable`s, and each only knows its own state —
/// nothing arbitrates between them. Game code deselects Personal when Team is
/// clicked and vice versa, but it has never heard of ours, so the deselecting
/// in both directions is done here.
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

/// Leaves the Builds tab.
///
/// Deliberately restores no panel. Every exit from the tab is a click on Team,
/// and game code's own handler for that click shows the Team columns — putting
/// them back here would race that.
fn close_editor(ctx: &mut StableClient<'_>) {
    close_list(ctx);
    paint_tabs(ctx, false);
    ctx.ui_set_visible(editor_path(), false);
    // The one thing put back, because it is the one thing hidden that game code
    // may not restore: "Closing Out" belongs to the Team tab, and if its handler
    // does not re-show it the block would stay gone for the rest of the screen.
    // Writing `true` when game code also does is harmless.
    ctx.ui_set_visible(GAME_FINISH_PATH, true);
    let _ = with_state(|state| state.showing = false);
}

/// Paints which of the two tabs looks selected.
///
/// The selection cannot be *set*. `ui_set_selectable_selected` is
/// `state_set_json` with `{"selected": …}`, and the host accepts that key only
/// for the `checkbox`, `text_edit`, `slider` and `selectable` runner kinds —
/// these tabs are `color_selectable`, a kind not on that list, so the write is
/// rejected and returns false. The highlight is therefore drawn on directly.
///
/// The two tabs are painted through different property pairs because they are in
/// different states underneath. Ours is never `selected`, so its plain
/// `image`/`label` are what render. Team *is* selected as far as game code is
/// concerned — nothing ever told it otherwise — so its `selected_image` and
/// `selected_label` are what render, and those are the ones that have to be
/// dulled to make it look inactive.
///
/// # `color` is the stroke, `back_color` is the fill
///
/// Only while `stroke` is non-zero. `strategy_option`'s `image` block sets
/// `stroke: 1`, so setting `color` alone there paints a light *outline* around a
/// still-dark tab rather than filling it — which is what the first version of
/// this did. Lighting our tab therefore means dropping the stroke to 0 and
/// setting the fill, and dimming it means putting the stroke back.
///
/// `selected_image` sets no stroke at all, which is why Team needs only a
/// colour and gets no `stroke` key here.
///
/// Hover is written on both states rather than left to the style. A lit tab has
/// nothing to signal, so its hover colours are its resting ones. A dim tab greys
/// its outline and brightens its text — vanilla's unselected `image`/`label`
/// behaviour, which `selected_image` does not have because a selected tab is
/// never normally the one being hovered towards.
fn paint_tabs(ctx: &mut StableClient<'_>, builds_active: bool) {
    if !ctx.ui_set_properties(BUILDS_TAB, &tab_style("image", "label", builds_active)) {}
    if !ctx.ui_set_properties(
        TEAM_TAB,
        &tab_style("selected_image", "selected_label", !builds_active),
    ) {}
}

/// Idle label colour of a pickable list row, and of the "Let Player Decide" row
/// above them, which is greyer because it is the absence of a pick.
const LIST_ROW_TEXT: &str = "#d7dbe4ff";
const LIST_CLEAR_TEXT: &str = "#a5a5abff";

/// One list row's appearance, lit the same way the Builds tab is.
///
/// The rows are `color_selectable` for the fill, which means their `selected`
/// flag cannot be written — `state_set_json` takes that key for `selectable` but
/// not for this kind (see [`paint_tabs`]). So "which row is picked" is painted
/// here rather than set, and the two list-opening functions repaint every row.
///
/// A bare `selectable` was the obvious choice and is what this used before: its
/// `selected` flag *is* writable, so one call per row did the whole job. But
/// that runner draws only a label — no vanilla layout uses it, and the
/// `selected_image` in `strategy_option` went nowhere — so the picked row could
/// only ever change text colour, never take a fill.
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

/// One tab's appearance, through whichever property pair actually renders for
/// it — `image`/`label` for a tab this mod added, which game code never
/// considers selected, and `selected_image`/`selected_label` for a vanilla one
/// it does.
///
/// Shared with [`crate::item_stats_ui`], whose fourth tab on the statistics
/// screen is the same trick against a different runner: same `strategy_option`
/// style underneath, so the same pair of property sets lights and dims it.
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

/// Holds the Matchup card on screen and "Closing Out" off it.
///
/// Called on entry and then every frame from `post_update`: game code re-asserts
/// both from its own tab state, which never becomes Builds, so a one-shot write
/// does not survive its next update.
fn keep_matchup(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(SUB4_PATH, true);
    ctx.ui_set_visible(GAME_FINISH_PATH, false);
    ctx.ui_set_visible(MATCHUP_PATH, true);
}

/// Places a floating list under the control that opened it, flipping above when
/// there is no room below the way a dropdown near the screen edge does.
///
/// The anchor's position comes from `ui_node_rect`, which the list probe
/// confirmed reports in the same design space the layouts are authored in. Rows
/// scroll, so their position cannot be computed from the layout constants the
/// way a fixed row's could.
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
    // Nudged left rather than pinned to the control's own x, so one at the right
    // edge of the window still gets the whole list on screen. Clamped in screen
    // space, which is what the canvas bounds are in, and only then moved into the
    // host's frame.
    let left = x.min(CANVAS_W - width - 8).max(8);

    let (origin_x, origin_y) = host_origin(ctx);
    let (local_left, local_top) = (left - origin_x, top - origin_y);
    if !ctx.ui_set_properties(panel, &format!("x: {local_left}px; y: {local_top}px;")) {}
    (left, top)
}

/// Origin of the node the editor is spawned under, in the absolute design space
/// [`StableClient::ui_node_rect`] reports.
///
/// A node's own `x`/`y` are relative to its parent while `ui_node_rect` answers
/// in absolute coordinates, so anything positioned *from a measurement* has to
/// have this taken off it first.
///
/// It is (0, 0) on the strategy screen — `main.contents` is a full-screen node at
/// the origin, which is why nothing needed this while that was the only host —
/// and it is not on the composition test, whose panel is a centred dialog. A
/// failed measurement falls back to (0, 0) rather than skipping the write: the
/// old behaviour, and a list in the wrong place still beats no list.
fn host_origin(ctx: &StableClient<'_>) -> (i32, i32) {
    ctx.ui_node_rect(paths().parent)
        .map(|(x, y, _, _)| (x.round() as i32, y.round() as i32))
        .unwrap_or((0, 0))
}

/// Shows the item list under the slot that was clicked, with the slot's current
/// pick ticked.
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

/// Shows the champion list under the row's champion button, with the row's
/// current champion ticked.
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

/// Opens the role list under a row's Role button.
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

/// Hides whichever floating list is showing. All are hidden unconditionally:
/// it costs one call and cannot leave a stale panel behind.
fn close_list(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(itemlist_path(), false);
    ctx.ui_set_visible(champlist_path(), false);
    ctx.ui_set_visible(rolelist_path(), false);
    ctx.ui_set_visible(listcatch_path(), false);
    focus_scroll(ctx, rowscroll_path());
    let _ = with_state(|state| state.open_list = None);
}

/// Gives the wheel to one scroll view and takes it from the other two.
///
/// A floating list is drawn over the row list, but overlapping is not something
/// the wheel takes account of: with a list open, one scroll moved both it and
/// the rows underneath it. `#listcatch` does not help — it is a button, and
/// buttons are about clicks.
///
/// Two keys are written, because the obvious one is not enough. `ignore_event`
/// is in the `scroll_view` runner's property table (beside `speed`, `bar_width`
/// and the rest) and the write is accepted, but it governs hit-testing — the
/// same thing it does on the row images, which use it to keep from swallowing a
/// click meant for the button behind them — and the wheel is read from hover
/// rather than routed as a click, so a scroll view with it set still scrolled.
/// `speed` is what scales the movement, and zero of it is none.
///
/// Written on all three every time rather than tracked, so there is no state to
/// get out of step: exactly one is live and it is the one named. Muting
/// `#rowscroll` while a list is up costs nothing else — the rows behind an open
/// list are already unclickable, that being `#listcatch`'s entire job — and
/// [`close_list`] restores it on every dismissal path.
/// Node name of the vanilla Item Info popup, opened by the button this tab
/// keeps on screen. Its layout is `strategy_component/item_info_popup`: a
/// full-screen `z: 200` panel, spawned by game code rather than declared in
/// `strategy.ui`, which is why its path has to be found rather than written
/// down.
const INFO_POPUP_NAME: &str = "item_info_popup";

/// The scroll view that is live when nothing may scroll at all. Not a path
/// anything has, which is the point: [`focus_scroll`] mutes every view that is
/// not the one named.
const SCROLL_NONE: &str = "";

/// Looks for the Item Info popup in the node tree, breadth-first from `main`.
///
/// Game code spawns it, so where it lands is not something the mod's own
/// layouts record — and a guessed path that is wrong fails silently, leaving the
/// bug in place with nothing to show for it. `ui_child_names` answers the
/// question directly. Two levels is enough for a full-screen popup (it is a
/// sibling of the screen, not a part of one) and bounds the walk.
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

/// Mutes the editor's scroll views while the Item Info popup is up, and hands
/// the wheel back when it closes.
///
/// The popup covers the editor and scrolls its own list, but it is game code's
/// node — the mod cannot make it consume the wheel on the way past, only stop
/// its own views from acting on what gets through. Same overlap as an open
/// dropdown, one screen further out.
///
/// Rewritten only on the transition. Closing hands the wheel to whichever list
/// is open rather than always to the rows, since the popup can be opened over an
/// open dropdown.
fn sync_info_popup(ctx: &mut StableClient<'_>) {
    // Its own source declares `visible: false`, which says it is spawned with
    // the screen and toggled — so the first search should find it. The retry is
    // for the other possibility, that game code spawns it on first use: a single
    // failed search would then never find it at all. Once a second is often
    // enough for something opened by hand, and the walk stops being run once it
    // has an answer.
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

/// Row index from an event path (`….rows.row2.slot1` -> `2`).
fn row_from_path(path: &str) -> Option<usize> {
    path.rsplit_once(".row")
        .and_then(|(_, rest)| rest.split('.').next())
        .and_then(|digits| digits.parse::<usize>().ok())
}

/// Trailing index of a `slotN` / `clearN` / `swapN` / `eN` / `cN` node name.
fn index_after(path: &str, prefix: &str) -> Option<usize> {
    path.rsplit_once(prefix)?.1.parse().ok()
}

/// Single handler for every registered control; dispatches on the firing path.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // `Remove` fires as a node is torn down, and this screen is torn down on
    // every match start — so the Builds tab's own destruction would otherwise
    // arrive here as if it had been clicked, spawning the editor into a dying
    // tree. Only real interactions should reach the dispatch below.
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
        // Put the popup back where it is authored. Belt and braces since the
        // hosts were split: the composition test moves its *own* subtree, not
        // this one, and a strategy screen that has been reloaded respawns this
        // from source at the authored rect anyway.
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
        // Save no longer leaves for the Team tab: the common thing to do after
        // saving is to keep editing, and being thrown out of the panel to come
        // back cost more than the exit was worth. So the button has to report
        // its own result, which is what the tick is for.
        //
        // The write is real rather than skipped-because-edits-autosave, so that
        // a failed one can leave the tick hidden — a tick that appears whatever
        // happened is not a report. It goes down again on the next edit (see
        // `clear_saved`), which is what keeps it about *this* state of the rows.
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
        let _ = with_state(|state| state.filter.clear());
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    // A segmented control's cells each *assert* a value rather than flipping the
    // setting: clicking the option that is already lit has to be a no-op, or the
    // control would contradict what it shows.
    // The setting is all this writes. It deliberately does not touch the saved
    // builds: a pinned duplicate is the player's, and enforcement is something
    // the match does to a build, not something the editor does to the config.
    if path == unique_on_path() || path == unique_off_path() {
        if build_config::set_unique_items(path == unique_on_path()) {
            refresh_unique(ctx);
        }
        return;
    }

    if path == scope_all_path() || path == scope_own_path() {
        if build_config::set_own_team_only(path == scope_own_path()) {
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

/// Commits a clicked item row: clicking the pinned item again clears the slot,
/// so a slot can be returned to AI choice without reaching for the X.
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

/// Commits a clicked role, assigning it to the open row.
///
/// Unlike the champion and item lists there is no click-again-to-clear: `Any` is
/// itself an entry, so clearing is picking it.
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

/// Commits a clicked champion row, assigning it to the open row.
///
/// Clicking the champion already assigned clears it, which takes the row out of
/// the saved file without deleting it from the editor.
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
        // The merged `tfm2_item_tactics` half, which was its own
        // `ModExtension::post_update` before it moved in here. It has to run
        // first and unconditionally: this is the only per-frame client hook the
        // mod owns, and everything below returns early off the strategy screen,
        // while the tactics half installs and self-heals its hooks (including
        // the one that captures the UI root) on every frame, everywhere.
        tactics::driver::post_update(ctx);

        // Also unconditional, and for the same reason: the screen it acts on is
        // not this one, so it cannot live behind the early return below. It is
        // inert unless `4items.cfg` says three slots.
        crate::solo_rank_ui::sync(ctx);

        // Which save's item totals are loaded. Unconditional because it has to
        // be settled before anything reads or writes them, and the two places
        // that do — the statistics screen and the management tick — are both
        // somewhere this early return would have skipped.
        crate::item_stats::sync(ctx);

        // Same again, for the statistics screen and its Item Stats tab. Inert
        // anywhere else: it returns on its first line unless that screen is up.
        crate::item_stats_ui::sync(ctx);

        // The composition test hosts the editor too. It has to be handled
        // before the gate below, which returns — and tears the editor down —
        // for any screen without the strategy tabs, this one included.
        //
        // Visibility, not existence: the panel is authored `visible: false` and
        // toggled by game code, so it *exists* for as long as the Training
        // screen is loaded. Gating on `ui_exists` would put the editor up over
        // the whole Training screen with the dialog shut.
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
            let stale = with_state(|state| {
                let stale = state.wired;
                state.wired = false;
                state.modal_ready = false;
                state.spawned_rows.clear();
                state.filter.clear();
                state.open_list = None;
                // The popup found on this screen dies with it, so the next one
                // searches again rather than writing to a stale path.
                state.info_popup = None;
                state.info_probe_tick = 0;
                state.info_showing = false;
                state.showing = false;
                stale
            })
            .unwrap_or(false);
            if stale {
                // The screen and everything registered on it is gone, so the
                // next one has to wire itself from scratch.
                forget_registrations();
            }

            return;
        }

        // On the strategy screen, so its host is the one every editor path is
        // addressed under. Asserted here rather than once on entry because the
        // composition test repoints them at itself and this is the only place
        // that knows they have to come back.
        set_host(UI_ROOT);

        if !with_state(|state| state.wired).unwrap_or(true) {
            // Loaded here, in `post_update`, not from a click handler:
            // `setting_get_json` returned None when called inside one, matching
            // the trait's note that only ui/asset calls are live there.
            cached_entries(ctx);
            cached_champions(ctx);
            // Team is registered as well as ours: a handler on it is what puts
            // the editor away when the player switches back, and registering is
            // observation only — game code's own handling for that path is
            // untouched.
            for path in [BUILDS_TAB, TEAM_TAB] {
                register_once(ctx, path);
            }
            let _ = with_state(|state| state.wired = true);
        }

        // Re-asserted every frame rather than once on entry. Game code drives
        // these from its own idea of the current tab, which never becomes
        // Builds — so on a screen entered from Team it undoes them on its next
        // update and a one-shot write does not survive. This runs in
        // `post_update`, after that, so ours is the last word.
        if with_state(|state| state.showing).unwrap_or(false) {
            ctx.ui_set_visible(ITEM_INFO_BTN, true);
            keep_matchup(ctx);
            sync_filter(ctx);
            sync_info_popup(ctx);
        }
    }

    /// Last call before the process goes away, so anything held back for the sake
    /// of not writing it every tick has to go out here.
    ///
    /// Nothing is owed a write here any more.
    ///
    /// The totals live in the save file's own namespace, which reaches disk when
    /// the player saves and never because this mod asked. `item_stats::sync`
    /// keeps the in-memory save current every frame, so whatever the player saves
    /// already has the numbers in it — and this hook is handed no `StableClient`
    /// to write one with anyway.
    fn on_end(&self) {}
}
