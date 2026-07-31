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
//! like it works and does not. Its `#personal` panel node is still in the
//! layout, unreachable, because nothing else needs it gone.
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

use std::sync::Mutex;

use mod_api_stable::*;

use crate::build_config::{self, ChampionRow, PICKER_SLOTS};
use crate::item_catalog;

/// Shown for a slot left to the game's own AI, and on the list row that puts a
/// slot back into that state. The vanilla strategy screen's own wording for it
/// (`strategy.i18n`'s `build_auto`), so the editor and the screen behind it call
/// the same thing by the same name.
const AI_SLOT_LABEL: &str = "Let Player Decide";

/// Shown for a row whose champion has not been chosen yet. Such a row is kept in
/// the editor but never written.
const NO_CHAMPION_LABEL: &str = "(champion)";

/// Root every runtime UI path hangs off.
///
/// `main` is the name of `strategy.ui`'s root node (`main:strategy_ui`), and
/// `contents` its first child. The bare `contents.…` prefix that appears in the
/// executable's string table is what game code *builds* paths with, one level
/// below the root the query API expects — confirmed by the path probe, which
/// reports `contents.*` absent and `main.contents.*` present.
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

/// The vanilla Personal panel. Its tab is gone, so nothing can bring it back
/// on-screen, but it is still hidden on entry: game code decides what is visible
/// when the screen is first built and may well pick Personal, having no idea its
/// tab no longer exists.
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

/// The Team tab's four columns, shown again when the Save button leaves the
/// Builds tab — the one exit with no vanilla tab click behind it.
const TEAM_PANELS: [&str; 4] = [
    "main.contents.strategy.sub1",
    "main.contents.strategy.sub2",
    "main.contents.strategy.sub3",
    "main.contents.strategy.sub4",
];

/// Parent for the spawned window, and its layout source.
const EDITOR_PARENT: &str = UI_ROOT;
const EDITOR_PATH: &str = "main.contents.build_editor";
const EDITOR_SOURCE: &str = include_str!("../ui/layout/build_editor.ui");

/// Sheet the item icons come from. The mod overrides this asset with its own
/// 640x640 sheet (see `mod.override_info`), so frame names are the mod's.
const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";

// Row geometry, inside the 1314px band `#rows` gives its children. The x offsets
// match `build_editor.ui`'s column headers. The band is the panel minus the
// right-hand column the vanilla Matchup card is left sitting in.
const ROW_WIDTH: u32 = 1306;
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
const CHAMP_W: u32 = 280;
const COMBO_X: [u32; PICKER_SLOTS] = [306, 630, 954];
const COMBO_W: u32 = 280;
const SWAP_X: [u32; PICKER_SLOTS - 1] = [590, 914];
const DELETE_X: u32 = 1250;

/// Sizes of the two floating lists, mirroring `build_editor.ui`. Kept here
/// because the open code has to decide whether a list fits below the control
/// that opened it.
const LIST_W: i32 = 320;
const LIST_H: i32 = 430;
const CHAMP_LIST_W: i32 = 260;

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
    /// How many row nodes are currently spawned, so a rebuild removes exactly
    /// those and no others.
    spawned_rows: usize,
    /// The item list, headers included. Cached for the process lifetime — the
    /// item pool cannot change without a restart.
    entries: Vec<ListEntry>,
    /// The champion list. Cached for the same reason.
    champions: Vec<ChampionChoice>,
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

/// Records one of the mod's final (radiant) items. Called from the registration
/// macros in `lib.rs`.
pub(crate) fn note_final_item(key: &str) {
    if let Ok(mut finals) = MOD_FINALS.lock() {
        finals.push(key.to_string());
    }
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

// -- paths --------------------------------------------------------------

/// Full-screen transparent button shown only while a floating list is open, so
/// that a click anywhere other than the list dismisses it.
///
/// Declared between `#popup` and the two lists, which at equal `z` puts it above
/// the window and below them — a click on a list row reaches the row, a click
/// anywhere else reaches this. Without it a list could only be dismissed by
/// choosing something or by hitting the backdrop outside the window, which left
/// clicks on the window itself doing nothing at all.
const LISTCATCH_PATH: &str = "main.contents.build_editor.listcatch";
const ITEMLIST_PATH: &str = "main.contents.build_editor.itemlist";
const CHAMPLIST_PATH: &str = "main.contents.build_editor.champlist";
/// In the footer beside Save. These paths are matched by exact string, so a
/// button moved between the two bars in `build_editor.ui` must be moved here
/// too — a stale path registers nothing and the control goes quietly dead.
const UNIQUE_PATH: &str = "main.contents.build_editor.popup.footer.unique";
/// In the footer rather than the toolbar: it is the panel's "done" button, and
/// bottom-right is where one is looked for.
const SAVE_PATH: &str = "main.contents.build_editor.popup.footer.save";
/// In the toolbar, above the column headers: adding a row acts on the list
/// below it.
const ADD_PATH: &str = "main.contents.build_editor.popup.toolbar.add";
const ROWS_PATH: &str = "main.contents.build_editor.popup.rowscroll.rows";

fn editor_row_path(row: usize) -> String {
    format!("{ROWS_PATH}.row{row}")
}

fn champ_path(row: usize) -> String {
    format!("{}.champ", editor_row_path(row))
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
    format!("{EDITOR_PATH}.itemlist.list.contents")
}

fn entry_path(index: usize) -> String {
    format!("{}.e{index}", list_contents_path())
}

fn champ_contents_path() -> String {
    format!("{EDITOR_PATH}.champlist.list.contents")
}

fn champ_entry_path(index: usize) -> String {
    format!("{}.c{index}", champ_contents_path())
}

/// Strips characters that would end a `.ui` string literal or be read as markup.
/// Item and player names are plain text, so this only ever guards against a
/// future rename.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '<' | '>' | '{' | '}' | ';'))
        .collect()
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
fn make_choice(ctx: &StableClient<'_>, key: &str) -> ItemChoice {
    let slug = build_config::base_slug(key);
    ItemChoice {
        frame: item_catalog::icon_frame(slug, is_mod_item(key)).map(sanitize),
        category: item_catalog::category_of(slug),
        name: sanitize(&item_display_name(ctx, key, slug)),
        key: sanitize(key),
    }
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
        choices.push(make_choice(ctx, &key));
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
        // `next_tier` empty means nothing upgrades from this item — the same
        // "is this a final item" test the hook's `enforce_unique_items` uses.
        let is_final = value
            .get("next_tier")
            .and_then(|next| next.as_array())
            .is_none_or(|next| next.is_empty());
        if !is_final {
            continue;
        }
        out.push(make_choice(ctx, key));
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
            name: sanitize(&champion_display_name(id)),
            id: sanitize(id),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Readable name for a champion id: `snake_case` -> `Snake Case`.
///
/// Not an i18n lookup, because there is none to do — `champion.i18n` carries
/// `skill_name` and `description` per champion but no display-name map, so the
/// game derives the shown name from the id the same way.
fn champion_display_name(id: &str) -> String {
    if id == "soldier" {
        // The one id whose prettified form is not what the game calls it.
        return "Soldier (Sniper)".to_string();
    }
    id.split('_')
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
        return NO_CHAMPION_LABEL.to_string();
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
            entry.slots.resize(PICKER_SLOTS, None);
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
        Some(key) => (format!("{ICON_PAD}{}", name_of(entries, key)), "#e8e8e8ff"),
        None => (format!("{PLAIN_PAD}{AI_SLOT_LABEL}"), "#a5a5abff"),
    };
    ctx.ui_set_properties(
        &combo_path(row, slot),
        &format!("text: {{ text: \"{}\"; color: {color}; }}", sanitize(&label)),
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

/// Repaints one row: its champion button and its three slot buttons.
fn refresh_row(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize) {
    let rows = snapshot_rows();
    refresh_champ(ctx, &snapshot_champions(), &rows, row);
    for slot in 0..PICKER_SLOTS {
        refresh_combo(ctx, entries, &rows, row, slot);
    }
}

/// Paints the unique-items toggle from the saved setting: accent-green
/// "Enforcing unique items" while on, plain "Enforce unique items" while off —
/// so the state reads off the button itself.
fn refresh_unique(ctx: &mut StableClient<'_>) {
    let (text, color) = if build_config::unique_items_enabled() {
        ("Enforcing unique items", "#60ddc2ff")
    } else {
        ("Enforce unique items", "#d7dbe4ff")
    };
    ctx.ui_set_properties(
        UNIQUE_PATH,
        &format!("text: {{ text: \"{text}\"; color: {color}; }}"),
    );
}

// -- spawning -----------------------------------------------------------

/// `.ui` source for one champion row.
fn row_source(row: usize) -> String {
    let mut source = format!(
        "row{row}:color {{\n\
         width: {ROW_WIDTH}px;\n\
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
         text: \"{NO_CHAMPION_LABEL}\";\n\
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

    for slot in 0..PICKER_SLOTS {
        let x = COMBO_X[slot];
        // The clear button sits inside the right end of its slot, left of the
        // drop arrow, so clearing a slot does not need the list opened first.
        let clear_x = x + COMBO_W - 52;
        source.push_str(&format!(
            "#slot{slot}:color_icon_button {{\n\
             @\"asset/base/style/main#tertiary_button\";\n\
             x: {x}px;\n\
             y: {COMBO_Y}px;\n\
             width: {COMBO_W}px;\n\
             height: {COMBO_H}px;\n\
             \n\
             text: {{\n\
             text: \"{AI_SLOT_LABEL}\";\n\
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

    for (slot, x) in SWAP_X.iter().enumerate() {
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
         x: {DELETE_X}px;\n\
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
    match entry {
        ListEntry::Clear => format!(
            "e{index}:selectable {{\n\
             @\"asset/base/style/main#strategy_option\";\n\
             width: 304px;\n\
             height: {ENTRY_ITEM_H}px;\n\
             label: {{ size: 14; align_x: Left; color: #a5a5abff; }}\n\
             selected_label: {{ size: 14; align_x: Left; color: #0f5b4dff; }}\n\
             text: \"{PLAIN_PAD}{AI_SLOT_LABEL}\";\n\
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
                "e{index}:selectable {{\n\
                 @\"asset/base/style/main#strategy_option\";\n\
                 width: 304px;\n\
                 height: {ENTRY_ITEM_H}px;\n\
                 label: {{ size: 14; align_x: Left; color: #d7dbe4ff; }}\n\
                 selected_label: {{ size: 14; align_x: Left; color: #0f5b4dff; }}\n\
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
        "c{index}:selectable {{\n\
         @\"asset/base/style/main#strategy_option\";\n\
         width: 244px;\n\
         height: {ENTRY_ITEM_H}px;\n\
         label: {{ size: 14; align_x: Left; color: #d7dbe4ff; }}\n\
         selected_label: {{ size: 14; align_x: Left; color: #0f5b4dff; }}\n\
         text: \"{PLAIN_PAD}{}\";\n\
         }}",
        choice.name
    )
}

/// Removes the spawned row nodes and spawns one per row in state, registering
/// every control. Called when the window is built and after any add or delete.
///
/// Rebuilding wholesale rather than splicing keeps row index and node name in
/// step: a row's identity is its position in the list, so deleting row 1 has to
/// renumber everything after it anyway.
fn rebuild_rows(ctx: &mut StableClient<'_>, entries: &[ListEntry]) {
    let previous = with_state(|state| state.spawned_rows).unwrap_or(0);
    for row in 0..previous {
        ctx.ui_remove_node(&editor_row_path(row));
    }

    let count = with_state(|state| state.rows.len()).unwrap_or(0);
    for row in 0..count {
        if !ctx.ui_spawn_source(ROWS_PATH, &row_source(row)) {
            break;
        }
        register_once(ctx, &champ_path(row));
        register_once(ctx, &delete_path(row));
        for slot in 0..PICKER_SLOTS {
            register_once(ctx, &combo_path(row, slot));
            register_once(ctx, &clear_path(row, slot));
        }
        for slot in 0..SWAP_X.len() {
            register_once(ctx, &format!("{}.swap{slot}", editor_row_path(row)));
        }
    }
    let _ = with_state(|state| state.spawned_rows = count);

    // `#rows` is authored `height: auto`, but an auto height measured over a
    // subtree that has not been laid out is zero, and a scroll view whose
    // contents are zero tall shows nothing however many children it has. The
    // height is knowable exactly, so it is stated.
    let height = count as i32 * (ROW_HEIGHT as i32 + ROW_SPACING);
    ctx.ui_set_properties(ROWS_PATH, &format!("height: {height}px;"));

    for row in 0..count {
        refresh_row(ctx, entries, row);
    }
}

/// Spawns the window, its rows and both dropdown lists, and registers every
/// control. Deferred until the first click so a failure costs nothing until the
/// player actually asks for the editor, and is retried on the next click.
fn ensure_editor(ctx: &mut StableClient<'_>) -> bool {
    if with_state(|state| state.modal_ready).unwrap_or(false) && ctx.ui_exists(EDITOR_PATH) {
        return true;
    }

    // Drop any half-built subtree from a previous attempt so this is idempotent.
    ctx.ui_remove_node(EDITOR_PATH);
    if !ctx.ui_spawn_source(EDITOR_PARENT, EDITOR_SOURCE) {
        return false;
    }
    if !ctx.ui_exists(EDITOR_PATH) {
        return false;
    }

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

    // Read from disk here rather than at every open, so a file edited by hand
    // while the game sits on this screen is picked up.
    let _ = with_state(|state| {
        state.rows = build_config::load_champion_rows();
        state.spawned_rows = 0;
    });
    rebuild_rows(ctx, &entries);

    for path in [SAVE_PATH, ADD_PATH, UNIQUE_PATH, LISTCATCH_PATH] {
        register_once(ctx, path);
    }

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

    for panel in CONTENT_PANELS {
        ctx.ui_set_visible(panel, false);
    }
    ctx.ui_set_visible(ITEM_INFO_BTN, true);
    keep_matchup(ctx);

    refresh_unique(ctx);
    let count = with_state(|state| state.rows.len()).unwrap_or(0);
    for row in 0..count {
        refresh_row(ctx, entries, row);
    }
    ctx.ui_set_visible(EDITOR_PATH, true);
    let _ = with_state(|state| state.showing = true);
}

/// Leaves the Builds tab.
///
/// Deliberately restores no panel. This runs off a click on Team, and game
/// code's own handler for that click shows the Team columns — putting them back
/// here would race that. The only exception is [`return_to_team`], which has no
/// such click behind it and so must do the showing itself.
fn close_editor(ctx: &mut StableClient<'_>) {
    close_list(ctx);
    paint_tabs(ctx, false);
    ctx.ui_set_visible(EDITOR_PATH, false);
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
    if !ctx.ui_set_properties(BUILDS_TAB, &tab_style("image", "label", builds_active)) {
    }
    if !ctx.ui_set_properties(
        TEAM_TAB,
        &tab_style("selected_image", "selected_label", !builds_active),
    ) {
    }
}

/// One tab's appearance, through whichever property pair actually renders for
/// it — `image`/`label` for our never-selected tab, `selected_image`/
/// `selected_label` for Team, which game code still considers selected.
fn tab_style(image_key: &str, label_key: &str, lit: bool) -> String {
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

/// Leaves the Builds tab for Team, for the Save button — which has no tab click
/// behind it, so nothing else would make a panel visible again and the content
/// area would be left empty.
fn return_to_team(ctx: &mut StableClient<'_>) {
    // No tab selection to restore: game code has considered Team selected the
    // whole time, and `close_editor` has already repainted it to look it.
    close_editor(ctx);
    for panel in TEAM_PANELS {
        ctx.ui_set_visible(panel, true);
    }
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
    // edge of the window still gets the whole list on screen.
    let left = x.min(CANVAS_W - width - 8).max(8);

    if !ctx.ui_set_properties(panel, &format!("x: {left}px; y: {top}px;")) {
    }
    (left, top)
}

/// Shows the item list under the slot that was clicked, with the slot's current
/// pick ticked.
fn open_item_list(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize, slot: usize) {
    place_list(ctx, ITEMLIST_PATH, &combo_path(row, slot), LIST_W);

    let pinned = pinned_key(&snapshot_rows(), row, slot);
    for (index, entry) in entries.iter().enumerate() {
        let selected = match entry {
            ListEntry::Clear => pinned.is_none(),
            ListEntry::Item(item) => pinned.as_deref() == Some(item.key.as_str()),
            ListEntry::Header(_) => continue,
        };
        ctx.ui_set_selectable_selected(&entry_path(index), selected);
    }

    ctx.ui_set_visible(LISTCATCH_PATH, true);
    ctx.ui_set_visible(ITEMLIST_PATH, true);
    let _ = with_state(|state| state.open_list = Some(OpenList::Item { row, slot }));
}

/// Shows the champion list under the row's champion button, with the row's
/// current champion ticked.
fn open_champ_list(ctx: &mut StableClient<'_>, champions: &[ChampionChoice], row: usize) {
    place_list(ctx, CHAMPLIST_PATH, &champ_path(row), CHAMP_LIST_W);

    let current = snapshot_rows()
        .get(row)
        .and_then(|entry| entry.champion.clone());
    for (index, choice) in champions.iter().enumerate() {
        let selected = current.as_deref() == Some(choice.id.as_str());
        ctx.ui_set_selectable_selected(&champ_entry_path(index), selected);
    }

    ctx.ui_set_visible(LISTCATCH_PATH, true);
    ctx.ui_set_visible(CHAMPLIST_PATH, true);
    let _ = with_state(|state| state.open_list = Some(OpenList::Champion { row }));
}

/// Hides whichever floating list is showing. Both are hidden unconditionally:
/// it costs one call and cannot leave a stale panel behind.
fn close_list(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(ITEMLIST_PATH, false);
    ctx.ui_set_visible(CHAMPLIST_PATH, false);
    ctx.ui_set_visible(LISTCATCH_PATH, false);
    let _ = with_state(|state| state.open_list = None);
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
        // Built on first entry rather than up front, so a strategy screen whose
        // Builds tab is never opened pays nothing for it.
        if !ensure_editor(ctx) {
            return;
        }
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

    if path == LISTCATCH_PATH {
        close_list(ctx);
        return;
    }

    let entries = snapshot_entries();

    if path == SAVE_PATH {
        // Every edit already wrote the file, so there is nothing here to flush —
        // the button is the panel's "done", and leaving the tab is all of what
        // it does.
        return_to_team(ctx);
        return;
    }

    if path == ADD_PATH {
        let _ = with_state(|state| state.rows.push(ChampionRow::default()));
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    if path == UNIQUE_PATH {
        let enabled = !build_config::unique_items_enabled();
        if build_config::set_unique_items(enabled) {
            refresh_unique(ctx);
        } else {
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

    let Some(row) = row_from_path(&path) else {
        return;
    };

    if path.ends_with(".delete") {
        let _ = with_state(|state| {
            (row < state.rows.len()).then(|| state.rows.remove(row).champion)
        })
        .flatten();
        let rows = snapshot_rows();
        build_config::save_champion_rows(&rows);
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        return;
    }

    if let Some(slot) = index_after(&path, ".clear") {
        if slot < PICKER_SLOTS {
            edit_row(row, |entry| entry.slots[slot] = None);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".swap") {
        if slot + 1 < PICKER_SLOTS {
            edit_row(row, |entry| entry.slots.swap(slot, slot + 1));
            close_list(ctx);
            refresh_row(ctx, &entries, row);
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
        if slot >= PICKER_SLOTS {
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

    close_list(ctx);
    refresh_row(ctx, entries, row);
}

pub struct StrategyPicker;

impl StableExtension for StrategyPicker {
    fn on_init(&self, _ctx: &mut StableClient<'_>) {
        // Proves two things at once when it lands in the log: the client
        // extension is registered and being called, and the log file is
        // writable at the path the DLL resolves.
    }

    fn post_update(&self, ctx: &mut StableClient<'_>, _dt_micros: u64) {
        if !ctx.ui_exists(BUILDS_TAB) {
            // Not on the (patched) strategy screen: forget the spawned panel so
            // the next match reinstalls it into the fresh screen.
            let stale = with_state(|state| {
                let stale = state.wired;
                state.wired = false;
                state.modal_ready = false;
                state.spawned_rows = 0;
                state.open_list = None;
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
        }
    }
}
