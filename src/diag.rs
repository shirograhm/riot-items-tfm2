//! Opt-in file log for the item-build path.
//!
//! The two halves that decide builds are both invisible from inside the game:
//! `hook::detour` runs on sim workers and `item_build_hook::decide_build` runs
//! once per player as a match's builds are decided. Neither can print anywhere
//! the player can see - `eprintln!` goes to a stderr no Steam launch attaches -
//! so "the editor's build was ignored, but at which step" has never been
//! answerable without a debugger.
//!
//! # Turning it on
//!
//! Create an empty file named [`MARKER`] next to the mod DLL - the *deployed*
//! copy, in the game's `mods/riot_items_tfm2/` folder, not the repo. Lines then
//! append to [`LOG`] in that same folder. Delete the marker to turn it off
//! again; no rebuild either way.
//!
//! The check is a `OnceLock`, so the marker is read once per session: creating
//! it while the game is running does nothing until the next launch.
//!
//! # Why it is capped
//!
//! `decide_build` fires for every player of every fixture on a league day, on
//! parallel rayon workers. Left uncapped, one day of background sims would bury
//! the match the player actually ran and grow the file without bound. Callers
//! also decide *what* is worth a line - see `item_build_hook`, which logs every
//! configured champion but only the first few of everything else.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Empty file next to the DLL that turns logging on.
const MARKER: &str = "item-build-diag.on";

/// Where the lines go, in the same folder.
const LOG: &str = "item-build-diag.log";

/// Total lines this session will write before it goes quiet.
const MAX_LINES: usize = 20_000;

static WRITTEN: AtomicUsize = AtomicUsize::new(0);

/// Whether the marker file was present when the mod loaded.
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| crate::config::dll_dir().is_some_and(|dir| dir.join(MARKER).exists()))
}

/// Appends one line, if logging is on and the cap is not spent.
///
/// The file is opened per line rather than held: the cap keeps that to a few
/// thousand opens for a whole session, and a held handle would have to survive
/// being written from several sim workers at once.
pub fn log(line: &str) {
    if !enabled() {
        return;
    }
    let seq = WRITTEN.fetch_add(1, Ordering::Relaxed);
    if seq >= MAX_LINES {
        return;
    }
    let Some(dir) = crate::config::dll_dir() else {
        return;
    };
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG))
    {
        // The sequence number is the point of the prefix, not decoration. A
        // build decision carries no match id, so the only way to say which
        // match a `decide` belongs to is where it falls relative to that
        // match's `start` - and sim workers append out of order, so raw file
        // order cannot answer it.
        let _ = writeln!(file, "{seq:06} {line}");
    }
}
