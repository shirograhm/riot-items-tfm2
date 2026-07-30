//! Append-only diagnostic log shared by the client UI and the hook.
//!
//! The mod has no other channel: `StableHost` owns `log` but is documented as
//! valid only inside the callback that receives it, extensions never get one,
//! and `eprintln!` goes nowhere when the game runs without a console — which is
//! how the hook's install result went unnoticed while every build config
//! silently failed to apply.

use std::fs::OpenOptions;
use std::io::Write as _;

/// File the log is written to, next to the DLL (see `config::mod_dir`).
const LOG_FILE: &str = "riot-items.log";

/// Appends one line. Failures are swallowed: diagnostics must never take the
/// game down, and there is nowhere to report a failed report.
pub(crate) fn write(msg: &str) {
    let path = crate::config::mod_dir().join(LOG_FILE);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{msg}");
    }
}
