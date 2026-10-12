//! The game's Options dialog as the mod finds it: where it is in the UI
//! tree, whether it is showing, and whether its Competition tab is the one
//! up. For the row the mod adds to that tab (`super::patch_settings`).
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
//! for a `color_selectable`'s selected state. The Competition tab is taken to
//! be up while none of the rows the layout authors for the other three tabs
//! is showing ([`AUTHORED_ROWS`]).

use std::sync::Mutex;

use mod_api_stable::StableClient;

// Where the dialog is in game 0.6.3, tried before any search.
const KNOWN_DIALOG: &str = "pause_ui.option.option";

// The pause menu's layout root, up only while that menu is.
const PAUSE_ROOT: &str = "pause_ui";

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

// Frames between two looks for the dialog while the pause menu is up and it
// has not been found.
const SWEEP_EVERY: u32 = 20;

// How deep [`sweep`] goes under the roots, and how many nodes it asks about
// at most. A depth alone does not bound it: one wide level can be hundreds.
const SWEEP_DEPTH: usize = 4;
const SWEEP_NODES: usize = 600;

// Where the dialog was last found. Kept when it goes, since it comes back in
// the same place and is then one lookup away.
static FOUND: Mutex<Option<String>> = Mutex::new(None);

// One of this mod's strings as a label takes it: the reference into the
// game's text where the mod's entry is there to resolve, which the label
// then shows in the game's language, else the English.
pub(super) fn text(ctx: &StableClient<'_>, (key, fallback): (&str, &str)) -> String {
    let reference = format!("#asset/base/text/ui?{key}");
    match ctx.i18n(&reference) {
        Some(text) if !text.is_empty() && !text.starts_with('#') => reference,
        _ => fallback.to_string(),
    }
}

// Strips what would end a `.ui` string literal.
pub(super) fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '{' | '}' | ';'))
        .collect()
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
pub(super) fn competition_up(ctx: &StableClient<'_>, dialog: &str) -> bool {
    ctx.ui_selectable_selected(&format!("{dialog}.tabs.competition")) == Some(true)
        || !AUTHORED_ROWS
            .iter()
            .any(|row| ctx.ui_visible(&format!("{dialog}.contents.{row}")) == Some(true))
}

// The dialog's path while it is showing, on the `frame`th frame of whoever
// asks. With the dialog out of the tree this is one lookup.
//
// The layout's own root is authored hidden and game code shows it; it is the
// dialog's parent wherever the layout was hung. A host that does not say is
// taken to be showing it: a row is a child of the dialog, so the worst that
// costs is the tab test on every frame.
pub(super) fn showing(ctx: &StableClient<'_>, frame: u32) -> Option<String> {
    let dialog = locate(ctx, frame)?;
    let parent = dialog.rsplit_once('.').map(|(parent, _)| parent);
    let up = [parent, Some(dialog.as_str())]
        .into_iter()
        .flatten()
        .all(|node| ctx.ui_visible(node) != Some(false));
    up.then_some(dialog)
}
