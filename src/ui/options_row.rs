//! The mod's row in the game's Options dialog, on the Competition tab:
//! **Item Rules**, a three-cell control reading *All Radiants*, *First Item
//! Radiant* and *No Radiants*. The setting is kept in the save, and where and
//! what it does is [`crate::item_tiers`]'s to say.
//!
//! The user asked for it there (2026-10-08): "put the setting here in the
//! escape menu as a toggle called \"Item Rules\"".
//!
//! # What is known of the dialog, and what is guessed
//!
//! `ui/layout/option.ui` (read out of the 0.6.3 bundle) declares
//! `option:option_ui`, holding `#option` and under it `#tabs` (system,
//! gameplay, competition, display) and `#contents`, a top-to-bottom list of
//! one row a setting. Every row of the other three tabs is authored there and
//! shown or hidden as the tab changes. The Competition tab's rows are not in
//! the file at all: game code builds League Rules and the rest at run time,
//! and where it puts them could not be read off the layout.
//!
//! Where the dialog hangs in the UI tree is not in any layout either. It is
//! not a root: the first version asked for `option.option.contents`, and with
//! the dialog open the roots were `main` and `pause_ui` and nothing else. It
//! is `pause_ui.option.option` ([`KNOWN_DIALOG`]), found by its shape, a node
//! with `tabs.competition` and `contents` under it ([`sweep`]). That search
//! is kept for a game update that moves it, and runs only while the pause
//! menu is up, which is where the dialog is opened from: a sweep of the tree
//! on every screen is how a mod makes a game stutter. The title screen's
//! Options is not covered for that reason.
//!
//! What the test log settled (2026-10-08, the user: "that worked"):
//!
//! - The game's Competition rows are children of `#contents` like every other
//!   row (`league_rule`, `cup_rule`, `group_stage_rule`,
//!   `simulation_intensity`), so a row appended there is laid out under them.
//!   Other mods append theirs the same way (`draft_ai` was in the list).
//! - Which tab is showing cannot be asked of the tab: the host answers
//!   nothing for a `color_selectable`'s selected state. The other test does
//!   it: none of the rows the layout authors for the other three tabs is
//!   showing ([`AUTHORED_ROWS`]). The live list holds more rows than the
//!   bundle's copy of the layout, but every other tab shows at least one of
//!   the twenty.
//!
//! The row's own shape is the layout's three-cell rows over again
//! (`#difficulty`), sizes and style included.
//!
//! # Only between matches
//!
//! The row is greyed out, and its cells take no click, anywhere but on the
//! management screens: through a draft, a match and everything else that is
//! a client screen of its own ([`locked_now`]). The user's rule (2026-10-08:
//! "we should not allow the setting to be flipped mid match. can we grey it
//! out during matches, pick ban phases"). The saved side stays marked, the
//! way the Builds tab greys its scope toggle on the test screens, and the
//! cells' hint says why.
//!
//! # The setting applies on the click
//!
//! Not on the dialog's OK, and its Cancel does not take it back: the cells
//! assert a value the way the Builds tab's footer toggles do, and the dialog's
//! own buttons are game code's. It goes into the save, so like anything else
//! in a save it lasts once the player saves the game.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;

use mod_api_stable::{ClientSceneKindV1, SceneKindV1, StableClient, UiEventKindV1};

use crate::item_tiers::Rule;

/// Whether `match-builds.log` gets a line whenever what this module makes of
/// the dialog changes: where it found it, which tab it took to be showing,
/// where its row landed, and for every event its cells are sent. Off since
/// the row, its clicks and its lock were all seen working (2026-10-08); the
/// first thing to turn on when a game update loses any of them.
const LOG: bool = false;

/// Where the dialog is in game 0.6.3, tried before any search.
const KNOWN_DIALOG: &str = "pause_ui.option.option";

/// The pause menu's layout root, up only while that menu is.
const PAUSE_ROOT: &str = "pause_ui";

/// The row's node, and under it the box its cells are in.
const ROW: &str = "riot_item_rules";
const CELLS_UNDER: &str = ".options.";

/// The rows `option.ui` authors under `#contents`, every one of them on a tab
/// other than Competition.
const AUTHORED_ROWS: [&str; 20] = [
    "screen_mode",
    "resolution",
    "vertical_sync",
    "frame_limit",
    "match_screen",
    "master_volume",
    "bgm_volume",
    "sfx_volume",
    "mute_on_background",
    "language",
    "currency",
    "difficulty",
    "always_delegate_to_staff",
    "banpick_tier_display",
    "banpick_main_position_display",
    "banpick_showcase_display",
    "highlight_speed_playback",
    "shortcut",
    "gamepad",
    "current_database_edit",
];

/// The row's title: this mod's key in the game's text, and the English.
const TITLE: (&str, &str) = ("builds.item_rules", "Item Rules");

/// One cell: the side it asserts, its node's name, and its label and its hint
/// the way [`TITLE`] is written.
struct Cell {
    rule: Rule,
    node: &'static str,
    label: (&'static str, &'static str),
    hint: (&'static str, &'static str),
}

/// The cells, left to right.
const CELLS: [Cell; 3] = [
    Cell {
        rule: Rule::AllRadiants,
        node: "all",
        label: ("builds.item_rules_all", "All Radiants"),
        hint: (
            "builds.item_rules_all_hint",
            "Every item in a build is upgraded to its Radiant form.",
        ),
    },
    Cell {
        rule: Rule::FirstItemRadiant,
        node: "first",
        label: ("builds.item_rules_first", "First Item Radiant"),
        hint: (
            "builds.item_rules_first_hint",
            "Only the first item in a build is upgraded to Radiant. Every other item stops at Legendary.",
        ),
    },
    Cell {
        rule: Rule::NoRadiants,
        node: "none",
        label: ("builds.item_rules_none", "No Radiants"),
        hint: (
            "builds.item_rules_none_hint",
            "Every item in a build stops at Legendary. Nothing is upgraded to Radiant.",
        ),
    },
];
/// Both cells' hint while the row is greyed out.
const LOCKED_HINT: (&str, &str) = (
    "builds.item_rules_locked",
    "Item Rules can only be changed between matches, not during a draft or a match.",
);

/// Frames between two looks for the dialog while the pause menu is up and it
/// has not been found, and between two of the test log's looks at it.
const SWEEP_EVERY: u32 = 20;

/// How deep [`sweep`] goes under the roots, and how many nodes it asks about
/// at most. A depth alone does not bound it: one wide level can be hundreds.
const SWEEP_DEPTH: usize = 4;
const SWEEP_NODES: usize = 600;

/// Frames [`sync`] has run.
static FRAME: AtomicU32 = AtomicU32::new(0);

/// Where the dialog was last found. Kept when it goes, since it comes back in
/// the same place and is then one lookup away.
static FOUND: Mutex<Option<String>> = Mutex::new(None);

/// Whether the dialog was up on the last frame.
static UP: AtomicBool = AtomicBool::new(false);

/// What the row was last painted as ([`look`]): 0 for not yet.
static PAINTED: AtomicU8 = AtomicU8::new(0);

/// Whether the setting may not be changed now. Worked out on the frame
/// ([`locked_now`]) and kept here for the click handler, which is not a
/// place the client can be asked what screen it is on.
static LOCKED: AtomicBool = AtomicBool::new(false);

fn log(key: &str, text: impl FnOnce() -> String) {
    if LOG {
        crate::match_builds::log_note(key, text);
    }
}

/// One of this mod's strings as a label takes it: the reference into the
/// game's text where the mod's entry is there to resolve, which the label
/// then shows in the game's language, else the English.
fn text(ctx: &StableClient<'_>, (key, fallback): (&str, &str)) -> String {
    let reference = format!("#asset/base/text/ui?{key}");
    match ctx.i18n(&reference) {
        Some(text) if !text.is_empty() && !text.starts_with('#') => reference,
        _ => fallback.to_string(),
    }
}

/// Strips what would end a `.ui` string literal.
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '{' | '}' | ';'))
        .collect()
}

/// The row, shaped like the layout's own three-cell rows: 846 pixels inside
/// the box's padding, 282 a cell.
fn row_source(ctx: &StableClient<'_>) -> String {
    let cells: String = CELLS
        .iter()
        .map(|cell| {
            format!(
                "#{}:color_selectable {{\n\
                 @\"asset/base/style/main#strategy_option\";\n\
                 width: 282px;\n\
                 height: 32px;\n\
                 label: {{ size: 16; }}\n\
                 selected_label: {{ size: 16; }}\n\
                 text: \"{}\";\n\
                 hint: \"{}\";\n\
                 }}\n",
                cell.node,
                escape(&text(ctx, cell.label)),
                escape(&text(ctx, cell.hint)),
            )
        })
        .collect();
    format!(
        "{ROW}:empty {{\n\
         width: 1160px;\n\
         height: 40px;\n\
         \n\
         #header:label {{\n\
         @\"asset/base/style/main#label\";\n\
         width: 305px;\n\
         height: 40px;\n\
         align_y: Center;\n\
         size: 16;\n\
         text: \"{title}\";\n\
         }}\n\
         \n\
         #options:color {{\n\
         x: 255px;\n\
         width: 852px;\n\
         height: 40px;\n\
         padding: {{ left: 3px; right: 3px; top: 4px; bottom: 4px; }}\n\
         color: #4a4c56ff;\n\
         stroke: 1;\n\
         back_color: #00000000;\n\
         child_type: LeftToRight {{ spacing: 0px; }}\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         {cells}\
         }}\n\
         }}\n",
        title = escape(&text(ctx, TITLE)),
    )
}

/// The row's look as one number: which side is the setting, and whether it
/// is greyed out. Never 0.
fn look(rule: Rule, locked: bool) -> u8 {
    1 + rule.code() + 4 * locked as u8
}

/// Lights the cell of the setting as it stands, in the row at `row`, greyed
/// out while it may not be changed, and gives the cells the hint that goes
/// with that.
///
/// Painted, not set: see `strategy_ui::paint_toggle` for why a
/// `color_selectable` is given the same look in both of its states.
fn paint(ctx: &mut StableClient<'_>, row: &str) {
    let rule = crate::item_tiers::rule();
    let locked = LOCKED.load(Ordering::Relaxed);
    PAINTED.store(look(rule, locked), Ordering::Relaxed);
    for cell in &CELLS {
        let lit = cell.rule == rule;
        let style = if locked {
            crate::strategy_ui::toggle_off_style_sized(lit, 16)
        } else {
            crate::strategy_ui::toggle_style_sized(lit, 16)
        };
        let hint = escape(&text(ctx, if locked { LOCKED_HINT } else { cell.hint }));
        let path = format!("{row}{CELLS_UNDER}{}", cell.node);
        ctx.ui_set_properties(&path, &style);
        ctx.ui_set_properties(&path, &format!("hint: \"{hint}\";"));
    }
}

/// Whether the setting may not be changed now: anywhere but the management
/// screens. A draft, a match, the screens around them and the ceremonies are
/// each a client screen of their own, and the pause menu opens over all of
/// them. A host that cannot say which screen is up is taken to be on the
/// management ones, so that it does not lock the setting for good; a test
/// game outside a save is a match.
fn locked_now(ctx: &StableClient<'_>) -> bool {
    match ctx.client_scene_kind() {
        Some(ClientSceneKindV1::Main) => false,
        Some(_) => true,
        None => ctx.scene_kind() == Some(SceneKindV1::GameTest),
    }
}

/// Registers the click handler on one cell of a row just spawned.
///
/// Every time the row is spawned or the dialog opens, not once a path as the
/// Builds tab does it
/// (`strategy_ui::register_once`). There a handler was seen to outlive its
/// node and fire for whatever stood at the path next. Here it did not: the
/// first version registered once, and after the pause menu had been closed
/// and opened again a click on a cell never arrived (the test log,
/// 2026-10-08: the setting could be switched to First Item Radiant and never
/// back). Whether that is the pause menu's layout taking its registrations
/// with it is not known. Registering again costs nothing that matters either
/// way: should the old one have lived, a click runs the handler twice, and
/// the handler only asserts a value.
fn register(ctx: &mut StableClient<'_>, path: &str) {
    let registered = ctx.ui_register_path_events(path, handle_event);
    log("register", || format!("`{path}` registered={registered}"));
}

/// A click on a cell asserts that cell's side.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    log("event", || format!("{:?} on `{}`", event.kind, event.path));
    // An unreported kind counts as a click, the gate `item_stats::ui` uses.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    let Some((row, node)) = event.path.rsplit_once(CELLS_UNDER) else {
        return;
    };
    let Some(cell) = CELLS.iter().find(|cell| cell.node == node) else {
        return;
    };
    if !row.ends_with(ROW) {
        return;
    }
    // Greyed out: the click changes nothing, and the row is painted again
    // in case the click itself changed how the cell looks.
    if LOCKED.load(Ordering::Relaxed) {
        paint(ctx, row);
        return;
    }
    if crate::item_tiers::rule() != cell.rule {
        crate::item_tiers::choose(cell.rule);
        log("click", || format!("{:?}", cell.rule));
    }
    paint(ctx, row);
}

/// Whether `path` is the dialog: the node holding the tab bar and the rows.
fn is_dialog(ctx: &StableClient<'_>, path: &str) -> bool {
    ctx.ui_exists(&format!("{path}.tabs.competition")) && ctx.ui_exists(&format!("{path}.contents"))
}

/// Breadth-first from the layout roots for the dialog, the pause menu's own
/// root first, since that is the menu it is opened from. Bounded by depth and
/// by node count.
fn sweep(ctx: &StableClient<'_>) -> Option<String> {
    let mut level: Vec<String> = ctx.ui_child_names("");
    level.sort_by_key(|root| root != PAUSE_ROOT);
    let mut budget = SWEEP_NODES;
    for _ in 0..SWEEP_DEPTH {
        let mut next = Vec::new();
        for path in level {
            if is_dialog(ctx, &path) {
                return Some(path);
            }
            budget = budget.saturating_sub(1);
            if budget == 0 {
                return None;
            }
            for name in ctx.ui_child_names(&path) {
                next.push(format!("{path}.{name}"));
            }
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    None
}

/// Two levels of the pause menu's tree by name, for the test log when the
/// dialog is not found: wherever it is, it is not far from here.
fn pause_tree(ctx: &StableClient<'_>) -> String {
    ctx.ui_child_names(PAUSE_ROOT)
        .into_iter()
        .take(24)
        .map(|child| {
            let under: Vec<String> = ctx
                .ui_child_names(&format!("{PAUSE_ROOT}.{child}"))
                .into_iter()
                .take(16)
                .collect();
            format!("{child}{under:?}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The dialog's path, when it is in the tree. Where it was last found is
/// asked first, on every frame; a search for it is made only now and then,
/// and only while the pause menu is up.
fn locate(ctx: &StableClient<'_>, frame: u32) -> Option<String> {
    let known = FOUND
        .lock()
        .ok()
        .and_then(|found| found.clone())
        .unwrap_or_else(|| KNOWN_DIALOG.to_string());
    if is_dialog(ctx, &known) {
        return Some(known);
    }
    if frame % SWEEP_EVERY != 0 || !ctx.ui_exists(PAUSE_ROOT) {
        return None;
    }
    let found = sweep(ctx);
    match &found {
        Some(path) => log("found", || format!("the dialog is `{path}`")),
        None => log("found", || {
            format!(
                "no dialog within {SWEEP_DEPTH} levels of the roots {:?}; the pause menu holds: {}",
                ctx.ui_child_names(""),
                pause_tree(ctx)
            )
        }),
    }
    if let (Some(path), Ok(mut kept)) = (&found, FOUND.lock()) {
        *kept = Some(path.clone());
    }
    found
}

/// Whether the Competition tab is the one showing. See the module notes for
/// why there are two tests.
fn competition_up(ctx: &StableClient<'_>, dialog: &str) -> bool {
    ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition")) == Some(true)
        || !AUTHORED_ROWS
            .iter()
            .any(|row| ctx.ui_visible(&format!("{dialog}.contents.{row}")) == Some(true))
}

/// Keeps the row in the dialog and on its tab. Every client frame; with the
/// dialog out of the tree this is a lookup or two.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    let Some(dialog) = locate(ctx, frame) else {
        UP.store(false, Ordering::Relaxed);
        return;
    };
    // The layout's own root is authored hidden and game code shows it; it is
    // the dialog's parent wherever the layout was hung. A host that does not
    // say is taken to be showing it: the row is a child of the dialog, so
    // the worst that costs is the tab test on every frame.
    let parent = dialog.rsplit_once('.').map(|(parent, _)| parent);
    let up = [parent, Some(dialog.as_str())]
        .into_iter()
        .flatten()
        .all(|node| ctx.ui_visible(node) != Some(false));
    let opened = up && !UP.swap(up, Ordering::Relaxed);
    if !up {
        UP.store(false, Ordering::Relaxed);
        return;
    }

    let contents = format!("{dialog}.contents");
    let row = format!("{contents}.{ROW}");
    // Spawned when the dialog has none, which is once for each time game
    // code builds the dialog: its own rows are all there from the start, so
    // a row appended then is the last and stays the last.
    let fresh = !ctx.ui_exists(&row);
    if fresh {
        let source = row_source(ctx);
        let spawned = ctx.ui_spawn_source(&contents, &source);
        if !spawned {
            log("row", || format!("could not be spawned under `{contents}`"));
            return;
        }
        paint(ctx, &row);
    }
    // On a row just spawned, and again each time the dialog opens: what ends
    // a registration is not known (see `register`), and saving the game from
    // the pause menu leaves the dialog and this row standing.
    if fresh || opened {
        for cell in &CELLS {
            register(ctx, &format!("{row}{CELLS_UNDER}{}", cell.node));
        }
    }
    // The setting can change under an open dialog, when the save is read,
    // and so can whether it may be changed: the row says what rules now.
    let locked = locked_now(ctx);
    LOCKED.store(locked, Ordering::Relaxed);
    if PAINTED.load(Ordering::Relaxed) != look(crate::item_tiers::rule(), locked) {
        paint(ctx, &row);
    }

    let selected = ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition"));
    let competition = competition_up(ctx, &dialog);
    ctx.ui_set_visible(&row, competition);
    if opened || frame % SWEEP_EVERY == 0 {
        log("dialog", || {
            format!(
                "`{dialog}` up; screen={:?} of {:?} -> locked={locked}; competition tab selected={selected:?} -> row shown={competition}; contents holds {:?}; the dialog holds {:?}; row at {:?}",
                ctx.client_scene_kind(),
                ctx.scene_kind(),
                ctx.ui_child_names(&contents),
                ctx.ui_child_names(&dialog),
                ctx.ui_node_rect(&row),
            )
        });
    }
}
