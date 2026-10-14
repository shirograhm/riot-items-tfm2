//! **Import** and **Export** in the Item Patcher Settings window: every
//! setting the window keeps for this save, New Value, Minimum, Maximum and
//! Protected, written to a file outside the save and read back from it into
//! another.
//!
//! The file is in a folder of this mod's in the game's own folder under
//! `%APPDATA%` (`TeamSamoyed\TeamfightManager2`, where the saves are too), not
//! beside the DLL: beside the DLL it would be one file for the Workshop's copy
//! of the mod and another for a copy put in the mods folder by hand, and an
//! update of the Workshop's copy is not known to leave it be.
//!
//! An import puts the file's settings in place of the save's, all of them: a
//! number the file has nothing for has nothing afterwards. What this save
//! cannot take is left out and counted: anything of an item or a number no
//! patch moves in this build, a New Value the window would refuse
//! ([`super::settable`]), a Minimum over the Maximum. A New Value the number
//! reads already is left out as well, as the window leaves it, but not
//! counted: there is nothing in it to release.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{clean, settable, ERROR_COLOR, WAITING_COLOR};
use crate::patches::base::{self, Member};
use crate::patches::state::{Limit, State};
use crate::patches::{balance, fields, text};

// The shape of the file this build writes, and the newest it reads.
const FORMAT: u32 = 1;

// The file, and its folder in the game's.
const FOLDER: &str = "riot_items_tfm2";
const FILE: &str = "item-patch-settings.json";

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Settings {
    format: u32,
    // Item, one tier by its key -> config field -> what is set for it.
    items: BTreeMap<String, BTreeMap<String, Setting>>,
}

// One number's settings, as its row in the window has them.
#[derive(Serialize, Deserialize, Default, Clone, Copy)]
#[serde(default)]
struct Setting {
    #[serde(rename = "new_value", skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    protected: bool,
}

// Where the file is. Beside the DLL only where Windows gives no `%APPDATA%`.
fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|roaming| PathBuf::from(roaming).join("TeamSamoyed").join("TeamfightManager2"))
        .unwrap_or_else(crate::config::mod_dir)
        .join(FOLDER)
        .join(FILE)
}

// One of the window's strings with a count in it, `{N}`, and the file,
// `{P}`. The file with forward slashes, which a label cannot take for
// anything but what they are.
fn said_with(key: &str, english: &str, count: usize) -> String {
    let path = path().display().to_string().replace('\\', "/");
    clean(
        &text::ui_string(key, english)
            .replace("{N}", &count.to_string())
            .replace("{P}", &path),
    )
}

// A line of the footer's that says something went wrong.
fn failed(line: String) -> String {
    format!("<{ERROR_COLOR}>{line}<>")
}

// Writes the save's settings to the file, over whatever it held. What the
// footer is to say of it.
pub(super) fn export() -> String {
    let settings = crate::patches::settings(|state| {
        let mut items: BTreeMap<String, BTreeMap<String, Setting>> = BTreeMap::new();
        for (key, fields) in &state.staged {
            for (field, &value) in fields {
                items
                    .entry(key.clone())
                    .or_default()
                    .entry(field.clone())
                    .or_default()
                    .value = Some(value);
            }
        }
        for (key, fields) in &state.limits {
            for (field, limit) in fields {
                let setting = items
                    .entry(key.clone())
                    .or_default()
                    .entry(field.clone())
                    .or_default();
                setting.min = limit.min;
                setting.max = limit.max;
                setting.protected = limit.protected;
            }
        }
        Settings {
            format: FORMAT,
            items,
        }
    })
    .unwrap_or_default();
    let count = settings.items.values().map(BTreeMap::len).sum();
    // A save with nothing set leaves the file as it is: it may hold another
    // save's settings, kept for this one.
    if count == 0 {
        return said_with(
            "settings_export_empty",
            "Nothing to export: no number has a setting in this save.",
            0,
        );
    }
    let path = path();
    let written = path
        .parent()
        .is_some_and(|folder| std::fs::create_dir_all(folder).is_ok())
        && serde_json::to_string_pretty(&settings)
            .is_ok_and(|json| std::fs::write(&path, json).is_ok());
    if written {
        said_with(
            "settings_exported",
            "Exported the settings of {N} number(s) to {P}",
            count,
        )
    } else {
        failed(said_with("settings_export_failed", "Could not write {P}", count))
    }
}

// Puts the file's settings in place of the save's. What the footer is to
// say of it; nothing where no save's state is there to put them in.
pub(super) fn import() -> Option<String> {
    let unreadable = || failed(said_with("settings_import_failed", "Could not read {P}", 0));
    let json = match std::fs::read_to_string(path()) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(failed(said_with(
                "settings_import_missing",
                "No exported settings found at {P}",
                0,
            )));
        }
        Err(_) => return Some(unreadable()),
    };
    let Some(settings) = serde_json::from_str::<Settings>(json.trim_start_matches('\u{feff}'))
        .ok()
        .filter(|settings| settings.format <= FORMAT)
    else {
        return Some(unreadable());
    };

    // The numbers that have something set once the file is in, and the
    // settings of the file that could not be.
    let (mut numbers, mut left) = (0, 0);
    let edited = crate::patches::edit_settings(|state| {
        let mut staged: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
        let mut limits: BTreeMap<String, BTreeMap<String, Limit>> = BTreeMap::new();
        for (key, fields) in &settings.items {
            for (field, setting) in fields {
                let (value, limit, refused) = take(state, key, field, setting);
                left += refused;
                if value.is_none() && limit.is_empty() {
                    continue;
                }
                numbers += 1;
                if let Some(value) = value {
                    staged.entry(key.clone()).or_default().insert(field.clone(), value);
                }
                if !limit.is_empty() {
                    limits.entry(key.clone()).or_default().insert(field.clone(), limit);
                }
            }
        }
        state.staged = staged;
        state.limits = limits;
    });
    if !edited {
        return None;
    }
    let mut notice = said_with(
        "settings_imported",
        "Imported the settings of {N} number(s).",
        numbers,
    );
    if left > 0 {
        let refused = said_with(
            "settings_import_left",
            "{N} setting(s) in the file could not be used in this save.",
            left,
        );
        notice.push_str(&format!(" <{WAITING_COLOR}>{refused}<>"));
    }
    Some(notice)
}

// What of `setting` the number `field` of the item `key` can have in this
// save: its New Value, what it allows an automatic patch, and how many of
// its settings it cannot have.
fn take(state: &State, key: &str, field: &str, setting: &Setting) -> (Option<f64>, Limit, usize) {
    let mut limit = Limit {
        min: setting.min,
        max: setting.max,
        protected: setting.protected,
    };
    let parts = [
        setting.value.is_some(),
        limit.min.is_some(),
        limit.max.is_some(),
        limit.protected,
    ]
    .into_iter()
    .filter(|&set| set)
    .count();
    // A number the window has a row for, found as it finds them.
    let found = member(key).and_then(|member| {
        let rule = fields::rule_for(key, field)?;
        let unpatched = *member.values.get(field)?;
        let current = balance::reads(state, member, field)?;
        Some((member, rule, unpatched, current))
    });
    let Some((member, rule, unpatched, current)) = found else {
        return (None, Limit::default(), parts);
    };
    let mut refused = 0;
    let value = setting.value.and_then(|value| {
        match settable(member.whole.contains(field), rule.unsigned, unpatched, value) {
            Ok(()) => ((value - current).abs() > 1e-9).then_some(value),
            Err(_) => {
                refused += 1;
                None
            }
        }
    });
    if let (Some(min), Some(max)) = (limit.min, limit.max) {
        if min > max {
            limit.min = None;
            limit.max = None;
            refused += 2;
        }
    }
    (value, limit, refused)
}

// The tier `key` of an item a patch can move.
fn member(key: &str) -> Option<&'static Member> {
    base::base()
        .families
        .values()
        .flat_map(|family| &family.members)
        .find(|member| member.key == key)
}
