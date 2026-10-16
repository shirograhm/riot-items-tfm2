//! **Import** and **Export** in the Item Patcher Settings window: every
//! setting the window keeps for this save, New Value, Minimum, Maximum and
//! Protected, written to a file outside the save and read back from it into
//! another.
//!
//! Both open a Windows file dialog in the mod's own folder, beside the DLL
//! (`config::mod_dir`, so the Workshop's copy and one put in the mods folder
//! by hand each start in their own): Export a Save As dialog with
//! `item-patch-settings.json` filled in, Import an Open dialog. The player can
//! take either anywhere else, and Import any file an export wrote.
//!
//! A dialog runs on a thread of its own ([`pick`]). On the game's, it would
//! hold the frame it was opened in until it closed, and run its own message
//! loop inside the game's. It is owned by the game's window, so it stays in
//! front of it and the game takes no clicks while it is up, and the game
//! keeps drawing. The window asks [`finished`] every frame, and the file is
//! written or read there, on the game's thread, once one was chosen.
//!
//! An import puts the file's settings in place of the save's, all of them: a
//! number the file has nothing for has nothing afterwards. What this save
//! cannot take is left out and counted: anything of an item or a number no
//! patch moves in this build, a New Value the window would refuse
//! ([`super::settable`]), a Minimum over the Maximum. A New Value the number
//! reads already is left out as well, as the window leaves it, but not
//! counted: there is nothing in it to release.

use std::collections::BTreeMap;
use std::ffi::{c_void, OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{clean, said, settable, ERROR_COLOR, WAITING_COLOR};
use crate::patches::base::{self, Member};
use crate::patches::state::{Limit, State};
use crate::patches::{balance, fields, text};

// The shape of the file this build writes, and the newest it reads.
const FORMAT: u32 = 1;

// The name an export suggests, in the mod's folder.
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

// One of the window's strings with a count in it, `{N}`, and a file, `{P}`.
// The file with forward slashes, which a label cannot take for anything but
// what they are.
fn said_with(key: &str, english: &str, count: usize, file: Option<&Path>) -> String {
    let file = file
        .map(|file| file.display().to_string().replace('\\', "/"))
        .unwrap_or_default();
    clean(
        &text::ui_string(key, english)
            .replace("{N}", &count.to_string())
            .replace("{P}", &file),
    )
}

// A line of the footer's that says something went wrong.
fn failed(line: String) -> String {
    format!("<{ERROR_COLOR}>{line}<>")
}

// What the footer says while a dialog is up.
fn choosing() -> String {
    said("settings_choosing", "Choose a file in the window that opened.")
}

// -- the dialogs ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Export,
    Import,
}

// What a dialog gave back: the file chosen, or nothing for one cancelled.
struct Picked {
    kind: Kind,
    file: Option<PathBuf>,
}

// A dialog is up. One at a time: a click on either button while one is up
// does nothing.
static PICKING: AtomicBool = AtomicBool::new(false);

// What the last dialog gave back, until [`finished`] takes it.
static PICKED: Mutex<Option<Picked>> = Mutex::new(None);

// `OPENFILENAMEW` (commdlg.h), the dialog's whole description.
#[repr(C)]
struct OpenFileName {
    struct_size: u32,
    owner: *mut c_void,
    instance: *mut c_void,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom_filter: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    default_ext: *const u16,
    custom_data: isize,
    hook: *mut c_void,
    template_name: *const u16,
    reserved_pointer: *mut c_void,
    reserved: u32,
    flags_ex: u32,
}

const OFN_OVERWRITEPROMPT: u32 = 0x2;
// The dialog leaves the process's working folder where it was: the game
// finds its own files from it.
const OFN_NOCHANGEDIR: u32 = 0x8;
const OFN_PATHMUSTEXIST: u32 = 0x800;
const OFN_FILEMUSTEXIST: u32 = 0x1000;
const OFN_EXPLORER: u32 = 0x80000;

const COINIT_APARTMENTTHREADED: u32 = 0x2;
const COINIT_DISABLE_OLE1DDE: u32 = 0x4;

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(dialog: *mut OpenFileName) -> i32;
    fn GetSaveFileNameW(dialog: *mut OpenFileName) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
    fn CoUninitialize();
}

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcessId() -> u32;
}

// The game's window, as a number a thread can take: the one in front, where
// it is this process's, which it is while the player clicks in it. Nothing
// (0) otherwise, and the dialog then has no owner.
fn game_window() -> usize {
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return 0;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        if process == GetCurrentProcessId() {
            window as usize
        } else {
            0
        }
    }
}

// Opens the dialog for `kind` on a thread of its own. Whether it opened: not
// while another is up.
fn pick(kind: Kind) -> bool {
    if PICKING.swap(true, Ordering::AcqRel) {
        return false;
    }
    let owner = game_window();
    let folder = crate::config::mod_dir();
    let name = text::ui_string("settings_row", "Item Patcher Settings");
    let title = match kind {
        Kind::Export => format!("{name} - {}", text::ui_string("settings_export", "Export")),
        Kind::Import => format!("{name} - {}", text::ui_string("settings_import", "Import")),
    };
    let spawned = std::thread::Builder::new()
        .name("riot-items-file-dialog".to_string())
        .spawn(move || {
            // SAFETY: everything the dialog is handed lives on this thread
            // until it returns.
            let file = unsafe { file_dialog(kind, owner, &folder, &title, &name) };
            if let Ok(mut picked) = PICKED.lock() {
                *picked = Some(Picked { kind, file });
            }
            PICKING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        PICKING.store(false, Ordering::Release);
        return false;
    }
    true
}

// The Save As or Open dialog, in `folder`, for JSON files: the file chosen,
// or nothing where the player cancelled (or the dialog could not open).
unsafe fn file_dialog(
    kind: Kind,
    owner: usize,
    folder: &Path,
    title: &str,
    files: &str,
) -> Option<PathBuf> {
    let com = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) >= 0;
    let wide = |text: &OsStr| -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() };
    // Pairs of strings, each ended by a NUL, and one more NUL after the last.
    let filter: Vec<u16> = format!("{files} (*.json)\0*.json\0\0").encode_utf16().collect();
    let folder = wide(folder.as_os_str());
    let title = wide(OsStr::new(title));
    let extension = wide(OsStr::new("json"));
    let mut file = [0u16; 4096];
    if kind == Kind::Export {
        for (slot, unit) in file.iter_mut().zip(FILE.encode_utf16()) {
            *slot = unit;
        }
    }
    let mut dialog: OpenFileName = std::mem::zeroed();
    dialog.struct_size = std::mem::size_of::<OpenFileName>() as u32;
    dialog.owner = owner as *mut c_void;
    dialog.filter = filter.as_ptr();
    dialog.filter_index = 1;
    dialog.file = file.as_mut_ptr();
    dialog.max_file = file.len() as u32;
    dialog.initial_dir = folder.as_ptr();
    dialog.title = title.as_ptr();
    dialog.default_ext = extension.as_ptr();
    dialog.flags = OFN_EXPLORER
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | match kind {
            Kind::Export => OFN_OVERWRITEPROMPT,
            Kind::Import => OFN_FILEMUSTEXIST,
        };
    let chosen = match kind {
        Kind::Export => GetSaveFileNameW(&mut dialog),
        Kind::Import => GetOpenFileNameW(&mut dialog),
    } != 0;
    if com {
        CoUninitialize();
    }
    if !chosen {
        return None;
    }
    let len = file.iter().position(|&unit| unit == 0).unwrap_or(file.len());
    Some(PathBuf::from(OsString::from_wide(&file[..len])))
}

// -- what the window asks -----------------------------------------------------------

// Export: the Save As dialog, where this save has anything to export. What
// the footer is to say now; nothing where a dialog is up already.
pub(super) fn start_export() -> Option<String> {
    if snapshot().items.is_empty() {
        return Some(said_with(
            "settings_export_empty",
            "Nothing to export: no number has a setting in this save.",
            0,
            None,
        ));
    }
    pick(Kind::Export).then(choosing)
}

// Import: the Open dialog. What the footer is to say now; nothing where a
// dialog is up already.
pub(super) fn start_import() -> Option<String> {
    pick(Kind::Import).then(choosing)
}

// A dialog that has closed, acted on: the file written or read.
pub(super) struct Finished {
    // What the footer is to say; nothing for a dialog cancelled, or an
    // import with no save's state to go into.
    pub(super) notice: Option<String>,
    // The save's settings were replaced: the page is to show them.
    pub(super) imported: bool,
}

// What the last dialog came to, once it has closed. Every frame, from the
// window; also while it is not up, so a file chosen is never acted on late.
pub(super) fn finished() -> Option<Finished> {
    let picked = PICKED.lock().ok()?.take()?;
    let Some(file) = picked.file else {
        return Some(Finished {
            notice: None,
            imported: false,
        });
    };
    Some(match picked.kind {
        Kind::Export => Finished {
            notice: Some(export_to(&file)),
            imported: false,
        },
        Kind::Import => Finished {
            notice: import_from(&file),
            imported: true,
        },
    })
}

// -- the file -------------------------------------------------------------------

// The save's settings as a file has them.
fn snapshot() -> Settings {
    crate::patches::settings(|state| {
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
    .unwrap_or_default()
}

// Writes the save's settings to `file`, over whatever it held. What the
// footer is to say of it.
fn export_to(file: &Path) -> String {
    let settings = snapshot();
    let count = settings.items.values().map(BTreeMap::len).sum();
    if count == 0 {
        return said_with(
            "settings_export_empty",
            "Nothing to export: no number has a setting in this save.",
            0,
            None,
        );
    }
    let written = serde_json::to_string_pretty(&settings)
        .is_ok_and(|json| std::fs::write(file, json).is_ok());
    if written {
        said_with(
            "settings_exported",
            "Exported the settings of {N} number(s) to {P}",
            count,
            Some(file),
        )
    } else {
        failed(said_with("settings_export_failed", "Could not write {P}", count, Some(file)))
    }
}

// Puts the settings in `file` in place of the save's. What the footer is to
// say of it; nothing where no save's state is there to put them in.
fn import_from(file: &Path) -> Option<String> {
    let unreadable = || {
        failed(said_with(
            "settings_import_failed",
            "Could not read {P}",
            0,
            Some(file),
        ))
    };
    let json = match std::fs::read_to_string(file) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(failed(said_with(
                "settings_import_missing",
                "No exported settings found at {P}",
                0,
                Some(file),
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
        None,
    );
    if left > 0 {
        let refused = said_with(
            "settings_import_left",
            "{N} setting(s) in the file could not be used in this save.",
            left,
            None,
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
