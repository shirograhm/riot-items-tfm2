//! The mod's Item Rules row on the Options dialog's Competition tab: *All
//! Radiants*, *First Item Radiant* or *No Radiants*, kept in the save and
//! applied by [`crate::item_tiers`]. It sits above the Item Patcher Settings
//! row (user, 2026-10-16), which waits for it to be appended.
//!
//! Greyed out everywhere but the management screens ([`locked_now`], user
//! 2026-10-08). A click applies at once, into the save; the dialog's OK and
//! Cancel do not touch it.
//!
//! Seen working on its own dialog finder (2026-10-08); not yet tried on the
//! shared one.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use mod_api_stable::{ClientSceneKindV1, SceneKindV1, StableClient, UiEventKindV1};

use super::options_dialog::{self, escape, text};
use crate::item_tiers::Rule;

// The row's node, and under it the box its cells are in.
pub(super) const ROW: &str = "riot_item_rules";
const CELLS_UNDER: &str = ".options.";

// The row's title: this mod's key in the game's text, and the English.
const TITLE: (&str, &str) = ("builds.item_rules", "Item Rules");

// One cell: the side it asserts, its node's name, and its label and its hint
// the way [`TITLE`] is written.
struct Cell {
    rule: Rule,
    node: &'static str,
    label: (&'static str, &'static str),
    hint: (&'static str, &'static str),
}

// The cells, left to right.
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
// Both cells' hint while the row is greyed out.
const LOCKED_HINT: (&str, &str) = (
    "builds.item_rules_locked",
    "Item Rules can only be changed between matches, not during a draft or a match.",
);

// Frames between two of the test log's looks at the dialog.
const LOG_EVERY: u32 = 20;

// Frames [`sync`] has run.
static FRAME: AtomicU32 = AtomicU32::new(0);

// Whether the dialog was up on the last frame.
static UP: AtomicBool = AtomicBool::new(false);

// What the row was last painted as ([`look`]): 0 for not yet.
static PAINTED: AtomicU8 = AtomicU8::new(0);

// Whether the setting may not be changed now. Worked out on the frame
// ([`locked_now`]) and kept here for the click handler, which is not a
// place the client can be asked what screen it is on.
static LOCKED: AtomicBool = AtomicBool::new(false);

// The `options.` lines of `match-builds.log` when its switch is on
// (`crate::match_builds::log`): the tab, where the row landed, and every cell
// event.
fn log(key: &str, text: impl FnOnce() -> String) {
    crate::match_builds::log(&format!("options.{key}"), text);
}

// The row, shaped like the layout's own three-cell rows: 846 pixels inside
// the box's padding, 282 a cell.
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

// The row's look as one number: which side is the setting, and whether it
// is greyed out. Never 0.
fn look(rule: Rule, locked: bool) -> u8 {
    1 + rule.code() + 4 * locked as u8
}

// Lights the cell of the current setting in the row at `row`, greyed out while
// locked, with the matching hint. Painted, not set: see
// `strategy_ui::paint_toggle`.
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

// Whether the setting may not be changed now: anywhere but the management
// screens. A host that cannot say which screen is up counts as management, so
// the setting never locks for good.
fn locked_now(ctx: &StableClient<'_>) -> bool {
    match ctx.client_scene_kind() {
        Some(ClientSceneKindV1::Main) => false,
        Some(_) => true,
        None => ctx.scene_kind() == Some(SceneKindV1::GameTest),
    }
}

// Registers the click handler on one cell, every time the row spawns or the
// dialog opens: registering once lost clicks after the pause menu was reopened
// (2026-10-08). A surviving old handler only makes a click assert its value
// twice.
fn register(ctx: &mut StableClient<'_>, path: &str) {
    let registered = ctx.ui_register_path_events(path, handle_event);
    log("register", || format!("`{path}` registered={registered}"));
}

// A click on a cell asserts that cell's side.
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

// Keeps the row in the dialog and on its tab. Every client frame; with the
// dialog closed this is one lookup.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    let Some(dialog) = options_dialog::showing(ctx, frame) else {
        UP.store(false, Ordering::Relaxed);
        return;
    };
    let opened = !UP.swap(true, Ordering::Relaxed);

    let contents = format!("{dialog}.contents");
    let row = format!("{contents}.{ROW}");
    // Spawned once each time game code builds the dialog, after its own rows
    // and before the patcher row, which waits for this one.
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
    // On a fresh row and each time the dialog opens, since what ends a
    // registration is not known (see `register`).
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

    let competition = options_dialog::competition_up(ctx, &dialog);
    ctx.ui_set_visible(&row, competition);
    if opened || frame % LOG_EVERY == 0 {
        log("dialog", || {
            format!(
                "`{dialog}` up; screen={:?} of {:?} -> locked={locked}; competition tab selected={:?} -> row shown={competition}; contents holds {:?}; the dialog holds {:?}; row at {:?}",
                ctx.client_scene_kind(),
                ctx.scene_kind(),
                ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition")),
                ctx.ui_child_names(&contents),
                ctx.ui_child_names(&dialog),
                ctx.ui_node_rect(&row),
            )
        });
    }
}
