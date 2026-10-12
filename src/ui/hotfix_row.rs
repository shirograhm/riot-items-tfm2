//! The mod's row in the game's Options dialog, on the Competition tab:
//! **Run Manual Item Patch**, one button reading *Queue*. Pressed, it queues
//! an item patch that goes out with the next news, as a hotfix of the game's
//! version. Until that patch has gone out the button is greyed out and takes
//! no click, and a line under it, a green check and "A manual patch has been
//! queued.", says why. What a queued hotfix is and when it lands is
//! [`crate::patches`]'s to say.
//!
//! # The dialog
//!
//! `ui/layout/option.ui` declares `option:option_ui`, holding `#option` and
//! under it `#tabs` (system, gameplay, competition, display) and `#contents`,
//! a top-to-bottom list of one row a setting. The rows of the other three
//! tabs are authored there and shown or hidden as the tab changes. The
//! Competition tab's are not: game code builds League Rules and the rest at
//! run time, as children of `#contents` like every other row, so a row
//! appended there is laid out under them.
//!
//! The dialog is not a layout root. It is `pause_ui.option.option`
//! ([`KNOWN_DIALOG`]), and should a game update move it, it is looked for by
//! its shape: a node with `tabs.competition` and `contents` under it
//! ([`sweep`]). That search runs only while the pause menu is up, which is
//! where the dialog is opened from: a sweep of the tree on every screen is
//! how a mod makes a game stutter. The title screen's Options has no row for
//! that reason, and no save to patch either.
//!
//! Which tab is showing cannot be asked of the tab: the host answers nothing
//! for a `color_selectable`'s selected state. The row is shown while none of
//! the rows the layout authors for the other three tabs is
//! ([`AUTHORED_ROWS`]).
//!
//! # The button
//!
//! The row is the layout's own rows over again, sizes and style included,
//! with one cell as wide as their two or three, and it grows by the height
//! of its note while that shows. It acts on the click: not on the dialog's
//! OK, and its Cancel does not take the click back.
//!
//! One event queues a patch, with nothing to confirm it, so the event has
//! to be the player's click and no other ([`handle_event`]). A hotfix was
//! seen to go out a second time, on the same matches, with no second click
//! behind it; what sent the button that second event is not known.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;

use mod_api_stable::{StableClient, UiEventKindV1};

// Where the dialog is in game 0.6.3, tried before any search.
const KNOWN_DIALOG: &str = "pause_ui.option.option";

// The pause menu's layout root, up only while that menu is.
const PAUSE_ROOT: &str = "pause_ui";

// The row's node, and under it its button and the note below the button.
const ROW: &str = "riot_manual_hotfix";
const BUTTON_UNDER: &str = ".options.queue";
const NOTE_UNDER: &str = ".note";

// The row's height in pixels: as the layout's own rows are, and with the
// note showing.
const ROW_HEIGHT: u32 = 40;
const ROW_HEIGHT_NOTED: u32 = 64;

// The note's check, and its words in the colour the mod's hints are in.
const CHECK_COLOR: &str = "#5fd38dff";
const NOTE_COLOR: &str = "#a5a5abff";

// The rows `option.ui` authors under `#contents`, every one of them on a tab
// other than Competition.
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

// The row's strings: this mod's key in the game's text, and the English.
const TITLE: (&str, &str) = ("item_patch.hotfix_row", "Run Manual Item Patch");
const QUEUE: (&str, &str) = ("item_patch.hotfix_queue", "Queue");
const NOTE: (&str, &str) = ("item_patch.hotfix_note", "A manual patch has been queued.");
const QUEUE_HINT: (&str, &str) = (
    "item_patch.hotfix_hint",
    "Queues a manual item patch. It is released with the next news, as a hotfix of the current version.",
);
const QUEUED_HINT: (&str, &str) = (
    "item_patch.hotfix_queued_hint",
    "A manual item patch is queued and will be released with the next news.",
);

// Frames between two looks for the dialog while the pause menu is up and it
// has not been found.
const SWEEP_EVERY: u32 = 20;

// How deep [`sweep`] goes under the roots, and how many nodes it asks about
// at most. A depth alone does not bound it: one wide level can be hundreds.
const SWEEP_DEPTH: usize = 4;
const SWEEP_NODES: usize = 600;

// Frames [`sync`] has run.
static FRAME: AtomicU32 = AtomicU32::new(0);

// Where the dialog was last found. Kept when it goes, since it comes back in
// the same place and is then one lookup away.
static FOUND: Mutex<Option<String>> = Mutex::new(None);

// Whether the dialog was up on the last frame.
static UP: AtomicBool = AtomicBool::new(false);

// What the row was last painted as ([`look`]): 0 for not yet.
static PAINTED: AtomicU8 = AtomicU8::new(0);

// Whether the row is on screen: the dialog up, and on the Competition tab.
static SHOWN: AtomicBool = AtomicBool::new(false);

// The frame the button was last disturbed on: the dialog opened, the row
// spawned or came into view, the button painted. An event that reaches the
// button within [`SETTLE_FRAMES`] of that is taken for the host's answer
// to it, and not for a click.
static SETTLED_AT: AtomicU32 = AtomicU32::new(0);
const SETTLE_FRAMES: u32 = 20;

// Events the button has been sent, to number them in the test log.
static EVENTS: AtomicU32 = AtomicU32::new(0);

// One of this mod's strings as a label takes it: the reference into the
// game's text where the mod's entry is there to resolve, which the label
// then shows in the game's language, else the English.
fn text(ctx: &StableClient<'_>, (key, fallback): (&str, &str)) -> String {
    let reference = format!("#asset/base/text/ui?{key}");
    match ctx.i18n(&reference) {
        Some(text) if !text.is_empty() && !text.starts_with('#') => reference,
        _ => fallback.to_string(),
    }
}

// Strips what would end a `.ui` string literal.
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '{' | '}' | ';'))
        .collect()
}

// The row, shaped like the layout's own: 846 pixels inside the box's
// padding, all of them the one button's. The note starts hidden, under the
// button and in line with its left edge.
fn row_source(ctx: &StableClient<'_>) -> String {
    format!(
        "{ROW}:empty {{\n\
         width: 1160px;\n\
         height: {ROW_HEIGHT}px;\n\
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
         #queue:color_selectable {{\n\
         @\"asset/base/style/main#strategy_option\";\n\
         width: 846px;\n\
         height: 32px;\n\
         label: {{ size: 16; }}\n\
         selected_label: {{ size: 16; }}\n\
         text: \"{queue}\";\n\
         hint: \"{hint}\";\n\
         }}\n\
         }}\n\
         \n\
         #note:empty {{\n\
         visible: false;\n\
         ignore_event: true;\n\
         x: 258px;\n\
         y: 42px;\n\
         width: 846px;\n\
         height: 20px;\n\
         \n\
         #check:image {{\n\
         ignore_event: true;\n\
         y: 3px;\n\
         width: 14px;\n\
         height: 14px;\n\
         source: \"asset/base/ui/icons/check\";\n\
         color: {CHECK_COLOR};\n\
         }}\n\
         \n\
         #words:label {{\n\
         @\"asset/base/style/main#label\";\n\
         x: 20px;\n\
         width: 826px;\n\
         height: 20px;\n\
         align_x: Left;\n\
         align_y: Center;\n\
         size: 13;\n\
         color: {NOTE_COLOR};\n\
         text: \"{note}\";\n\
         }}\n\
         }}\n\
         }}\n",
        title = escape(&text(ctx, TITLE)),
        queue = escape(&text(ctx, QUEUE)),
        hint = escape(&text(ctx, QUEUE_HINT)),
        note = escape(&text(ctx, NOTE)),
    )
}

// The row's look as one number: whether a hotfix is queued. Never 0.
fn look() -> u8 {
    1 + crate::patches::hotfix_queued() as u8
}

// Paints the row at `row` as the queue stands. With a hotfix queued the
// button is greyed out, the note under it shows and the row is as much
// taller; with none the button is the plain one it was spawned as.
//
// Painted, not set: see `strategy_ui::paint_toggle` for why a
// `color_selectable` is given the same look in both of its states.
fn paint(ctx: &mut StableClient<'_>, row: &str) {
    let queued = crate::patches::hotfix_queued();
    PAINTED.store(look(), Ordering::Relaxed);
    SETTLED_AT.store(FRAME.load(Ordering::Relaxed), Ordering::Relaxed);
    let (style, hint, height) = if queued {
        (
            crate::strategy_ui::toggle_off_style_sized(false, 16),
            QUEUED_HINT,
            ROW_HEIGHT_NOTED,
        )
    } else {
        (
            crate::strategy_ui::toggle_style_sized(false, 16),
            QUEUE_HINT,
            ROW_HEIGHT,
        )
    };
    let hint = format!("hint: \"{}\";", escape(&text(ctx, hint)));
    let button = format!("{row}{BUTTON_UNDER}");
    ctx.ui_set_properties(&button, &style);
    ctx.ui_set_properties(&button, &hint);
    ctx.ui_set_properties(row, &format!("height: {height}px;"));
    ctx.ui_set_visible(&format!("{row}{NOTE_UNDER}"), queued);
}

// A click on the button queues a hotfix.
//
// A click and nothing else: an event that says it is one (an unreported
// kind is not taken for one here, as it is where a handler only asserts a
// value), sent while the row is on screen, and not in the moment after the
// button was disturbed ([`SETTLED_AT`]). A selectable that the host selects
// again when it builds or shows the dialog, or that answers a property
// write with an event, would otherwise queue a patch nobody asked for.
// Every event is put in the test log with what was made of it.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    let Some(row) = event.path.strip_suffix(BUTTON_UNDER) else {
        return;
    };
    if !row.ends_with(ROW) {
        return;
    }
    let shown = SHOWN.load(Ordering::Relaxed);
    let since = FRAME
        .load(Ordering::Relaxed)
        .wrapping_sub(SETTLED_AT.load(Ordering::Relaxed));
    let click = event.kind == Some(UiEventKindV1::Click) && shown && since >= SETTLE_FRAMES;
    let number = EVENTS.fetch_add(1, Ordering::Relaxed);
    crate::patches::log("patch.button", || {
        format!(
            "event {number}: {:?}, {since} frames after the button was last disturbed, row on screen={shown}, queued already={}: {}",
            event.kind,
            crate::patches::hotfix_queued(),
            if click { "taken as a click" } else { "left alone" }
        )
    });
    if !click {
        return;
    }
    // Nothing more to queue while one waits: the greyed-out button takes the
    // click and does nothing with it.
    crate::patches::queue_hotfix();
    paint(ctx, row);
}

// Whether `path` is the dialog: the node holding the tab bar and the rows.
fn is_dialog(ctx: &StableClient<'_>, path: &str) -> bool {
    ctx.ui_exists(&format!("{path}.tabs.competition")) && ctx.ui_exists(&format!("{path}.contents"))
}

// Breadth-first from the layout roots for the dialog, the pause menu's own
// root first, since that is the menu it is opened from. Bounded by depth and
// by node count.
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

// The dialog's path, when it is in the tree. Where it was last found is
// asked first, on every frame; a search for it is made only now and then,
// and only while the pause menu is up.
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
    if let (Some(path), Ok(mut kept)) = (&found, FOUND.lock()) {
        *kept = Some(path.clone());
    }
    found
}

// Whether the Competition tab is the one showing. See the module notes for
// why there are two tests.
fn competition_up(ctx: &StableClient<'_>, dialog: &str) -> bool {
    ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition")) == Some(true)
        || !AUTHORED_ROWS
            .iter()
            .any(|row| ctx.ui_visible(&format!("{dialog}.contents.{row}")) == Some(true))
}

// Keeps the row in the dialog and on its tab. Every client frame; with the
// dialog out of the tree this is one lookup.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    // No row where there are no patches to queue.
    if !crate::patches::hotfixes_offered() {
        return;
    }
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    let Some(dialog) = locate(ctx, frame) else {
        UP.store(false, Ordering::Relaxed);
        SHOWN.store(false, Ordering::Relaxed);
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
        SHOWN.store(false, Ordering::Relaxed);
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
        if !ctx.ui_spawn_source(&contents, &source) {
            return;
        }
        paint(ctx, &row);
    }
    // On a row just spawned, and again each time the dialog opens: in the
    // pause menu's layout a registration does not outlast the dialog closing
    // or the game being saved, and a button registered once goes dead. Where
    // the host has kept an earlier one the click arrives once for each,
    // which queues nothing twice.
    if fresh || opened {
        ctx.ui_register_path_events(&format!("{row}{BUTTON_UNDER}"), handle_event);
        SETTLED_AT.store(frame, Ordering::Relaxed);
    }
    // The queue can empty under an open dialog, when its patch goes out or
    // another save is read: the row says how it stands now.
    if PAINTED.load(Ordering::Relaxed) != look() {
        paint(ctx, &row);
    }
    let competition = competition_up(ctx, &dialog);
    ctx.ui_set_visible(&row, competition);
    // Coming into view disturbs the button as opening the dialog does.
    if competition && !SHOWN.swap(competition, Ordering::Relaxed) {
        SETTLED_AT.store(frame, Ordering::Relaxed);
    }
    if !competition {
        SHOWN.store(false, Ordering::Relaxed);
    }
}
