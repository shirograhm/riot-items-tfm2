//! Append-only logging support for the item-build hook.
//!
//! Records land in a `reports/` folder next to the mod DLL so they survive a
//! renamed mod folder. Logging failures are non-fatal: callers use
//! `let _ = report::record(..)` and the hook keeps working without a log.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static REPORT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn report_path() -> Result<PathBuf, String> {
    let directory = crate::config::dll_dir()
        .ok_or_else(|| "could not resolve mod directory".to_string())?
        .join("reports");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory.join("item-build-hook.log"))
}

pub fn record(message: &str) -> Result<(), String> {
    let _guard = REPORT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|error| format!("report lock poisoned: {error}"))?;
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(report_path()?)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{unix}\t{message}").map_err(|error| error.to_string())
}
