//! In-game Item Build Editor, reached from the strategy screen's Personal tab.
//!
//! This is the desktop editor (`item-build-editor.ps1` / `.exe`) rebuilt inside
//! the game: one window, one row per position, three item slots per row, with
//! the same palette, the same column layout, the same swap and clear buttons and
//! the same category-grouped item list. It replaces an earlier design that put
//! an "Edit Build" button on each of the five rows and opened a separate picker
//! for each.
//!
//! Layout lives in two places by necessity:
//!
//! - `ui/layout/strategy.ui` is an asset override (see `mod.override_info`). It
//!   adds the single `#build_editor_btn` to the Personal tab's column header.
//!   Nothing else on that screen is touched: the build itself is only ever shown
//!   inside the editor, so the vanilla rows stay exactly as the game drew them.
//! - `ui/layout/build_editor.ui` is the window chrome, compiled in with
//!   `include_str!` and spawned under [`UI_ROOT`]. The five rows and the item
//!   list are spawned from source here, because their contents depend on the
//!   match and on the loaded item pool.
//!
//! Picks are written to `item-builds-strategy.json`, keyed by row index, which
//! `build_config::apply_positions` folds into the hook's routes. The vanilla
//! category dropdowns are left alone, so a player who never opens the editor —
//! or a game update that breaks the override — still gets the stock screen.
//!
//! # Why the window is spawned from source rather than as a template asset
//!
//! `ui_spawn_template` resolves a *registered* asset. `strategy.ui` is
//! registered because `mod.override_info` remaps it over a base asset, but
//! `override_info` can only remap base→mod — it cannot declare a standalone
//! asset — so there is no supported way to make `build_editor` resolvable by
//! path. `ui_spawn_source` takes the same `.ui` grammar as literal text, so the
//! layout is compiled into the DLL and the asset system is left out of it.
//!
//! # Why there is no real dropdown
//!
//! A dropdown's option list is populated by game code: no `.ui` property
//! declares one (the runner parses only styling and layout keys), and the stable
//! ABI's `state_set_json` accepts `checkbox`, `text_edit`, `slider` and
//! `selectable` but not `dropdown`. So each item slot is a `color_icon_button`
//! that opens `#itemlist`, a floating panel of `selectable` rows moved under
//! whichever slot was clicked — which is what a dropdown looks like anyway.
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
//! - `selectable` has no `z` in its property table, so the item rows cannot be
//!   lifted to match. There is no z value that puts the list above the rows.
//!
//! What does work is tree order at equal `z`: a child draws over its parent, and
//! a later sibling over an earlier one. Everything here is therefore at the
//! default `z`, and `#itemlist` wins by being the last child of the window root.
//! The one remaining `z` is on the root itself, which is an `empty` and draws
//! nothing — it is there to sit the whole window above the strategy screen.
//!
//! `ignore_event: true` on the child images is unrelated and still required: it
//! is about hit-testing, not drawing, and without it a child swallows the click
//! meant for the button it sits on.
//!
//! # Handlers leak, boundedly
//!
//! `ui_register_path_events` leaks its closure ("handlers live until process
//! exit") and the strategy screen is rebuilt every match, so the roughly one
//! hundred handlers this registers — chrome, five rows of eight controls, and
//! one per item row — accumulate once per appearance of the screen. Each is a
//! small box and the count per appearance is fixed, so it is a real cost but not
//! a growing one.

use std::sync::Mutex;

use mod_api_stable::*;

use crate::build_config::{self, PICKER_ROWS, PICKER_SLOTS};
use crate::item_catalog;

/// Row order on the Personal tab, which is also the hook's route order.
const POSITION_NAMES: [&str; PICKER_ROWS] = ["Top", "Jungle", "Mid", "Bottom", "Support"];

/// Position icons, parallel to [`POSITION_NAMES`]. Same assets the vanilla rows
/// use, so the editor's rows read as the same five players.
const POSITION_ICONS: [&str; PICKER_ROWS] = ["top", "jungle", "mid", "bottom", "support"];

/// Shown for a slot the player left to the AI. The desktop editor's placeholder,
/// verbatim.
const AI_SLOT_LABEL: &str = "-- AI picks --";

/// Root every runtime UI path hangs off.
///
/// `main` is the name of `strategy.ui`'s root node (`main:strategy_ui`), and
/// `contents` its first child. The bare `contents.…` prefix that appears in the
/// executable's string table is what game code *builds* paths with, one level
/// below the root the query API expects — confirmed by the path probe, which
/// reports `contents.*` absent and `main.contents.*` present.
const UI_ROOT: &str = "main.contents";

/// The one button that opens the editor. Its presence also means the patched
/// strategy screen is live, so it doubles as the screen probe.
const OPEN_BUTTON: &str = "main.contents.strategy.personal.personal_header.build_editor_btn";

/// Parent for the spawned window, and its layout source.
const EDITOR_PARENT: &str = UI_ROOT;
const EDITOR_PATH: &str = "main.contents.build_editor";
const EDITOR_SOURCE: &str = include_str!("../ui/layout/build_editor.ui");

/// Sheet the item icons come from. The mod overrides this asset with its own
/// 640×640 sheet (see `mod.override_info`), so frame names are the mod's.
const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";

// Row geometry, in the same 1330px band `build_editor.ui`'s column headers use.
const ROW_HEIGHT: u32 = 56;
const COMBO_X: [u32; PICKER_SLOTS] = [306, 652, 998];
const COMBO_W: u32 = 300;
const SWAP_X: [u32; PICKER_SLOTS - 1] = [612, 958];

/// Size of the floating item list, mirroring `#itemlist` in `build_editor.ui`.
/// Kept here because the open/close code has to decide whether the list fits
/// below the slot that opened it.
const LIST_W: i32 = 320;
const LIST_H: i32 = 430;

/// Canvas the layouts are authored against; the clamps that keep the item list
/// on screen measure against this.
const CANVAS_W: i32 = 1920;
const CANVAS_H: i32 = 1080;

/// Where the window's own nodes sit, all mirroring `build_editor.ui`.
///
/// The item list is placed from these rather than from `ui_node_rect`, which is
/// the only source of a node's on-screen position and does not say what space it
/// reports in — the first build placed the list from a rect and the list was
/// never seen, exactly what a physical-pixel rect written back as design-space
/// `px` would look like on a display that is not 1920×1080. This arithmetic has
/// no such ambiguity: `#popup` is centred on the canvas at a fixed size, so
/// every slot's canvas position is known without asking.
const POPUP_W: i32 = 1360;
const POPUP_H: i32 = 600;
const POPUP_X: i32 = (CANVAS_W - POPUP_W) / 2;
const POPUP_Y: i32 = (CANVAS_H - POPUP_H) / 2;
const ROWS_X: i32 = 15;
const ROWS_Y: i32 = 188;
/// Row height plus the `#rows` container's `spacing`.
const ROW_PITCH: i32 = ROW_HEIGHT as i32 + 8;
const COMBO_Y: i32 = 8;
const COMBO_H: i32 = 40;

/// Heights of the two kinds of item-list node, shared by [`entry_source`] (which
/// lays them out) and [`entry_height`] (which adds them up).
const ENTRY_HEADER_H: i32 = 26;
const ENTRY_ITEM_H: i32 = 30;

/// Update ticks between path probes while the screen has not been found. Once a
/// second is plenty for a diagnostic that only writes when its answer changes.
const PROBE_INTERVAL: u32 = 60;

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

/// One node in the item list: either a category header (inert) or a pickable
/// item. Both occupy an index so a click path resolves straight into this list.
#[derive(Clone)]
enum ListEntry {
    Header(&'static str),
    Item(ItemChoice),
}

#[derive(Default)]
struct EditorState {
    /// Whether the open button has been wired for the current screen.
    wired: bool,
    /// Whether the window subtree is spawned and its controls registered.
    modal_ready: bool,
    /// Whether the window is showing.
    open: bool,
    /// Row and slot whose item list is open, if any.
    open_slot: Option<(usize, usize)>,
    /// The item list, headers included. Cached for the process lifetime — the
    /// item pool cannot change without a restart.
    entries: Vec<ListEntry>,
    probe_tick: u32,
}

static STATE: Mutex<Option<EditorState>> = Mutex::new(None);

/// Last line written by [`diag`]. Deliberately a separate lock from [`STATE`]:
/// `diag` is called from inside functions that also touch the state, and
/// `Mutex` is not reentrant.
static LAST_NOTE: Mutex<String> = Mutex::new(String::new());

/// Runs `f` against the editor state, initializing it on first use. Returns
/// `None` if the lock is poisoned, which disables the editor rather than
/// panicking across the FFI boundary.
///
/// Never call [`diag`] from inside `f`.
fn with_state<T>(f: impl FnOnce(&mut EditorState) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(EditorState::default)))
}

/// Appends a line to the mod log, skipping it when it repeats the previous
/// line — `post_update` runs every frame, so an undeduplicated note would be
/// written sixty times a second.
///
/// This is the only diagnostic channel available here: `StableHost` (which owns
/// `log`) is documented as valid only inside the callback that receives it, and
/// the extension is never handed one.
fn diag(msg: &str) {
    let Ok(mut last) = LAST_NOTE.lock() else {
        return;
    };
    if *last == msg {
        return;
    }
    last.clear();
    last.push_str(msg);

    crate::diag::write(msg);
}

/// Clears the [`diag`] dedup so the next visit to the screen logs its progress
/// again instead of being swallowed as a repeat of the last visit's last line.
fn reset_diag() {
    if let Ok(mut last) = LAST_NOTE.lock() {
        last.clear();
    }
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

fn is_mod_item(key: &str) -> bool {
    MOD_FINALS
        .lock()
        .map(|finals| finals.iter().any(|final_key| final_key == key))
        .unwrap_or(false)
}

// -- paths --------------------------------------------------------------

fn row_name_path(row: usize) -> String {
    format!("{UI_ROOT}.strategy.personal.row{row}.name")
}

fn editor_row_path(row: usize) -> String {
    format!("{EDITOR_PATH}.popup.rows.row{row}")
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

fn list_contents_path() -> String {
    format!("{EDITOR_PATH}.itemlist.list.contents")
}

fn entry_path(index: usize) -> String {
    format!("{}.e{index}", list_contents_path())
}

const ITEMLIST_PATH: &str = "main.contents.build_editor.itemlist";
const STATUS_PATH: &str = "main.contents.build_editor.popup.toolbar.status";
const VERSION_PATH: &str = "main.contents.build_editor.popup.footer.version";
const UNIQUE_PATH: &str = "main.contents.build_editor.popup.optionbar.unique";
const SAVE_PATH: &str = "main.contents.build_editor.popup.toolbar.save";
const FADE_PATH: &str = "main.contents.build_editor.fade";
const CANCEL_PATH: &str = "main.contents.build_editor.popup.titlebar.cancel";
const ROWS_PATH: &str = "main.contents.build_editor.popup.rows";

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
/// grouped into the desktop editor's categories and sorted by name within each.
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

    // Category first, then name: exactly the desktop editor's dropdown order.
    choices.sort_by(|a, b| {
        item_catalog::category_rank(a.category)
            .cmp(&item_catalog::category_rank(b.category))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut entries = Vec::with_capacity(choices.len() + item_catalog::CATEGORY_ORDER.len() + 1);
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
    let name = ctx
        // The `#` prefix is part of the key: the probe found this spelling
        // resolves ("Radiant Bloodthirster") while the bare path returns
        // nothing. Same form the `.ui` text properties use.
        .i18n(&format!("#asset/base/text/item?{key}.name"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| key.to_string());
    let slug = build_config::base_slug(key);
    ItemChoice {
        frame: item_catalog::icon_frame(slug, is_mod_item(key)).map(sanitize),
        category: item_catalog::category_of(slug),
        key: sanitize(key),
        name: sanitize(&name),
    }
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
    if loaded.is_empty() {
        diag("item settings unreadable from the client — item list is empty");
    } else {
        let items = loaded
            .iter()
            .filter(|entry| matches!(entry, ListEntry::Item(_)))
            .count();
        diag(&format!("loaded {items} final items"));
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

// -- painting -----------------------------------------------------------

/// The pinned key for one slot, or `None` when the AI owns it.
fn pinned_key(row: usize, slot: usize) -> Option<String> {
    build_config::load_position_builds()
        .get(&row.to_string())
        .and_then(|build| build.get(slot))
        .and_then(Option::as_ref)
        .cloned()
}

/// Repaints one slot's button: its label, its icon, and whether its clear button
/// is offered at all — an unpinned slot has nothing to clear, so its X is hidden
/// exactly as in the desktop editor.
fn refresh_combo(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize, slot: usize) {
    let pinned = pinned_key(row, slot);
    let combo = combo_path(row, slot);

    let (label, color) = match &pinned {
        Some(key) => (name_of(entries, key), "#e8e8e8ff"),
        None => (AI_SLOT_LABEL.to_string(), "#a5a5abff"),
    };
    ctx.ui_set_properties(
        &combo,
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

/// Repaints one row's three slot buttons from its saved build.
fn refresh_row(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize) {
    for slot in 0..PICKER_SLOTS {
        refresh_combo(ctx, entries, row, slot);
    }
}

fn set_status(ctx: &mut StableClient<'_>, text: &str) {
    ctx.ui_set_text(STATUS_PATH, text);
}

/// Paints the unique-items toggle from the saved setting: accent-green
/// "Enforcing unique items" while on, plain "Enforce unique items" while off —
/// the desktop editor's checkbox states, in a button.
fn refresh_unique(ctx: &mut StableClient<'_>) {
    let (text, color) = if build_config::unique_items_enabled() {
        ("Enforcing unique items", "#60ddc2ff")
    } else {
        ("Enforce unique items", "#d7dbe4ff")
    };
    ctx.ui_set_properties(UNIQUE_PATH, &format!("text: {{ text: \"{text}\"; color: {color}; }}"));
}

// -- spawning -----------------------------------------------------------

/// `.ui` source for one position row. Everything that varies between rows is
/// substituted here; the column offsets match `build_editor.ui`'s headers.
fn row_source(row: usize, player: &str) -> String {
    let position = POSITION_NAMES[row];
    let icon = POSITION_ICONS[row];
    let name = if player.is_empty() {
        position.to_string()
    } else {
        format!("{position}  ·  {player}")
    };

    let mut source = format!(
        "row{row}:color {{\n\
         width: 1330px;\n\
         height: {ROW_HEIGHT}px;\n\
         color: #1d1f2cff;\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         #position:image {{\n\
         x: 8px;\n\
         anchor_y: 0.5;\n\
         pivot_y: 0.5;\n\
         width: 26px;\n\
         height: 26px;\n\
         source: \"asset/base/ui/icons/{icon}\";\n\
         }}\n\
         \n\
         #name:label {{\n\
         @\"asset/base/style/main#label\";\n\
         x: 44px;\n\
         width: 254px;\n\
         height: {ROW_HEIGHT}px;\n\
         align_y: Center;\n\
         size: 15;\n\
         color: #e8e8e8ff;\n\
         text: \"{name}\";\n\
         }}\n"
    );

    for slot in 0..PICKER_SLOTS {
        let x = COMBO_X[slot];
        // The clear button sits inside the right end of its slot, left of the
        // drop arrow, the way the desktop editor overlays its X on the combo.
        let clear_x = x + COMBO_W - 52;
        source.push_str(&format!(
            "#slot{slot}:color_icon_button {{\n\
             @\"asset/base/style/main#tertiary_button\";\n\
             x: {x}px;\n\
             y: 8px;\n\
             width: {COMBO_W}px;\n\
             height: 40px;\n\
             \n\
             text: {{\n\
             text: \"{AI_SLOT_LABEL}\";\n\
             align_x: Center;\n\
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
             y: 17px;\n\
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
             y: 8px;\n\
             width: 34px;\n\
             height: 40px;\n\
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

    source.push_str("}\n");
    source
}

/// Height of one item-list node. Paired with [`entry_source`], which lays the
/// node out at exactly this height; the two are read together to give `#contents`
/// an explicit height.
fn entry_height(entry: &ListEntry) -> i32 {
    match entry {
        ListEntry::Header(_) => ENTRY_HEADER_H,
        ListEntry::Item(_) => ENTRY_ITEM_H,
    }
}

/// `.ui` source for one item-list node: a category header, or a pickable item
/// with its sheet icon.
///
/// The item node is deliberately the shape the earlier picker used and proved
/// renders — style ref, size, `label`/`selected_label` overrides, `text` — with
/// nothing added but the icon child and explicit label colors. An attempt at the
/// desktop editor's left-aligned rows using `text_offset` is not repeated here:
/// the whole list came up blank on the build that had it, and while the cause
/// turned out to be the panel painting over its children, `text_offset` appears
/// in no shipped layout and is not worth carrying as a second unknown. The style
/// centres its label, so the name sits centred with the icon at the left edge.
fn entry_source(index: usize, entry: &ListEntry) -> String {
    match entry {
        ListEntry::Header(name) => format!(
            "e{index}:label {{\n\
             @\"asset/base/style/main#bold_label\";\n\
             width: 304px;\n\
             height: {ENTRY_HEADER_H}px;\n\
             align_x: Center;\n\
             align_y: Center;\n\
             size: 13;\n\
             color: #a5a5abff;\n\
             text: \"{name}\";\n\
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
            format!(
                "e{index}:selectable {{\n\
                 @\"asset/base/style/main#strategy_option\";\n\
                 width: 304px;\n\
                 height: {ENTRY_ITEM_H}px;\n\
                 label: {{ size: 14; color: #d7dbe4ff; }}\n\
                 selected_label: {{ size: 14; color: #0f5b4dff; }}\n\
                 text: \"{}\";\n\
                 {icon}\
                 }}",
                item.name
            )
        }
    }
}

/// Spawns the window, its five rows and its item list, and registers every
/// control. Deferred until the first click so a failure costs nothing until the
/// player actually asks for the editor, and is retried on the next click.
fn ensure_editor(ctx: &mut StableClient<'_>) -> bool {
    if with_state(|state| state.modal_ready).unwrap_or(false) && ctx.ui_exists(EDITOR_PATH) {
        return true;
    }

    // Drop any half-built subtree from a previous attempt so this is idempotent.
    ctx.ui_remove_node(EDITOR_PATH);
    if !ctx.ui_spawn_source(EDITOR_PARENT, EDITOR_SOURCE) {
        diag("ui_spawn_source refused the build_editor layout");
        return false;
    }
    if !ctx.ui_exists(EDITOR_PATH) {
        diag(&format!("build_editor spawned but {EDITOR_PATH} does not exist"));
        return false;
    }

    let entries = cached_entries(ctx);

    for row in 0..PICKER_ROWS {
        // The vanilla row's name label carries whoever is in that lane this
        // match; an empty read just leaves the row labelled by position.
        let player = ctx.ui_text(&row_name_path(row)).unwrap_or_default();
        if !ctx.ui_spawn_source(ROWS_PATH, &row_source(row, &sanitize(&player))) {
            diag(&format!("row {row} source refused under {ROWS_PATH}"));
            return false;
        }
        for slot in 0..PICKER_SLOTS {
            ctx.ui_register_path_events(&combo_path(row, slot), handle_event);
            ctx.ui_register_path_events(&clear_path(row, slot), handle_event);
        }
        for slot in 0..SWAP_X.len() {
            ctx.ui_register_path_events(
                &format!("{}.swap{slot}", editor_row_path(row)),
                handle_event,
            );
        }
    }

    let contents = list_contents_path();
    for (index, entry) in entries.iter().enumerate() {
        if !ctx.ui_spawn_source(&contents, &entry_source(index, entry)) {
            diag(&format!("item list source refused under {contents}"));
            break;
        }
        if matches!(entry, ListEntry::Item(_)) {
            ctx.ui_register_path_events(&entry_path(index), handle_event);
        }
    }

    // `#contents` is authored `height: auto`, but these children are added while
    // the list is still hidden, and an auto height measured over a subtree that
    // has never been laid out is zero — a scroll view whose contents are zero
    // tall shows nothing however many children it has. The height is knowable
    // exactly, so it is stated rather than inferred.
    let content_height: i32 = entries.iter().map(entry_height).sum();
    ctx.ui_set_properties(&contents, &format!("height: {content_height}px;"));

    for path in [FADE_PATH, CANCEL_PATH, SAVE_PATH, UNIQUE_PATH] {
        ctx.ui_register_path_events(path, handle_event);
    }

    ctx.ui_set_text(
        VERSION_PATH,
        &format!("Riot Items  v{}", env!("CARGO_PKG_VERSION")),
    );

    let _ = with_state(|state| state.modal_ready = true);
    diag("item build editor ready");
    true
}

// -- interaction --------------------------------------------------------

fn open_editor(ctx: &mut StableClient<'_>, entries: &[ListEntry]) {
    refresh_unique(ctx);
    for row in 0..PICKER_ROWS {
        refresh_row(ctx, entries, row);
    }
    set_status(ctx, "Every change saves as you make it.");
    ctx.ui_set_visible(EDITOR_PATH, true);
    let _ = with_state(|state| state.open = true);
}

fn close_editor(ctx: &mut StableClient<'_>) {
    close_list(ctx);
    ctx.ui_set_visible(EDITOR_PATH, false);
    let _ = with_state(|state| state.open = false);
}

/// Reports where the item list was put and what is actually inside it.
///
/// Kept after the blank-list hunt as the cheapest way to tell the three failure
/// modes apart, since they look identical on screen: nodes missing (child count
/// zero), panel misplaced (its rect nowhere near the computed position), or
/// nodes present and placed but not drawn — which is what it turned out to be,
/// the panel's `z` putting its own fill above its children.
///
/// It also prints the engine's rect for the clicked slot beside the computed
/// one. Placement no longer depends on that rect, but the two numbers side by
/// side are the only evidence available for which space `ui_node_rect` reports
/// in, and the next thing to need a node's position will want to know.
fn probe_list(ctx: &StableClient<'_>, row: usize, slot: usize, left: i32, top: i32) {
    let contents = list_contents_path();
    let first = entry_path(0);
    diag(&format!(
        "item list for row {row} slot {slot}: placed at ({left}, {top}), \
         engine rect of slot = {:?}, list rect = {:?}, contents children = {:?}, \
         e0 exists = {}, e0 runner = {:?}, e0 rect = {:?}",
        ctx.ui_node_rect(&combo_path(row, slot)),
        ctx.ui_node_rect(ITEMLIST_PATH),
        ctx.ui_child_count(&contents),
        ctx.ui_exists(&first),
        ctx.ui_runner_name(&first),
        ctx.ui_node_rect(&first),
    ));
}

/// Shows the item list under the slot that was clicked, with the slot's current
/// pick ticked. Flips above the slot when there is no room below, the way a
/// dropdown near the bottom of the screen does.
fn open_list(ctx: &mut StableClient<'_>, entries: &[ListEntry], row: usize, slot: usize) {
    // Canvas position of the clicked slot, from the layout constants rather than
    // from the engine — see the note on POPUP_X.
    let x = POPUP_X + ROWS_X + COMBO_X[slot] as i32;
    let y = POPUP_Y + ROWS_Y + row as i32 * ROW_PITCH + COMBO_Y;

    let below = y + COMBO_H + 4;
    let top = if below + LIST_H <= CANVAS_H - 8 {
        below
    } else {
        (y - LIST_H - 4).max(8)
    };
    // Nudged left rather than pinned to the slot's own x, so a slot at the right
    // edge of the window still gets the whole list on screen.
    let left = x.min(CANVAS_W - LIST_W - 8).max(8);

    if !ctx.ui_set_properties(ITEMLIST_PATH, &format!("x: {left}px; y: {top}px;")) {
        diag("ui_set_properties refused x/y on the item list");
    }

    probe_list(ctx, row, slot, left, top);

    let pinned = pinned_key(row, slot);
    for (index, entry) in entries.iter().enumerate() {
        if let ListEntry::Item(item) = entry {
            let selected = pinned.as_deref() == Some(item.key.as_str());
            ctx.ui_set_selectable_selected(&entry_path(index), selected);
        }
    }

    ctx.ui_set_visible(ITEMLIST_PATH, true);
    let _ = with_state(|state| state.open_slot = Some((row, slot)));
}

fn close_list(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(ITEMLIST_PATH, false);
    let _ = with_state(|state| state.open_slot = None);
}

/// Row index from an event path (`….rows.row2.slot1` → `2`).
fn row_from_path(path: &str) -> Option<usize> {
    path.rsplit_once(".row")
        .and_then(|(_, rest)| rest.split('.').next())
        .and_then(|digits| digits.parse::<usize>().ok())
        .filter(|row| *row < PICKER_ROWS)
}

/// Trailing index of a `slotN` / `clearN` / `swapN` / `eN` node name.
fn index_after(path: &str, prefix: &str) -> Option<usize> {
    path.rsplit_once(prefix)?.1.parse().ok()
}

/// Single handler for every registered control; dispatches on the firing path.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    let path = event.path.clone();

    if path.ends_with("build_editor_btn") || event.payload_json.contains("build_editor_btn") {
        if !ensure_editor(ctx) {
            return;
        }
        let entries = snapshot_entries();
        open_editor(ctx, &entries);
        return;
    }

    if path == FADE_PATH || path == CANCEL_PATH {
        // A click on the backdrop while the item list is open dismisses just the
        // list, so picking is escapable without closing the whole editor.
        if with_state(|state| state.open_slot).flatten().is_some() && path == FADE_PATH {
            close_list(ctx);
        } else {
            close_editor(ctx);
        }
        return;
    }

    let entries = snapshot_entries();

    if path == SAVE_PATH {
        // Every edit already wrote the file; this reports that, which is what
        // the desktop editor's Save button amounts to as well.
        let rows = build_config::load_position_builds().len();
        set_status(ctx, &format!("Saved {rows} position build(s)."));
        return;
    }

    if path == UNIQUE_PATH {
        let enabled = !build_config::unique_items_enabled();
        if build_config::set_unique_items(enabled) {
            refresh_unique(ctx);
            set_status(
                ctx,
                if enabled {
                    "Unique item builds enforced - duplicates get replaced with same-category items."
                } else {
                    "Unique enforcement off - champions may build duplicate items."
                },
            );
        } else {
            set_status(ctx, "Could not write mod-settings.json.");
        }
        return;
    }

    if path.starts_with(&list_contents_path()) {
        let Some(index) = index_after(&path, ".e") else {
            return;
        };
        pick_item(ctx, &entries, index);
        return;
    }

    let Some(row) = row_from_path(&path) else {
        return;
    };

    if let Some(slot) = index_after(&path, ".clear") {
        if slot < PICKER_SLOTS {
            build_config::set_position_slot(row, slot, None);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
            set_status(ctx, &format!("{} item {} left to the AI.", POSITION_NAMES[row], slot + 1));
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".swap") {
        if slot + 1 < PICKER_SLOTS {
            build_config::swap_position_slots(row, slot, slot + 1);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
            set_status(ctx, &format!("{} items {} and {} swapped.", POSITION_NAMES[row], slot + 1, slot + 2));
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".slot") {
        if slot >= PICKER_SLOTS {
            return;
        }
        // Clicking the open slot again closes its list, so the button toggles.
        if with_state(|state| state.open_slot) == Some(Some((row, slot))) {
            close_list(ctx);
        } else {
            open_list(ctx, &entries, row, slot);
        }
    }
}

/// Commits a clicked item row: clicking the pinned item again clears the slot,
/// so a slot can be returned to AI choice without reaching for the X.
fn pick_item(ctx: &mut StableClient<'_>, entries: &[ListEntry], index: usize) {
    let Some(Some((row, slot))) = with_state(|state| state.open_slot) else {
        return;
    };
    let Some(ListEntry::Item(item)) = entries.get(index) else {
        return;
    };

    let already = pinned_key(row, slot).as_deref() == Some(item.key.as_str());
    let picked = (!already).then_some(item.key.as_str());
    build_config::set_position_slot(row, slot, picked);

    close_list(ctx);
    refresh_row(ctx, entries, row);
    set_status(
        ctx,
        &format!(
            "{} item {} -> {}",
            POSITION_NAMES[row],
            slot + 1,
            picked.map_or(AI_SLOT_LABEL, |_| item.name.as_str())
        ),
    );
}

pub struct StrategyPicker;

impl StableExtension for StrategyPicker {
    fn on_init(&self, _ctx: &mut StableClient<'_>) {
        // Proves two things at once when it lands in the log: the client
        // extension is registered and being called, and the log file is
        // writable at the path the DLL resolves.
        diag("client extension initialised");
        diag(&format!(
            "paths: dll_dir={:?} cwd={:?} using={:?}",
            crate::config::dll_dir(),
            std::env::current_dir().ok(),
            crate::config::mod_dir()
        ));
    }

    fn post_update(&self, ctx: &mut StableClient<'_>, _dt_micros: u64) {
        // Reports the call-counting probe (a no-op unless `hook-probe.json`
        // exists). Runs here because it is the only per-frame client hook the
        // mod has, and the counts are incremented on the server side.
        crate::probe::report_changes();

        if !ctx.ui_exists(OPEN_BUTTON) {
            // Not on the (patched) strategy screen: forget the spawned window so
            // the next match reinstalls it into the fresh screen.
            let stale = with_state(|state| {
                let stale = state.wired;
                state.wired = false;
                state.modal_ready = false;
                state.open = false;
                state.open_slot = None;
                stale
            })
            .unwrap_or(false);
            if stale {
                // Leaving the strategy screen means the match is about to be
                // played: this is the closing bracket of the probe measurement.
                crate::probe::snapshot("after-strategy");
                reset_diag();
            }

            // Keep probing while the screen has not been found. `diag`
            // suppresses repeats, so this writes a line only when the answer
            // changes. Runs after the reset above so `reset_diag` cannot clear
            // the dedup between two identical results and log the same line
            // twice.
            let due = with_state(|state| {
                state.probe_tick = state.probe_tick.wrapping_add(1);
                state.probe_tick % PROBE_INTERVAL == 0
            })
            .unwrap_or(false);
            if due {
                diag("waiting for the patched strategy screen");
            }
            return;
        }

        if !with_state(|state| state.wired).unwrap_or(true) {
            diag("patched strategy screen detected — wiring the editor button");
            // Opening bracket of the probe measurement, before the match runs.
            crate::probe::snapshot("at-strategy");

            // Loaded here, in `post_update`, not from a click handler:
            // `setting_get_json` returned None when called inside one, matching
            // the trait's note that only ui/asset calls are live there.
            cached_entries(ctx);
            if !ctx.ui_register_path_events(OPEN_BUTTON, handle_event) {
                diag(&format!("event registration refused for {OPEN_BUTTON}"));
            }
            let _ = with_state(|state| state.wired = true);
        }
    }
}
