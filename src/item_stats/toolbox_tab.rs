//! Bows' Drafter's Toolbox on the statistics screen: its Advanced Stats button,
//! made a fifth tab beside Item Stats.
//!
//! A soft dependency. Nothing in `mod.mod_info` names the Toolbox; [`enabled`]
//! asks once whether the game has it switched on, and without it this file is a
//! single cached `false` per frame.
//!
//! # Why it needs anything at all
//!
//! The Item Stats tab puts four filter dropdowns to the right of the tab bar.
//! The Toolbox (`drafters_toolkit`) puts an Advanced Stats button to the right
//! of the tab bar too. Each is right on its own; together the button sits on
//! the dropdowns. So the button goes *in* the bar, and the tabs narrow so the
//! bar still ends short of the dropdowns.
//!
//! # What the Toolbox does (read from its 1.2.0 DLL)
//!
//! It ships no layout. While the statistics screen is up it checks for
//! `drafters_toolkit_adv_tab` under the screen root and, when that is missing,
//! spawns it: a bordered 240x39 box of its own holding one 232px
//! `color_selectable` named `button`. Its `x` is chosen once, at spawn - `944`
//! when `tabs.item` exists, `712` otherwise - so it already steps aside for the
//! wider Item Stats bar, just not for the dropdowns past it. Nothing re-asserts
//! that `x` afterwards, which is what makes it movable from here.
//!
//! That check runs on every tenth frame only (`frame % 10`). Game code rebuilds
//! this screen when one of its own tabs is clicked, the box goes with it, and
//! for up to ten frames there is no fifth tab to merge. See [`hold_place`].
//!
//! Clicking the button spawns `drafters_toolkit_adv_panel` over the tables. It
//! closes when the button is clicked again, or when one of
//! `tabs.{champion,athlete,team,item}` reports itself `selected`.
//!
//! # Why this is code and not part of `statistics.ui`
//!
//! The bar is this mod's layout override, but the fifth tab is in no layout at
//! all, and whether it exists depends on another mod's config. A layout cannot
//! be conditional, so the handful of values that differ are written at runtime.
//!
//! # A tab has to behave like one
//!
//! Moving the button is the visible half. The other half is that a button
//! beside the bar could get away with things a tab in it cannot:
//!
//! - The highlight never moved. The Toolbox selects its button and deselects
//!   the game's tab through the engine's `selected` state, which the host does
//!   not accept for this kind of node, so the game's tab stayed lit and the
//!   Advanced one never was. It is painted on instead, the way
//!   [`super::ui`] paints the Item Stats tab. See [`keep_lit`].
//! - The button is a toggle: clicking it while open closes it. A tab stays
//!   open, so the click is kept from reaching the Toolbox. See [`SHIELD`].
//! - Nothing else closed it reliably. The Toolbox waits for a game tab to
//!   report itself `selected`, which it cannot read either; what closed the
//!   panel was the game rebuilding the screen, which a click on Item Stats or
//!   on the already-selected game tab does not cause. See [`on_game_tab`].
//! - Advanced opened from Item Stats leaves this tab's dropdowns up over a
//!   table they do not filter, and the Item tab lit beside the Advanced one.
//!   See [`keep_clear`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use mod_api_stable::*;

use crate::strategy_ui::tab_style;

// -- the screen ---------------------------------------------------------------
//
// Spelled out rather than resolved the way `super::ui` finds its screen: the
// Toolbox hard-codes this path, so its button can only ever exist here.

const SCREEN: &str = "main.top.right.statistics";
const BAR: &str = "main.top.right.statistics.tabs";
const GAME_TABS: [&str; 4] = [
    "main.top.right.statistics.tabs.champion",
    "main.top.right.statistics.tabs.athlete",
    "main.top.right.statistics.tabs.team",
    "main.top.right.statistics.tabs.item",
];
/// The game's own three tabs: the ones game code can consider selected.
const VANILLA_TABS: [&str; 3] = [GAME_TABS[0], GAME_TABS[1], GAME_TABS[2]];
const ITEM_TAB: &str = "main.top.right.statistics.tabs.item";
const ITEM_PANEL: &str = "main.top.right.statistics.data.item_stats";
const ADV_TAB: &str = "main.top.right.statistics.drafters_toolkit_adv_tab";
const ADV_BUTTON: &str = "main.top.right.statistics.drafters_toolkit_adv_tab.button";
const ADV_PANEL: &str = "main.top.right.statistics.drafters_toolkit_adv_panel";

/// Everything the Item Stats tab draws outside its panel: the four dropdown
/// buttons, their lists, and the click catcher behind an open list.
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

/// The game's own filters, which drive its three tables and not the Toolbox's.
/// The same four `super::ui` hides for the Item Stats tab.
const GAME_FILTERS: [&str; 4] = [
    "main.top.right.statistics.position",
    "main.top.right.statistics.patch",
    "main.top.right.statistics.year_filter",
    "main.top.right.statistics.league_filter",
];

// -- geometry -----------------------------------------------------------------

/// Champ, Player, Team, Item, Advanced.
const TAB_COUNT: u32 = 5;

/// Width of one tab, down from the 232px `statistics.ui` and the Toolbox both
/// author.
///
/// Five tabs at 232 would make the bar 1168px and leave 432px for dropdowns
/// that need 646. At 186 the bar is 942px - six more than the four-tab bar it
/// replaces - so nothing to its right has to move.
const TAB_W: u32 = 186;

/// `#tabs` padding, which the Toolbox's own box copies.
const BAR_PAD: u32 = 4;

/// Where the Toolbox's box goes: where the fourth tab ends.
///
/// The box keeps its own 4px padding, so its button starts 4px further in and
/// the fifth tab stands that far off the fourth. Starting the box 4px earlier
/// would close the gap, and lay its padding over the Item Stats tab's right
/// edge - a sibling drawn above swallows clicks, so that strip of the tab
/// would stop responding. A gap nobody can see beats a tab that misses clicks.
const ADV_X: u32 = BAR_PAD + (TAB_COUNT - 1) * TAB_W;
const ADV_W: u32 = TAB_W + 2 * BAR_PAD;
/// The box's right padding stands in for the bar's.
const BAR_W: u32 = ADV_X + ADV_W;

/// The screen is 1600px wide and the four Item Stats dropdowns, right-anchored
/// with their gaps, reach 646px in from the right edge (`statistics.ui`). A
/// fifth dropdown, or wider ones, has to come out of [`TAB_W`].
const SCREEN_W: u32 = 1600;
const DROPDOWNS_W: u32 = 646;
const _: () = assert!(BAR_W + 8 <= SCREEN_W - DROPDOWNS_W);

// -- is the Toolbox's tab coming? ---------------------------------------------

const TOOLBOX_ID: &str = "drafters_toolkit";
/// The game's record of which mods are enabled, relative to the game folder.
const MODS_JSON: &str = "config/game/mods.json";
/// The Toolbox's settings, beside its DLL, and the one that turns its tab off.
const TOOLBOX_CONFIG: &str = "config.ini";
const TOOLBOX_TAB_KEY: &str = "advanced_stats";
/// Steam app id, which names the game's Workshop content folder.
const STEAM_APP_ID: &str = "3009300";

// -- the gap before the Toolbox's tab exists ----------------------------------

/// A stand-in for the fifth tab, shown where the Toolbox's button will be.
///
/// A plain label rather than a copy of the Toolbox's node. Spawning a node
/// under the Toolbox's own name would close the gap just as well, and would
/// also stop the Toolbox spawning its own - the step where it registers its
/// click handler. A label that does nothing for a sixth of a second cannot
/// leave the tab dead; an impostor could.
const STUB: &str = "main.top.right.statistics.riot_tabs_stub";
/// The Toolbox's label is a literal in its DLL, not a text reference.
const ADV_TEXT: &str = "Advanced Stats";
/// `label.color` of `main#strategy_option`: an unselected tab's text.
const IDLE_TEXT: &str = "#a3a9b6ff";

// -- label size ---------------------------------------------------------------

/// The size the tab labels are authored at.
const LABEL_SIZE: u32 = 18;
/// Below this a tab label stops being readable, however long the text is.
const MIN_LABEL_SIZE: u32 = 13;
/// Clear space kept either side of a label inside its tab.
const LABEL_MARGIN: f32 = 8.0;

/// A throwaway node that measures the tab labels in the game's language.
///
/// Every English label fits a 186px tab at size 18. The Russian and Spanish
/// ones do not, and there is no API that says which language is in use -
/// `i18n` answers in English regardless. So the labels are laid out for real,
/// off-screen, in the tabs' own bold font, and measured.
const PROBE: &str = "main.top.right.statistics.riot_tabs_probe";
const PROBE_W: u32 = 900;
const PROBE_TEXTS: [&str; 5] = [
    "#asset/base/text/ui?statistics.champion",
    "#asset/base/text/ui?statistics.athlete",
    "#asset/base/text/ui?statistics.team",
    "#asset/base/text/ui?item_stats.tab",
    ADV_TEXT,
];
/// Frames to wait for a measurement before settling for the last one.
const PROBE_FRAMES: u32 = 30;
/// Probes spawned per visit before giving up. A rebuild can take one away.
const PROBE_SPAWNS: u32 = 3;

/// The label size last measured, used until this visit's probe answers.
///
/// A rebuild can take the screen root with it, and with that everything this
/// visit knew. Without a remembered size a shrunk label would snap back to 18
/// for the frames the probe takes, on every tab switch.
static LAST_SIZE: AtomicU32 = AtomicU32::new(LABEL_SIZE);
/// Whether any size but the authored one has ever been written. Once it has,
/// sizes are always written, so a label shrunk earlier can grow back.
static SHRUNK: AtomicBool = AtomicBool::new(false);

// -- rebuild detection --------------------------------------------------------

/// A zero-sized child left in the tab bar and in the Toolbox's box.
///
/// A rebuild puts the tabs back at 232px, and the Toolbox respawns its box at
/// `x: 944` whenever that goes missing. `ui_node_rect` would notice, but it
/// reports the *last* layout pass, so it notices a frame late and the bar would
/// visibly jump on every switch. A node that was rebuilt has lost its children,
/// and `ui_exists` says so the same frame - the signal `super::ui` takes from
/// its missing `row0`.
const BAR_MARK: &str = "main.top.right.statistics.tabs.riot_tabs_mark";
const ADV_MARK: &str = "main.top.right.statistics.drafters_toolkit_adv_tab.riot_tabs_mark";
const MARK_SOURCE: &str =
    "riot_tabs_mark:empty {\nignore_event: true;\nwidth: 0px;\nheight: 0px;\n}\n";

/// Frames a fresh merge is given to show up in `ui_node_rect` before it is
/// judged not to have taken and written again.
const SETTLE_FRAMES: u32 = 2;
/// Consecutive frames the marks may be missing before they are written off as
/// not working and the rects alone are trusted.
const MARK_STREAK: u32 = 5;

// -- which tab looks open -----------------------------------------------------

/// An invisible button laid over the Advanced tab while its panel is open.
///
/// The Toolbox's click handler is a toggle: a click on the open tab closes it.
/// A tab does not do that, and the handler is not this mod's to change, so the
/// click is kept from reaching it. Same shape as `#item_category_catch`.
const SHIELD: &str = "main.top.right.statistics.riot_tabs_shield";

/// Frames between re-asserts while the Advanced panel is open. `super::ui`
/// repaints the tabs when it heals a rebuilt screen, on a cadence of its own.
const REASSERT_EVERY: u32 = 10;

#[derive(Default)]
struct State {
    /// Whether [`on_game_tab`] is registered for this visit to the screen.
    wired: bool,
    /// The label size that fits, once the probe has answered.
    size: Option<u32>,
    probe_spawns: u32,
    probe_age: u32,
    /// Set when the marks turn out not to work, so the rects are used alone.
    markless: bool,
    mark_streak: u32,
    settle: u32,
    frame: u32,
    /// Whether the Advanced panel is currently over the Item Stats tab.
    covered: bool,
    /// Whether the Advanced tab is currently painted as the open one.
    lit: bool,
    /// Which of [`GAME_FILTERS`] were showing the last frame the Advanced panel
    /// was closed: what to put back when it closes again.
    filters: [bool; 4],
}

impl State {
    /// The size to draw labels at: this visit's measurement, or the last one.
    fn label_size(&self) -> u32 {
        self.size
            .unwrap_or_else(|| LAST_SIZE.load(Ordering::Relaxed))
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

// -- the soft dependency ------------------------------------------------------

/// Whether `id` is in the `enabled_mods` array of the game's `mods.json`, or
/// `None` if the document has no such array.
fn listed_enabled(document: &str, id: &str) -> Option<bool> {
    let rest = &document[document.find("\"enabled_mods\"")?..];
    let open = rest.find('[')?;
    let close = open + rest[open..].find(']')?;
    Some(rest[open..close].contains(&format!("\"{id}\"")))
}

/// Whether a Toolbox `config.ini` leaves its Advanced Stats tab on.
///
/// The Toolbox reads `true` and `1` as on. A file that does not mention the
/// key at all is an older or trimmed config, and the tab defaults to on.
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

/// The Toolbox's folder: placed by hand under its mod id, or subscribed, under
/// a published file id that is found by its DLL rather than written down here.
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

/// Whether the Toolbox is going to add its Advanced Stats tab this session:
/// the game lists it as enabled, and its own config has not switched the tab
/// off.
///
/// A list that cannot be read is a no. Nothing requires the Toolbox to be
/// there, and guessing yes would leave players without it a fifth tab that is
/// only a label. A config that cannot be read is a yes: the tab is on unless
/// its owner said otherwise.
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

/// [`toolbox_tab_enabled`], asked once.
///
/// Once is enough because once is all the game and the Toolbox ask: both read
/// those files at start. It is also what makes the bar right from the first
/// frame of a visit. Finding out by watching for the Toolbox's box was tried
/// first, and the bar fell back to four wide tabs every time the box was
/// briefly gone.
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(toolbox_tab_enabled)
}

// -- label size ---------------------------------------------------------------

/// The largest label size at which a label `widest` pixels wide at
/// [`LABEL_SIZE`] fits a tab.
fn size_for(widest: f32) -> u32 {
    let room = TAB_W as f32 - 2.0 * LABEL_MARGIN;
    if widest <= room {
        return LABEL_SIZE;
    }
    ((LABEL_SIZE as f32 * room / widest).floor() as u32).clamp(MIN_LABEL_SIZE, LABEL_SIZE)
}

fn probe_source() -> String {
    // Off-screen rather than `visible: false`: a hidden node may be skipped by
    // the layout pass, and the layout pass is the thing being asked.
    let mut source = String::from(
        "riot_tabs_probe:empty {\nignore_event: true;\ny: -4000px;\nwidth: 10px;\nheight: 10px;\n",
    );
    for (index, text) in PROBE_TEXTS.iter().enumerate() {
        // Much wider than any label, so the text is laid out at its natural
        // width and the contents rect is the text's, not the box's.
        source.push_str(&format!(
            "#t{index}:label {{\n@\"asset/base/style/main#bold_label\";\nignore_event: true;\n\
             width: {PROBE_W}px;\nheight: 32px;\nsize: {LABEL_SIZE};\ntext: \"{text}\";\n}}\n"
        ));
    }
    source.push_str("}\n");
    source
}

/// The widest tab label at [`LABEL_SIZE`], or `None` while there is no answer.
fn widest_label(ctx: &StableClient<'_>) -> Option<f32> {
    let mut widest = 0f32;
    for index in 0..PROBE_TEXTS.len() {
        let (_, _, width, _) = ctx.ui_contents_rect(&format!("{PROBE}.t{index}"))?;
        // The box handed back unchanged means the host measures the node here
        // and not the text, which is no measurement at all.
        if width >= (PROBE_W - 1) as f32 {
            return None;
        }
        // A label with no text is fine: `item_stats.tab` is translated into
        // seven languages, and the reference resolves to nothing in the others.
        widest = widest.max(width);
    }
    // All zero is a probe that has not been through a layout pass yet.
    (widest > 0.0).then_some(widest)
}

/// Measures the labels once per visit. Returns whether the size to draw them
/// at just changed, which the tabs then have to be told.
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

/// Whether the last layout pass had the bar merged, or `None` if it cannot say.
fn merged(ctx: &StableClient<'_>) -> Option<bool> {
    // Both rects come from the same call, so their difference is the box's `x`
    // inside the screen whatever space the call reports in.
    let (screen_x, ..) = ctx.ui_node_rect(SCREEN)?;
    let (_, _, bar_w, _) = ctx.ui_node_rect(BAR)?;
    let (adv_x, _, adv_w, _) = ctx.ui_node_rect(ADV_TAB)?;
    Some(near(bar_w, BAR_W) && near(adv_x - screen_x, ADV_X) && near(adv_w, ADV_W))
}

/// One tab's properties: its width, and its label size once a label somewhere
/// has needed shrinking. The layout authors 18, so in a session where every
/// label fits the labels are never touched at all.
fn tab_props(size: u32) -> String {
    if SHRUNK.load(Ordering::Relaxed) {
        format!("width: {TAB_W}px; label: {{ size: {size}; }} selected_label: {{ size: {size}; }}")
    } else {
        format!("width: {TAB_W}px;")
    }
}

/// The game's half of the five-tab bar: narrower tabs in a bar sized for five.
fn write_bar(ctx: &mut StableClient<'_>, size: u32) {
    let tab = tab_props(size);
    ctx.ui_set_properties(BAR, &format!("width: {BAR_W}px;"));
    for path in GAME_TABS {
        ctx.ui_set_properties(path, &tab);
    }
}

/// Writes the five-tab bar, with the Toolbox's box moved to the end of it and
/// its border dropped.
fn merge(ctx: &mut StableClient<'_>, size: u32) {
    write_bar(ctx, size);
    // `color` is the box's 1px outline. Inside the bar it would be a second
    // frame around one tab; its fill is already transparent.
    ctx.ui_set_properties(
        ADV_TAB,
        &format!("x: {ADV_X}px; width: {ADV_W}px; color: #00000000;"),
    );
    ctx.ui_set_properties(ADV_BUTTON, &tab_props(size));
}

fn stub_source(size: u32) -> String {
    // Where the Toolbox's button ends up: one padding inside its box. Bold and
    // centred like `main#strategy_option`, whose idle `image` draws nothing, so
    // a label alone is what an unselected tab looks like.
    let x = ADV_X + BAR_PAD;
    format!(
        "riot_tabs_stub:label {{\n@\"asset/base/style/main#bold_label\";\n\
         ignore_event: true;\nx: {x}px;\ny: {BAR_PAD}px;\nwidth: {TAB_W}px;\nheight: 32px;\n\
         size: {size};\nalign_x: Center;\nalign_y: Center;\ncolor: {IDLE_TEXT};\n\
         text: \"{ADV_TEXT}\";\n}}\n"
    )
}

/// Keeps the bar in its five-tab shape while the Toolbox's tab does not exist.
///
/// The Toolbox looks for its tab on every tenth frame. After a rebuild - a
/// click on Champ, Player or Team - that is up to ten frames with the tabs
/// back at their authored width and no fifth one, and then everything jumping
/// into place: long enough to read as the bar breaking and fixing itself. So
/// the bar is kept as it will be, with a label standing where the tab will.
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

/// Keeps the bar merged across rebuilds and respawns. Returns whether one was
/// just caught, so the highlights can be repeated on the fresh nodes too.
fn keep_merged(state: &mut State, ctx: &mut StableClient<'_>, resized: bool) -> bool {
    let rebuilt = !state.markless && !(ctx.ui_exists(BAR_MARK) && ctx.ui_exists(ADV_MARK));

    if rebuilt {
        // Marks that are gone again every frame are marks that do not work.
        // Spawning one per frame forever is the failure to avoid.
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

/// Puts the Item Stats dropdowns and tab highlight away while the Advanced
/// panel is open over that tab, and lets them back when it closes.
///
/// [`super::ui`] is still "showing" underneath - a panel spawned over its table
/// is not the player leaving the tab - so it goes on asserting its dropdowns
/// visible every frame. This runs straight after it in the same hook, which is
/// why hiding them here holds, and why nothing has to put them back: the frame
/// the panel is gone, that assert is the last word again.
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
        // Only when Item Stats is still the tab underneath. If the player left
        // for a game tab instead, `super::ui` has already dimmed it itself.
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

/// Makes the tab bar say which table is showing, and makes Advanced a tab
/// rather than a toggle, for as long as the Advanced panel is open.
///
/// The Toolbox means to do the first half itself: on open it deselects the
/// game's tab and selects its own button, through the `selected` state. The
/// host only takes that write for `checkbox`, `text_edit`, `slider` and
/// `selectable` nodes, and every tab here is a `color_selectable`, so nothing
/// changes - the game's tab stays lit, the Advanced tab never is. This is the
/// wall `super::ui::paint_tabs` paints around, and the same paint: a tab the
/// engine never selects is drawn through `image` / `label`, the one game code
/// has selected through `selected_image` / `selected_label`.
///
/// All three of the game's tabs are dimmed without asking which one that is:
/// the other two are drawing `image` / `label` and ignore it.
///
/// # The game's filters
///
/// The Toolbox hides the four of them when it opens, once. Game code drives
/// them from its own tab state and puts them back, so they sat over a table
/// they do not filter - exactly what `super::ui` found over the Item Stats tab,
/// and the fix is the same: hide them every frame the panel is open.
///
/// Putting them back is the half `super::ui` gets for free, because leaving its
/// tab is always a click that names the panel to show. Here the panel can close
/// with nothing rebuilt - a click on the game tab it was opened from - so which
/// filters were up is remembered from the last frame before it opened.
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

    // Checked every frame, not only on the way out: a shield left behind by a
    // visit this state no longer remembers would make the tab unclickable.
    if shielded {
        ctx.ui_remove_node(SHIELD);
    }
    if state.lit {
        ctx.ui_set_properties(ADV_BUTTON, &tab_style("image", "label", false));
        // With Item Stats up, the game's tabs stay dim and its filters stay
        // hidden: `super::ui` did both for its own tab, and [`keep_clear`] has
        // just lit that one again.
        if ctx.ui_visible(ITEM_PANEL) != Some(true) {
            let lit = tab_style("selected_image", "selected_label", true);
            for path in VANILLA_TABS {
                ctx.ui_set_properties(path, &lit);
            }
            // Not after a rebuild. That was a switch to another game tab, whose
            // filters are a different pair, and game code has just set them.
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

    // Closed, and was last frame too: what is showing now is what game code
    // and `super::ui` want showing.
    for (shown, path) in state.filters.iter_mut().zip(GAME_FILTERS) {
        *shown = ctx.ui_visible(path) == Some(true);
    }
}

/// Closes the Advanced panel when any of the other four tabs is clicked.
///
/// The Toolbox closes itself when a game tab becomes `selected`, which it reads
/// through the same state the host will not let it write - so that never fires.
/// What closed the panel in practice was the game rebuilding the screen under
/// it, and that does not happen for the two clicks that matter most:
///
/// - Item Stats, which game code knows nothing about. The item table would
///   open underneath the panel.
/// - The game tab the panel was opened from. Game code still considers it
///   selected, so the click changes nothing - and now that the Advanced tab no
///   longer toggles, it is the only way back.
///
/// Removing the panel is a close the Toolbox already handles: its own check
/// finds the panel gone and clears its open flag, the same path it takes when
/// the game rebuilds the screen. That path does not put back the filters the
/// Toolbox hid; [`keep_lit`] does.
///
/// Registered beside `super::ui`'s own handler on the same four paths rather
/// than called from it: that one drops a click it has already seen this frame,
/// and this has to act on every delivery or none. It is idempotent, so the
/// duplicates a path-keyed, never-dropped registration produces cost nothing.
fn on_game_tab(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // Every event on the path lands here, `Remove` included. An unreported kind
    // is treated as a click, as `super::ui`'s handler does.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    if GAME_TABS.contains(&event.path.as_str()) && ctx.ui_exists(ADV_PANEL) {
        ctx.ui_remove_node(ADV_PANEL);
    }
}

/// Per-frame entry point, called from the mod's one client hook straight after
/// [`super::ui::sync`] - an order [`keep_clear`] depends on.
///
/// Returns on its first line unless the Toolbox is enabled, and on its second
/// unless the statistics screen is up.
pub fn sync(ctx: &mut StableClient<'_>) {
    if !enabled() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        return;
    };

    // Off the screen. Everything remembered about the last visit died with its
    // nodes.
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
        // Between a rebuild and the Toolbox's next check, or before its first.
        // Either way these are fresh nodes, which is what `rebuilt` means.
        hold_place(state, ctx, resized);
        true
    };

    // Both run with or without the Toolbox's tab. Its panel is what they
    // follow, and a panel that went with a rebuild still has to be tidied up
    // after: highlights back, shield gone.
    keep_clear(state, ctx, rebuilt);
    keep_lit(state, ctx, rebuilt);
}
