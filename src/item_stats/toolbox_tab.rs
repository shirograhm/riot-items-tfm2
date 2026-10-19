//! Turns Bows' Drafter's Toolbox's Advanced Stats button into a fifth tab
//! beside Item Stats, so it stops sitting on the Item Stats dropdowns. A soft
//! dependency: nothing happens unless [`enabled`].
//!
//! The Toolbox respawns its button every tenth frame and cannot set the
//! engine's `selected` state, so this narrows the tabs to fit five, holds the
//! bar's shape between respawns, paints the highlights and makes the button
//! behave like a tab instead of a toggle.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use mod_api_stable::*;

use crate::strategy_ui::tab_style;

// -- the screen (the Toolbox hard-codes these paths) ------------------------

const SCREEN: &str = "main.top.right.statistics";
const BAR: &str = "main.top.right.statistics.tabs";
const GAME_TABS: [&str; 4] = [
    "main.top.right.statistics.tabs.champion",
    "main.top.right.statistics.tabs.athlete",
    "main.top.right.statistics.tabs.team",
    "main.top.right.statistics.tabs.item",
];
// The game's own three tabs: the ones game code can consider selected.
const VANILLA_TABS: [&str; 3] = [GAME_TABS[0], GAME_TABS[1], GAME_TABS[2]];
const ITEM_TAB: &str = "main.top.right.statistics.tabs.item";
const ITEM_PANEL: &str = "main.top.right.statistics.data.item_stats";
const ADV_TAB: &str = "main.top.right.statistics.drafters_toolkit_adv_tab";
const ADV_BUTTON: &str = "main.top.right.statistics.drafters_toolkit_adv_tab.button";
const ADV_PANEL: &str = "main.top.right.statistics.drafters_toolkit_adv_panel";

// Everything the Item Stats tab draws outside its panel: the four dropdown
// buttons, their lists, and the click catcher behind an open list.
const ITEM_FILTERS: [&str; 9] = [
    "main.top.right.statistics.item_lane",
    "main.top.right.statistics.item_tier",
    "main.top.right.statistics.item_category",
    "main.top.right.statistics.item_patch",
    "main.top.right.statistics.item_lane_list",
    "main.top.right.statistics.item_tier_list",
    "main.top.right.statistics.item_category_list",
    "main.top.right.statistics.item_patch_list",
    "main.top.right.statistics.item_category_catch",
];

// The game's own filters for its three tables, hidden as `super::ui` hides them
// for Item Stats.
const GAME_FILTERS: [&str; 4] = [
    "main.top.right.statistics.position",
    "main.top.right.statistics.patch",
    "main.top.right.statistics.year_filter",
    "main.top.right.statistics.league_filter",
];

// -- geometry -----------------------------------------------------------------

// Champ, Player, Team, Item, Advanced.
const TAB_COUNT: u32 = 5;

// Tab width, down from the authored 232px: five tabs then make a 942px bar,
// which leaves room for the 646px of dropdowns.
const TAB_W: u32 = 186;

// `#tabs` padding, which the Toolbox's own box copies.
const BAR_PAD: u32 = 4;

// Where the Toolbox's box goes: the end of the fourth tab. Its own 4px padding
// leaves a small gap; closing it would lay the padding over the Item Stats tab
// and swallow its clicks.
const ADV_X: u32 = BAR_PAD + (TAB_COUNT - 1) * TAB_W;
const ADV_W: u32 = TAB_W + 2 * BAR_PAD;
// The box's right padding stands in for the bar's.
const BAR_W: u32 = ADV_X + ADV_W;

// The four Item Stats dropdowns reach 646px in from the right of the 1600px
// screen; a fifth or wider dropdown must come out of [`TAB_W`].
const SCREEN_W: u32 = 1600;
const DROPDOWNS_W: u32 = 646;
const _: () = assert!(BAR_W + 8 <= SCREEN_W - DROPDOWNS_W);

// -- is the Toolbox's tab coming? ---------------------------------------------

const TOOLBOX_ID: &str = "drafters_toolkit";
// The game's record of which mods are enabled, relative to the game folder.
pub(crate) const MODS_JSON: &str = "config/game/mods.json";
// The Toolbox's settings, beside its DLL, and the one that turns its tab off.
const TOOLBOX_CONFIG: &str = "config.ini";
const TOOLBOX_TAB_KEY: &str = "advanced_stats";
// Steam app id, which names the game's Workshop content folder.
const STEAM_APP_ID: &str = "3009300";

// -- the gap before the Toolbox's tab exists ----------------------------------

// A label standing in for the fifth tab until the Toolbox respawns its button.
// Not a node under the Toolbox's own name, which would stop the Toolbox
// spawning its own and registering its click handler.
const STUB: &str = "main.top.right.statistics.riot_tabs_stub";
// The Toolbox's label is a literal in its DLL, not a text reference.
const ADV_TEXT: &str = "Advanced Stats";
// `label.color` of `main#strategy_option`: an unselected tab's text.
const IDLE_TEXT: &str = "#a3a9b6ff";

// -- label size ---------------------------------------------------------------

// The size the tab labels are authored at.
const LABEL_SIZE: u32 = 18;
// Below this a tab label stops being readable, however long the text is.
const MIN_LABEL_SIZE: u32 = 13;
// Clear space kept either side of a label inside its tab.
const LABEL_MARGIN: f32 = 8.0;

// Off-screen node that measures the tab labels in the game's language: no API
// reports the language, and Russian or Spanish labels may not fit at size 18.
const PROBE: &str = "main.top.right.statistics.riot_tabs_probe";
const PROBE_W: u32 = 900;
const PROBE_TEXTS: [&str; 5] = [
    "#asset/base/text/ui?statistics.champion",
    "#asset/base/text/ui?statistics.athlete",
    "#asset/base/text/ui?statistics.team",
    "#asset/base/text/ui?item_stats.tab",
    ADV_TEXT,
];
// Frames to wait for a measurement before settling for the last one.
const PROBE_FRAMES: u32 = 30;
// Probes spawned per visit before giving up. A rebuild can take one away.
const PROBE_SPAWNS: u32 = 3;

// The label size last measured, used until this visit's probe answers, so
// shrunk labels do not snap back to 18 after a rebuild.
static LAST_SIZE: AtomicU32 = AtomicU32::new(LABEL_SIZE);
// Whether any size but the authored one has ever been written. Once it has,
// sizes are always written, so a label shrunk earlier can grow back.
static SHRUNK: AtomicBool = AtomicBool::new(false);

// -- rebuild detection --------------------------------------------------------

// Zero-sized marker child in the bar and in the Toolbox's box: a rebuild drops
// children, and `ui_exists` notices the same frame where `ui_node_rect` lags
// one.
const BAR_MARK: &str = "main.top.right.statistics.tabs.riot_tabs_mark";
const ADV_MARK: &str = "main.top.right.statistics.drafters_toolkit_adv_tab.riot_tabs_mark";
const MARK_SOURCE: &str =
    "riot_tabs_mark:empty {\nignore_event: true;\nwidth: 0px;\nheight: 0px;\n}\n";

// Frames a fresh merge is given to show up in `ui_node_rect` before it is
// judged not to have taken and written again.
const SETTLE_FRAMES: u32 = 2;
// Consecutive frames the marks may be missing before they are written off as
// not working and the rects alone are trusted.
const MARK_STREAK: u32 = 5;

// -- which tab looks open -----------------------------------------------------

// Invisible button over the open Advanced tab, so a click cannot reach the
// Toolbox's toggle and close it.
const SHIELD: &str = "main.top.right.statistics.riot_tabs_shield";

// Frames between re-asserts while the Advanced panel is open.
const REASSERT_EVERY: u32 = 10;

#[derive(Default)]
struct State {
    // Whether [`on_game_tab`] is registered for this visit to the screen.
    wired: bool,
    // The label size that fits, once the probe has answered.
    size: Option<u32>,
    probe_spawns: u32,
    probe_age: u32,
    // Set when the marks turn out not to work, so the rects are used alone.
    markless: bool,
    mark_streak: u32,
    settle: u32,
    frame: u32,
    // Whether the Advanced panel is currently over the Item Stats tab.
    covered: bool,
    // Whether the Advanced tab is currently painted as the open one.
    lit: bool,
    // Which of [`GAME_FILTERS`] were showing the last frame the Advanced panel
    // was closed: what to put back when it closes again.
    filters: [bool; 4],
}

impl State {
    // The size to draw labels at: this visit's measurement, or the last one.
    fn label_size(&self) -> u32 {
        self.size
            .unwrap_or_else(|| LAST_SIZE.load(Ordering::Relaxed))
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

// -- the soft dependency ------------------------------------------------------

// Whether `id` is in the `enabled_mods` array of the game's `mods.json`, or
// `None` if the document has no such array.
pub(crate) fn listed_enabled(document: &str, id: &str) -> Option<bool> {
    let rest = &document[document.find("\"enabled_mods\"")?..];
    let open = rest.find('[')?;
    let close = open + rest[open..].find(']')?;
    Some(rest[open..close].contains(&format!("\"{id}\"")))
}

// Whether a Toolbox `config.ini` leaves its Advanced Stats tab on: `true` or
// `1` mean on, and a missing key defaults to on.
fn tab_on(config: &str) -> bool {
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == TOOLBOX_TAB_KEY {
                return matches!(value.trim(), "true" | "1");
            }
        }
    }
    true
}

// The Toolbox's folder: under `mods/` by its id, or in the Workshop folder that
// holds its DLL.
fn toolbox_dir(game: &Path) -> Option<PathBuf> {
    let dll = format!("{TOOLBOX_ID}.dll");
    let local = game.join("mods").join(TOOLBOX_ID);
    if local.join(&dll).is_file() {
        return Some(local);
    }
    // steamapps/common/<game> -> steamapps/workshop/content/<app id>
    let workshop = game
        .parent()?
        .parent()?
        .join("workshop")
        .join("content")
        .join(STEAM_APP_ID);
    std::fs::read_dir(workshop)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|dir| dir.join(&dll).is_file())
}

// Whether the Toolbox will add its tab this session: listed as enabled in
// `mods.json` and not switched off in its config. An unreadable list means no,
// an unreadable config yes.
fn toolbox_tab_enabled() -> bool {
    let Some(game) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return false;
    };
    let listed = std::fs::read_to_string(game.join(MODS_JSON))
        .ok()
        .and_then(|document| listed_enabled(&document, TOOLBOX_ID))
        .unwrap_or(false);
    if !listed {
        return false;
    }
    toolbox_dir(&game)
        .and_then(|dir| std::fs::read_to_string(dir.join(TOOLBOX_CONFIG)).ok())
        .map_or(true, |config| tab_on(&config))
}

// [`toolbox_tab_enabled`], asked once: the game and the Toolbox read those
// files only at start.
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(toolbox_tab_enabled)
}

// -- label size ---------------------------------------------------------------

// The largest label size at which a label `widest` pixels wide at
// [`LABEL_SIZE`] fits a tab.
fn size_for(widest: f32) -> u32 {
    let room = TAB_W as f32 - 2.0 * LABEL_MARGIN;
    if widest <= room {
        return LABEL_SIZE;
    }
    ((LABEL_SIZE as f32 * room / widest).floor() as u32).clamp(MIN_LABEL_SIZE, LABEL_SIZE)
}

fn probe_source() -> String {
    // Off-screen rather than hidden, since a hidden node may skip the layout
    // pass being measured.
    let mut source = String::from(
        "riot_tabs_probe:empty {\nignore_event: true;\ny: -4000px;\nwidth: 10px;\nheight: 10px;\n",
    );
    for (index, text) in PROBE_TEXTS.iter().enumerate() {
        // Much wider than any label, so the contents rect is the text's natural
        // width.
        source.push_str(&format!(
            "#t{index}:label {{\n@\"asset/base/style/main#bold_label\";\nignore_event: true;\n\
             width: {PROBE_W}px;\nheight: 32px;\nsize: {LABEL_SIZE};\ntext: \"{text}\";\n}}\n"
        ));
    }
    source.push_str("}\n");
    source
}

// The widest tab label at [`LABEL_SIZE`], or `None` while there is no answer.
fn widest_label(ctx: &StableClient<'_>) -> Option<f32> {
    let mut widest = 0f32;
    for index in 0..PROBE_TEXTS.len() {
        let (_, _, width, _) = ctx.ui_contents_rect(&format!("{PROBE}.t{index}"))?;
        // The box's own width back means the host measured the node, not the
        // text.
        if width >= (PROBE_W - 1) as f32 {
            return None;
        }
        // An empty label is fine: `item_stats.tab` resolves to nothing in
        // untranslated languages.
        widest = widest.max(width);
    }
    // All zero means no layout pass yet.
    (widest > 0.0).then_some(widest)
}

// Measures the labels once per visit. Returns whether the size to draw them
// at just changed, which the tabs then have to be told.
fn fit_labels(state: &mut State, ctx: &mut StableClient<'_>) -> bool {
    if state.size.is_some() {
        return false;
    }

    if !ctx.ui_exists(PROBE) {
        state.probe_spawns += 1;
        if state.probe_spawns > PROBE_SPAWNS || !ctx.ui_spawn_source(SCREEN, &probe_source()) {
            // No measuring this visit, so the last measurement stands.
            state.size = Some(LAST_SIZE.load(Ordering::Relaxed));
        }
        state.probe_age = 0;
        return false;
    }

    state.probe_age += 1;
    let size = match widest_label(ctx) {
        Some(widest) => size_for(widest),
        None if state.probe_age < PROBE_FRAMES => return false,
        None => LAST_SIZE.load(Ordering::Relaxed),
    };
    ctx.ui_remove_node(PROBE);
    state.size = Some(size);
    if size != LABEL_SIZE {
        SHRUNK.store(true, Ordering::Relaxed);
    }
    LAST_SIZE.swap(size, Ordering::Relaxed) != size
}

// -- the five-tab bar ---------------------------------------------------------

fn near(measured: f32, authored: u32) -> bool {
    (measured - authored as f32).abs() < 0.5
}

// Whether the last layout pass had the bar merged, or `None` if it cannot say.
fn merged(ctx: &StableClient<'_>) -> Option<bool> {
    // Both rects come from one call, so their difference is the box's `x`
    // within the screen.
    let (screen_x, ..) = ctx.ui_node_rect(SCREEN)?;
    let (_, _, bar_w, _) = ctx.ui_node_rect(BAR)?;
    let (adv_x, _, adv_w, _) = ctx.ui_node_rect(ADV_TAB)?;
    Some(near(bar_w, BAR_W) && near(adv_x - screen_x, ADV_X) && near(adv_w, ADV_W))
}

// One tab's properties: its width, plus the label sizes once any label has
// needed shrinking (otherwise the authored 18 is left alone).
fn tab_props(size: u32) -> String {
    if SHRUNK.load(Ordering::Relaxed) {
        format!("width: {TAB_W}px; label: {{ size: {size}; }} selected_label: {{ size: {size}; }}")
    } else {
        format!("width: {TAB_W}px;")
    }
}

// The game's half of the five-tab bar: narrower tabs in a bar sized for five.
fn write_bar(ctx: &mut StableClient<'_>, size: u32) {
    let tab = tab_props(size);
    ctx.ui_set_properties(BAR, &format!("width: {BAR_W}px;"));
    for path in GAME_TABS {
        ctx.ui_set_properties(path, &tab);
    }
}

// Writes the five-tab bar, with the Toolbox's box moved to the end of it and
// its border dropped.
fn merge(ctx: &mut StableClient<'_>, size: u32) {
    write_bar(ctx, size);
    // `color` is the box's 1px outline, which would frame the tab twice inside
    // the bar.
    ctx.ui_set_properties(
        ADV_TAB,
        &format!("x: {ADV_X}px; width: {ADV_W}px; color: #00000000;"),
    );
    ctx.ui_set_properties(ADV_BUTTON, &tab_props(size));
}

fn stub_source(size: u32) -> String {
    // Where the Toolbox's button will be; a bold centred label is all an
    // unselected tab draws.
    let x = ADV_X + BAR_PAD;
    format!(
        "riot_tabs_stub:label {{\n@\"asset/base/style/main#bold_label\";\n\
         ignore_event: true;\nx: {x}px;\ny: {BAR_PAD}px;\nwidth: {TAB_W}px;\nheight: 32px;\n\
         size: {size};\nalign_x: Center;\nalign_y: Center;\ncolor: {IDLE_TEXT};\n\
         text: \"{ADV_TEXT}\";\n}}\n"
    )
}

// Holds the five-tab bar while the Toolbox's tab is missing (up to ten frames
// after a rebuild), with a label where the tab will be, so the bar does not
// visibly break and fix itself.
fn hold_place(state: &mut State, ctx: &mut StableClient<'_>, resized: bool) {
    let size = state.label_size();
    let marked = !state.markless && ctx.ui_exists(BAR_MARK);
    if resized || !marked {
        write_bar(ctx, size);
        if !state.markless && !marked && !ctx.ui_spawn_source(BAR, MARK_SOURCE) {
            state.markless = true;
        }
    }

    // A stand-in drawn at the old size is replaced rather than resized.
    if resized {
        ctx.ui_remove_node(STUB);
    }
    if !ctx.ui_exists(STUB) {
        ctx.ui_spawn_source(SCREEN, &stub_source(size));
    }
}

// Keeps the bar merged across rebuilds and respawns. Returns whether one was
// just caught, so the highlights can be repeated on the fresh nodes too.
fn keep_merged(state: &mut State, ctx: &mut StableClient<'_>, resized: bool) -> bool {
    let rebuilt = !state.markless && !(ctx.ui_exists(BAR_MARK) && ctx.ui_exists(ADV_MARK));

    if rebuilt {
        // Marks that vanish every frame do not work; stop respawning them.
        state.mark_streak += 1;
        if state.mark_streak > MARK_STREAK {
            state.markless = true;
        }
    } else {
        state.mark_streak = 0;
        if !resized {
            if merged(ctx) == Some(true) {
                state.settle = 0;
                return false;
            }
            if state.settle > 0 {
                state.settle -= 1;
                return false;
            }
        }
    }

    merge(ctx, state.label_size());
    if !state.markless {
        for (parent, mark) in [(BAR, BAR_MARK), (ADV_TAB, ADV_MARK)] {
            if !ctx.ui_exists(mark) && !ctx.ui_spawn_source(parent, MARK_SOURCE) {
                state.markless = true;
            }
        }
    }
    state.settle = SETTLE_FRAMES;
    rebuilt
}

// -- tabs that behave like tabs -----------------------------------------------

// Hides the Item Stats dropdowns and dims its tab while the Advanced panel
// covers it. Runs right after [`super::ui`], which re-shows them every frame,
// so nothing needs putting back when the panel closes.
fn keep_clear(state: &mut State, ctx: &mut StableClient<'_>, rebuilt: bool) {
    let item_up = ctx.ui_visible(ITEM_PANEL) == Some(true);
    let covered = item_up && ctx.ui_exists(ADV_PANEL);

    if covered {
        for path in ITEM_FILTERS {
            ctx.ui_set_visible(path, false);
        }
        if !state.covered || rebuilt || state.frame % REASSERT_EVERY == 0 {
            ctx.ui_set_properties(ITEM_TAB, &tab_style("image", "label", false));
        }
    } else if state.covered && item_up {
        // Only when Item Stats is still underneath; on leaving for a game tab,
        // `super::ui` dims it itself.
        ctx.ui_set_properties(ITEM_TAB, &tab_style("image", "label", true));
    }
    state.covered = covered;
}

fn shield_source() -> String {
    let x = ADV_X + BAR_PAD;
    format!(
        "riot_tabs_shield:color_icon_button {{\nx: {x}px;\ny: {BAR_PAD}px;\n\
         width: {TAB_W}px;\nheight: 32px;\nbtn: {{\ncolor: #00000000;\n}}\n}}\n"
    )
}

// While the Advanced panel is open, paints it as the selected tab, dims the
// game's tabs and hides the game's filters every frame, because the host
// ignores the Toolbox's `selected` writes on `color_selectable` nodes.
//
// On close the filters that were showing before come back, since closing from
// the tab it was opened on rebuilds nothing.
fn keep_lit(state: &mut State, ctx: &mut StableClient<'_>, rebuilt: bool) {
    let open = ctx.ui_exists(ADV_PANEL);
    let shielded = ctx.ui_exists(SHIELD);

    if open {
        for path in GAME_FILTERS {
            ctx.ui_set_visible(path, false);
        }
        if !shielded {
            ctx.ui_spawn_source(SCREEN, &shield_source());
        }
        if !state.lit || rebuilt || state.frame % REASSERT_EVERY == 0 {
            ctx.ui_set_properties(ADV_BUTTON, &tab_style("image", "label", true));
            let idle = tab_style("selected_image", "selected_label", false);
            for path in VANILLA_TABS {
                ctx.ui_set_properties(path, &idle);
            }
            state.lit = true;
        }
        return;
    }

    // Checked every frame: a stale shield would make the tab unclickable.
    if shielded {
        ctx.ui_remove_node(SHIELD);
    }
    if state.lit {
        ctx.ui_set_properties(ADV_BUTTON, &tab_style("image", "label", false));
        // With Item Stats up, `super::ui` keeps the game's tabs dim and their
        // filters hidden.
        if ctx.ui_visible(ITEM_PANEL) != Some(true) {
            let lit = tab_style("selected_image", "selected_label", true);
            for path in VANILLA_TABS {
                ctx.ui_set_properties(path, &lit);
            }
            // Not after a rebuild: that was a switch to another game tab, whose
            // filters game code has just set.
            if !rebuilt {
                for (path, shown) in GAME_FILTERS.iter().zip(state.filters) {
                    if shown {
                        ctx.ui_set_visible(path, true);
                    }
                }
            }
        }
        state.lit = false;
        return;
    }

    // Closed for a second frame: remember what is showing, to restore after the
    // next open.
    for (shown, path) in state.filters.iter_mut().zip(GAME_FILTERS) {
        *shown = ctx.ui_visible(path) == Some(true);
    }
}

// Closes the Advanced panel when any other tab is clicked: the Toolbox's own
// close check never fires, and clicking Item Stats or the tab it was opened
// from rebuilds nothing. Registered beside `super::ui`'s handler rather than
// called from it, because that one drops repeated clicks.
fn on_game_tab(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // Every event on the path lands here; an unreported kind counts as a click,
    // as in `super::ui`.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    if GAME_TABS.contains(&event.path.as_str()) && ctx.ui_exists(ADV_PANEL) {
        ctx.ui_remove_node(ADV_PANEL);
    }
}

// Per-frame entry, called right after [`super::ui::sync`], which [`keep_clear`]
// relies on.
pub fn sync(ctx: &mut StableClient<'_>) {
    if !enabled() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        return;
    };

    // Off the screen: forget the last visit, whose nodes are gone.
    if !ctx.ui_exists(ITEM_TAB) {
        *guard = None;
        return;
    }
    let state = guard.get_or_insert_with(State::default);
    state.frame = state.frame.wrapping_add(1);

    if !state.wired {
        // All four exist by now: the Item Stats tab is the last one declared.
        let mut wired = true;
        for path in GAME_TABS {
            wired &= ctx.ui_register_path_events(path, on_game_tab);
        }
        state.wired = wired;
    }

    let resized = fit_labels(state, ctx);

    let rebuilt = if ctx.ui_exists(ADV_TAB) {
        let rebuilt = keep_merged(state, ctx, resized);
        // After the merge, so the stand-in and the tab swap in one frame.
        if ctx.ui_exists(STUB) {
            ctx.ui_remove_node(STUB);
        }
        rebuilt
    } else {
        // Between a rebuild and the Toolbox's next check: fresh nodes either
        // way.
        hold_place(state, ctx, resized);
        true
    };

    // Both run even without the Toolbox's tab, to tidy up after a panel lost to
    // a rebuild.
    keep_clear(state, ctx, rebuilt);
    keep_lit(state, ctx, rebuilt);
}
