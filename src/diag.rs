use std::fs::OpenOptions;
use std::io::Write as _;

const LOG_FILE: &str = "riot-items.log";

pub(crate) fn write(msg: &str) {
    let path = crate::config::mod_dir().join(LOG_FILE);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{msg}");
    }
}
