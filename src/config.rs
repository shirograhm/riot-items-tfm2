use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

#[derive(Deserialize, Default)]
pub struct ItemConfig {
    pub price: Option<usize>,
    pub attack: Option<i32>,
    pub attack_mult: Option<i32>,
    pub magic_power: Option<i32>,
    pub magic_power_mult: Option<i32>,
    pub crit_chance: Option<i32>,
}


pub fn load() -> HashMap<String, ItemConfig> {
    let path = dll_dir()
        .map(|d| d.join("config.json"))
        .unwrap_or_else(|| PathBuf::from("config.json"));

    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// Returns the directory containing this DLL by passing a static address within
// it to GetModuleHandleExW (FROM_ADDRESS flag), then resolving the full path.
fn dll_dir() -> Option<PathBuf> {
    extern "system" {
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, filename: *mut u16, size: u32) -> u32;
    }

    #[used]
    static ANCHOR: u8 = 0;
    let mut hmodule: *mut c_void = std::ptr::null_mut();
    // Flags 0x6 = FROM_ADDRESS | UNCHANGED_REFCOUNT
    let ok = unsafe { GetModuleHandleExW(0x6, &ANCHOR as *const u8 as *const u16, &mut hmodule) };
    if ok == 0 {
        return None;
    }

    let mut buf = [0u16; 32768];
    let len = unsafe { GetModuleFileNameW(hmodule, buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return None;
    }

    let os_str = OsString::from_wide(&buf[..len as usize]);
    std::path::Path::new(&os_str)
        .parent()
        .map(|p| p.to_path_buf())
}
