//! Test log for `own_team_only`: why the player's athletes do or do not get
//! their pin-aware build at spawn, the first match of a session especially.
//!
//! Writes `own-team-debug.log` next to the DLL, truncated at the first line of
//! a session. Turn [`ENABLED`] off before a release: the spawn and buy detours
//! call this from the sim thread.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

pub const ENABLED: bool = false;

/// The open file and when the session started, once the first line is written.
static LOG: Mutex<Option<(std::fs::File, Instant)>> = Mutex::new(None);

/// Appends one line, prefixed with the seconds since the session's first line
/// and the thread it came from.
pub fn line(text: impl FnOnce() -> String) {
    if !ENABLED {
        return;
    }
    let Ok(mut log) = LOG.lock() else {
        return;
    };
    if log.is_none() {
        let path = crate::config::mod_dir().join("own-team-debug.log");
        let Ok(file) = std::fs::File::create(path) else {
            return;
        };
        *log = Some((file, Instant::now()));
    }
    if let Some((file, start)) = log.as_mut() {
        let thread = std::thread::current();
        let _ = writeln!(
            file,
            "[{:9.3}] [{}] {}",
            start.elapsed().as_secs_f64(),
            thread
                .name()
                .map_or_else(|| format!("{:?}", thread.id()), str::to_string),
            text()
        );
    }
}

/// The last text [`on_change`] wrote under each key.
static LAST: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// [`line`], but only when `text` differs from what was last written under
/// `key`: for state read every frame.
pub fn on_change(key: &str, text: String) {
    if !ENABLED {
        return;
    }
    {
        let Ok(mut last) = LAST.lock() else {
            return;
        };
        let last = last.get_or_insert_with(HashMap::new);
        if last.get(key) == Some(&text) {
            return;
        }
        last.insert(key.to_string(), text.clone());
    }
    line(|| format!("{key}: {text}"));
}
