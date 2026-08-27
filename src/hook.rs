//! Native tap on the game's item-build route function.
//!
//! This locates `<LogisticSGDAgent as AbstractItemNetwork>::get_item_builds_list`
//! inside the loaded Teamfight Manager 2 executable and installs a trampoline
//! detour. It is intentionally fail-closed: if the target cannot be identified
//! *unambiguously*, the hook refuses to patch rather than touching an unknown
//! function.
//!
//! # This no longer decides builds
//!
//! It used to rewrite the routes it returned; that job moved to
//! [`crate::item_build_hook`], a `StableItemBuildHook`, which is told which
//! champion each build is for instead of inferring it from route order. The
//! detour returns the game's routes untouched.
//!
//! What keeps it here is that it is the only place three things are reachable at
//! all, none of which cross the stable boundary:
//!
//! | taken from the arguments | needed by | why nothing else can |
//! |---|---|---|
//! | `&LogisticSGDAgent` | `tactics::driver::record_item_net` | the `Database` base is derived from it; it is the tactics half's only route to the game database, and so to 4-slot mode, per-athlete build injection and the auto-4th pick |
//! | `&Vec<Box<dyn ItemInfo>>` | `tactics::driver::record_item_catalog` | which items exist and which are final, without scanning the `Database` for something Vec-shaped |
//! | `&Vec<String>` (`champion_ids`) | `build_config::record_champion_roster` | the whole 60-entry roster, mod champions included; the stable context sees only the ten champions of one match |
//!
//! When the detour fails to install, all three degrade on their own: the tactics
//! half reports 3 slots and stops injecting, and the editor falls back to the
//! client's champion list. Builds are unaffected — they are on the stable API.
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
//! 2. Otherwise [`FALLBACK_SIGNATURE`], which is current for game 0.6.0-beta.
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

/// Signature for game 0.5.7, where the target is `0x1ce9d90` (size 2270).
///
/// The bytes below have now survived the 0.5.6 -> 0.5.7 update **unchanged**: they
/// are address independent, and re-scanning the new executable finds them in
/// exactly one place, at a `.pdata` function start of the same size 2270. The
/// old/new pair is instruction-isomorphic with zero differing displacements, so
/// only the address moved. This is the second update in a row that needed no
/// edit here — which is the point of fingerprinting the argument shape rather
/// than the emitted code.
///
/// 48 bytes, not 40: the first 40 are a prologue idiom shared with four other
/// functions, so a shorter signature is ambiguous. `tools/find_item_build_hook.py`
/// grows the signature until it is unique and prints the result — re-run it after
/// a game update and paste the new bytes here, or ship a `hook-target.json`,
/// which takes precedence and needs no rebuild.
///
/// Decoded, this is the prologue the target has had since 0.5.2, with only the
/// frame size and the displacements that follow from it changing between
/// versions:
///
/// ```text
///   push rbp,r15,r14,r13,r12,rsi,rdi,rbx
///   sub  rsp, 0x228
///   lea  rbp, [rsp+0x80]
///   movaps [rbp+0x190], xmm6
///   mov  qword [rbp+0x188], -2
///   mov  [rbp+0x40], r9
/// ```
///
/// 0.5.3 -> 0.5.4 moved it from `0x2155a90` and grew the frame `0x208` -> `0x228`,
/// with both displacements shifted by exactly that `0x20` (`0x170` -> `0x190`,
/// `0x168` -> `0x188`). That the shift is uniform is what identifies it as the
/// same function recompiled rather than a lookalike: on 0.5.4 the argument-shape
/// filter alone returns *two* candidates, and the other one (`0x2566180`) has a
/// `0x4d8` frame and saves four xmm registers.
///
/// 0.5.4 -> 0.5.5 moved it again, `0x1e76c50` -> `0x1a347a0`, but these 48 bytes
/// are unchanged: the frame and both displacements are the same, so the constant
/// below did not have to be touched — only re-confirmed. It is still unique in
/// `.text` at 48 bytes and still ambiguous at 40. Two independent methods agree
/// on the new address: the argument-shape filter again returns exactly two
/// candidates that map 1:1 onto 0.5.4's by size and call counts (2270/14/7 here,
/// 1489/23/10 for the same decoy, now `0x1ae8b30`), and `rederive.py match` from
/// the 0.5.4 binary gives a 157-byte masked signature with a single hit, at a
/// function start, of identical size.
///
/// 0.5.5 -> 0.5.6 moved it a long way, `0x1a347a0` -> `0x2598dd0`, and again
/// **0.5.7 -> 0.6.0-beta (2026-08-27): the streak ended — these bytes DID
/// change**, and for the first time since 0.5.3 the body changed with them, so
/// none of the cheap checks apply: the old 48 bytes get 0 hits, `match` gets 0
/// hits at every length from 48 to 157 bytes (strict *and* `--loose`), and the
/// callees/callers changed too — the whole `item_network` cluster was
/// recompiled. Do not reach for `pairdiff` here; it cannot pair them.
///
/// The target is **0x25f2b10** (size 2040, was 0x1ce9d90 / 2270). It is the
/// documented argument-shape discriminator that found it, exactly as the
/// module note above says to use first, plus a correspondence argument:
///
///   * arg shape passes — reads 0x28/0x30/0x38 as `&Vec` + 0x40 as a bool and
///     nothing above 0x40 (3 candidates in 0.6.0; the 1489-byte / 23-call one
///     is the same decoy 0.5.7 had, and the third has 1 caller and an unrelated
///     callee profile).
///   * the prologue is the *same idiom* with the frame grown 0x228 -> 0x248 and
///     **both displacements shifted by that same 0x20** — the identical
///     relationship the 0.5.3 -> 0.5.4 migration saw.
///   * 8 caller sites in exactly 4 functions, 2 each, as in 0.5.7, and the four
///     caller sizes correspond 1:1 (31248/31219, 75968/75744, 2957/3101,
///     1239/1415). The 75.9KB one is the match-sim megafunction, independently
///     tied to its 0.5.7 self through `CL_LAUNCHER`'s caller list.
///   * it calls the independently derived 0.6.0 allocator (0x2dd4b50), and its
///     6.3KB callee sits at **+0xa10 from `itemnet_forward` in both builds**.
///   * the two adjacent equal-size siblings (team1/team2 `spec_from_iter`) are
///     called at +0x91 and +0xc1 — inside the ground-truth 0xCA window.
///
/// Still unique in `.text` at 48 bytes (1 hit); the 38-byte prefix collides
/// with 3, so the length is doing real work and must not be shortened.
const FALLBACK_SIGNATURE: [u8; 48] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x48,
    0x02, 0x00, 0x00, 0x48, 0x8D, 0xAC, 0x24, 0x80, 0x00, 0x00, 0x00, 0x0F, 0x29, 0xB5, 0xB0, 0x01,
    0x00, 0x00, 0x48, 0xC7, 0x85, 0xA8, 0x01, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0x4C, 0x89, 0x4D,
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
            "{error}; the built-in signature is for game 0.5.5 and this build differs — \
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
    // game is actually using, so it is handed over directly. Idempotent.
    //
    // The guard is not redundant with that idempotence: the argument is built
    // by the *caller*, so without it every call after the first still allocated
    // two `String`s per item — for the whole catalog, this mod's additions
    // included — only for the callee to drop them.
    if !crate::tactics::driver::item_catalog_recorded() {
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
    }

    // Hand the champion roster to the client-side editor, which cannot
    // enumerate champions from inside a UI handler. Same process, so this is a
    // static rather than a file (see `build_config::record_champion_roster`).
    //
    // This is the last thing here that is not for the tactics half, and it is
    // here because the argument is the whole 60-entry roster — mod champions
    // included. `StableItemBuildContext` sees ten champions, the two lineups of
    // one match, so the item-build hook cannot produce this list.
    build_config::record_champion_roster(champion_ids);

    // The routes are returned exactly as the game made them. Builds are decided
    // in `crate::item_build_hook` now, on the stable API, where the champion a
    // build belongs to is stated rather than inferred from route order.
    ORIGINAL
        .get()
        .copied()
        .expect("item build hook original function missing")(
        agent,
        items,
        champions,
        champion_ids,
        team1,
        team2,
        mode,
    )
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
