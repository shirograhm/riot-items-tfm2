//! Experimental native hook for the game's item-build route function.
//!
//! This locates `LogisticSGDAgent::get_item_builds_list` inside the loaded
//! Teamfight Manager 2 executable by scanning the `.text` section for a verified
//! byte signature (captured on game version `0.4.13`), installs a trampoline
//! detour, and records a probe of each call. It is intentionally fail-closed: if
//! the signature is missing, ambiguous, or the prologue does not match, the hook
//! refuses to patch instead of touching an unknown function.
//!
//! NOTE: This is the pointer-finding / native-hook portion. After calling the
//! original function and logging a probe, `detour` applies the minimal
//! config-driven setter in `build_config` (item-key build paths, no lineup
//! analysis). The richer lineup-aware route rewriting from the source snapshot
//! is not included.

use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

extern crate game_core;
use game_core::{ChampionInfoSheet, ItemInfo, LogisticSGDAgent, Position};

use crate::build_config;
use crate::report;

const STOLEN_LEN: usize = 19;
const ABSOLUTE_JUMP_LEN: usize = 12;

const FUNCTION_SIGNATURE: [u8; 55] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x28,
    0x01, 0x00, 0x00, 0x48, 0x8D, 0xAC, 0x24, 0x80, 0x00, 0x00, 0x00, 0x48, 0xC7, 0x85, 0xA0, 0x00,
    0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0x4C, 0x89, 0x4D, 0x08, 0x4C, 0x89, 0x45, 0x10, 0x48, 0x89,
    0x55, 0x18, 0x48, 0x89, 0xCF, 0x48, 0x8B,
];

const EXPECTED_PROLOGUE: [u8; STOLEN_LEN] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x28,
    0x01, 0x00, 0x00,
];

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn VirtualAlloc(
        address: *mut c_void,
        size: usize,
        allocation_type: u32,
        protect: u32,
    ) -> *mut c_void;
    fn VirtualProtect(
        address: *mut c_void,
        size: usize,
        new_protect: u32,
        old_protect: *mut u32,
    ) -> i32;
    fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
}

type OriginalFn = unsafe fn(
    &LogisticSGDAgent,
    &Vec<Box<dyn ItemInfo>>,
    &ChampionInfoSheet,
    &Vec<String>,
    &Vec<(Position, String)>,
    &Vec<(Position, String)>,
    bool,
) -> Vec<Vec<usize>>;

static ORIGINAL: OnceLock<OriginalFn> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static INSTALLED: AtomicBool = AtomicBool::new(false);
static BUILD_CONFIG_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);
static LINEUP_MISMATCH_REPORTED: AtomicBool = AtomicBool::new(false);

unsafe fn read_u16(address: *const u8) -> u16 {
    ptr::read_unaligned(address.cast::<u16>())
}

unsafe fn read_u32(address: *const u8) -> u32 {
    ptr::read_unaligned(address.cast::<u32>())
}

unsafe fn text_section() -> Result<(*mut u8, usize), String> {
    let base = GetModuleHandleW(ptr::null()).cast::<u8>();
    if base.is_null() {
        return Err("GetModuleHandleW returned null".to_string());
    }
    if read_u16(base) != 0x5A4D {
        return Err("main module has no MZ header".to_string());
    }

    let nt = base.add(read_u32(base.add(0x3C)) as usize);
    if read_u32(nt) != 0x0000_4550 {
        return Err("main module has no PE header".to_string());
    }

    let section_count = read_u16(nt.add(6)) as usize;
    let optional_size = read_u16(nt.add(20)) as usize;
    let section_table = nt.add(24 + optional_size);
    for index in 0..section_count {
        let section = section_table.add(index * 40);
        let name = std::slice::from_raw_parts(section, 8);
        let name_len = name.iter().position(|byte| *byte == 0).unwrap_or(8);
        let section_name = &name[..name_len];
        if section_name == b".text" {
            let virtual_size = read_u32(section.add(8)) as usize;
            let virtual_address = read_u32(section.add(12)) as usize;
            return Ok((base.add(virtual_address), virtual_size));
        }
    }
    Err("main module .text section not found".to_string())
}

unsafe fn locate_target() -> Result<*mut u8, String> {
    let (text, text_len) = text_section()?;
    if text_len < FUNCTION_SIGNATURE.len() {
        return Err(".text section is smaller than function signature".to_string());
    }

    let bytes = std::slice::from_raw_parts(text, text_len);
    let matches = bytes
        .windows(FUNCTION_SIGNATURE.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == FUNCTION_SIGNATURE).then_some(offset))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!("signature match count was {}", matches.len()));
    }

    let target = text.add(matches[0]);
    let prologue = std::slice::from_raw_parts(target, STOLEN_LEN);
    if prologue != EXPECTED_PROLOGUE {
        return Err("prologue mismatch".to_string());
    }
    Ok(target)
}

unsafe fn write_absolute_jump(destination: *mut u8, target: *const u8, total_len: usize) {
    debug_assert!(total_len >= ABSOLUTE_JUMP_LEN);
    let mut jump = [0x90u8; ABSOLUTE_JUMP_LEN];
    jump[0] = 0x48;
    jump[1] = 0xB8;
    jump[2..10].copy_from_slice(&(target as usize as u64).to_le_bytes());
    jump[10] = 0xFF;
    jump[11] = 0xE0;
    ptr::copy_nonoverlapping(jump.as_ptr(), destination, jump.len());
    if total_len > jump.len() {
        ptr::write_bytes(destination.add(jump.len()), 0x90, total_len - jump.len());
    }
}

unsafe fn create_trampoline(target: *mut u8) -> Result<OriginalFn, String> {
    let size = STOLEN_LEN + ABSOLUTE_JUMP_LEN;
    let trampoline = VirtualAlloc(
        ptr::null_mut(),
        size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    )
    .cast::<u8>();
    if trampoline.is_null() {
        return Err("VirtualAlloc failed for trampoline".to_string());
    }

    ptr::copy_nonoverlapping(target, trampoline, STOLEN_LEN);
    write_absolute_jump(
        trampoline.add(STOLEN_LEN),
        target.add(STOLEN_LEN),
        ABSOLUTE_JUMP_LEN,
    );
    if FlushInstructionCache(GetCurrentProcess(), trampoline.cast(), size) == 0 {
        return Err("FlushInstructionCache failed for trampoline".to_string());
    }
    Ok(mem::transmute::<*mut u8, OriginalFn>(trampoline))
}

unsafe fn patch_target(target: *mut u8) -> Result<Vec<String>, String> {
    if std::slice::from_raw_parts(target, STOLEN_LEN) != EXPECTED_PROLOGUE {
        return Err("patch target already contains detour or prologue changed".to_string());
    }

    let mut old_protect = 0u32;
    if VirtualProtect(
        target.cast(),
        STOLEN_LEN,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return Err("VirtualProtect failed before target patch".to_string());
    }

    write_absolute_jump(target, detour as *const u8, STOLEN_LEN);
    let flushed = FlushInstructionCache(GetCurrentProcess(), target.cast(), STOLEN_LEN);
    let mut ignored = 0u32;
    let restored = VirtualProtect(target.cast(), STOLEN_LEN, old_protect, &mut ignored);
    let mut warnings = Vec::new();
    if flushed == 0 {
        warnings.push("FlushInstructionCache failed after target patch".to_string());
    }
    if restored == 0 {
        warnings.push("VirtualProtect failed while restoring target protection".to_string());
    }
    Ok(warnings)
}

fn format_lineup(team: &[(Position, String)]) -> String {
    team.iter()
        .map(|(position, champion)| format!("{position:?}:{champion}"))
        .collect::<Vec<_>>()
        .join(",")
}

unsafe fn detour(
    agent: &LogisticSGDAgent,
    items: &Vec<Box<dyn ItemInfo>>,
    champions: &ChampionInfoSheet,
    champion_ids: &Vec<String>,
    team1: &Vec<(Position, String)>,
    team2: &Vec<(Position, String)>,
    mode: bool,
) -> Vec<Vec<usize>> {
    let original = ORIGINAL
        .get()
        .copied()
        .expect("item build hook original function missing");
    let mut routes = original(agent, items, champions, champion_ids, team1, team2, mode);
    let _ = report::record(&format!(
        "item_build_probe mode={mode} item_count={} champion_id_count={} route_count={} champion_ids=[{}] team1=[{}] team2=[{}]",
        items.len(),
        champion_ids.len(),
        routes.len(),
        champion_ids.join(","),
        format_lineup(team1),
        format_lineup(team2),
    ));

    let item_keys = items
        .iter()
        .map(|item| item.key().to_string())
        .collect::<Vec<_>>();

    // Routes are keyed off the lineup the game builds for (`team1`), in route
    // order — NOT the 60-entry `champion_ids` roster. The probe confirmed
    // `route_count` tracks `team1` size, not `champion_id_count`. If that ever
    // stops holding, the position-by-index mapping below is unsafe, so record
    // it once for diagnosis.
    let lineup = team1
        .iter()
        .map(|(_, champion)| champion.clone())
        .collect::<Vec<_>>();
    if routes.len() != lineup.len() && !LINEUP_MISMATCH_REPORTED.swap(true, Ordering::SeqCst) {
        let _ = report::record(&format!(
            "item_build_lineup_mismatch route_count={} team1_len={}",
            routes.len(),
            lineup.len()
        ));
    }

    match build_config::load() {
        Ok(Some(config)) if !config.is_empty() => {
            let applied = build_config::apply(&config, &item_keys, &lineup, &mut routes);
            let _ = report::record(&format!(
                "item_build_set routes_set={} unknown_keys=[{}]",
                applied.routes_set,
                applied.unknown_keys.join(",")
            ));
        }
        // No config file, or an empty one: leave the game's routes untouched.
        Ok(_) => {}
        Err(error) => {
            if !BUILD_CONFIG_ERROR_REPORTED.swap(true, Ordering::SeqCst) {
                let _ = report::record(&format!("build_config_refused error={error}"));
            }
        }
    }

    routes
}

pub fn install_hook() -> Result<usize, String> {
    let _install_guard = INSTALL_LOCK
        .lock()
        .map_err(|error| format!("install lock poisoned: {error}"))?;
    if INSTALLED.load(Ordering::SeqCst) {
        return Err("hook already installed".to_string());
    }

    unsafe {
        let target = locate_target()?;
        let original = create_trampoline(target)?;
        ORIGINAL
            .set(original)
            .map_err(|_| "original trampoline was already set".to_string())?;
        let warnings = patch_target(target)?;
        INSTALLED.store(true, Ordering::SeqCst);
        for warning in warnings {
            let _ = report::record(&format!("post_patch_warning warning={warning}"));
        }
        Ok(target as usize)
    }
}
