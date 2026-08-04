//! Experimental native hook for the game's item-build route function.
//!
//! This locates `<LogisticSGDAgent as AbstractItemNetwork>::get_item_builds_list`
//! inside the loaded Teamfight Manager 2 executable, installs a trampoline
//! detour, and overrides the routes it returns. It is intentionally fail-closed:
//! if the target cannot be identified *unambiguously*, the hook refuses to patch
//! rather than touching an unknown function.
//!
//! # How the target is found (0.5.3 onwards)
//!
//! Earlier versions scanned `.text` for a 55-byte signature lifted from the SDK's
//! `libgame_view` rlib. That route is gone: from SDK 0.5.3 every engine rlib ships
//! LLVM bitcode instead of object code, so there is no machine code left to read a
//! prologue from. The signature also broke on every game update by construction.
//!
//! The target is therefore **data, not code**. Resolution order:
//!
//! 1. `hook-target.json` next to the DLL, if present — either an explicit `rva` or
//!    a hex `signature`. Update that file after a game patch instead of rebuilding.
//! 2. Otherwise [`FALLBACK_SIGNATURE`], which is current for game 0.5.3.
//!
//! The finder identifies the target by its **argument shape** rather than by
//! anything in its body: the return type is 24 bytes so it comes back via `sret`
//! in rcx, which pushes four arguments onto the stack — three `&Vec` (thin
//! pointers, read as qwords) and a trailing `bool` (read as a byte). Exactly one
//! function in the executable does that. Every fingerprint taken from emitted
//! code has broken on a game update; this one is taken from the type signature
//! the mod already depends on, so it survives them.
//!
//! Whatever the source, the candidate is verified against the PE exception table
//! (`.pdata`) before anything is patched:
//!
//! - the address must be the *start* of a real function, per `.pdata`
//! - its prologue must be exactly [`PROLOGUE_PUSHES`]
//! - a signature must match in exactly one place
//!
//! `.pdata` is what the OS unwinder uses, so it gives exact function boundaries —
//! strictly better than inferring them from a `.text` window scan.
//!
//! When resolution fails, [`candidate_report`] lists every function sharing the
//! target's shape, which narrows ~130k functions to a few dozen for the finder
//! tool. It deliberately never picks one: several dozen functions match, and
//! detouring the wrong one corrupts the simulation.
//!
//! # Why 12 stolen bytes
//!
//! The eight callee-saved pushes are exactly 12 bytes — the size of an absolute
//! jump — and are position independent. Stealing precisely those is instruction
//! aligned *and* independent of the frame size. The old `STOLEN_LEN = 19` also
//! swallowed `sub rsp, imm32`; when codegen picks the `imm8` encoding that
//! instruction is four bytes, so 19 would split the following instruction and
//! corrupt the trampoline.
//!
//! After calling the original function, `detour` applies the config-driven build
//! overrides from `build_config` (pinned slots + AI-filled blanks), then enforces
//! unique builds: no champion ever builds duplicate copies of the same item — each
//! duplicate is swapped for the closest-price final item of the same category (see
//! `enforce_unique_items`).

use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

extern crate game_core;
use game_core::{ChampionInfoSheet, ItemInfo, LogisticSGDAgent, Position};

use crate::build_config;

/// `push rbp; push r15; push r14; push r13; push r12; push rsi; push rdi; push rbx`
const PROLOGUE_PUSHES: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];

const STOLEN_LEN: usize = PROLOGUE_PUSHES.len();
const ABSOLUTE_JUMP_LEN: usize = 12;

/// Signature for game 0.5.3, where the target is `0x2155a90` (size 2179).
///
/// 48 bytes, not 40: the first 40 are a prologue idiom shared with four other
/// functions, so a shorter signature is ambiguous. `tools/find_item_build_hook.py`
/// grows the signature until it is unique and prints the result — re-run it after
/// a game update and paste the new bytes here, or ship a `hook-target.json`,
/// which takes precedence and needs no rebuild.
///
/// Decoded, this is the prologue the target has had since 0.5.2, with only the
/// frame size and an added `xmm6` save changing between versions:
///
/// ```text
///   push rbp,r15,r14,r13,r12,rsi,rdi,rbx
///   sub  rsp, 0x208
///   lea  rbp, [rsp+0x80]
///   movaps [rbp+0x170], xmm6
///   mov  qword [rbp+0x168], -2
///   mov  [rbp+0x40], r9
/// ```
const FALLBACK_SIGNATURE: [u8; 48] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x08,
    0x02, 0x00, 0x00, 0x48, 0x8D, 0xAC, 0x24, 0x80, 0x00, 0x00, 0x00, 0x0F, 0x29, 0xB5, 0x70, 0x01,
    0x00, 0x00, 0x48, 0xC7, 0x85, 0x68, 0x01, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0x4C, 0x89, 0x4D,
];

/// Plausible size range for the target in bytes (1869 in SDK 0.5.2). Narrows the
/// diagnostic candidate list only; never used to choose a target.
const CANDIDATE_SIZE_RANGE: (u32, u32) = (1200, 2800);

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

/// Contents of `hook-target.json`. Both fields are optional; `rva` wins.
#[derive(serde::Deserialize, Default)]
struct TargetConfig {
    /// Function start as a module-relative virtual address, e.g. `"0x10591f0"`.
    rva: Option<String>,
    /// Hex byte signature, e.g. `"554157..."`. Must match exactly once.
    signature: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() || cleaned.len() % 2 != 0 {
        return Err("signature must be a non-empty even number of hex digits".to_string());
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("signature has a non-hex byte at offset {i}"))
        })
        .collect()
}

fn parse_rva(text: &str) -> Result<usize, String> {
    let trimmed = text.trim();
    match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(digits) => usize::from_str_radix(digits, 16),
        None => trimmed.parse(),
    }
    .map_err(|_| format!("could not parse rva {trimmed:?}"))
}

fn load_target_config() -> TargetConfig {
    let Some(path) = crate::config::dll_dir().map(|dir| dir.join("hook-target.json")) else {
        return TargetConfig::default();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

unsafe fn read_u16(address: *const u8) -> u16 {
    ptr::read_unaligned(address.cast::<u16>())
}

unsafe fn read_u32(address: *const u8) -> u32 {
    ptr::read_unaligned(address.cast::<u32>())
}

unsafe fn module_base() -> Result<*mut u8, String> {
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
    Ok(base)
}

/// Address and virtual size of a named section in the loaded main module.
unsafe fn section(base: *mut u8, want: &[u8]) -> Result<(*mut u8, usize), String> {
    let nt = base.add(read_u32(base.add(0x3C)) as usize);
    let section_count = read_u16(nt.add(6)) as usize;
    let optional_size = read_u16(nt.add(20)) as usize;
    let table = nt.add(24 + optional_size);
    for index in 0..section_count {
        let header = table.add(index * 40);
        let name = std::slice::from_raw_parts(header, 8);
        let len = name.iter().position(|byte| *byte == 0).unwrap_or(8);
        if &name[..len] == want {
            let virtual_size = read_u32(header.add(8)) as usize;
            let virtual_address = read_u32(header.add(12)) as usize;
            return Ok((base.add(virtual_address), virtual_size));
        }
    }
    Err(format!("section {} not found", String::from_utf8_lossy(want)))
}

/// Every function in the main module as `(start_rva, end_rva)`, sorted by start,
/// read from the PE exception table.
unsafe fn pdata_functions(base: *mut u8) -> Result<Vec<(u32, u32)>, String> {
    let (pdata, pdata_len) = section(base, b".pdata")?;
    let mut functions = Vec::with_capacity(pdata_len / 12);
    for index in 0..pdata_len / 12 {
        let entry = pdata.add(index * 12);
        let start = read_u32(entry);
        let end = read_u32(entry.add(4));
        if start != 0 && end > start {
            functions.push((start, end));
        }
    }
    if functions.is_empty() {
        return Err("exception table held no functions".to_string());
    }
    functions.sort_unstable();
    Ok(functions)
}

/// The `.pdata` entry containing `rva`, if any.
fn enclosing(functions: &[(u32, u32)], rva: u32) -> Option<(u32, u32)> {
    let index = functions.partition_point(|(start, _)| *start <= rva);
    let (start, end) = *functions.get(index.checked_sub(1)?)?;
    (start <= rva && rva < end).then_some((start, end))
}

/// Confirms `target` is a function start whose prologue is the expected pushes.
unsafe fn verify(
    base: *mut u8,
    functions: &[(u32, u32)],
    target: *mut u8,
) -> Result<(u32, u32), String> {
    let rva = target.offset_from(base) as u32;
    let Some((start, end)) = enclosing(functions, rva) else {
        return Err(format!("rva {rva:#x} is not inside any known function"));
    };
    if start != rva {
        return Err(format!(
            "rva {rva:#x} is not a function start (enclosing function begins at {start:#x})"
        ));
    }
    let prologue = std::slice::from_raw_parts(target, STOLEN_LEN);
    if prologue != PROLOGUE_PUSHES {
        return Err(format!(
            "prologue mismatch at {rva:#x}: found {}",
            hex(prologue)
        ));
    }
    Ok((start, end))
}

unsafe fn find_signature(base: *mut u8, signature: &[u8]) -> Result<*mut u8, String> {
    let (text, text_len) = section(base, b".text")?;
    if text_len < signature.len() {
        return Err(".text is smaller than the signature".to_string());
    }
    let bytes = std::slice::from_raw_parts(text, text_len);
    let mut first = None;
    let mut count = 0usize;
    for (offset, window) in bytes.windows(signature.len()).enumerate() {
        if window == signature {
            count += 1;
            first.get_or_insert(offset);
            if count > 1 {
                break;
            }
        }
    }
    match (first, count) {
        (Some(offset), 1) => Ok(text.add(offset)),
        (_, n) => Err(format!("signature matched {n} times, need exactly 1")),
    }
}

/// Resolves the function to detour. See the module docs for the order.
unsafe fn locate_target(base: *mut u8, functions: &[(u32, u32)]) -> Result<*mut u8, String> {
    let config = load_target_config();

    if let Some(text) = config.rva.as_deref() {
        let target = base.add(parse_rva(text)?);
        verify(base, functions, target)?;
        return Ok(target);
    }

    if let Some(text) = config.signature.as_deref() {
        let signature = parse_hex_bytes(text)?;
        if signature.len() < STOLEN_LEN {
            return Err(format!(
                "configured signature is {} bytes, need at least {STOLEN_LEN}",
                signature.len()
            ));
        }
        let target = find_signature(base, &signature)?;
        verify(base, functions, target)?;
        return Ok(target);
    }

    let target = find_signature(base, &FALLBACK_SIGNATURE).map_err(|error| {
        format!(
            "{error}; the built-in signature is for game 0.5.3 and this build differs — \
             re-run tools/find_item_build_hook.py and ship the hook-target.json it writes"
        )
    })?;
    verify(base, functions, target)?;
    Ok(target)
}

/// Every function sharing the target's prologue and size range, as
/// `rva=<start> size=<bytes>`. Diagnostic aid for re-deriving the target after a
/// game update; see the module docs for why it never selects one.
pub fn candidate_report() -> Result<Vec<String>, String> {
    unsafe {
        let base = module_base()?;
        let functions = pdata_functions(base)?;
        let (text, text_len) = section(base, b".text")?;
        let text_start = text.offset_from(base) as u32;
        let text_end = text_start + text_len as u32;

        let mut report = Vec::new();
        for (start, end) in functions {
            let size = end - start;
            if size < CANDIDATE_SIZE_RANGE.0 || size > CANDIDATE_SIZE_RANGE.1 {
                continue;
            }
            if start < text_start || end > text_end {
                continue;
            }
            let code = std::slice::from_raw_parts(base.add(start as usize), STOLEN_LEN);
            if code == PROLOGUE_PUSHES {
                report.push(format!("rva={start:#x} size={size}"));
            }
        }
        Ok(report)
    }
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
    if std::slice::from_raw_parts(target, STOLEN_LEN) != PROLOGUE_PUSHES {
        return Err("patch target already contains a detour or its prologue changed".to_string());
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
    // Logged BEFORE anything is read, including the arguments. The target was
    // identified structurally, so if it is the wrong function these references
    // point at whatever the real signature passes and dereferencing them is
    // undefined — the crash would happen before any later report could be
    // written. `hook_entered` with no following `hook_call` therefore means
    // "reached the detour, died reading the arguments", i.e. wrong function.
    // Both lines present means the target is right.
    {
        static ENTERED: AtomicUsize = AtomicUsize::new(0);
        if ENTERED.fetch_add(1, Ordering::Relaxed) < 3 {
        }
    }

    // The merged `tfm2_item_tactics` half needs the game's `Database`, which a
    // stable-ABI mod is never handed. `agent` is the item recommendation network
    // that lives at a fixed offset inside it, so this argument is the one route
    // to that address in the whole mod — see `tactics::driver::record_item_net`,
    // which validates the network before believing it and ignores every call
    // after the first that sticks.
    crate::tactics::driver::record_item_net(agent as *const LogisticSGDAgent as usize);

    // The same half needs to know which items exist and which are final, to
    // offer a mod item as the 4th build slot. It used to find that by scanning
    // `Database + 0..0x60000` for something Vec-shaped; this is the list the
    // game is actually using, so it is handed over directly. Idempotent, and
    // cheap after the first call that sticks.
    crate::tactics::driver::record_item_catalog(
        items
            .iter()
            .map(|item| {
                (
                    item.key().to_string(),
                    item.next_tier().iter().map(ToString::to_string).collect(),
                )
            })
            .collect(),
    );

    let original = ORIGINAL
        .get()
        .copied()
        .expect("item build hook original function missing");
    let mut routes = original(agent, items, champions, champion_ids, team1, team2, mode);

    // `routes` covers `team1` only (5 routes for 5 entries), and the function is
    // called once per team — so a rule keyed by route index fires for the enemy
    // team too, which is why a build pinned to "Top" reached both top laners.
    // Nothing in the arguments says which team is the player's: `mode` was false
    // on every call observed. This is why builds are keyed by champion.

    // Hand the champion roster to the client-side editor, which cannot
    // enumerate champions from inside a UI handler. Same process, so this is a
    // static rather than a file (see `build_config::record_champion_roster`).
    build_config::record_champion_roster(champion_ids);

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

    // There is no second, position-keyed pass any more. Builds are keyed by
    // champion whichever editor wrote them, because this function computes one
    // team per call and never says which team is the player's — so a rule keyed
    // by route index applied to the enemy as well.

    // Enforce unique builds after `build_config` so AI routes, category-forced
    // routes, and configured builds are all covered. Toggled from the item build
    // editor via `mod-settings.json` (defaults on); read per call, so flipping
    // the toggle applies to the next match without restarting the game.
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
        let base = module_base()?;
        let functions = pdata_functions(base)?;
        let target = locate_target(base, &functions)?;
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
