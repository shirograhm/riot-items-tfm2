//! Experimental native hook for the game's item-build route function.
//!
//! This locates `LogisticSGDAgent::get_item_builds_list` inside the loaded
//! Teamfight Manager 2 executable by scanning the `.text` section for a verified
//! byte signature (captured on game version `0.5.1`), installs a trampoline
//! detour, and overrides the routes it returns. It is intentionally fail-closed: if
//! the signature is missing, ambiguous, or the prologue does not match, the hook
//! refuses to patch instead of touching an unknown function.
//!
//! Re-deriving the signature after a game update: the function is a generic trait
//! method (`<LogisticSGDAgent as AbstractItemNetwork>::get_item_builds_list`)
//! monomorphized into the EXE, so it is not a plain symbol there. It *is* compiled
//! into the SDK's `libgame_view` rlib with symbols (byte-identical to the EXE), so
//! extract that object, read the function's first ~55 prologue bytes as the new
//! `FUNCTION_SIGNATURE`, and confirm they appear exactly once in the EXE. The
//! Win64 arg layout (RVO return ptr in rcx, `&self` in rdx, then items/champions/
//! …) is unchanged from 0.4.13, so `OriginalFn` did not need updating — only the
//! codegen (stack frame size, register allocation) shifted.
//!
//! NOTE: This is the pointer-finding / native-hook portion. After calling the
//! original function, `detour` applies the config-driven build overrides from
//! `build_config` (pinned slots + AI-filled blanks), then enforces unique
//! builds: no champion ever builds duplicate copies of the same item — each
//! duplicate is swapped for the closest-price final item of the same category
//! (see `enforce_unique_items`).

use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

extern crate game_core;
use game_core::{ChampionInfoSheet, ItemInfo, LogisticSGDAgent, Position};

use crate::build_config;

const STOLEN_LEN: usize = 19;
const ABSOLUTE_JUMP_LEN: usize = 12;

const FUNCTION_SIGNATURE: [u8; 55] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x58,
    0x01, 0x00, 0x00, 0x48, 0x8D, 0xAC, 0x24, 0x80, 0x00, 0x00, 0x00, 0x48, 0xC7, 0x85, 0xD0, 0x00,
    0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0x4C, 0x89, 0x4D, 0x08, 0x4C, 0x89, 0xC3, 0x48, 0x89, 0x55,
    0x10, 0x48, 0x89, 0xCF, 0x48, 0x8B, 0xB5,
];

const EXPECTED_PROLOGUE: [u8; STOLEN_LEN] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x58,
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

/// Rewrites each route so a champion never builds the same item twice. The first
/// occurrence keeps its slot; each later duplicate is replaced with the final
/// item (one with no further upgrades — vanilla tier 5s and the mod's radiants)
/// of the same category that is not already in the build and whose price is
/// closest to the duplicate's, keeping the swap roughly balance-neutral. A slot
/// with no viable candidate is left as-is rather than dropped, so route lengths
/// never change. Categories are compared by discriminant so this does not depend
/// on `ItemCategory` implementing `PartialEq`.
fn enforce_unique_items(items: &[Box<dyn ItemInfo>], routes: &mut [Vec<usize>]) {
    for route in routes.iter_mut() {
        let mut seen = std::collections::HashSet::new();
        for slot in route.iter_mut() {
            if seen.insert(*slot) {
                continue;
            }
            let Some(duplicate) = items.get(*slot) else {
                continue;
            };
            let category = std::mem::discriminant(&duplicate.category());
            let price = duplicate.price();
            let replacement = items
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    !seen.contains(index)
                        && item.next_tier().is_empty()
                        && std::mem::discriminant(&item.category()) == category
                })
                .min_by_key(|(_, item)| item.price().abs_diff(price));
            if let Some((index, _)) = replacement {
                *slot = index;
                seen.insert(index);
            }
        }
    }
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

    let item_keys = items
        .iter()
        .map(|item| item.key().to_string())
        .collect::<Vec<_>>();

    // Routes are keyed off the lineup the game builds for (`team1`), in route
    // order — NOT the 60-entry `champion_ids` roster. The position-by-index
    // mapping below relies on `route_count` tracking `team1` size; verified
    // in-game.
    let lineup = team1
        .iter()
        .map(|(_, champion)| champion.clone())
        .collect::<Vec<_>>();

    match build_config::load() {
        Ok(Some(config)) if !config.is_empty() => {
            build_config::apply(&config, &item_keys, &lineup, &mut routes);
        }
        // No config file, or an empty one: leave the game's routes untouched.
        Ok(_) => {}
        // A malformed config is ignored so the game's routes stay untouched.
        Err(_) => {}
    }

    // Enforce unique builds after `build_config` so AI routes, category-forced
    // routes, and configured builds are all covered. Toggled from the item build
    // editor via `mod-settings.json` (defaults on); read per call, so flipping
    // the checkbox applies to the next match without restarting the game.
    if build_config::unique_items_enabled() {
        enforce_unique_items(items, &mut routes);
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
            eprintln!("riot_items_tfm2: post_patch_warning warning={warning}");
        }
        Ok(target as usize)
    }
}
