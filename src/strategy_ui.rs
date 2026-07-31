//! In-game Item Build Editor, reached from the strategy screen's Personal tab.
//!
//! One row per champion, three item slots per row, a category-grouped item list,
//! and swap/clear buttons per row. It reads and writes `item-builds.json` next
//! to the DLL, which is the same file the hook applies — so a build takes effect
//! on the next match with no restart.
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
//!   adds the single `#build_editor_btn` to the Personal tab's column header.
//!   Nothing else on that screen is touched.
//! - `ui/layout/build_editor.ui` is the window chrome, compiled in with
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
//! # Handlers leak, boundedly
//!
//! `ui_register_path_events` leaks its closure ("handlers live until process
//! exit") and the strategy screen is rebuilt every match, so the handlers this
//! registers — chrome, ten per champion row, one per list row — accumulate once
//! per appearance of the screen, and again whenever a row is added or removed
//! (which respawns the row list). Each is a small box and the count is bounded
//! by the number of configured champions, so it is a real cost but not a growing
//! one.

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

/// The one button that opens the editor. Its presence also means the patched
/// strategy screen is live, so it doubles as the screen probe.
const OPEN_BUTTON: &str = "main.contents.strategy.personal.personal_header.build_editor_btn";

/// Parent for the spawned window, and its layout source.
const EDITOR_PARENT: &str = UI_ROOT;
const EDITOR_PATH: &str = "main.contents.build_editor";
const EDITOR_SOURCE: &str = include_str!("../ui/layout/build_editor.ui");

/// Sheet the item icons come from. The mod overrides this asset with its own
/// 640x640 sheet (see `mod.override_info`), so frame names are the mod's.
const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";

// Row geometry, inside the 1310px band `#rows` gives its children. The x offsets
// match `build_editor.ui`'s column headers.
const ROW_HEIGHT: u32 = 56;
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
    /// Whether the open button has been wired for the current screen.
    wired: bool,
    /// Whether the window subtree is spawned and its controls registered.
    modal_ready: bool,
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
const STATUS_PATH: &str = "main.contents.build_editor.popup.toolbar.status";
const VERSION_PATH: &str = "main.contents.build_editor.popup.footer.version";
const UNIQUE_PATH: &str = "main.contents.build_editor.popup.optionbar.unique";
const SAVE_PATH: &str = "main.contents.build_editor.popup.toolbar.save";
const ADD_PATH: &str = "main.contents.build_editor.popup.toolbar.add";
const FADE_PATH: &str = "main.contents.build_editor.fade";
const CANCEL_PATH: &str = "main.contents.build_editor.popup.titlebar.cancel";
const ROWS_PATH: &str = "main.contents.build_editor.popup.rowscroll.rows";

/// The strategy screen itself, hidden while the editor is open. `ui_exists` is
/// unaffected by visibility, so `post_update` still finds `OPEN_BUTTON` and does
/// not mistake a hidden screen for having left it.
const STRATEGY_PATH: &str = "main.contents.strategy";

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
        diag("item settings unreadable from the client — item list is empty");
    } else {
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
        diag(
            "no champions to offer — champion_names() gave nothing and the hook has \
             not recorded a roster yet, so no match has simulated this session",
        );
    } else {
        diag(&format!("loaded {} champions", loaded.len()));
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

fn set_status(ctx: &mut StableClient<'_>, text: &str) {
    ctx.ui_set_text(STATUS_PATH, text);
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
         width: 1310px;\n\
         height: {ROW_HEIGHT}px;\n\
         color: #1d1f2cff;\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         #champ:color_icon_button {{\n\
         @\"asset/base/style/main#tertiary_button\";\n\
         x: {CHAMP_X}px;\n\
         y: 8px;\n\
         width: {CHAMP_W}px;\n\
         height: 40px;\n\
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
             y: 8px;\n\
             width: {COMBO_W}px;\n\
             height: 40px;\n\
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

    source.push_str(&format!(
        "#delete:color_icon_button {{\n\
         x: {DELETE_X}px;\n\
         y: 17px;\n\
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
            diag(&format!("row {row} source refused under {ROWS_PATH}"));
            break;
        }
        ctx.ui_register_path_events(&champ_path(row), handle_event);
        ctx.ui_register_path_events(&delete_path(row), handle_event);
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
    let _ = with_state(|state| state.spawned_rows = count);

    // `#rows` is authored `height: auto`, but an auto height measured over a
    // subtree that has not been laid out is zero, and a scroll view whose
    // contents are zero tall shows nothing however many children it has. The
    // height is knowable exactly, so it is stated.
    let height = count as i32 * (ROW_HEIGHT as i32 + 8);
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
        diag("ui_spawn_source refused the build_editor layout");
        return false;
    }
    if !ctx.ui_exists(EDITOR_PATH) {
        diag(&format!("build_editor spawned but {EDITOR_PATH} does not exist"));
        return false;
    }

    let entries = cached_entries(ctx);
    let champions = cached_champions(ctx);

    let contents = list_contents_path();
    for (index, entry) in entries.iter().enumerate() {
        if !ctx.ui_spawn_source(&contents, &entry_source(index, entry)) {
            diag(&format!("item list source refused under {contents}"));
            break;
        }
        if !matches!(entry, ListEntry::Header(_)) {
            ctx.ui_register_path_events(&entry_path(index), handle_event);
        }
    }
    let content_height: i32 = entries.iter().map(entry_height).sum();
    ctx.ui_set_properties(&contents, &format!("height: {content_height}px;"));

    let champ_contents = champ_contents_path();
    for (index, choice) in champions.iter().enumerate() {
        if !ctx.ui_spawn_source(&champ_contents, &champ_entry_source(index, choice)) {
            diag(&format!("champion list source refused under {champ_contents}"));
            break;
        }
        ctx.ui_register_path_events(&champ_entry_path(index), handle_event);
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

    for path in [FADE_PATH, CANCEL_PATH, SAVE_PATH, ADD_PATH, UNIQUE_PATH, LISTCATCH_PATH] {
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
    let count = with_state(|state| state.rows.len()).unwrap_or(0);
    for row in 0..count {
        refresh_row(ctx, entries, row);
    }
    set_status(ctx, "Every change saves as you make it.");
    // Hidden, not just covered. The screen's champion tooltip is spawned by game
    // code somewhere outside this subtree — `strategy.ui` contains no tooltip
    // node at all — so it is not something a panel of ours can be drawn over,
    // and the full-screen `#fade` does not stop the hover that summons it. With
    // the screen hidden there is nothing left to hover.
    ctx.ui_set_visible(STRATEGY_PATH, false);
    ctx.ui_set_visible(EDITOR_PATH, true);
}

fn close_editor(ctx: &mut StableClient<'_>) {
    close_list(ctx);
    ctx.ui_set_visible(EDITOR_PATH, false);
    ctx.ui_set_visible(STRATEGY_PATH, true);
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
        diag(&format!("no layout rect for {anchor} — leaving the list where it was"));
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
        diag(&format!("ui_set_properties refused x/y on {panel}"));
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
    let path = event.path.clone();

    if path.ends_with("build_editor_btn") || event.payload_json.contains("build_editor_btn") {
        if !ensure_editor(ctx) {
            return;
        }
        let entries = snapshot_entries();
        open_editor(ctx, &entries);
        return;
    }

    if path == LISTCATCH_PATH {
        close_list(ctx);
        return;
    }

    if path == FADE_PATH || path == CANCEL_PATH {
        // A click on the backdrop while a list is open dismisses just the list,
        // so picking is escapable without closing the whole editor.
        if with_state(|state| state.open_list).flatten().is_some() && path == FADE_PATH {
            close_list(ctx);
        } else {
            close_editor(ctx);
        }
        return;
    }

    let entries = snapshot_entries();

    if path == SAVE_PATH {
        // Every edit already wrote the file; this just reports what is on disk,
        // for the reassurance of having pressed something.
        let saved = snapshot_rows().iter().filter(|row| row.is_complete()).count();
        set_status(ctx, &format!("Saved {saved} champion build(s)."));
        return;
    }

    if path == ADD_PATH {
        let _ = with_state(|state| state.rows.push(ChampionRow::default()));
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        set_status(ctx, "Pick a champion for the new row — a row without one is not saved.");
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
        let removed = with_state(|state| {
            (row < state.rows.len()).then(|| state.rows.remove(row).champion)
        })
        .flatten();
        let rows = snapshot_rows();
        build_config::save_champion_rows(&rows);
        close_list(ctx);
        rebuild_rows(ctx, &entries);
        let champions = snapshot_champions();
        set_status(
            ctx,
            &format!(
                "Removed the build for {}.",
                champion_label(&champions, removed.flatten().as_deref())
            ),
        );
        return;
    }

    if let Some(slot) = index_after(&path, ".clear") {
        if slot < PICKER_SLOTS {
            edit_row(row, |entry| entry.slots[slot] = None);
            close_list(ctx);
            refresh_row(ctx, &entries, row);
            set_status(ctx, &format!("Item {} left to the AI.", slot + 1));
        }
        return;
    }

    if let Some(slot) = index_after(&path, ".swap") {
        if slot + 1 < PICKER_SLOTS {
            edit_row(row, |entry| entry.slots.swap(slot, slot + 1));
            close_list(ctx);
            refresh_row(ctx, &entries, row);
            set_status(ctx, &format!("Items {} and {} swapped.", slot + 1, slot + 2));
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
    let name = picked.as_deref().map(|key| name_of(entries, key));
    let wrote = edit_row(row, |entry| entry.slots[slot] = picked.clone());

    close_list(ctx);
    refresh_row(ctx, entries, row);
    set_status(
        ctx,
        &match (wrote, name) {
            (false, _) => "Could not write item-builds.json.".to_string(),
            (true, Some(name)) => format!("Item {} -> {name}", slot + 1),
            (true, None) => format!("Item {} -> {AI_SLOT_LABEL}", slot + 1),
        },
    );
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
    let wrote = edit_row(row, |entry| entry.champion = picked.clone());

    close_list(ctx);
    refresh_row(ctx, entries, row);
    set_status(
        ctx,
        &match (wrote, &picked) {
            (false, _) => "Could not write item-builds.json.".to_string(),
            (true, Some(_)) => format!("Build set for {}.", choice.name),
            (true, None) => "Row has no champion, so it is not saved.".to_string(),
        },
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
                state.spawned_rows = 0;
                state.open_list = None;
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
            cached_champions(ctx);
            if !ctx.ui_register_path_events(OPEN_BUTTON, handle_event) {
                diag(&format!("event registration refused for {OPEN_BUTTON}"));
            }
            let _ = with_state(|state| state.wired = true);
        }
    }
}
