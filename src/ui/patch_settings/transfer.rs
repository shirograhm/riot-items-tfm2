//! **Import** and **Export** in the Item Patcher Settings window: every setting
//! the window keeps for this save, written to a file and read back into another
//! save.
//!
//! Both open a Windows file dialog in the mod's own folder every time
//! ([`show_dialog`]), on a thread of its own owned by the game's window
//! ([`pick`]), so the game keeps drawing. The window polls [`finished`] and
//! does the file work on the game's thread.
//!
//! An import replaces all of the save's settings. What this save cannot take
//! (an unpatched item or number, a New Value the window would refuse, a Minimum
//! over the Maximum) is left out and counted.

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

// The shell's `IFileOpenDialog`/`IFileSaveDialog` through `IFileDialog`, not
// `GetOpenFileNameW`, which opens in the given folder only the first time;
// `SetFolder` works every time.

// A COM GUID.
#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

const CLSID_FILE_OPEN_DIALOG: Guid = Guid(
    0xdc1c_5a9c,
    0xe88a,
    0x4dde,
    [0xa5, 0xa1, 0x60, 0xf8, 0x2a, 0x20, 0xae, 0xf7],
);
const CLSID_FILE_SAVE_DIALOG: Guid = Guid(
    0xc0b4_e2f3,
    0xba21,
    0x4773,
    [0x8d, 0xba, 0x33, 0x5e, 0xc9, 0x46, 0xeb, 0x8b],
);
const IID_FILE_DIALOG: Guid = Guid(
    0x42f8_5136,
    0xdb7e,
    0x439c,
    [0x85, 0xf1, 0xe4, 0x07, 0x5d, 0x13, 0x5f, 0xc8],
);
const IID_SHELL_ITEM: Guid = Guid(
    0x4382_6d1e,
    0xe718,
    0x42ee,
    [0xbc, 0x55, 0xa1, 0xe2, 0x61, 0xc3, 0x7b, 0xfe],
);

// `IFileDialog`'s methods up to the last one called, in their order; a
// method not called is a pointer this never reads.
#[repr(C)]
struct FileDialogCalls {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut FileDialog) -> u32,
    show: unsafe extern "system" fn(*mut FileDialog, *mut c_void) -> i32,
    set_file_types: unsafe extern "system" fn(*mut FileDialog, u32, *const FilterSpec) -> i32,
    set_file_type_index: usize,
    get_file_type_index: usize,
    advise: usize,
    unadvise: usize,
    set_options: unsafe extern "system" fn(*mut FileDialog, u32) -> i32,
    get_options: unsafe extern "system" fn(*mut FileDialog, *mut u32) -> i32,
    set_default_folder: usize,
    set_folder: unsafe extern "system" fn(*mut FileDialog, *mut ShellItem) -> i32,
    get_folder: usize,
    get_current_selection: usize,
    set_file_name: unsafe extern "system" fn(*mut FileDialog, *const u16) -> i32,
    get_file_name: usize,
    set_title: unsafe extern "system" fn(*mut FileDialog, *const u16) -> i32,
    set_ok_button_label: usize,
    set_file_name_label: usize,
    get_result: unsafe extern "system" fn(*mut FileDialog, *mut *mut ShellItem) -> i32,
    add_place: usize,
    set_default_extension: unsafe extern "system" fn(*mut FileDialog, *const u16) -> i32,
}

#[repr(C)]
struct FileDialog {
    calls: *const FileDialogCalls,
}

// `IShellItem`'s methods up to `GetDisplayName`.
#[repr(C)]
struct ShellItemCalls {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut ShellItem) -> u32,
    bind_to_handler: usize,
    get_parent: usize,
    get_display_name: unsafe extern "system" fn(*mut ShellItem, u32, *mut *mut u16) -> i32,
}

#[repr(C)]
struct ShellItem {
    calls: *const ShellItemCalls,
}

// `COMDLG_FILTERSPEC`: one entry of the file type list.
#[repr(C)]
struct FilterSpec {
    name: *const u16,
    spec: *const u16,
}

// A COM object this thread holds, let go of when this is dropped.
struct Held<T>(*mut T, unsafe extern "system" fn(*mut T) -> u32);

impl<T> Drop for Held<T> {
    fn drop(&mut self) {
        // SAFETY: the pointer is a live object this holds one reference on.
        unsafe {
            (self.1)(self.0);
        }
    }
}

const FOS_OVERWRITEPROMPT: u32 = 0x2;
// The dialog leaves the process's working folder where it was: the game
// finds its own files from it.
const FOS_NOCHANGEDIR: u32 = 0x8;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const FOS_PATHMUSTEXIST: u32 = 0x800;
const FOS_FILEMUSTEXIST: u32 = 0x1000;
const SIGDN_FILESYSPATH: u32 = 0x8005_8000;
const CLSCTX_INPROC_SERVER: u32 = 0x1;

const COINIT_APARTMENTTHREADED: u32 = 0x2;
const COINIT_DISABLE_OLE1DDE: u32 = 0x4;

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        class: *const Guid,
        outer: *mut c_void,
        context: u32,
        interface: *const Guid,
        object: *mut *mut c_void,
    ) -> i32;
    fn CoTaskMemFree(memory: *mut c_void);
}

#[link(name = "shell32")]
extern "system" {
    fn SHCreateItemFromParsingName(
        path: *const u16,
        bind: *mut c_void,
        interface: *const Guid,
        item: *mut *mut c_void,
    ) -> i32;
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
    // Every object the dialog made is let go of inside, before COM is.
    let file = show_dialog(kind, owner, folder, title, files);
    if com {
        CoUninitialize();
    }
    file
}

unsafe fn show_dialog(
    kind: Kind,
    owner: usize,
    folder: &Path,
    title: &str,
    files: &str,
) -> Option<PathBuf> {
    let class = match kind {
        Kind::Export => &CLSID_FILE_SAVE_DIALOG,
        Kind::Import => &CLSID_FILE_OPEN_DIALOG,
    };
    let mut object: *mut c_void = std::ptr::null_mut();
    let made = CoCreateInstance(
        class,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_FILE_DIALOG,
        &mut object,
    );
    if made < 0 || object.is_null() {
        return None;
    }
    let dialog = object.cast::<FileDialog>();
    let calls = &*(*dialog).calls;
    let _dialog = Held(dialog, calls.release);

    let wide = |text: &OsStr| -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() };
    let type_name = wide(OsStr::new(&format!("{files} (*.json)")));
    let type_spec = wide(OsStr::new("*.json"));
    let types = [FilterSpec {
        name: type_name.as_ptr(),
        spec: type_spec.as_ptr(),
    }];
    (calls.set_file_types)(dialog, 1, types.as_ptr());
    let title = wide(OsStr::new(title));
    (calls.set_title)(dialog, title.as_ptr());
    let extension = wide(OsStr::new("json"));
    (calls.set_default_extension)(dialog, extension.as_ptr());
    if kind == Kind::Export {
        let name = wide(OsStr::new(FILE));
        (calls.set_file_name)(dialog, name.as_ptr());
    }
    let mut options = 0u32;
    (calls.get_options)(dialog, &mut options);
    options |= FOS_FORCEFILESYSTEM
        | FOS_PATHMUSTEXIST
        | FOS_NOCHANGEDIR
        | match kind {
            Kind::Export => FOS_OVERWRITEPROMPT,
            Kind::Import => FOS_FILEMUSTEXIST,
        };
    (calls.set_options)(dialog, options);

    // The DLL's folder, whichever the player chose last time. A folder the
    // shell cannot find leaves the dialog where it would have opened.
    let path = wide(folder.as_os_str());
    let mut item: *mut c_void = std::ptr::null_mut();
    let found = SHCreateItemFromParsingName(
        path.as_ptr(),
        std::ptr::null_mut(),
        &IID_SHELL_ITEM,
        &mut item,
    );
    if found >= 0 && !item.is_null() {
        let item = item.cast::<ShellItem>();
        let _item = Held(item, (*(*item).calls).release);
        (calls.set_folder)(dialog, item);
    }

    // Cancelled is a failure too.
    if (calls.show)(dialog, owner as *mut c_void) < 0 {
        return None;
    }
    let mut result: *mut ShellItem = std::ptr::null_mut();
    if (calls.get_result)(dialog, &mut result) < 0 || result.is_null() {
        return None;
    }
    let result_calls = &*(*result).calls;
    let _result = Held(result, result_calls.release);
    let mut name: *mut u16 = std::ptr::null_mut();
    if (result_calls.get_display_name)(result, SIGDN_FILESYSPATH, &mut name) < 0 || name.is_null() {
        return None;
    }
    let mut len = 0;
    while *name.add(len) != 0 {
        len += 1;
    }
    let file = OsString::from_wide(std::slice::from_raw_parts(name, len));
    CoTaskMemFree(name.cast());
    Some(PathBuf::from(file))
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
