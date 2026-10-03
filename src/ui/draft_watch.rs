//! Which champions each side picked in the draft just played, for the Build
//! Editor's Blue Team / Red Team buttons.
//!
//! # Why it is read off the draft screen
//!
//! Nothing hands a mod the lineup while the player can still act on it. The
//! draft hook does not fire for the player's own match, the item-build hook
//! only sees the lineup at Start Match, no record holds an upcoming draft, and
//! the Tactics screen's Matchup card and the draft's own pick slots show a
//! champion as an image, which reads back as nothing.
//!
//! The draft screen's champion grid is the one place it is text. Every slot
//! (`banpick/champion_slot.ui`) carries the champion's name in a `#name` label,
//! and a `#blue` or `#red` badge that the game shows once that side has picked
//! it, with a number in the badge's `#text`. So the grid is read while the
//! draft is up, and what it showed is kept for the Tactics screen after it.
//!
//! # What it keeps
//!
//! What the grid showed, as it showed it: the label's text in whatever form
//! the label holds it, and the slot's node name beside it. Turning those into
//! a champion the editor knows is `strategy_ui`'s business, which has the
//! champion list.
//!
//! A pick is added and never taken back for as long as one draft lasts. The
//! grid can be filtered by class, position and search, so a champion picked
//! earlier is often not on it; and what the grid does in the moment the draft
//! ends is not known, so a slot seen without its badge is not taken as proof
//! of anything. A new draft starts from nothing.
//!
//! None of this has been confirmed in game. [`crate::own_team_log`] reports
//! what the grid looked like and every pick read off it.

use std::sync::Mutex;

use mod_api_stable::StableClient;

/// The draft screen's champion grid. The screen's root is `main`, like every
/// screen's; `#champions` is its scroll view and `#contents` the table of
/// slots inside it (`banpick/layout.ui`).
const GRID: &str = "main.champions.contents";

/// Frames between two readings of the grid. A pick stands for seconds, and a
/// reading is two lookups for each of some seventy slots.
const SCAN_FRAMES: u32 = 15;

/// One champion a side picked, as the grid showed it.
#[derive(Clone, PartialEq)]
pub(crate) struct Pick {
    /// The slot's node name. The champion's id if the game names slots for
    /// their champion, a position in the grid if it does not.
    pub(crate) node: String,
    /// The `#name` label's text: a name in the game's language, or the
    /// `#asset/...` reference the label resolves one from. Empty when the label
    /// gave nothing.
    pub(crate) name: String,
    red: bool,
    /// The number on the badge, or `usize::MAX` when it is not one.
    order: usize,
}

impl Pick {
    /// Whether this is the champion in the slot `node` showing `name`. By name
    /// where there is one: a filtered grid may reuse a node for another
    /// champion, and a name cannot be reused.
    fn is(&self, node: &str, name: &str) -> bool {
        if self.name.is_empty() || name.is_empty() {
            self.node == node
        } else {
            self.name == name
        }
    }
}

struct Watch {
    /// Whether the grid was up on the last frame: a draft is on.
    drafting: bool,
    frames: u32,
    /// Whether this draft's grid has been written to the log yet.
    described: bool,
    picks: Vec<Pick>,
    /// Goes up whenever `picks` changes, so a reader can tell without
    /// comparing them.
    revision: u64,
}

static WATCH: Mutex<Watch> = Mutex::new(Watch {
    drafting: false,
    frames: 0,
    described: false,
    picks: Vec::new(),
    revision: 0,
});

/// Reads the draft screen's picks while it is up. Called every client frame;
/// off the draft screen it is one failed path lookup.
pub(crate) fn sync(ctx: &StableClient<'_>) {
    let up = ctx.ui_exists(GRID);
    let Ok(mut watch) = WATCH.lock() else {
        return;
    };
    if !up {
        // The picks stay: the Tactics screen comes after the draft.
        watch.drafting = false;
        return;
    }
    if !watch.drafting {
        // A new draft. The picks held are another match's, or another game's
        // of the same set.
        watch.drafting = true;
        watch.frames = 0;
        watch.described = false;
        if !watch.picks.is_empty() {
            watch.picks.clear();
            watch.revision += 1;
        }
    }
    let due = watch.frames % SCAN_FRAMES == 0;
    watch.frames = watch.frames.wrapping_add(1);
    if !due {
        return;
    }

    let children = ctx.ui_child_names(GRID);
    if let (false, Some(first)) = (watch.described, children.first()) {
        // Once per draft, and not before the grid has slots to describe.
        watch.described = true;
        crate::own_team_log::line(|| {
            format!(
                "draft grid: {} slots, first `{first}` name={:?}",
                children.len(),
                ctx.ui_text(&format!("{GRID}.{first}.name"))
            )
        });
    }
    for child in children {
        let slot = format!("{GRID}.{child}");
        let Some((badge, red)) = [("blue", false), ("red", true)]
            .into_iter()
            .find(|(badge, _)| ctx.ui_visible(&format!("{slot}.{badge}")) == Some(true))
        else {
            continue;
        };
        let name = ctx
            .ui_text(&format!("{slot}.name"))
            .map(|name| name.trim().to_string())
            .unwrap_or_default();
        let order = ctx
            .ui_text(&format!("{slot}.{badge}.text"))
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(usize::MAX);
        let held = watch.picks.iter().position(|pick| pick.is(&child, &name));
        let pick = Pick {
            node: child,
            name,
            red,
            order,
        };
        if held.is_some_and(|index| watch.picks[index] == pick) {
            continue;
        }
        crate::own_team_log::line(|| {
            format!(
                "draft pick: {badge} #{order} node=`{}` name={:?}",
                pick.node, pick.name
            )
        });
        match held {
            Some(index) => watch.picks[index] = pick,
            None => watch.picks.push(pick),
        }
        watch.revision += 1;
    }
}

/// One side's picks, in the order of the numbers on their badges.
pub(crate) fn picks(red: bool) -> Vec<Pick> {
    let Ok(watch) = WATCH.lock() else {
        return Vec::new();
    };
    let mut side: Vec<Pick> = watch
        .picks
        .iter()
        .filter(|pick| pick.red == red)
        .cloned()
        .collect();
    // Stable, so picks whose badge held no number keep the order they were
    // seen in, after the numbered ones.
    side.sort_by_key(|pick| pick.order);
    side
}

/// A number that changes whenever the picks held do.
pub(crate) fn revision() -> u64 {
    WATCH.lock().map_or(0, |watch| watch.revision)
}

/// Drops the picks held. For when the match they were drafted for has begun:
/// a Tactics screen reached without a draft before it must not be told about
/// the last one.
pub(crate) fn forget() {
    let Ok(mut watch) = WATCH.lock() else {
        return;
    };
    if !watch.picks.is_empty() {
        watch.picks.clear();
        watch.revision += 1;
    }
}
