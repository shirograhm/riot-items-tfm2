//! tfm2_item_tactics — inject mod items into the native Personal Tactics item dropdowns
//! ===========================================================================
//! Goal: on the pre-match "Strategy -> Personal" screen (strategy.ui #personal), expose
//!       mod-added items as options in each player's item dropdowns (#item0/#item1/#item2).
//!       When picked, force-inject them into the live match build (approach B, save-safe).
//!
//! - Phase 1a (done)    - detect the strategy screen + inject native dropdown options + poll selection.
//! - Phase 1b (current) - enumerate real mod final items (dump_mod_items -> MOD_REGISTRY/MOD_FINALS,
//!                        active filter, i18n labels). No game function hooking = no crashes.
//! - Phase 2  (next)    - detour the 3 write sites in FUN_140c6c430 to inject into the live build.
//!
//! Reused from: C:\tfm2mods\tfm2_scrim\src\lib.rs (nat_dd_*, SEH, dump_mod_items, item machinery).
//! ===========================================================================
#![allow(dead_code, unused_imports, unused_variables)]
// MERGED INTO riot_items_tfm2 (2026-08-04).
//
// This was a standalone classic-ABI mod. The host mod is a stable-ABI mod, and a
// DLL gets exactly one entry point (see `mod-api-stable/src/entry.rs`: exporting
// `tfm2_mod_entry_stable` tells the loader to skip the legacy path), so the
// classic `init`/`declare_mod!`/`ModExtension`/`ModServerExtension` scaffolding
// is gone and the bodies are driven from the host's stable extensions instead —
// see `driver` below and the call sites in `src/lib.rs`.
//
// `mod_api` is still LINKED, but only for its *types* (`Node`, `Database`,
// `GameUI`, `find_node`, …). Their `repr(Rust)` layout is fixed by the compiler,
// not by the SDK version, and `rust-toolchain.toml` pins the compiler the game
// is built with — the same reasoning that already lets `src/hook.rs` link
// `game_core`. What the classic API used to *hand* us (`ctx.database`,
// `&mut GameUI`, `Scene`) is now sourced from raw addresses the mod captures
// itself; see `driver::db()` and `driver::ui_root()`.
extern crate mod_api;
use mod_api::*;
// `ctx.database` used to supply this; the merged build reaches it through
// `driver::db()` instead, so the type has to be named directly.
use game_core::Database;
// The client half of what the classic `Scene`/`ClientDatabase` used to give.
use mod_api_stable::{RecordKindV1, StableClient};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "ui_inject.rs"]
mod uinj; // 4th-slot UI injection (chained loader hook): item3 dropdown + in-match slot3 display

pub mod driver;
mod ui_root;

const MOD_ID: &str = "tfm2_item_tactics";

/// Kill switch for everything in `tactics_post_update` that walks the live UI
/// node tree — the strategy and comp-test handlers, the in-match 4th slot icon
/// and its tooltip, and the compact slot spacing.
///
/// The root itself comes from [`ui_root::resolve`], which *finds and validates*
/// the address rather than assuming one. That distinction is the whole reason
/// this constant exists: the merge's first attempt reused `TIP_ROOT` — the
/// tooltip function's search root, which is not a `Node` — and walking it was an
/// access violation on the first UI frame, in both slot modes, which no
/// `catch_unwind` can catch.
///
/// Set to `false` to rule this half out if a startup crash ever returns. The
/// engine half does not depend on it: the byte patches, the `buy_item` detour
/// and its build injection, and `ui_inject`'s loader hook (which edits
/// *templates* through offsets it owns) all run regardless.
///
/// One caveat if you do turn it off: `seh_install` used to be reached only from
/// `handle_tactics_screen`, so disabling this silently disabled every
/// `safe_read_*` in the module. That call now lives in `tactics_init`, which is
/// where it belongs, so the two are independent.
const UI_TREE_WALK_ENABLED: bool = true;

// Native dropdown set-options function (0.4.14 hotfix, same RVA as scrim).
//   Prologue 55 56 57 48 83 ec 70, options Vec@+0x1528, selected idx@+0x1788.
//   WARNING: moves with every patch -> re-locate during MIGRATION.
// Confirmed for 0.5.0 (was 0x218a5f0). Validated by the dd_addr_valid() prologue guard (55 56 57 48 83 ec 70) before use.
// ** 0.5.4 re-derivation (2026-08-04, `tools/rederive.py fields` + `calls`). The recorded fingerprints made this
//   the easiest of the set: 9 functions in the whole image touch **all six** documented offsets
//   (+0x1788 selected / +0x1528,0x1530,0x1538 option Vec / +0x1570,0x1578 callback), and of those exactly one
//   has **103 direct callers — the same count recorded for 0.5.3**. It also sits in the same region
//   (0.5.3 was 0x1bfc80) and its prologue is byte-identical, so `dd_addr_valid`'s expectation is unchanged.
const FN_DD_SETOPT_RVA: usize = 0x1c1ad0; // 0.5.4 (0.5.3 was 0x1bfc80). History for 0.5.3 follows. (0.5.2 was 0x242f250). ghidra-re confirmed: 103 direct callers, an exact match with the old exe, plus 4 offset fingerprints (+0x1788 selected / +0x1528,0x1530,0x1538 option Vec / +0x1570,0x1578 callback / element 0xf8 / input stride 0x28) all unchanged. WARNING: the prologue DID change (dd_addr_valid expectation below was updated).

// * Production master diagnostic gate (07-11): this session's diagnostics (nn_moditem, timing, liveroster, p6/channel scan, shadow-call catalog name lookup) plus
//   the older diagnostic flush/hooks (c6new, countprobe, auto4, teamgate) are all OFF. The team gate (is_live/is_player) and SLOT012 injection live outside the gate = unaffected.
const DIAG_ENABLED: bool = false;

/// Trace files this half drops in its own folder: `4items_mode.txt` and
/// `4items_patches.txt` at every init, `version_gate.txt` when the version gate
/// closes, and `4items_netscan.txt` once if the item-network probe misses.
///
/// Off by user request (2026-08-04) — no `.txt` files in the mod folder.
///
/// They were unconditional because a config read that silently falls back to 4
/// slots, a byte patch that silently skips, and a version gate that silently
/// disables this half all look *exactly* like the feature working. With this
/// off there is no evidence of any of them, so the diagnostic route is
/// `BUILD_EXT_DIAG`, which reports the same facts — `mode(slot_count)`, patch
/// state, hook install state — into `build_ext_diag.txt`.
const TRACE_FILES: bool = false;

const MAX_ROWS: usize = 5; // 5 players (#row0..#row4)
const ITEM_SLOTS: usize = 4; // max slot count (array stride). Actually active slots = slot_count() (3/4 toggle)

// -- 3/4 item toggle (cfg `4items.cfg`, next to the dll) --
//   Content '4' = 4 slots (item0/1/2/3) / '3' = 3 slots (vanilla item_tactics behaviour). Missing = default 4. Changing it needs a restart.
static ITEM_MODE: AtomicU64 = AtomicU64::new(4);
fn load_mode() -> u64 {
    // * Config loading must always leave a trace (07-21): if a read failure silently falls back to 4,
    //   a user who wants 3 slots and creates the cfg cannot tell that the path was wrong ("3 slots just doesn't work", cause unknown).
    //   -> always write mod_dir path / read success / parsed mode to a file (regardless of LOG_ENABLED; once at init, so the cost is irrelevant).
    let mut mode = 4u64;
    let mut diag = String::new();
    match mod_dir() {
        None => diag.push_str(
            "! mod_dir()=None (could not resolve the mod directory) -> defaulting to 4 slots
",
        ),
        Some(d) => {
            let p = d.join("4items.cfg");
            diag.push_str(&format!(
                "mod_dir = {}
cfg path = {}
exists = {}
",
                d.display(),
                p.display(),
                p.exists()
            ));
            match fs::read_to_string(&p) {
                Err(e) => diag.push_str(&format!("! cfg read failed ({}) -> defaulting to 4 slots. For 3 slots put a 'slots = 3' file at this path
", e)),
                Ok(s) => {
                    let scan = match s.rfind('=') { Some(i) => &s[i + 1..], None => &s[..] };
                    let mut found = false;
                    for c in scan.chars() { if c == '3' { mode = 3; found = true; break; } if c == '4' { mode = 4; found = true; break; } }
                    diag.push_str(&format!("cfg read OK ({}B) - parsed from={:?} - digit found={}
", s.len(), scan.trim(), found));
                    if !found { diag.push_str("! no 3 or 4 after the '=' -> defaulting to 4 slots
"); }
                }
            }
        }
    }
    diag.push_str(&format!(
        "=> final mode = {} (slot_count={})
",
        mode,
        if mode == 4 { 4 } else { 3 }
    ));
    if TRACE_FILES {
        if let Some(d) = mod_dir() {
            let _ = fs::create_dir_all(&d);
            let _ = fs::write(d.join("4items_mode.txt"), &diag);
        }
    }
    ITEM_MODE.store(mode, Ordering::Relaxed);
    uinj::MODE4.store(mode == 4, Ordering::Relaxed);
    uinj::IN_MATCH_UI.store(mode == 4, Ordering::Relaxed); // enable the in-match 4th slot UI (together with the patches). mode=3 keeps vanilla 3 slots.
    uinj::STRAT_INJECT.store(mode == 3 || mode == 4, Ordering::Relaxed); // strategy screen overlay (item0m/1m/2m) = common to modes 3 and 4
    mode
}
fn slot_count() -> usize {
    if ITEM_MODE.load(Ordering::Relaxed) == 4 {
        4
    } else {
        3
    }
}

/// Whether this half is actually applying `item-builds.json` per athlete.
///
/// Every term is a *measured* state rather than an assumption: the version gate
/// passed, the injection is compiled in, and the `buy_item` detour reported a
/// successful install (`1` = OK). If any is false nothing here touches a build,
/// and `hook::detour` has to keep applying builds itself — to both teams, since
/// its arguments cannot tell them apart.
pub(crate) fn injects_builds() -> bool {
    version_ok() && SLOT012_INJECT_ENABLED && BUY_PROBE_INSTALLED.load(Ordering::Relaxed) == 1
}

// Vanilla 7 option labels (idx 0~6). 1:1 with the game's personal_tactics ItemBuildOverride.
//   * References the game i18n assets -> the dropdown is localized automatically to the game language (base.json lang),
//   the same way mod items (vi>=7) are; verified. Single whole-string labels, so LabelRunner substitutes them (only inline composition is unsupported). Hardcoded Korean was dropped.
//   Key sources: strategy.i18n (build_auto) / ui.i18n (attack, magic_power, attack_speed, defence, magic_resistance, hp).
const VANILLA_OPTS: [&str; 7] = [
    "#asset/base/text/strategy?personal.build_auto",
    "#asset/base/text/item?category.ad",
    "#asset/base/text/item?category.magic",
    "#asset/base/text/item?category.attack_speed",
    "#asset/base/text/item?category.defense",
    "#asset/base/text/item?category.magic_resistance",
    "#asset/base/text/item?category.hp",
];

// ===========================================================================
//  WinAPI FFI
// ===========================================================================
type HMODULE = isize;
type DWORD = u32;
type BOOL = i32;
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HMODULE;
    fn GetModuleFileNameW(h: HMODULE, buf: *mut u16, n: DWORD) -> DWORD;
    fn VirtualQuery(addr: *const core::ffi::c_void, buf: *mut MemBasicInfo, len: usize) -> usize;
    fn VirtualProtect(addr: usize, size: usize, new_protect: u32, old_protect: *mut u32) -> BOOL;
    fn VirtualAlloc(addr: usize, size: usize, alloc_type: u32, protect: u32) -> usize;
    fn FlushInstructionCache(proc: usize, addr: usize, size: usize) -> BOOL;
    fn GetCurrentProcess() -> usize;
    fn AddVectoredExceptionHandler(first: u32, handler: VehHandler) -> usize;
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentThread() -> isize;
    fn GetThreadContext(h: isize, ctx: *mut u8) -> BOOL;
    fn SetThreadContext(h: isize, ctx: *const u8) -> BOOL;
    fn OpenThread(access: u32, inherit: BOOL, tid: u32) -> isize;
    fn SuspendThread(h: isize) -> u32;
    fn ResumeThread(h: isize) -> u32;
    fn CloseHandle(h: isize) -> BOOL;
    fn CreateThread(
        sa: *const u8,
        stack: usize,
        start: extern "system" fn(*mut u8) -> u32,
        param: *mut u8,
        flags: u32,
        tid: *mut u32,
    ) -> isize;
    fn Sleep(ms: u32);
}
#[repr(C)]
#[derive(Default)]
struct MemBasicInfo {
    base: usize,
    alloc_base: usize,
    alloc_protect: u32,
    _pad0: u32,
    region_size: usize,
    state: u32,
    protect: u32,
    mtype: u32,
    _pad1: u32,
}

// One dropdown option = 0x28 (40 bytes): color 16B + text String 24B (game String = {len, ptr, cap})
#[repr(C)]
struct DdOpt {
    color: u64,   // +0  R@0=1.0, G@4=1.0
    color2: u32,  // +8  B@8=1.0
    alpha: f32,   // +12 A=1.0
    s_len: usize, // +16
    s_ptr: usize, // +24
    s_cap: usize, // +32
}

// ===========================================================================
//  Memory-safety helpers (ported from scrim)
// ===========================================================================
unsafe fn readable(addr: usize, len: usize) -> bool {
    if addr < 0x10000 || len == 0 {
        return false;
    }
    let mut mbi = MemBasicInfo::default();
    let n = VirtualQuery(
        addr as *const _,
        &mut mbi,
        core::mem::size_of::<MemBasicInfo>(),
    );
    if n == 0 {
        return false;
    }
    const MEM_COMMIT: u32 = 0x1000;
    const READABLE: u32 = 0x02 | 0x04 | 0x20 | 0x40;
    const NOACCESS_GUARD: u32 = 0x01 | 0x100;
    if mbi.state != MEM_COMMIT {
        return false;
    }
    if mbi.protect & NOACCESS_GUARD != 0 {
        return false;
    }
    if mbi.protect & READABLE == 0 {
        return false;
    }
    addr + len <= mbi.base + mbi.region_size
}
unsafe fn writable(addr: usize, len: usize) -> bool {
    if addr < 0x10000 || len == 0 {
        return false;
    }
    let mut mbi = MemBasicInfo::default();
    let n = VirtualQuery(
        addr as *const _,
        &mut mbi,
        core::mem::size_of::<MemBasicInfo>(),
    );
    if n == 0 {
        return false;
    }
    const MEM_COMMIT: u32 = 0x1000;
    const WRITABLE: u32 = 0x04 | 0x08 | 0x40 | 0x80;
    const GUARD: u32 = 0x100;
    if mbi.state != MEM_COMMIT {
        return false;
    }
    if mbi.protect & GUARD != 0 {
        return false;
    }
    if mbi.protect & WRITABLE == 0 {
        return false;
    }
    addr + len <= mbi.base + mbi.region_size
}
// * Stability: verify the function pointer really points at an executable code page (before a shadow-call). "Readable" alone
//   still leaves a DEP AV on a non-executable page -> check PAGE_EXECUTE_*. Pre-empts the AV that VEH cannot catch.
unsafe fn code_ptr_ok(p: usize) -> bool {
    if p < 0x10000 {
        return false;
    }
    let mut mbi = MemBasicInfo::default();
    if VirtualQuery(
        p as *const _,
        &mut mbi,
        core::mem::size_of::<MemBasicInfo>(),
    ) == 0
    {
        return false;
    }
    const MEM_COMMIT: u32 = 0x1000;
    const EXEC: u32 = 0x10 | 0x20 | 0x40 | 0x80; // PAGE_EXECUTE / _READ / _READWRITE / _WRITECOPY
    const BAD: u32 = 0x100 | 0x01; // GUARD | NOACCESS
    mbi.state == MEM_COMMIT && (mbi.protect & BAD) == 0 && (mbi.protect & EXEC) != 0
}
fn looks_heap(v: u64) -> bool {
    v & 0x7 == 0 && v >= 0x10000 && v < 0x0000_8000_0000_0000 && (v & 0xffff) != 0
}

// ===========================================================================
//  SEH-safe reads - a VEH intercepts access violations (0xC0000005) and returns failure instead of crashing.
//  SEH[]: 0=active 1=tid 2=land_rip 3=land_rsp 4=land_rbp 5=code_lo 6=code_hi 7=faults
// ===========================================================================
#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    rec: usize,
    addr: usize,
    nparams: u32,
    _p: u32,
    params: [usize; 15],
}
#[repr(C)]
struct ExceptionPointers {
    rec: *mut ExceptionRecord,
    ctx: *mut core::ffi::c_void,
}
type VehHandler = extern "system" fn(*mut ExceptionPointers) -> i32;

// * 2026-07-22 switch: global SEH[8] + spinlock -> **per-thread TLS**. (justified by perf measurements)
//   Before: safe_copy shared one global state, so `while SEH_BUSY.swap(true) { spin_loop() }`
//   **serialized every rayon worker**. The buy early-exit path calls safe_read_u64 on every call
//   (6.89M times in 130.7s), so spin contention scaled with worker count = one of the mod's biggest costs.
//   The VEH handler runs **on the very thread that faulted**, so reading its own TLS is enough
//   => no lock needed, and no tid comparison needed either (TLS is thread-scoped by construction).
//   WARNING: keep the VEH safety requirements: Cell array + `const` init + **no Drop** => no lazy-init flag and no
//     TLS destructor registration = there is no path that allocates, locks or panics inside the handler (rule §3).
//   Layout is identical to the old [u64;8] (asm offsets unchanged). idx1 (formerly tid) is left unused.
#[repr(C)]
struct SehTls {
    v: [core::cell::Cell<u64>; 8],
}
thread_local! {
    static SEH_T: SehTls = const { SehTls { v: [const { core::cell::Cell::new(0) }; 8] } };
}
#[inline(always)]
fn seh_ptr() -> *mut u64 {
    // Cell<u64> is repr(transparent) -> [Cell<u64>;8] and [u64;8] have identical layout.
    SEH_T.with(|s| s.v.as_ptr() as *mut u64)
}
static SEH_INSTALLED: AtomicBool = AtomicBool::new(false);

extern "system" fn seh_veh(p: *mut ExceptionPointers) -> i32 {
    const CONTINUE_EXECUTION: i32 = -1;
    const CONTINUE_SEARCH: i32 = 0;
    unsafe {
        if p.is_null() {
            return CONTINUE_SEARCH;
        }
        let rec = (*p).rec;
        if rec.is_null() {
            return CONTINUE_SEARCH;
        }
        if (*rec).code != 0xC0000005 {
            return CONTINUE_SEARCH;
        }
        // * TLS switch: this handler runs on the faulting thread, so its own TLS *is* that thread's state
        //   (the old tid comparison became unnecessary). try_with = silently pass if TLS is being destroyed (no-panic requirement).
        let Ok(g) = SEH_T.try_with(|s| s.v.as_ptr() as *mut u64) else {
            return CONTINUE_SEARCH;
        };
        if *g.add(0) == 0 {
            return CONTINUE_SEARCH;
        }
        let ctx = (*p).ctx as usize;
        if ctx == 0 {
            return CONTINUE_SEARCH;
        }
        let rip = *((ctx + 0xF8) as *const u64);
        if rip < *g.add(5) || rip >= *g.add(6) {
            return CONTINUE_SEARCH;
        }
        *((ctx + 0xF8) as *mut u64) = *g.add(2); // Rip = land_rip
        *((ctx + 0x98) as *mut u64) = *g.add(3); // Rsp = land_rsp
        *((ctx + 0xA0) as *mut u64) = *g.add(4); // Rbp = land_rbp
        *g.add(7) += 1; // fault counter (now per-thread)
        CONTINUE_EXECUTION
    }
}
fn seh_install() {
    if SEH_INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    unsafe {
        AddVectoredExceptionHandler(1, seh_veh);
    }
}
#[inline(never)]
unsafe fn safe_copy(dst: *mut u8, src: *const u8, len: usize) -> bool {
    if !SEH_INSTALLED.load(Ordering::Relaxed) {
        return false;
    }
    // * No lock: state is per-thread, so workers do not contend (the old SEH_BUSY spinlock is gone).
    let g = seh_ptr();
    let mut ok: u64;
    core::arch::asm!(
        "lea rax, [rip + 200f]",
        "mov [{g} + 40], rax",
        "lea rax, [rip + 201f]",
        "mov [{g} + 48], rax",
        "lea rax, [rip + 202f]",
        "mov [{g} + 16], rax",
        "mov [{g} + 24], rsp",
        "mov [{g} + 32], rbp",
        "mov qword ptr [{g} + 0], 1",
        "cld",
        "200:",
        "rep movsb",
        "201:",
        "mov {ok}, 1",
        "jmp 203f",
        "202:",
        "mov {ok}, 0",
        "203:",
        "mov qword ptr [{g} + 0], 0",
        g = in(reg) g,
        ok = out(reg) ok,
        inout("rcx") len => _,
        inout("rdi") dst => _,
        inout("rsi") src => _,
        out("rax") _,
    );
    ok != 0
}
unsafe fn safe_read_u64(addr: usize) -> Option<u64> {
    let mut b = [0u8; 8];
    if safe_copy(b.as_mut_ptr(), addr as *const u8, 8) {
        Some(u64::from_le_bytes(b))
    } else {
        None
    }
}
unsafe fn safe_read_bytes(addr: usize, len: usize, out: &mut Vec<u8>) -> bool {
    if len == 0 || len > 4096 {
        return false;
    }
    out.clear();
    out.resize(len, 0);
    safe_copy(out.as_mut_ptr(), addr as *const u8, len)
}

// ===========================================================================
//  Logging / paths
// ===========================================================================
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
// Game exe path (GetModuleHandleW(NULL) = main exe). Never hardcode - derive the path dynamically.
fn exe_path() -> Option<PathBuf> {
    let mut buf = [0u16; 1024];
    let n = unsafe {
        GetModuleFileNameW(
            GetModuleHandleW(core::ptr::null()),
            buf.as_mut_ptr(),
            buf.len() as u32,
        )
    };
    if n == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..n as usize])))
}
// Game root = the exe folder (...\Teamfight Manager2).
fn game_root() -> Option<PathBuf> {
    exe_path()?.parent().map(|p| p.to_path_buf())
}
/// Where this half's config (`4items.cfg`) and diagnostic files live.
///
/// Was `<game>/mods/tfm2_item_tactics`. After the merge there is no such folder:
/// the code ships inside the host mod, so it reads and writes beside the host's
/// DLL. `config::dll_dir` is used rather than `game_root()/mods/<id>` because
/// the host mod may be installed from the Steam Workshop, in which case its
/// folder is under `steamapps/workshop/content/<appid>/<published_file_id>/` —
/// outside the game directory, and named for a published file id rather than a
/// mod id. The old expression resolves to a path that simply does not exist for
/// those users, which silently disabled every file this reads.
fn mod_dir() -> Option<PathBuf> {
    crate::config::dll_dir()
}

// ===========================================================================
//  Native dropdown control (ported from scrim)
// ===========================================================================
// * Re-enabled for 0.5.1 (07-15): DD_SETOPT (0x2450f40) - ghidra-re confirmed OLD 0x2416070 <-> NEW are line-for-line identical (HIGH confidence, the correct one of 3 siblings; offsets +0x1788/+0x1528/+0x1570, stride 0x28 / 0xf8 all match). Not a misidentification -> ON.
const DD_ENABLED: bool = true;
static DD_VALID: AtomicU64 = AtomicU64::new(0);
unsafe fn prologue_hex(addr: usize, n: usize) -> String {
    if !readable(addr, n) {
        return "UNREADABLE".to_string();
    }
    (0..n)
        .map(|i| format!("{:02x}", *((addr + i) as *const u8)))
        .collect::<Vec<_>>()
        .join(" ")
}
unsafe fn dd_addr_valid() -> bool {
    if !DD_ENABLED {
        return false;
    } // 0.5.1 misidentification mitigation gate
    match DD_VALID.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let fa = GetModuleHandleW(core::ptr::null()) as usize + FN_DD_SETOPT_RVA;
    // 0.5.3: push rbp/r15/r14/rsi/rdi/rbx + sub rsp,0x88 (0.5.2 was 55 56 57 48 83 ec 70).
    //   Byte-identical on 0.5.4 too (verified at 0x1c1ad0), so only the RVA ever moves.
    let expect = [
        0x55u8, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x88,
    ];
    let mut ok = readable(fa, 12);
    if ok {
        for i in 0..12 {
            if *((fa + i) as *const u8) != expect[i] {
                ok = false;
                break;
            }
        }
    }
    DD_VALID.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
    ok
}
unsafe fn runner_base(n: &Node) -> usize {
    let any: &dyn std::any::Any = n.runner.as_any();
    let parts: [usize; 2] = std::mem::transmute::<*const dyn std::any::Any, [usize; 2]>(
        any as *const dyn std::any::Any,
    );
    parts[0]
}
fn find_rb(n: &Node, t: &str) -> Option<usize> {
    if n.id.as_str() == t {
        return Some(unsafe { runner_base(n) });
    }
    for c in n.child.iter() {
        if let Some(b) = find_rb(c, t) {
            return Some(b);
        }
    }
    None
}
fn find_node<'a>(n: &'a Node, t: &str) -> Option<&'a Node> {
    if n.id.as_str() == t {
        return Some(n);
    }
    for c in n.child.iter() {
        if let Some(x) = find_node(c, t) {
            return Some(x);
        }
    }
    None
}
fn type_name_of(root: &Node, id: &str) -> Option<String> {
    fn rec(n: &Node, id: &str) -> Option<String> {
        if n.id.as_str() == id {
            return Some(n.runner.type_name().to_string());
        }
        for c in n.child.iter() {
            if let Some(x) = rec(c, id) {
                return Some(x);
            }
        }
        None
    }
    rec(root, id)
}
fn find_mut<'a>(n: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if n.id.as_str() == id {
        return Some(n);
    }
    for c in n.child.iter_mut() {
        if let Some(x) = find_mut(c, id) {
            return Some(x);
        }
    }
    None
}
// Replace an ImageRunner source (asset key) - game String {len@0, ptr@8, cap@16}. Static string, so nothing leaks.
//   For an empty slot (cap=0) the game does not free it (safe). Layout verified via tfm2_fog set_img_source_ptr.
unsafe fn set_img_src(n: &Node, s: &'static str) -> bool {
    if !n.runner.type_name().contains("ImageRunner") {
        return false;
    }
    let dp = runner_base(n);
    if dp < 0x10000 {
        return false;
    }
    std::ptr::write_unaligned(dp as *mut u64, s.len() as u64);
    std::ptr::write_unaligned((dp + 8) as *mut u64, s.as_ptr() as u64);
    true
}
// * Verification: set every in-match #slot3 icon source to a test item (t5_0) and see whether it shows.
//   If the #slot3 node exists in the live match tree and the source write takes, the whole approach is validated.
const TEST_ITEM_SRC: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18#t5_0";
static SLOT3_TEST_LOGGED: AtomicBool = AtomicBool::new(false);
unsafe fn runner_bytes(n: &Node) -> String {
    let dp = runner_base(n);
    let mut s = format!("rb={:#x}", dp);
    for o in (0..0x48).step_by(8) {
        s.push_str(&format!(
            " +{:#x}={:#x}",
            o,
            std::ptr::read_unaligned((dp + o) as *const u64)
        ));
    }
    s
}
// ImageRunner source string (champion portrait path etc.). Data ptr: len@+0, ptr@+8.
unsafe fn read_img_source(n: &Node) -> Option<String> {
    if !n.runner.type_name().contains("ImageRunner") {
        return None;
    }
    let dp = runner_base(n);
    let len = std::ptr::read_unaligned(dp as *const u64) as usize;
    let ptr = std::ptr::read_unaligned((dp + 8) as *const u64) as *const u8;
    if ptr.is_null() || len == 0 || len > 512 || !readable(ptr as usize, len) {
        return None;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).to_string())
}
// LabelRunner text (text@+352, len@+352, ptr@+360).
const TEXT_OFFSET: usize = 352;
unsafe fn read_label(n: &Node) -> Option<String> {
    if !n.runner.type_name().contains("LabelRunner") {
        return None;
    }
    let dp = runner_base(n);
    let len = std::ptr::read_unaligned((dp + TEXT_OFFSET) as *const u64) as usize;
    let ptr = std::ptr::read_unaligned((dp + TEXT_OFFSET + 8) as *const u64) as *const u8;
    if ptr.is_null() || len == 0 || len > 512 || !readable(ptr as usize, len) {
        return None;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).to_string())
}
// Dump a node subtree as id + (image source / label text) (diagnostic for locating the champion key).
// * Diagnostic (design work for filling the slot3 icon): dump the in-match player_info slot icon nodes -
//   filled slot0/1/2 vs the empty slot3 ImageRunner source + runner base bytes (to pin the layout).
static SLOTDIAG_CNT: AtomicU64 = AtomicU64::new(0);
// * Force 42px spacing on blue slots (0.5.0): the game resets blue slot0/1/2 to vanilla (50px spacing), so we
//   overwrite the authored x that the renderer reads (node+0x84 family) every frame, forcing slot1/2/3 to 42px spacing relative to slot0's real position.
//   slot0 is left alone and only the trailing 3 are re-laid-out -> independent of base_x and resolution.
//   WARNING: ~~"the renderer reads +0x240 (screen_x) to draw"~~ -> corrected: the actual working implementation uses the +0x84 family (the function comment below,
//     "+0x240 turned out to be hit-testing and had no effect", is the after-the-fact correction). **What +0x240 really is, and whether y/w/h are contiguous, is unconfirmed** -
//     if the hitbox ever needs updating, measure it (struct dump) before touching it.
const FORCE_BLUE_SPACING: f32 = 42.0;
// Node authored x = +0x84 (normal). It is duplicated into the hover/press/disabled state blocks at +0x80 stride, so all 4 must be written for
// the value to survive a game reset / state transition (tfm2-ui-runtime-layout: value = block+0x14, blocks 0x70/0xf0/0x170/0x1f0).
#[inline]
unsafe fn set_node_x_all_states(node: &Node, x: f32) {
    let na = node as *const Node as usize;
    if na <= 0x10000 {
        return;
    }
    for off in [0x84usize, 0x104, 0x184, 0x204] {
        if writable(na + off, 4) {
            *((na + off) as *mut f32) = x;
        }
    }
}
// The game resets blue_player slot/stat x (+0x84) to vanilla (50px spacing) every frame -> re-force 42px spacing + left alignment in post_update.
// (Icon rendering uses the authored +0x84. +0x240 turned out to be hit-testing and had no effect.)
unsafe fn force_blue_slot_spacing(n: &Node) {
    if n.id.as_str() == "blue_player" {
        // Use slot0's current x as the (leftmost) baseline - fall back to 59 if absent.
        let mut base = 59.0f32;
        if let Some(s0) = find_node(n, "slot0") {
            let na = s0 as *const Node as usize;
            if na > 0x10000 && readable(na + 0x84, 4) {
                let v = *((na + 0x84) as *const f32);
                if v.is_finite() && v > 1.0 && v < 2000.0 {
                    base = v;
                }
            }
        }
        // Slots: base + 42*i (packed tight, same spacing as red)
        for i in 0..4u32 {
            if let Some(sl) = find_node(n, &format!("slot{}", i)) {
                set_node_x_all_states(sl, base + FORCE_BLUE_SPACING * i as f32);
            }
        }
        // kda/cs: shift left (resolves the overlap with champion 372). Force the same target values as the .ui (in case of a reset).
        if let Some(k) = find_node(n, "kda") {
            set_node_x_all_states(k, 242.0);
        }
        if let Some(c) = find_node(n, "cs") {
            set_node_x_all_states(c, 290.0);
        }
    }
    for c in n.child.iter() {
        force_blue_slot_spacing(c);
    }
}
// Set the options on the target node in the root subtree, and select sel.
unsafe fn nat_dd_set_options(root: &Node, target: &str, items: &[&str], sel: u64) -> bool {
    if !dd_addr_valid() {
        return false;
    }
    let Some(rb) = find_rb(root, target) else {
        return false;
    };
    let mut opts: Vec<DdOpt> = Vec::with_capacity(items.len());
    for &it in items {
        let s = it.to_string();
        opts.push(DdOpt {
            color: 0x3f800000_3f800000,
            color2: 0x3f800000,
            alpha: 1.0,
            s_len: s.len(),
            s_ptr: s.as_ptr() as usize,
            s_cap: s.capacity(),
        });
        std::mem::forget(s);
    }
    let param3: [usize; 3] = [0, opts.as_ptr() as usize, opts.len()];
    let addr = GetModuleHandleW(core::ptr::null()) as usize + FN_DD_SETOPT_RVA;
    let f: unsafe extern "system" fn(usize, u64, *const [usize; 3]) = std::mem::transmute(addr);
    f(rb, sel, &param3);
    std::mem::forget(opts);
    true
}
unsafe fn nat_dd_selected(root: &Node, target: &str) -> Option<usize> {
    if !dd_addr_valid() {
        return None;
    }
    let rb = find_rb(root, target)?;
    let v = *((rb + 0x1788) as *const u64);
    if v == u64::MAX {
        None
    } else {
        Some(v as usize)
    }
}
// Max height of the expanded list (px). If total option height exceeds it, the engine adds scrollbar/clipping automatically.
//   The proper way is `max_items_height:NNN;` in the .ui, but we cannot edit the native strategy.ui -> runtime write instead.
//   * 0.4.14 offsets (ghidra-re): present flag@runner+0x1150 (u32=1) + value@runner+0x1154 (f32 px).
//   (The older 0x1d8 was dropped in 0.4.14 -> it had no effect.) Parser FUN_14218cb20 sets both, and the popup builder
//   FUN_14218a780 reads +0x1154 on every call -> a runtime write is viable (takes effect on the next expand).
const MAX_ITEMS_HEIGHT: f32 = 280.0; // measured in game: pause.ui product_dropdown = 280
unsafe fn set_dd_max_height(root: &Node, target: &str, h: f32) {
    if let Some(rb) = find_rb(root, target) {
        if writable(rb + 0x1150, 8) {
            *((rb + 0x1150) as *mut u32) = 1; // present flag (Option = Some)
            *((rb + 0x1154) as *mut f32) = h; // max_items_height (px)
        }
    }
}

// ===========================================================================
//  JSON parser (for mods.json / item.i18n, ported from scrim)
// ===========================================================================
enum JsonValue {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<JsonValue>),
    Obj(Vec<(String, JsonValue)>),
}
impl JsonValue {
    fn as_obj(&self) -> Option<&Vec<(String, JsonValue)>> {
        if let JsonValue::Obj(o) = self {
            Some(o)
        } else {
            None
        }
    }
    fn get<'b>(&'b self, key: &str) -> Option<&'b JsonValue> {
        self.as_obj()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    fn as_str(&self) -> Option<&str> {
        if let JsonValue::Str(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }
}
struct JsonParser<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> JsonParser<'a> {
    fn new(s: &'a str) -> Self {
        JsonParser {
            b: s.as_bytes(),
            i: 0,
        }
    }
    fn skip_ws(&mut self) {
        while self.i < self.b.len() {
            match self.b[self.i] {
                b' ' | b'\t' | b'\r' | b'\n' | b',' => self.i += 1,
                _ => break,
            }
        }
    }
    fn parse_value(&mut self) -> Option<JsonValue> {
        self.skip_ws();
        if self.i >= self.b.len() {
            return None;
        }
        match self.b[self.i] {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => self.parse_string().map(JsonValue::Str),
            b't' => {
                self.i += 4;
                Some(JsonValue::Bool(true))
            }
            b'f' => {
                self.i += 5;
                Some(JsonValue::Bool(false))
            }
            b'n' => {
                self.i += 4;
                Some(JsonValue::Null)
            }
            _ => self.parse_number(),
        }
    }
    fn parse_string(&mut self) -> Option<String> {
        if self.b.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return Some(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'u' => {
                            if self.i + 4 <= self.b.len() {
                                if let Ok(hex) = std::str::from_utf8(&self.b[self.i..self.i + 4]) {
                                    if let Ok(cp) = u32::from_str_radix(hex, 16) {
                                        if let Some(ch) = char::from_u32(cp) {
                                            let mut buf = [0u8; 4];
                                            out.extend_from_slice(
                                                ch.encode_utf8(&mut buf).as_bytes(),
                                            );
                                        }
                                    }
                                }
                                self.i += 4;
                            }
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        None
    }
    fn parse_number(&mut self) -> Option<JsonValue> {
        let start = self.i;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => self.i += 1,
                _ => break,
            }
        }
        let tok = std::str::from_utf8(&self.b[start..self.i]).ok()?;
        tok.parse::<f64>().ok().map(JsonValue::Num)
    }
    fn parse_array(&mut self) -> Option<JsonValue> {
        self.i += 1;
        let mut arr = Vec::new();
        loop {
            self.skip_ws();
            if self.i >= self.b.len() {
                return None;
            }
            if self.b[self.i] == b']' {
                self.i += 1;
                break;
            }
            arr.push(self.parse_value()?);
        }
        Some(JsonValue::Arr(arr))
    }
    fn parse_object(&mut self) -> Option<JsonValue> {
        self.i += 1;
        let mut pairs = Vec::new();
        loop {
            self.skip_ws();
            if self.i >= self.b.len() {
                return None;
            }
            if self.b[self.i] == b'}' {
                self.i += 1;
                break;
            }
            let k = self.parse_string()?;
            self.skip_ws();
            if self.b.get(self.i) != Some(&b':') {
                return None;
            }
            self.i += 1;
            let v = self.parse_value()?;
            pairs.push((k, v));
        }
        Some(JsonValue::Obj(pairs))
    }
}

// ===========================================================================
//  Mod item registry (dump_mod_items fills it once at server start, ported from scrim)
// ===========================================================================
static MOD_REGISTRY: Mutex<Vec<String>> = Mutex::new(Vec::new()); // idx i -> key (game ID = 30+i)
static MOD_FINALS: Mutex<Vec<u64>> = Mutex::new(Vec::new()); // mod item IDs whose next_tier is empty
static MOD_BUF: AtomicU64 = AtomicU64::new(0); // mod_items array base (element = MOD_BUF + i*stride, key@element+0)
static MOD_STRIDE: AtomicU64 = AtomicU64::new(0);
static NT_OFFSET: AtomicUsize = AtomicUsize::new(0);
static MODITEMS_DONE: AtomicBool = AtomicBool::new(false);
// * 0.5.2: ModItemEntry +0x190 = active flag (!=0 active / ==0 inactive). idx i -> active?
//   Evidence = the game's own Debug impl (0x21a0c10) branches on this field to build "ModItemEntry(<id>, active|inactive)"
//   (cmp qword [rcx+0x190],0 / sete / cmove ", inactive" vs ", active"). Independent confirmation = 0x1408f0870
//   loops over the mod_items array and only processes entries with [rsi+0x190]==0.
//   WARNING: the old rule "present in the mod_items Vec = active" (demonstrated 2026-07-05) died in 0.5.2 - items of disabled mods
//   land in the same Vec as inactive (the game filters them out of the codex; we could not, which is exactly why this field was adopted).
static MOD_ACTIVE: Mutex<Vec<bool>> = Mutex::new(Vec::new());
const MODITEM_ACTIVE_OFF: usize = 0x190;
// One-shot verification dump (key/ID/flag) - written regardless of LOG_ENABLED.
// * OFF for release (2026-07-22): the rule is settled by a two-way demonstration - with riot **disabled** all 104 had raw=0 (X),
//   with riot **enabled** all 110 had raw=pointer (O). No remaining chance of misjudgement -> the dump is unnecessary.

// The 30 vanilla JSON keys (order = ID 0..29). A fingerprint for validating the in-memory master list.
const VANILLA_KEYS: [&str; 30] = [
    "iron_blade",
    "soldiers_longsword",
    "ruinous_blade",
    "conquerors_greatsword",
    "warlords_final_judgement",
    "dagger",
    "wind_dagger",
    "twin_stormblade",
    "thunderclaw",
    "storm_sovereign",
    "steel_armor",
    "gatekeepers_armor",
    "black_knights_heavy_plate",
    "eternal_iron_plate",
    "impregnable_fortress",
    "mystic_cloak",
    "night_hood",
    "dusk_raven",
    "souls_edge",
    "veil_of_annihilation",
    "arcane_crystal",
    "spirit_crystal",
    "staff_of_rapture",
    "angels_fang",
    "prophet_of_the_abyss",
    "vital_orb",
    "hardened_heart",
    "ring_of_reincarnation",
    "hourglass_of_eternity",
    "giants_horn_shard",
];

/// Fills `MOD_REGISTRY`/`MOD_FINALS` from the game's own item catalog, which the
/// host mod's item-build detour is handed as `&Vec<Box<dyn ItemInfo>>`.
///
/// This replaces `dump_mod_items` below, which finds the same information by
/// scanning `Database + 0..0x60000` for something Vec-shaped. That scan needs a
/// correct `Database` base, and the merged build derives one as
/// `item_network - 0x1558` — a value whose only self-check is circular
/// (`sig_ok(db + 0x1558)` is true by construction). It found 0 items, so the
/// 4th-item candidate list was `VANILLA_FINAL` alone and the auto-picked 4th
/// item could never be a mod item.
///
/// The catalog is strictly better evidence: it is the list the game is actually
/// using, it arrives typed, and it needs no base address at all.
///
/// `catalog` is `(key, next_tier)` per entry, in catalog order.
///
/// # Item ids
///
/// `item_id_to_key` defines the id space as `0..30` vanilla (`VANILLA_KEYS`) and
/// `30 + i` for `MOD_REGISTRY[i]`. Those ids stay *inside* this module — the
/// injection path turns an id into a key and then resolves the key against the
/// live catalog by name, so all that matters is that ids and `MOD_REGISTRY`
/// agree with each other. Catalog order is therefore fine even though it is not
/// the game's mod-item order.
/// Whether the mod-item registry has already been built, so a caller can skip
/// assembling the catalog argument. The registry is built once per process; the
/// hook that supplies it fires once per team per match, background league
/// fixtures included, and building that argument is two allocations per item.
pub(crate) fn item_catalog_recorded() -> bool {
    MODITEMS_DONE.load(Ordering::Relaxed)
}

fn record_item_catalog(catalog: Vec<(String, Vec<String>)>) {
    if catalog.is_empty() || MODITEMS_DONE.swap(true, Ordering::Relaxed) {
        return;
    }

    // Pass 1: every key that something upgrades into. `next_tier` is read across
    // the WHOLE catalog, vanilla included — a mod item can be the upgrade target
    // of a vanilla component.
    let built_into: std::collections::HashSet<&str> = catalog
        .iter()
        .flat_map(|(_, next)| next.iter().map(String::as_str))
        .collect();

    let is_vanilla = |k: &str| k == "ironsword" || VANILLA_KEYS.contains(&k);

    let mut registry: Vec<String> = Vec::new();
    let mut finals: Vec<u64> = Vec::new();
    for (key, next_tier) in catalog.iter() {
        if is_vanilla(key) {
            continue;
        }
        let id = 30 + registry.len() as u64;
        // Pass 2: final = nothing to upgrade into, AND something upgrades into
        // it. Both halves matter — "no next tier" alone also accepts a base
        // component nothing builds into, which is not a legal build goal.
        if next_tier.is_empty() && built_into.contains(key.as_str()) {
            finals.push(id);
        }
        registry.push(key.clone());
    }

    let registry_len = registry.len();
    let finals_len = finals.len();
    *MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner()) = registry;
    *MOD_FINALS.lock().unwrap_or_else(|e| e.into_inner()) = finals;
    // `auto_cands` memoizes on first call and never reconsiders, so a list built
    // before this ran would pin the 4th item to vanilla for the whole session.
    *AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner()) = None;

    *CATALOG_NOTE.lock().unwrap_or_else(|e| e.into_inner()) = format!(
        "from host item-build hook catalog: {} entries, {registry_len} mod items, {finals_len} finals",
        catalog.len()
    );
}

/// How `MOD_REGISTRY` was populated, for the diagnostic report.
static CATALOG_NOTE: Mutex<String> = Mutex::new(String::new());

// Scan the Database mod_items Vec in memory -> fill MOD_REGISTRY/MOD_FINALS. (ported from scrim's dump_mod_items)
unsafe fn dump_mod_items(db: usize) {
    if MODITEMS_DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    seh_install();
    let mut s = format!("[{}ms] mod_items walk (db={:#x})\n", now_ms(), db);

    let key_at = |pa: usize| -> Option<String> {
        let ptr = safe_read_u64(pa)? as usize;
        if ptr <= 0x10000 {
            return None;
        }
        for &m in &[64usize, 32, 16, 8] {
            let mut b = Vec::new();
            if !safe_read_bytes(ptr, m, &mut b) {
                continue;
            }
            let mut v = Vec::new();
            for &c in b.iter() {
                if c == b'_' || c.is_ascii_alphanumeric() {
                    v.push(c);
                } else {
                    break;
                }
            }
            if v.len() >= 3 && (v[0] as char).is_ascii_alphabetic() {
                return String::from_utf8(v).ok();
            }
        }
        None
    };
    let is_vanilla = |k: &str| k == "ironsword" || VANILLA_KEYS.contains(&k);
    let item_strides: [usize; 3] = [0x1a8, 0x198, 0x1b0];
    let detect_stride = |buf: usize| -> usize {
        for &st in item_strides.iter() {
            let k: Vec<Option<String>> = (0..4).map(|i| key_at(buf + i * st + 0x8)).collect();
            if k.iter().all(|x| x.is_some()) && k[0] != k[1] && k[1] != k[2] && k[2] != k[3] {
                return st;
            }
        }
        0
    };
    let mut found: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut o = 0usize;
    while o + 0x18 <= 0x60000 && found.len() < 16 {
        let a = db + o;
        o += 8;
        let (Some(q0), Some(q1), Some(q2)) = (
            safe_read_u64(a),
            safe_read_u64(a + 8),
            safe_read_u64(a + 0x10),
        ) else {
            continue;
        };
        for &(p, c) in [(q1, q0), (q1, q2), (q0, q2), (q0, q1)].iter() {
            let (p, c) = (p as usize, c as usize);
            if !looks_heap(p as u64) || c < 3 || c > 2000 {
                continue;
            }
            let Some(k0) = key_at(p + 0x8) else {
                continue;
            };
            if is_vanilla(&k0) {
                continue;
            }
            let cst = detect_stride(p);
            if cst == 0 {
                continue;
            }
            let probe = c.min(48);
            let valid = (0..probe)
                .filter(|&i| key_at(p + i * cst + 0x8).is_some())
                .count();
            if valid * 10 < probe * 8 || valid < 3 {
                continue;
            }
            if found.iter().any(|&(b, _, _, _)| b == p) {
                continue;
            }
            found.push((p, c, cst, a));
        }
    }
    if found.is_empty() {
        s.push_str("  X no non-vanilla item-struct array found (item mods not applied?)\n");
        return;
    }
    found.sort_by(|x, y| y.1.cmp(&x.1));
    let key_of_elem = |elem: usize| -> Option<String> {
        let a = safe_read_u64(elem)? as usize;
        let ptr = safe_read_u64(elem + 8)? as usize;
        let c = safe_read_u64(elem + 0x10)? as usize;
        let len = a.min(c);
        if ptr <= 0x10000 || len < 2 || len > 48 {
            return None;
        }
        let mut b = Vec::new();
        if !safe_read_bytes(ptr, len, &mut b) {
            return None;
        }
        if b.iter().all(|&x| x == b'_' || x.is_ascii_alphanumeric())
            && (b[0] as char).is_ascii_alphabetic()
        {
            String::from_utf8(b).ok()
        } else {
            None
        }
    };
    // read_nt: read elem's next_tier Vec (at offset o) as a key list. (core of item-tree detection)
    let read_nt = |elem: usize, o: usize| -> Option<Vec<String>> {
        let len = safe_read_u64(elem + o)? as usize;
        if len == 0 {
            return Some(Vec::new());
        }
        if len > 8 {
            return None;
        }
        let ptr = safe_read_u64(elem + o + 8)? as usize;
        let cap = safe_read_u64(elem + o + 0x10)? as usize;
        if ptr <= 0x10000 || cap < len {
            return None;
        }
        let mut out = Vec::new();
        for j in 0..len {
            out.push(key_of_elem(ptr + j * 0x18)?);
        }
        Some(out)
    };
    // Extract the key list of a candidate array.
    let build_keys = |buf: usize, st: usize, hdr_cnt: usize| -> Vec<String> {
        let mut keys = Vec::new();
        let mut cnt = 0usize;
        while cnt < hdr_cnt.max(1) && cnt < 500 {
            if let Some(k) = key_of_elem(buf + cnt * st) {
                keys.push(k);
                cnt += 1;
            } else {
                break;
            }
        }
        keys
    };
    // Best next_tier offset for a candidate array + votes (item-tree strength). Player/champion arrays score low votes.
    let best_nt = |buf: usize, st: usize, keys: &[String]| -> (usize, u32) {
        let mut best_off = 0usize;
        let mut best_votes = 0u32;
        let mut o = 0x18usize;
        while o + 0x18 <= st {
            let mut votes = 0u32;
            for i in 0..keys.len() {
                if let Some(v) = read_nt(buf + i * st, o) {
                    if !v.is_empty()
                        && v.iter()
                            .all(|k| keys.iter().any(|x| x.as_str() == k.as_str()))
                    {
                        votes += 1;
                    }
                }
            }
            if votes > best_votes {
                best_votes = votes;
                best_off = o;
            }
            o += 8;
        }
        (best_off, best_votes)
    };
    // * Adopt the candidate array that has a next_tier (item tree) (fixes the bug of picking purely by max count -
    //   player/champion mod arrays can be larger than the item array yet we still pick items correctly. 2026-07-04).
    // * Adoption rule (hardened 07-22): the old rule was "the **first** candidate with votes>=3" (= #1 by descending cnt in found), so
    //   if a disabled mod's staging array was bigger than the active merged array it picked that one. -> switched the **primary key to
    //   the number of active entries** (the array that actually has active items is the one the game uses). Ties break by larger cnt.
    //   If every candidate has 0 active (= the normal state with no item mods enabled), fall back to the old rule and take #1 by cnt.
    let mut diag = String::from("  --- candidate scan (all) ---\n");
    let mut cands: Vec<(usize, usize, Vec<String>, usize, u32, usize)> = Vec::new();
    for &(fbuf, fcnt, fst, _) in &found {
        let keys = build_keys(fbuf, fst, fcnt);
        let (bo, bv) = best_nt(fbuf, fst, &keys);
        let act = (0..keys.len())
            .filter(|&i| {
                safe_read_u64(fbuf + i * fst + MODITEM_ACTIVE_OFF)
                    .map(|v| v != 0)
                    .unwrap_or(false)
            })
            .count();
        diag.push_str(&format!(
            "  buf={:#x} cnt={} stride={:#x} first={:?} nt_off={:#x} votes={} active={}\n",
            fbuf,
            keys.len(),
            fst,
            keys.first(),
            bo,
            bv,
            act
        ));
        if bv >= 3 {
            cands.push((fbuf, fst, keys, bo, bv, act));
        }
    }
    // active desc -> cnt desc (found is already cnt-desc, so a stable sort keeps the original order on ties)
    cands.sort_by(|a, b| b.5.cmp(&a.5));
    let chosen = cands
        .into_iter()
        .next()
        .map(|(b, st, k, o, v, _)| (b, st, k, o, v));
    let Some((buf, st, keys, best_off, best_votes)) = chosen else {
        s.push_str("  X no array carrying an item tree (next_tier) -> item mod probably not loaded or not recognised\n");
        s.push_str(&diag);

        return;
    };
    let cnt = keys.len();
    MOD_BUF.store(buf as u64, Ordering::Relaxed);
    MOD_STRIDE.store(st as u64, Ordering::Relaxed);
    NT_OFFSET.store(best_off, Ordering::Relaxed);
    {
        let mut reg = MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        reg.clear();
        for k in keys.iter() {
            reg.push(k.clone());
        }
    }
    // * Collect the active flags (+0x190). Entries that fail to read fall back to true (active) - dropping something from
    //   the list just because we could not read it would silently erase a user's selection, so when unsure, showing it is safer.
    let actives: Vec<bool> = (0..cnt)
        .map(|i| {
            safe_read_u64(buf + i * st + MODITEM_ACTIVE_OFF)
                .map(|v| v != 0)
                .unwrap_or(true)
        })
        .collect();
    let n_act = actives.iter().filter(|&&a| a).count();
    *MOD_ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = actives.clone();
    s.push_str(&format!("  [chosen] buf={:#x} cnt={} stride={:#x} nt_off={:#x} votes={} active={}/{}\n  idx | ID | act | key\n",
        buf, cnt, st, best_off, best_votes, n_act, cnt));
    for (i, k) in keys.iter().enumerate() {
        s.push_str(&format!(
            "  {:>3} | {:>3} | {} | {}\n",
            i,
            30 + i,
            if actives.get(i).copied().unwrap_or(true) {
                "O"
            } else {
                "X"
            },
            k
        ));
    }
    s.push_str(&diag);

    // * Pass 1: collect all next_tier targets (built_set) - if anything builds into this item, it is a real final candidate.
    //   (Base components like needlessly_large_rod have an empty next_tier but are not targets either, so they are excluded.)
    let mut built: std::collections::HashSet<String> = std::collections::HashSet::new();
    for i in 0..cnt {
        if let Some(nt) = read_nt(buf + i * st, best_off) {
            for k in nt {
                built.insert(k);
            }
        }
    }
    let mut finals: Vec<u64> = Vec::new();
    let mut tree = format!(
        "[{}ms] next_tier offset=+{:#x} votes={}/{} built_targets={}\n",
        now_ms(),
        best_off,
        best_votes,
        cnt,
        built.len()
    );
    for i in 0..cnt {
        let elem = buf + i * st;
        let k = key_of_elem(elem).unwrap_or_default();
        // * Handoff §3 fix: branch read_nt with a match. None (next_tier undecidable at that offset) is
        //   excluded from finals (the old unwrap_or_default() mistook None for an empty Vec -> wrong final items. It really happened with overrides.)
        match read_nt(elem, best_off) {
            Some(nt) if nt.is_empty() => {
                if built.contains(&k) {
                    finals.push(30 + i as u64);
                    tree.push_str(&format!("  {:>3} {} *FINAL\n", 30 + i, k));
                } else {
                    tree.push_str(&format!(
                        "  {:>3} {} (base component - excluded)\n",
                        30 + i,
                        k
                    ));
                }
            }
            Some(nt) => {
                tree.push_str(&format!("  {:>3} {} -> {}\n", 30 + i, k, nt.join(", ")));
            }
            None => {
                tree.push_str(&format!(
                    "  {:>3} {} (next_tier undecidable - excluded)\n",
                    30 + i,
                    k
                ));
            }
        }
    }
    tree.push_str(&format!(
        "  -> {} final items: {:?}\n",
        finals.len(),
        finals
    ));
    *MOD_FINALS.lock().unwrap_or_else(|e| e.into_inner()) = finals;
}

// ===========================================================================
//  Active mod item filter (ported from scrim) - mods.json enabled_mods x each mod's text/item.i18n
// ===========================================================================
fn enabled_mods() -> Vec<String> {
    let mut out = Vec::new();
    let Some(root) = game_root() else {
        return out;
    };
    let Ok(txt) = fs::read_to_string(root.join("config").join("game").join("mods.json")) else {
        return out;
    };
    if let Some(p) = txt.find("\"enabled_mods\"") {
        if let Some(lb) = txt[p..].find('[') {
            let start = p + lb + 1;
            if let Some(rb) = txt[start..].find(']') {
                for part in txt[start..start + rb].split(',') {
                    let s = part.trim().trim_matches('"').trim();
                    if !s.is_empty() {
                        out.push(s.to_string());
                    }
                }
            }
        }
    }
    out
}
fn build_active_item_keys() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let enabled = enabled_mods();
    if enabled.is_empty() {
        return set;
    }
    let Some(root) = game_root() else {
        return set;
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(root.join("mods")) {
        for e in rd.flatten() {
            dirs.push(e.path());
        }
    }
    if let Some(ws) = root
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("workshop").join("content").join("3009300"))
    {
        if let Ok(rd) = fs::read_dir(&ws) {
            for e in rd.flatten() {
                dirs.push(e.path());
            }
        }
    }
    for d in dirs {
        let Ok(info) = fs::read_to_string(d.join("mod.mod_info")) else {
            continue;
        };
        let Some(iv) = JsonParser::new(&info).parse_value() else {
            continue;
        };
        let Some(mid) = iv.get("mod_id").and_then(|x| x.as_str()) else {
            continue;
        };
        if !enabled.iter().any(|e| e == mid) {
            continue;
        }
        let Ok(i18n) = fs::read_to_string(d.join("text").join("item.i18n")) else {
            continue;
        };
        if let Some(JsonValue::Obj(langs)) = JsonParser::new(&i18n).parse_value() {
            for (_, lobj) in langs {
                if let JsonValue::Obj(items) = lobj {
                    for (k, _) in items {
                        set.insert(k);
                    }
                }
            }
        }
    }
    set
}
static ACTIVE_KEYS: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
fn active_item_keys() -> std::collections::HashSet<String> {
    {
        let g = ACTIVE_KEYS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = g.as_ref() {
            return s.clone();
        }
    }
    let set = build_active_item_keys();
    *ACTIVE_KEYS.lock().unwrap_or_else(|e| e.into_inner()) = Some(set.clone());
    set
}

// All dynamic final items = (game ID, key). Map MOD_FINALS (empty next_tier) through MOD_REGISTRY to keys.
fn mod_final_opts_all() -> Vec<(u64, String)> {
    let finals = MOD_FINALS.lock().unwrap_or_else(|e| e.into_inner());
    let reg = MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    finals
        .iter()
        .filter_map(|&id| {
            let i = (id as usize).checked_sub(30)?;
            reg.get(i).map(|k| (id, k.clone()))
        })
        .collect()
}
// Finals exposed in the picker = only the **active** ones (+0x190 != 0) among the DB scan results.
//   ~~Old: the scan result as-is (disabled mods were never merged in anyway)~~ -> invalidated in 0.5.2 (2026-07-22):
//   disabled mods' items also arrive in the same Vec as inactive and showed up in the dropdown (user-confirmed:
//   they do not appear in the in-game codex = the game filters them and only we failed to). Mirror the game's own Debug impl criterion.
//   WARNING: the fail-safe is **only the "flags not yet collected (pre-scan)" layer**. The first version (07-22) added "if everything is inactive, suspect
//   a misjudgement -> no filter", which **inverted the correct answer**: in an environment with no item-adding mod enabled, 0 active is
//   normal (measured: the enabled mods map_free / leefs_variety* / banpick_illust have no item.i18n at all), and treating that as
//   a misjudgement re-exposed the 104 inactive items = the exact symptom again. Treat **0 active as a valid state**.
//   (Even with an empty list the 7 vanilla categories always remain, so the dropdown is never completely empty.)
fn mod_final_opts() -> Vec<(u64, String)> {
    let all = mod_final_opts_all();
    let act = MOD_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    if act.is_empty() {
        return all;
    } // not scanned yet = undecidable -> no filter
    all.iter()
        .filter(|(id, _)| {
            (*id as usize)
                .checked_sub(30)
                .and_then(|i| act.get(i).copied())
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}
// Total picker options = auto (1) + 6 categories + dynamic finals
fn item_opt_count() -> usize {
    7 + mod_final_opts().len()
}
// Picker value v -> label. 0~6 = fixed, 7+ = mod finals (references the game i18n -> localized automatically).
fn item_opt_label(v: u8) -> String {
    let vi = v as usize;
    if vi < 7 {
        return VANILLA_OPTS[vi].to_string();
    }
    match mod_final_opts().get(vi - 7) {
        Some((_, key)) => format!("#asset/base/text/item?{}.name", key),
        None => VANILLA_OPTS[0].to_string(),
    }
}

// ===========================================================================
//  Mod state
// ===========================================================================
static SCREEN_OPEN: AtomicBool = AtomicBool::new(false);
static OPTS_INJECTED: AtomicBool = AtomicBool::new(false);
static LAST_SEL: Mutex<[i64; MAX_ROWS * ITEM_SLOTS]> = Mutex::new([-1i64; MAX_ROWS * ITEM_SLOTS]);
// * (champion key, slot) -> selected option index. Champion-keyed, so it persists per champion even as the lineup changes each match.
//   idx 0~6 = vanilla categories, 7+ = mod items (mod_final_opts[idx-7]). Persisted (item_tactics_sel.txt).
static SEL_BY_CHAMP: Mutex<Option<HashMap<(String, u8), u8>>> = Mutex::new(None);
static SEL_LOADED: AtomicBool = AtomicBool::new(false);
// * Snapshot of the game's personal_tactics: champion -> [3 category bytes (0~6)]. Used to restore the vanilla display that our NOP broke.
//   Refreshed in post_update (InGame, personal screen) from db().team(pid).champion_personal_tactics.
static PT_SNAPSHOT: Mutex<Option<HashMap<String, [u8; 3]>>> = Mutex::new(None);

// Extract the champion key from the #champion/#icon ImageRunner source
//   "asset/base/aseprite_resources/champions/{champ}#sheet" → champ
fn row_champ(row: &Node) -> Option<String> {
    let icon = find_node(row, "icon")?;
    let src = unsafe { read_img_source(icon) }?;
    let a = src.find("champions/")? + "champions/".len();
    let rest = &src[a..];
    let end = rest.find('#').unwrap_or(rest.len());
    let champ = rest[..end].trim();
    if champ.is_empty() {
        None
    } else {
        Some(champ.to_string())
    }
}
/// Where the dropdown selections live.
///
/// JSON, and named like the mod's other state, because the plain-text original
/// was the last thing in this half that could put a `.txt` in the mod folder.
/// Same shape as `item-builds.json` — champion to a slot-indexed list, `null`
/// for a slot with no selection — since they hold the same kind of thing.
fn sel_path() -> Option<PathBuf> {
    Some(mod_dir()?.join("item-tactics-selections.json"))
}

/// The pre-JSON file, read once if the JSON is absent so an existing install
/// keeps its selections. Never written: after the first save the JSON is the
/// live copy and this is left alone as its own backup.
fn legacy_sel_path() -> Option<PathBuf> {
    Some(mod_dir()?.join("item_tactics_sel.txt"))
}

// ═══════════════════════════════════════════════════════════════════════════
//  * Comp-test side scope (added 2026-07-30)
// ═══════════════════════════════════════════════════════════════════════════
//  Problem: the SEL key was only (champion, slot), so putting **the same champion on both sides in comp test** merged the
//    two selections into one (whichever was edited last overwrote the other, and a re-seed synced both rows to the same value). Worse,
//    comp-test selections share the same store, so they **leaked into that champion's league/background matches too.**
//  Fix: prefix the champ column of the SEL key with a **scope tag**. The HashMap type, the file format (`champ slot token`,
//    3 space-separated columns) and the SEL_PENDING structure are **all left untouched** - only the key string is extended, so there is no fallout.
//    - plain (league/spectate/background) = no prefix => **existing files stay valid** (legacy compatible)
//    - comp test blue = `@b:` / red = `@r:`
//    champ is an asset key (no spaces, no `@`), so the prefix cannot collide with a name, and an older dll reading this file
//    simply ignores those lines as "no such champion" (downgrade-safe).
const CT_PFX_B: &str = "@b:";
const CT_PFX_R: &str = "@r:";
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Scope {
    Plain,
    CtBlue,
    CtRed,
}
fn scope_pfx(s: Scope) -> &'static str {
    match s {
        Scope::Plain => "",
        Scope::CtBlue => CT_PFX_B,
        Scope::CtRed => CT_PFX_R,
    }
}
fn scoped_key(s: Scope, champ: &str) -> String {
    match s {
        Scope::Plain => champ.to_string(),
        _ => format!("{}{}", scope_pfx(s), champ),
    }
}
fn is_scoped(k: &str) -> bool {
    k.starts_with(CT_PFX_B) || k.starts_with(CT_PFX_R)
}
// The bare champion name with the scope prefix stripped (designation checks and side votes must be scope-independent).
fn strip_scope(k: &str) -> &str {
    if let Some(r) = k.strip_prefix(CT_PFX_B) {
        return r;
    }
    if let Some(r) = k.strip_prefix(CT_PFX_R) {
        return r;
    }
    k
}
// * Explicit Auto sentinel: used only on scoped keys. When a comp-test slot is set back to Auto,
//   simply deleting the entry would **resurrect the unprefixed (plain) selection as a fallback**, which to the user looks like "nothing changed".
//   => record "this side, this slot has no selection" explicitly to block the fallback. File token = `auto`.
const SEL_AUTO: u8 = 255;
const SEL_AUTO_TOKEN: &str = "auto";
// * Persistence format = item "key" based (switched 2026-07-22).
//   The old format stored the dropdown option index (u8) verbatim -> whenever the list composition changed (mod on/off,
//   introduction of the active filter, ...) every stored selection shifted to a different item. In memory it is still an index,
//   but the file stores `1`~`6` (vanilla categories) or the mod item key string.
//   Legacy (numbers >= 7) = an old index -> resolve it to a key against the **unfiltered list** (the same composition
//   that existed when the selection was made) and then remap to the current index.
// WARNING: the item registry is only filled after dump_mod_items at server start, but SEL loading is lazy and
//   can happen first -> unresolvable entries are not discarded but kept verbatim in SEL_PENDING and absorbed once the
//   registry is ready. On save, pending entries are written back verbatim too = **nothing is ever lost**.
static SEL_PENDING: Mutex<Vec<(String, u8, String)>> = Mutex::new(Vec::new());
static SEL_PENDING_ANY: AtomicBool = AtomicBool::new(false);
fn registry_ready() -> bool {
    !MOD_REGISTRY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_empty()
}
// key -> option index (7+) within the currently exposed list.
fn key_to_opt_index(key: &str) -> Option<u8> {
    mod_final_opts()
        .iter()
        .position(|(_, k)| k == key)
        .map(|i| (i + 7) as u8)
}
// * Promote a legacy numeric index (>=7) to a key when possible. If it cannot be promoted, keep it verbatim.
//   Leaving the number in pending keeps it dependent on "the list composition at that time", so once it is resolved
//   after the mod set has changed it points at the wrong item -> pin it to a key the moment it becomes resolvable.
fn normalize_token(tok: &str) -> String {
    if let Ok(n) = tok.parse::<u8>() {
        if n >= 7 && registry_ready() {
            if let Some((_, k)) = mod_final_opts_all().get(n as usize - 7) {
                return k.clone();
            }
        }
    }
    tok.to_string()
}
// File token -> option index.
fn token_to_opt_index(tok: &str) -> Option<u8> {
    if tok == SEL_AUTO_TOKEN {
        return Some(SEL_AUTO);
    } // * explicit Auto (scoped keys only) - no registry needed
    if let Ok(n) = tok.parse::<u8>() {
        if n == 0 {
            return None;
        }
        if n < 7 {
            return Some(n);
        } // vanilla categories 1~6
        if !registry_ready() {
            return None;
        } // legacy index but not resolvable yet
        let key = mod_final_opts_all()
            .get(n as usize - 7)
            .map(|(_, k)| k.clone())?;
        return key_to_opt_index(&key);
    }
    if !registry_ready() {
        return None;
    }
    key_to_opt_index(tok)
}
// Option index -> file token.
fn opt_index_to_token(idx: u8) -> Option<String> {
    if idx == 0 {
        return None;
    }
    if idx == SEL_AUTO {
        return Some(SEL_AUTO_TOKEN.to_string());
    } // * explicit Auto must always be preserved
    if idx < 7 {
        return Some(idx.to_string());
    }
    mod_final_opts()
        .get(idx as usize - 7)
        .map(|(_, k)| k.clone())
}
/// `(champion, slot, token)` rows from the JSON file, or from the legacy text
/// file when the JSON does not exist yet.
///
/// Both formats carry the same three things, so the caller's normalization and
/// pending handling do not care which one answered.
fn read_sel_rows() -> Vec<(String, u8, String)> {
    if let Some(text) = sel_path().and_then(|p| fs::read_to_string(p).ok()) {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) {
            let mut rows = Vec::new();
            for (champ, value) in map {
                let Some(slots) = value.as_array() else {
                    continue;
                };
                for (slot, token) in slots.iter().enumerate() {
                    if slot >= ITEM_SLOTS {
                        break;
                    }
                    if let Some(token) = token.as_str() {
                        rows.push((champ.clone(), slot as u8, token.to_string()));
                    }
                }
            }
            return rows;
        }
    }
    // Pre-JSON format: one `champion slot token` per line.
    let Some(text) = legacy_sel_path().and_then(|p| fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 3 {
                return None;
            }
            let slot = parts[1].parse::<u8>().ok()?;
            Some((parts[0].to_string(), slot, parts[2].to_string()))
        })
        .collect()
}

fn load_sel() -> HashMap<(String, u8), u8> {
    let mut m = HashMap::new();
    let mut pend = Vec::new();
    for (champ, slot, tok) in read_sel_rows() {
        let tok = normalize_token(&tok); // pin to a key if resolvable, then store
        match token_to_opt_index(&tok) {
            // idx 0 ("leave to the player") is not an override -> fall back to delegate. Not saved/loaded (removes spurious 0s).
            Some(idx) if idx >= 1 => {
                m.insert((champ, slot), idx);
            }
            Some(_) => {}
            None => pend.push((champ, slot, tok)),
        }
    }
    SEL_PENDING_ANY.store(!pend.is_empty(), Ordering::Relaxed);
    *SEL_PENDING.lock().unwrap_or_else(|e| e.into_inner()) = pend;
    m
}
// Absorb pending entries once the registry is ready. Only called inside with_sel (SEL lock held).
//   * Hot-path consideration: with_sel is called often, so when pending is empty this reads one atomic and returns
//   immediately (it does not even take the MOD_REGISTRY lock).
fn drain_pending(m: &mut HashMap<(String, u8), u8>) {
    if !SEL_PENDING_ANY.load(Ordering::Relaxed) {
        return;
    }
    if !registry_ready() {
        return;
    }
    let mut pend = SEL_PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pend.is_empty() {
        return;
    }
    for e in pend.iter_mut() {
        e.2 = normalize_token(&e.2);
    } // registry is ready -> pin numbers to keys
    pend.retain(|(champ, slot, tok)| match token_to_opt_index(tok) {
        Some(idx) if idx >= 1 => {
            m.insert((champ.clone(), *slot), idx);
            false
        }
        Some(_) => false,
        None => true, // still unresolvable (e.g. that mod is disabled) -> keep verbatim
    });
    SEL_PENDING_ANY.store(!pend.is_empty(), Ordering::Relaxed);
}
fn save_sel(m: &HashMap<(String, u8), u8>) {
    let mut rows: Vec<(String, u8, String)> = m
        .iter()
        .filter_map(|((champ, slot), &idx)| {
            opt_index_to_token(idx).map(|t| (champ.clone(), *slot, t))
        })
        .collect();
    // Entries that are still unresolved are kept verbatim -> no loss.
    rows.extend(
        SEL_PENDING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned(),
    );
    rows.sort();

    // Champion -> one entry per slot, `null` where nothing is selected.
    // `BTreeMap` so the file keeps a stable order between writes.
    let mut by_champ: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        std::collections::BTreeMap::new();
    for (champ, slot, tok) in rows {
        if slot as usize >= ITEM_SLOTS {
            continue;
        }
        let entry = by_champ
            .entry(champ)
            .or_insert_with(|| vec![serde_json::Value::Null; ITEM_SLOTS]);
        entry[slot as usize] = serde_json::Value::String(tok);
    }
    let map: serde_json::Map<String, serde_json::Value> = by_champ
        .into_iter()
        .map(|(champ, slots)| (champ, serde_json::Value::Array(slots)))
        .collect();

    if let (Some(p), Ok(text)) = (sel_path(), serde_json::to_string_pretty(&map)) {
        let _ = fs::write(p, text + "\n");
    }
}

// -- One-shot application of dashboard-recommended builds (added 2026-07-22) --------------------
//  Purpose: push the recommended builds that the TFM2.gg dashboard derived from statistics into the personal-tactics dropdowns as their **initial selection**.
//  Why a separate file: mixed into the user's selections (item_tactics_sel.txt) it becomes impossible to tell which values were hand-picked.
//    Recommendations live in item_tactics_recommend.txt, and "already applied" is decided by a content hash (.applied).
//  Behaviour: when the strategy screen opens, if the recommendation file's hash differs from the last applied one, overwrite SEL **once** at that moment.
//    - Same hash = do nothing => any value the user changed by hand in the meantime survives.
//    - Closing the screen resets OPTS_INJECTED, so refreshing the dashboard while the game is running takes effect
//      as soon as the strategy screen is reopened (no game restart needed).
//  WARNING: vanilla categories are already handled by delegate via champion_personal_tactics (the PT_SNAPSHOT path).
//    This file exists for **mod item** selections, which PT cannot hold (though it does accept vanilla tokens 1~6).
//  Revert: set RECO_ENABLED=false or delete the recommendation file and behaviour returns to normal immediately.
// WARNING 2026-07-22 OFF: the recommendation formula is unfinished (it aggregates per full build, so a build with a single-match sample ranks #1) ->
//   it actually overwrote the user's manual selections (sel.txt 74 lines -> 250 lines). Re-enable after switching the formula to per-item shrinkage+lift.
const RECO_ENABLED: bool = false;
fn reco_path() -> Option<PathBuf> {
    Some(mod_dir()?.join("item_tactics_recommend.txt"))
}
fn reco_stamp_path() -> Option<PathBuf> {
    Some(mod_dir()?.join("item_tactics_recommend.applied"))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// Called when entering the strategy screen. If there is a new recommendation, apply it to SEL and return true.
fn apply_recommendations() -> bool {
    if !RECO_ENABLED {
        return false;
    }
    let Some(p) = reco_path() else {
        return false;
    };
    // Missing file = silently skip - that is the normal state for users who do not use the dashboard.
    let Ok(txt) = fs::read_to_string(&p) else {
        return false;
    };
    let hash = fnv1a64(txt.as_bytes()).to_string();
    let stamp = reco_stamp_path();
    let prev = stamp.as_ref().and_then(|s| fs::read_to_string(s).ok());
    if prev.as_deref().map(str::trim) == Some(hash.as_str()) {
        return false; // already-applied recommendation -> preserve the user's later manual changes.
    }

    // Parsing uses the same format as the sel file (`champ slot token`), so the existing parser is reused.
    let mut rows: Vec<(String, u8, String)> = Vec::new();
    for line in txt.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 3 {
            continue;
        }
        let Ok(slot) = parts[1].parse::<u8>() else {
            continue;
        };
        if slot as usize >= ITEM_SLOTS {
            continue;
        }
        rows.push((parts[0].to_string(), slot, normalize_token(parts[2])));
    }
    if rows.is_empty() {
        // An empty recommendation must also be marked "applied", otherwise we retry every frame.
        if let Some(s) = stamp {
            let _ = fs::write(s, &hash);
        }
        return false;
    }

    let mut applied = 0usize;
    with_sel(|m| {
        for (champ, slot, tok) in rows.iter() {
            match token_to_opt_index(tok) {
                // 0 (auto) is not an override, so drop it from SEL and let the delegate (PT) value show.
                Some(idx) if idx >= 1 => {
                    m.insert((champ.clone(), *slot), idx);
                    applied += 1;
                }
                Some(_) => {
                    m.remove(&(champ.clone(), *slot));
                }
                // Not resolvable yet (registry not ready / that mod disabled) -> keep verbatim, absorb later.
                None => {
                    SEL_PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((
                        champ.clone(),
                        *slot,
                        tok.clone(),
                    ));
                    SEL_PENDING_ANY.store(true, Ordering::Relaxed);
                }
            }
        }
        save_sel(m);
    });
    if let Some(s) = stamp {
        let _ = fs::write(s, &hash);
    }
    update_override_snapshot();

    true
}

/// Entry count of `SEL_BY_CHAMP`, readable without taking its lock.
///
/// `usize::MAX` until the map has been loaded, so "not loaded yet" and "loaded
/// and empty" stay distinguishable — only the second is safe to short-circuit
/// on. Kept in step inside [`with_sel`], under the same lock as the map.
static SEL_LEN: AtomicUsize = AtomicUsize::new(usize::MAX);

// SEL_BY_CHAMP access (loads the file once). Manipulated inside the lock via a closure.
fn with_sel<R>(f: impl FnOnce(&mut HashMap<(String, u8), u8>) -> R) -> R {
    let mut g = SEL_BY_CHAMP.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(load_sel());
        SEL_LOADED.store(true, Ordering::Relaxed);
    }
    let m = g.as_mut().unwrap();
    drain_pending(m); // absorbed here even if the registry becomes ready late
    let r = f(m);
    // After `f`, so a closure that inserted or cleared is accounted for.
    SEL_LEN.store(m.len(), Ordering::Relaxed);
    r
}
// * The single entry point for SEL lookups (scope-aware). A scoped key wins; otherwise fall back to the plain key.
//   - `SEL_AUTO` = the user explicitly set that side/slot to Auto => resolve to 0 (no selection) and do NOT fall back.
//   - Scope::Plain performs exactly the old lookup (plain key only) = league/spectate/background behaviour unchanged.
fn sel_get(scope: Scope, champ: &str, si: u8) -> u8 {
    // Hot: the buy detour reaches this up to six times per buy decision for a
    // player athlete (two lookups per slot, three slots), and every one of them
    // took the global `SEL_BY_CHAMP` lock and allocated a `String` purely to
    // build a tuple key to probe with. An empty map — which is the normal state
    // now that the build editor owns item designation and the dropdowns only
    // carry stat categories — can answer without doing either.
    //
    // `SEL_PENDING_ANY` is part of the condition, not decoration: `with_sel` is
    // also where `drain_pending` absorbs selections that could not be resolved
    // until the mod-item registry was ready. Skipping the lock while entries are
    // still queued would skip the drain that lands them, so the short circuit is
    // only taken when there is genuinely nothing to do.
    //
    // Otherwise safe because the map only gains entries through a dropdown, i.e.
    // from the strategy screen between matches. The worst case is one stale
    // lookup if that raced a buy, and the next lookup sees the new value.
    if SEL_LEN.load(Ordering::Relaxed) == 0 && !SEL_PENDING_ANY.load(Ordering::Relaxed) {
        return 0;
    }
    with_sel(|m| {
        if scope != Scope::Plain {
            if let Some(&v) = m.get(&(scoped_key(scope, champ), si)) {
                return if v == SEL_AUTO { 0 } else { v };
            }
        }
        m.get(&(champ.to_string(), si)).copied().unwrap_or(0)
    })
}
// === Comp-test side detection (athlete -> blue/red inside the buy detour) ===
//  The comp-test UI (handle_comptest_screen) publishes the row composition, and buy decides the side from that snapshot.
//  (1) If a champion appears on only one side, that side is certain + we learn the athlete+0x820 (side) value observed then.
//  (2) If the same champion is on both sides, tell them apart using the side value learned in (1).
//  (3) Before learning, or when undecidable, fall back to Scope::Plain (= previous behaviour) => never a regression.
//  WARNING: which of the side values (0/1) is blue vs red is **not hardcoded** (not measured). Learning only needs one
//    non-overlapping champion, so we only fall to (3) in the extreme case where all 10 picked the same champion.
//  NOTE: the snapshot uses the same leak pattern as OVERRIDE_SNAPSHOT (a parallel detour may be reading it, so never free).
type CtRoster = (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
); // (blue, red)
static CT_ROSTER: AtomicPtr<CtRoster> = AtomicPtr::new(core::ptr::null_mut());
static CT_SIDE_B: AtomicU64 = AtomicU64::new(u64::MAX); // athlete+0x820 value learned as blue
static CT_SIDE_R: AtomicU64 = AtomicU64::new(u64::MAX); // value learned as red
fn publish_ct_roster(
    blue: std::collections::HashSet<String>,
    red: std::collections::HashSet<String>,
) {
    let cur = CT_ROSTER.load(Ordering::Acquire);
    if !cur.is_null() {
        let c = unsafe { &*cur };
        if c.0 == blue && c.1 == red {
            return;
        } // unchanged -> do not republish (bounded leak)
    }
    CT_ROSTER.store(Box::into_raw(Box::new((blue, red))), Ordering::Release);
}
fn ct_scope_for(champ: &str, side: u64) -> Scope {
    let p = CT_ROSTER.load(Ordering::Acquire);
    if p.is_null() {
        return Scope::Plain;
    }
    let (blue, red) = unsafe { &*p };
    let (in_b, in_r) = (blue.contains(champ), red.contains(champ));
    match (in_b, in_r) {
        (true, false) => {
            if side != u64::MAX {
                CT_SIDE_B.store(side, Ordering::Relaxed);
            }
            Scope::CtBlue
        }
        (false, true) => {
            if side != u64::MAX {
                CT_SIDE_R.store(side, Ordering::Relaxed);
            }
            Scope::CtRed
        }
        (true, true) => {
            // same champion on both sides -> distinguish only by the learned side value
            if side != u64::MAX {
                if side == CT_SIDE_B.load(Ordering::Relaxed) {
                    return Scope::CtBlue;
                }
                if side == CT_SIDE_R.load(Ordering::Relaxed) {
                    return Scope::CtRed;
                }
            }
            Scope::Plain
        }
        (false, false) => Scope::Plain, // champion not in the comp-test composition (= not seen on screen) -> previous behaviour
    }
}
// * Performance (0.5.1): "is this champion designated" = SEL snapshot (zero-alloc contains). Removes a with_sel lock + 4x champ.to_string() allocations per buy.
//   SEL only changes when a dropdown changes -> invalidated via SEL_DIRTY, and the snapshot is rebuilt only then. Reads = short Arc clone, then contains outside the lock.
// * The scope prefix is **stripped** before insertion (2026-07-30): a champion that only has comp-test-scoped selections must still be
//   seen as designated, otherwise buy never enters the lookup path. Storing it with the prefix would ignore that champion forever.
static DESIGNATED_SNAP: Mutex<Option<std::sync::Arc<std::collections::HashSet<String>>>> =
    Mutex::new(None);
static SEL_DIRTY: AtomicBool = AtomicBool::new(true);
fn designated_set() -> std::collections::HashSet<String> {
    with_sel(|m| m.keys().map(|(c, _)| strip_scope(c).to_string()).collect())
}
fn is_champ_designated(champ: &str) -> bool {
    // A champion the build editor pins is designated even with no `SEL` entry —
    // this is the early exit the spawn path takes before it ever looks at a
    // slot, so missing it here would make `item-builds.json` invisible to the
    // injection no matter what the slot lookups say.
    if crate::build_config::has_pins(champ) {
        return true;
    }
    if SEL_DIRTY.swap(false, Ordering::Relaxed) {
        *DESIGNATED_SNAP.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(std::sync::Arc::new(designated_set()));
    }
    let snap = {
        DESIGNATED_SNAP
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    };
    match snap {
        Some(s) => s.contains(champ),
        None => {
            // first-time race (snapshot not built yet) -> force a build
            let arc = std::sync::Arc::new(designated_set());
            let hit = arc.contains(champ);
            *DESIGNATED_SNAP.lock().unwrap_or_else(|e| e.into_inner()) = Some(arc);
            hit
        }
    }
}
static SETTER_NOPED: AtomicU64 = AtomicU64::new(0); // 0=not attempted, 1=success, 2=failure (RVA mismatch)

// * NOP-patch the `call FUN_14218a230` (RVA 0xf1a74b, 5B `e8 rel32`) inside StrategyUIRunner update (FUN_140f17b40) that
//   force-syncs (reverts) personal_tactics -> dropdown +0x1788 every frame.
//   -> revert gone -> the user's mod item (7+) click persists in +0x1788 -> polling can capture it.
//   (ghidra-re 2026-06-30: the dropdown click handler itself does not reject 7+; the revert was the real culprit.)
//   Side effect: the dropdown's automatic display sync is lost -> the mod keeps it in sync directly via sel when injecting options.
// * 0.5.0_2 call site confirmed (ghidra-re 2026-07-08): inside StrategyUIRunner update (starts at 0x140da1da0),
//   the `call FUN_140d98720` (RVA 0xda42ee, e8 2d 44 ff ff) in the 3-iteration item0/1/2 loop. FUN_140d98720 rebuilds
//   the option list and calls `FUN_142418cf0(runner,index,opts)` -> whose first line `*(runner+0x1788)=index` is the actual revert.
//   WARNING: migrator candidates 0xf2a899/0xf2aae8 are both wrong (that is the draft UI runner = FUN_140f29840; NOPing it only breaks the draft).
const SETTER_NOP_RVA: usize = 0xda42ee; // WARNING: STALE for 0.5.2/0.5.3 (not migrated; harmless because SETTER_NOP_ENABLED=false) // WARNING: not migrated for 0.5.0_3 (STALE, mask-sig NONE -> follow-up via ghidra-re). Harmless because SETTER_NOP_ENABLED=false. This is the 0.5.0_2 model->+0x1788 label-sync call inside the StrategyUIRunner update item0/1/2 loop.
                                        // * 2026-07-08 second ghidra-re confirmation: this NOP is "not" the cause of the slot1/2/3 mod-item commit problem. It is the only
                                        //   +0x1788 revert-writer in the whole binary, yet even with it NOPed polling never saw item0/1/2 become 7+ = the click itself is
                                        //   rejected by the native dropdown's validation against its vanilla 7-option vector, so 7+ never commits. Only #item3 (a mod-owned dropdown) works. -> the NOP is useless and costs the label sync, so it went back OFF.
                                        //   The real fix for slots 0/1/2 mod items = replace item0/1/2 with mod-owned vectors like item3 (separate task).
const SETTER_NOP_ENABLED: bool = false;
unsafe fn nop_revert_setter() {
    if !SETTER_NOP_ENABLED {
        return;
    }
    match SETTER_NOPED.load(Ordering::Relaxed) {
        1 | 2 => return,
        _ => {}
    }
    let base = GetModuleHandleW(core::ptr::null()) as usize;
    if base == 0 {
        return;
    }
    let addr = base + SETTER_NOP_RVA;
    // Safety check: only patch after confirming it is a call rel32 (0xe8) (abort if the RVA is off).
    if !readable(addr, 5) || *(addr as *const u8) != 0xe8 {
        SETTER_NOPED.store(2, Ordering::Relaxed);

        return;
    }
    const RWX: u32 = 0x40;
    let mut old: u32 = 0;
    if VirtualProtect(addr, 5, RWX, &mut old) == 0 {
        SETTER_NOPED.store(2, Ordering::Relaxed);
        return;
    }
    for i in 0..5 {
        *((addr + i) as *mut u8) = 0x90;
    } // 5× NOP
    VirtualProtect(addr, 5, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), addr, 5);
    SETTER_NOPED.store(1, Ordering::Relaxed);
}
// Option label cache computed once per screen entry (avoids per-frame file I/O).
static OPTS_CACHE: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn compute_options() -> Vec<String> {
    let n = item_opt_count();
    (0..n).map(|i| item_opt_label(i as u8)).collect()
}

// * Mod-owned item dropdown ids. slot0/1/2 = item{N}m (overlaid on native item0/1/2; the click commits straight to +0x1788),
//   slot3 = item3 (the 4th). Native item0/1/2 commit clicks only into the model, so they cannot hold mod items (ghidra-re) -> replaced by mod-owned ones.
/// Node id of the mod-owned dropdown for slot `si` on the strategy screen.
///
/// `&'static str`, not `String`: the max-height re-apply below calls this for
/// every row and slot on *every frame* the screen is open, and formatting the
/// same four constants 20 times a frame was the whole cost. Mirrors
/// `uinj::CT_DD_IDS`, which is already a static array for the comp-test screen.
fn slot_dd_id(si: usize) -> &'static str {
    const IDS: [&str; 4] = ["item0m", "item1m", "item2m", "item3"];
    IDS[si.min(3)]
}

/// Row node ids, for the same reason — the per-frame loop below was building
/// these with `format!`.
const ROW_IDS: [&str; MAX_ROWS] = ["row0", "row1", "row2", "row3", "row4"];

// * Hide the native item0/1/2 dropdowns (mod-owned item0m/1m/2m replace them). Only while the personal-tactics screen is open.
//   With only an overlay, the native "leave it to the player" text showed through on the left and looked overlapped -> hide completely with visible=false.
//   The game may reset visible every frame, so re-apply on every post_update.
fn hide_native_item_dds(root: &mut Node) {
    if !find_node(root, "personal")
        .map(|n| n.visible)
        .unwrap_or(false)
    {
        return;
    } // do not interfere unless on personal tactics
    for ri in 0..MAX_ROWS {
        let rid = format!("row{}", ri);
        if let Some(row) = find_mut(root, &rid) {
            for si in 0..3u8 {
                if let Some(nat) = find_mut(row, &format!("item{}", si)) {
                    nat.visible = false;
                }
            }
        }
    }
}

// ===========================================================================
//  Personal tactics screen handling (every post_update)
// ===========================================================================
// ═══════════════════════════════════════════════════════════════════════════
//  Comp test (training.ui) personal tactics - designating the 4th slot item
//    Slots 1~3 (item0/1/2) = the game's own business (and the comp-test restriction-patch mod). We own only item3.
//    Store = SEL_BY_CHAMP (same as the strategy screen, keyed by champion) => a selection made here applies both to real
//    matches and to comp-test sims (buy injection is keyed by champion name, so no extra wiring is needed).
//    Rows = blue0..4 / red0..4 (10 players). Making the UI 4-wide is ui_inject::inject_training's job.
// ═══════════════════════════════════════════════════════════════════════════
const CT_ROWS: [&str; 10] = [
    "blue0", "blue1", "blue2", "blue3", "blue4", "red0", "red1", "red2", "red3", "red4",
];
static CT_OPEN: AtomicBool = AtomicBool::new(false);
static CT_INJECTED: AtomicBool = AtomicBool::new(false);
static CT_LAST: Mutex<[i64; 40]> = Mutex::new([-1; 40]); // 10 rows x 4 slots
static CT_CHAMPS: Mutex<Vec<String>> = Mutex::new(Vec::new()); // last champion observed per row (change detection)
                                                               // * Diagnostics: comp-test wiring, stage by stage (printed in buy_report)
                                                               // (CTD_CALL / CTD_BUILDS / CTD_VIS / CTD_ROW / CTD_CHAMP removed 2026-08-05:
                                                               //  write-only counters no diagnostic ever read, one of which cost a whole-tree
                                                               //  `find_node` per frame. `CTD_SET` is write-only too, but it is bumped once per
                                                               //  comp-test screen entry rather than per frame, so it costs nothing and is left
                                                               //  for whoever next debugs that injection.)
static CTD_SET: AtomicU64 = AtomicU64::new(0); // it4_slot3 option injection succeeded
                                               // * +0x240 measurement (07-21): the in-source comments contradict each other (L375 "render screen_x" vs L389 "hit-test, no effect")
                                               //   and it is unconfirmed whether y/w/h continue at +0x244/+0x248/+0x24c. Dump the region on a node whose coordinates we know to pin the layout.
                                               //   Hitbox updates are to be implemented only after checking this measurement (no guess-implementations).
                                               // Champion of a training row: the child id is #champion_icon (unlike #icon on the strategy screen) -> dedicated lookup.
fn ct_row_champ(row: &Node) -> Option<String> {
    let icon = find_node(row, "champion_icon")?;
    let src = unsafe { read_img_source(icon) }?;
    let a = src.find("champions/")? + "champions/".len();
    let rest = &src[a..];
    let end = rest.find('#').unwrap_or(rest.len());
    let champ = rest[..end].trim();
    if champ.is_empty() {
        None
    } else {
        Some(champ.to_string())
    }
}
// ** Forcing 4-slot comp-test coordinates (07-21) - runtime adjustment instead of rewriting the template (preserves other mods' nodes).
//   Vanilla item0/1/2 = x146/296/446, w140 (up to 586) -> shrunk and re-laid-out for 4 slots.
//   it4_slot3 (which we append) already carries x482/w104 in its fragment, but force it too in case the game resets it.
//   WARNING: only when MODE4 (4 slots). In 3-slot mode the vanilla coordinates are left alone.
//   The game may revert it every frame, so re-apply every frame just like force_blue_slot_spacing.
// * Runtime coordinate forcing dropped (07-21): we declare all 4 comp-test slots ourselves, so there is no reason to move the natives.
//   The old force_comptest_slot_layout() wrote the native 4-state boxes every frame, and that approach causes
//   (1) hitboxes not following, so clicks pass through, and (2) jitter from fighting the game's own recalculation (measured with comptest_unlock).
//   => use only the coordinates declared in the template, and merely hide the natives.
// Hide the comp-test native item0/1/2 - the mod-owned dropdowns take their place.
// * One-shot diagnostic (07-22 report "slightly overlapping in 4-slot mode"): dump the **actual** child dropdowns of a comp-test row
//   as id + authored x (+0x84) + visible. Confirms by coordinates whether leftover natives or another mod's injected nodes
//   are mixed in (= the reason only the 4th looked wider). Result = item_tactics_ctrow.txt.
// * OFF for release (2026-07-22): job done - the coordinates proved the overlap was comptest_unlock's duplicate ct_i* injection,
//   and it was resolved by setting ITEM_DD_ENABLED=false in that mod (the ct_i* nodes are gone in the re-check dump).
fn hide_comptest_native_dds(root: &mut Node) {
    for rid in CT_ROWS.iter() {
        if let Some(row) = find_mut(root, rid) {
            for si in 0..3u8 {
                if let Some(nat) = find_mut(row, &format!("item{}", si)) {
                    if nat.visible {
                        nat.visible = false;
                    }
                }
            }
        }
    }
}
// ═══════════════════════════════════════════════════════════════════════════
//  * In-match 4th slot icon - **direct node writing** approach (2026-07-30, no game code modification)
// ═══════════════════════════════════════════════════════════════════════════
//  Surgery on game code (frame extension + array relocation) failed with a freeze on match entry => approach changed.
//  The game's icon-filling contract, established by ghidra-re measurement (reproducible with zero game function calls):
//    (1) descend the node path `<side>.slotN.bg.icon` (the game splits on '.' then searches recursively, 0x19f170)
//    ② `Node.visible`(+0x260) 1/0
//    (3) on the ImageRunner (4 states, stride 208 = normal/hover/active/disabled),
//       `source` (+0) = **fixed to the shared item spritesheet path**, `rect_tag` (+0x18) = Some(tag within the sheet)
//    => items are distinguished by **rect_tag**, not by source (not the old set_img_src "path#tag" scheme).
//  Icon tag rule (measured against the entire bundled item_setting): vanilla index 0..29 -> `t{idx%5+1}_{idx/5}`.
//    Mod items (idx>=30) have no tag in this sheet, so **not even the game can render them** -> hidden.
//  WARNING stage 1 (current): to verify that display works at all we write a **fixed tag**. Real item mapping is deferred to
//    stage 2 because "which player is on screen" (view-model lookup) is still unsolved.
const SLOT3_ICON_ENABLED: bool = true; // set false on trouble = immediate return to the previous state (no icon)
                                       // ═══════════════════════════════════════════════════════════════════════════
                                       //  ** Stage 3 = **reading the view model (GameView) directly** (full RE confirmed 2026-07-30) - no game code patching
                                       // ═══════════════════════════════════════════════════════════════════════════
                                       //  Two abandoned approaches and why they failed (do not retry):
                                       //    (1) extending the game loop's upper bound (frame extension + array relocation) = freeze on match entry (failed even with all 84/84 sites applied).
                                       //    (2) champion name cache (champ -> icon cache from the buy hook) = **inherently contaminated**. My players exist simultaneously in the
                                       //      background pre-sim and in the on-screen match (the athlete+0x810 join is valid for both = canonical), so a build completed with 4 items in the
                                       //      background leaks onto the on-screen player (who owns 3). On top of that we assumed a single `blue_player` node (there is really one per lane = 5+5),
                                       //      so the first lane was written with someone else's values => the true identity of the user-reported "wrong items".
                                       //  CORRECT = the mod reads **exactly the same data** the game reads when drawing slot0~2:
                                       //    GameView (= App+0x4a50, constant for the whole process lifetime) -> player_view HashMap (key = (team, position))
                                       //    -> PlayerViewInfo.items: Vec<u64> (indices into item_list) -> item_list[idx] = (data, vtable) -> vtable+0x60 = icon()
                                       //  * items[3] already exists: the `cmp rbx,0x30` in the game's slot loop is not an item count limit but the
                                       //    **byte size of the hardcoded 3-element node-name array ("slotN")**, while the actual item iteration is guarded by
                                       //    `i < items.len()` (0xa6339f). There is no take(3)/min(3) anywhere along the view chain (apply_frame 0x952170 = capless collect).
const GV_OFF_ITEMLIST_CAP: usize = 0xa8; // -1 means None
const GV_OFF_ITEMLIST_PTR: usize = 0xb0;
const GV_OFF_ITEMLIST_LEN: usize = 0xb8;
const GV_OFF_PV_CTRL: usize = 0x1d0; // hashbrown RawTable ctrl
const GV_OFF_PV_MASK: usize = 0x1d8;
const GV_OFF_PV_ITEMS: usize = 0x1e8; // element count (0 = not in a match)
const PV_STRIDE: usize = 0x260; // PlayerViewInfo
const PV_OFF_TEAM: usize = 0x00; // u64 tag: 0=blue(Team0) 1=red(Team1)
const PV_OFF_POS: usize = 0x08; // u32: 0 top /1 jungle /2 mid /3 bottom /4 support
const PV_OFF_ITEMS_PTR: usize = 0x58; // Vec<u64> = {cap@0x50, ptr@0x58, len@0x60}
const PV_OFF_ITEMS_LEN: usize = 0x60;
const LANES: [&str; 5] = ["top", "jungle", "mid", "bottom", "support"];
// === slot3 tooltip = **reusing the game's `#item_tooltip` node** (2026-07-30) ===
//  The game's tooltip code walks only the 3 hardcoded paths `"<side>_player.item0/1/2"` and **never visits #slot3**
//  (setting focus does not catch it = plan A impossible; the emit is tightly bound to the mega-function's frame locals = not callable from outside = plan B impossible).
//  => But **the tooltip node itself already exists in `ingame.ui`** (`#item_tooltip`, visible:false, z on top) =>
//     if the mod fills that node's labels/icon and sets position + visible, the result looks **100% identical to the game's**.
//  Node structure (measured from the bundle): #item_tooltip(274x250) > #bg / #data > {#slot>#icon, #name, #tier, #price, #desc}
//  WARNING: on frames where the game shows its own tooltip (hovering slot0~2) we **do not touch it** - avoids an ownership race.
//    We only borrow it on frames the game does not use, and restore visible=false when our hover ends.
// * Re-enabled (2026-07-30): the crash causes were a **misunderstood vtable slot** (+0x50 assumed to be name and dereferenced; it is really a bool) and
//   calling the game's show function directly (mismatch against its 11-argument contract). Both were dropped in favour of **confirmed slots + filling the labels ourselves**.
//   Set false on trouble for an immediate revert (the icon keeps working).
// * Re-enabled (2026-07-30, after full RE): the crash cause = **arguments shifted by one slot** (p1 <- arg4; the correct one is arg5).
//   The empty-tooltip cause = wrong bundle path (`bundle_unpacked` - the game only has `_full`) + layout not refreshed.
//   => switched to calling the game's show function wholesale (content, size and position all handled by the game). Set false on trouble for an immediate revert.
// ** OFF for 0.5.4 (2026-08-04) **: RVA_TIP_SHOW could not be re-derived. Its body diverges ~100 bytes in,
//   so exe2exe leaves 6 candidates; the only one of a comparable size (0x1470450, 10591 vs 9912) has 8 callers
//   where 0.5.3 had 3. This function is called with ELEVEN arguments - a wrong target is a crash, not a
//   degradation - so the tooltip is disabled rather than guessed. The 4th-item ICON is unaffected
//   (`SLOT3_ICON_ENABLED`); only hovering it for a tooltip is lost. RVA_TIP_SHOW/RVA_TIP_MEASURE_VT below
//   are 0.5.3 values and are never reached while this is false.
const TOOLTIP_ENABLED: bool = false;
const LABEL_TEXT_OFF: usize = 352; // LabelRunner.text (ui_kit canonical, assign the whole String)
const NODE_OFF_FOCUS: usize = 0x262; // 1|2 = hover
const NODE_OFF_RECT: usize = 0x240; // x,y,w,h (f32 ×4)
static TIP_SHOWN: AtomicU64 = AtomicU64::new(0); // frames we displayed (diagnostic)
static TIP_OWNED: AtomicBool = AtomicBool::new(false); // are we currently borrowing it?
                                                       // * The game's tooltip show function = `game-view\src\ui\item_tooltip.rs` (RE-confirmed on 0.5.3).
                                                       //   Contract: (p1 = asset/i18n registry, p2 = text measurement ctx, p3 = its vtable (a constant), node = #item_tooltip,
                                                       //          item_data, item_vtable, x, y, pivot_x, pivot_y, clamp_rect{x,y,w,h})
                                                       //   * The item (data, vtable) is only **borrowed** (never dropped inside) => passing the item_list originals straight through is safe.
const RVA_TIP_SHOW: usize = 0x1ab52f0;
const RVA_TIP_MEASURE_VT: usize = 0x318b4c0; // p3 = vtable of the text measurement ctx (constant)
static TIP_P1: AtomicUsize = AtomicUsize::new(0);
static TIP_P2: AtomicUsize = AtomicUsize::new(0);
static TIP_ROOT: AtomicUsize = AtomicUsize::new(0);
// Node field read/write (all after VEH-protected range validation)
unsafe fn node_focus(n: &Node) -> u8 {
    let p = (n as *const Node as usize) + NODE_OFF_FOCUS;
    if readable(p, 1) {
        *(p as *const u8)
    } else {
        0
    }
}
unsafe fn node_rect(n: &Node) -> Option<(f32, f32, f32, f32)> {
    let p = (n as *const Node as usize) + NODE_OFF_RECT;
    if !readable(p, 16) {
        return None;
    }
    Some((
        *(p as *const f32),
        *((p + 4) as *const f32),
        *((p + 8) as *const f32),
        *((p + 12) as *const f32),
    ))
}
unsafe fn node_set_xy(n: &Node, x: f32, y: f32) {
    // Layout x/y = authored position (the game recomputes the rect itself every frame rather than the +0x84 family,
    // so for a node like the tooltip whose position the game does not touch, writing the rect directly works).
    let p = (n as *const Node as usize) + NODE_OFF_RECT;
    if writable(p, 8) {
        *(p as *mut f32) = x;
        *((p + 4) as *mut f32) = y;
    }
}
// item_list[idx] → (data, vtable)
// -- Item vtable access (fully RE-confirmed 2026-07-30) ---------------------------
//  OK  +0x58 key(&String) / +0x60 icon(&String) / +0x68 price(u64 **value**) / +0x70 tier(u64 **value**, 0-based)
//  NO  +0x50 = bool (self+0x190 != 0) - **not name**. Name has no vtable slot; the i18n key is assembled from key.
//     (Mistaking this for a String pointer and dereferencing it caused a crash. Do not repeat.)
const RVA_GAME_ALLOC: usize = 0x29bb920; // 0.5.4 (0.5.3 was 0x28f7df0). Same helper `ui_inject::ALLOC_RVA` pins - see the evidence there.  // (rcx = ignored, rdx = flags 0, r8 = size) -> ptr
unsafe fn item_obj_at(gv: usize, idx: u64) -> Option<(usize, usize)> {
    if !readable(gv + GV_OFF_ITEMLIST_CAP, 24) {
        return None;
    }
    if rd_u64(gv + GV_OFF_ITEMLIST_CAP) == u64::MAX {
        return None;
    }
    let ptr = rd_u64(gv + GV_OFF_ITEMLIST_PTR) as usize;
    let len = rd_u64(gv + GV_OFF_ITEMLIST_LEN);
    if idx >= len || ptr < 0x10000 {
        return None;
    }
    let e = ptr + (idx as usize) * 0x10;
    if !readable(e, 16) {
        return None;
    }
    let (d, v) = (rd_u64(e) as usize, rd_u64(e + 8) as usize);
    if d < 0x10000 || v < 0x10000 {
        None
    } else {
        Some((d, v))
    }
}
// GameView pointer (read-only capture). rcx of game.rs update (0x960df0) = GameView. The value never changes, so capturing once is enough.
static GAME_VIEW: AtomicUsize = AtomicUsize::new(0);
// 0.5.4 (2026-08-04): exe2exe `match`, 1 hit at 320 and 640 bytes, size 4575, 12-push prologue intact.
const RVA_GV_UPDATE: usize = 0xaa06c0;
const GV_UPDATE_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];
static GV_HOOK_INSTALLED: AtomicU64 = AtomicU64::new(0);
static GV_HITS: AtomicU64 = AtomicU64::new(0);
// WARNING minimal detour: this UI path fires every frame, so atomic stores only (no locks, allocation or file I/O).
unsafe extern "C" fn cap_game_view(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let gv = *saved as usize; // rcx = &mut GameView
    if gv >= 0x10000 && gv < 0x0000_8000_0000_0000 {
        GAME_VIEW.store(gv, Ordering::Relaxed);
        GV_HITS.fetch_add(1, Ordering::Relaxed);
    }
    // * Capturing the arguments of the game's tooltip show function (full RE confirmed 2026-07-30 - the previous indices were **shifted by one** and crashed)
    //   Call chain: 0x960df0 (game.rs update) -> 0xa5c1e0 (ingame_ui) -> 0x1ab52f0 (tooltip show)
    //   The mega-function passes its own arg1/arg2 as the tooltip's p1/p2, and uses arg4 as the node search root.
    //   And the values the 0x960df0 call site passes = rcx <- [rbp+0x140] = entry rsp+0x28 = **arg5**, rdx <- [rbp+0x148] = **arg6**, r9 <- **arg4**.
    //   ⟹ p1 = arg5 / p2 = arg6 / root = arg4(r9).
    //   NO old implementation: p1 <- r9 (arg4), p2 <- arg5, root <- arg7 => it passed **the UI root node as the registry** and died instantly in the hash lookup.
    //   Stub layout: push r12,rsi,rdi,rbx,r11,r10,r9,r8,rdx,rcx -> r9 = saved+3, entry rsp = saved+10.
    let root = *saved.add(3) as usize; // arg4 (r9)
    let sp = saved.add(10) as usize; // entry rsp
    let p1 = safe_read_u64(sp + 0x28).unwrap_or(0) as usize; // arg5 = asset/settings registry
    let p2 = safe_read_u64(sp + 0x30).unwrap_or(0) as usize; // arg6 = text measurement ctx
    if p1 >= 0x10000 {
        TIP_P1.store(p1, Ordering::Relaxed);
    }
    if p2 >= 0x10000 {
        TIP_P2.store(p2, Ordering::Relaxed);
    }
    if root >= 0x10000 {
        TIP_ROOT.store(root, Ordering::Relaxed);
    }
    0
}
fn install_game_view_hook() {
    let state = GV_HOOK_INSTALLED.load(Ordering::Relaxed);
    if state == 1 {
        return;
    }
    // See `install_retry_due`: a wrong RVA here would otherwise cost a loader
    // lock and an address-space lock on every frame for the whole session.
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if state == 2 && !install_retry_due(&RETRY) {
        return;
    }
    let r = unsafe {
        install_detour_generic(
            RVA_GV_UPDATE,
            12,
            cap_game_view as usize,
            &GV_UPDATE_PROLOGUE,
        )
    };
    GV_HOOK_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
}
// Icon string of item_list[idx] (vtable +0x60 = icon()). Same path as the game's set_item_icon (0x97b540).
//   WARNING: this is a shadow-call, so the code_ptr_ok guard + range validation of the returned String are mandatory.
unsafe fn item_icon_by_index(gv: usize, idx: u64) -> Option<String> {
    if !readable(gv + GV_OFF_ITEMLIST_CAP, 24) {
        return None;
    }
    if rd_u64(gv + GV_OFF_ITEMLIST_CAP) == u64::MAX {
        return None;
    } // None sentinel
    let ptr = rd_u64(gv + GV_OFF_ITEMLIST_PTR) as usize;
    let len = rd_u64(gv + GV_OFF_ITEMLIST_LEN);
    if idx >= len || ptr < 0x10000 {
        return None;
    }
    let e = ptr + (idx as usize) * 0x10;
    if !readable(e, 16) {
        return None;
    }
    let data = rd_u64(e) as usize;
    let vt = rd_u64(e + 8) as usize;
    if data < 0x10000 || vt < 0x10000 || !readable(vt + 0x60, 8) {
        return None;
    }
    let f = rd_u64(vt + 0x60) as usize;
    if !code_ptr_ok(f) {
        return None;
    }
    let g: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(f);
    let s = g(data);
    if s < 0x10000 || !readable(s, 0x18) {
        return None;
    }
    let sp = rd_u64(s + 8) as usize; // String = {cap@0, ptr@8, len@0x10}
    let sl = rd_u64(s + 0x10) as usize;
    if sp < 0x10000 || sl == 0 || sl > 64 || !readable(sp, sl) {
        return None;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(sp as *const u8, sl)).into_owned())
}
// Walk the whole player_view hash map -> (team, position) -> items[3] icon. No hashing needed (linear bucket scan).
//   hashbrown: a ctrl byte with the top bit clear = FULL, and entries run **backwards** from ctrl (ctrl - (i+1)*stride).
unsafe fn collect_slot3_icons(gv: usize) -> HashMap<(u64, u32), String> {
    let mut out = HashMap::new();
    if !readable(gv + GV_OFF_PV_CTRL, 32) {
        return out;
    }
    let ctrl = rd_u64(gv + GV_OFF_PV_CTRL) as usize;
    let mask = rd_u64(gv + GV_OFF_PV_MASK) as usize;
    let nitems = rd_u64(gv + GV_OFF_PV_ITEMS);
    if ctrl < 0x10000 || nitems == 0 || nitems > 64 || mask > 0x1000 {
        return out;
    }
    for i in 0..=mask {
        if !readable(ctrl + i, 1) {
            break;
        }
        if *((ctrl + i) as *const u8) & 0x80 != 0 {
            continue;
        } // not FULL
        let e = ctrl.wrapping_sub((i + 1) * PV_STRIDE);
        if e < 0x10000 || !readable(e, PV_STRIDE) {
            continue;
        }
        let team = rd_u64(e + PV_OFF_TEAM);
        let pos = (rd_u64(e + PV_OFF_POS) & 0xffff_ffff) as u32;
        if team > 1 || pos > 4 {
            continue;
        }
        let it_ptr = rd_u64(e + PV_OFF_ITEMS_PTR) as usize;
        let it_len = rd_u64(e + PV_OFF_ITEMS_LEN);
        if it_len < 4 || it_ptr < 0x10000 || !readable(it_ptr + 3 * 8, 8) {
            continue;
        } // does not own a 4th
        let idx = rd_u64(it_ptr + 3 * 8);
        if let Some(tag) = item_icon_by_index(gv, idx) {
            out.insert((team, pos), tag);
        }
    }
    out
}
const ICON_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";
const IMG_STATE_OFF: [usize; 4] = [0, 208, 416, 624]; // normal/hover/active/disabled
const IMG_OFF_SOURCE: usize = 0;
const IMG_OFF_RECT_TAG: usize = 24;
const NODE_OFF_VISIBLE: usize = 0x260;
static SLOT3_ICON_N: AtomicU64 = AtomicU64::new(0); // nodes successfully set (cumulative)
static SLOT3_ICON_MISS: AtomicU64 = AtomicU64::new(0); // skipped due to a node/runner mismatch
                                                       // Write sheet + tag into all 4 states of the ImageRunner data area. Assign the whole String (never write partial fields -
                                                       //   the old set_img_src wrote {len@0, ptr@8}, corrupting cap, which was a latent bug that HeapFree'd a static ptr at
                                                       //   teardown. The real layout is {cap@0, ptr@8, len@0x10}). Both game and mod use the process heap (GetProcessHeap),
                                                       //   so it is safe for the game to drop a String the mod created.
unsafe fn set_icon_rect_tag(n: &Node, tag: &str) -> bool {
    if !n.runner.type_name().contains("ImageRunner") {
        return false;
    }
    let base = runner_base(n);
    if base < 0x10000 || !readable(base, 848) {
        return false;
    }
    for st in IMG_STATE_OFF {
        let sp = base + st + IMG_OFF_SOURCE;
        let tp = base + st + IMG_OFF_RECT_TAG;
        if !writable(sp, 24) || !writable(tp, 24) {
            return false;
        }
        *(sp as *mut String) = ICON_SHEET.to_string();
        *(tp as *mut Option<String>) = Some(tag.to_string());
    }
    true
}
unsafe fn node_set_visible(n: &Node, v: bool) {
    let p = (n as *const Node as usize) + NODE_OFF_VISIBLE;
    if writable(p, 1) {
        *(p as *mut u8) = if v { 1 } else { 0 };
    }
}
// Read the current rect_tag and skip rewriting if it is the same value (avoids a String alloc per frame).
unsafe fn icon_tag_is(n: &Node, tag: &str) -> bool {
    let base = runner_base(n);
    if base < 0x10000 || !readable(base + IMG_OFF_RECT_TAG, 24) {
        return false;
    }
    // Option<String> niche optimization: ptr == 0 means None
    let ptr = rd_u64(base + IMG_OFF_RECT_TAG + 8) as usize;
    let len = rd_u64(base + IMG_OFF_RECT_TAG + 0x10) as usize;
    if ptr < 0x10000 || len != tag.len() || !readable(ptr, len) {
        return false;
    }
    std::slice::from_raw_parts(ptr as *const u8, len) == tag.as_bytes()
}
// * Stage 2 preparation diagnostic: dump "player identification clues + the real slot0~2 tags" from the in-match player_info subtree once.
//   Purpose = decide how to find the 4th item (matching a node's name/champion label against the athlete the mod knows).
//   Reversing the slot0~2 tags (t{a}_{b} -> idx = b*5 + (a-1)) reveals that player's items[0..2].
unsafe fn read_icon_tag(n: &Node) -> Option<String> {
    if !n.runner.type_name().contains("ImageRunner") {
        return None;
    }
    let base = runner_base(n);
    if base < 0x10000 || !readable(base + IMG_OFF_RECT_TAG, 24) {
        return None;
    }
    let ptr = rd_u64(base + IMG_OFF_RECT_TAG + 8) as usize;
    let len = rd_u64(base + IMG_OFF_RECT_TAG + 0x10) as usize;
    if ptr < 0x10000 || len == 0 || len > 64 || !readable(ptr, len) {
        return None;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len)).into_owned())
}
// Tag -> catalog index (t{a}_{b} -> b*5 + (a-1))
fn tag_to_idx(t: &str) -> Option<usize> {
    let rest = t.strip_prefix('t')?;
    let (a, b) = rest.split_once('_')?;
    let a: usize = a.parse().ok()?;
    let b: usize = b.parse().ok()?;
    if a == 0 || a > 5 {
        return None;
    }
    Some(b * 5 + (a - 1))
}
static SLOT3_PV_N: AtomicU64 = AtomicU64::new(0); // number of players seen owning a 4th item in the view model
fn handle_ingame_slot3(ui: &Node) {
    if !SLOT3_ICON_ENABLED || slot_count() != 4 {
        return;
    }
    let gv = GAME_VIEW.load(Ordering::Relaxed);
    if gv < 0x10000 {
        return;
    } // not captured yet (before entering the match screen)
      // * Read exactly the data the game reads when drawing slot0~2 = no cache, no champion matching, no is_live needed.
    let icons = unsafe { collect_slot3_icons(gv) };
    SLOT3_PV_N.store(icons.len() as u64, Ordering::Relaxed);
    // * Node path = player_info.<lane>.{blue_player|red_player}.slot3.bg.icon (normal)
    //             + wide_data.player_info.<lane>....                        (wide)
    //   WARNING: blue_player/red_player exist **once per lane (5+5)**, and counting both layouts that is up to 20.
    //     The old code handling only the **first match** via find_node(root,"blue_player") was the real cause of the wrong display.
    let roots: [Option<&Node>; 2] = [
        find_node(ui, "player_info"),
        find_node(ui, "wide_data").and_then(|w| find_node(w, "player_info")),
    ];
    let mut hover: Option<(u64, u32, f32, f32, f32, f32)> = None; // (team,pos,x,y,w,h)
    for root in roots.iter().flatten() {
        for (pos, lane) in LANES.iter().enumerate() {
            let Some(ln) = find_node(root, lane) else {
                continue;
            };
            for (team, side) in ["blue_player", "red_player"].iter().enumerate() {
                let Some(sp) = find_node(ln, side) else {
                    continue;
                };
                let Some(slot3) = find_node(sp, "slot3") else {
                    continue;
                };
                let Some(bg) = find_node(slot3, "bg") else {
                    continue;
                };
                let Some(icon) = find_node(bg, "icon") else {
                    SLOT3_ICON_MISS.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                let tag = icons.get(&(team as u64, pos as u32));
                unsafe {
                    match tag {
                        // No 4th item = behave like the game's empty slot handling (visible=false only; image fields untouched)
                        None => node_set_visible(icon, false),
                        Some(t) => {
                            // * Hover detection: focus of the slot node (or bg) is in {1,2}. The game's hit-test sets it.
                            if TOOLTIP_ENABLED
                                && hover.is_none()
                                && (node_focus(slot3) == 1
                                    || node_focus(slot3) == 2
                                    || node_focus(bg) == 1
                                    || node_focus(bg) == 2)
                            {
                                if let Some((x, y, w, h)) = node_rect(slot3) {
                                    hover = Some((team as u64, pos as u32, x, y, w, h));
                                }
                            }
                            if icon_tag_is(icon, t) {
                                node_set_visible(icon, true);
                                continue;
                            } // same value = skip the rewrite
                            if set_icon_rect_tag(icon, t) {
                                node_set_visible(icon, true);
                                node_set_visible(slot3, true);
                                SLOT3_ICON_N.fetch_add(1, Ordering::Relaxed);
                            } else {
                                SLOT3_ICON_MISS.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        }
    }
    if TOOLTIP_ENABLED {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            drive_slot3_tooltip(ui, gv, hover);
        }));
    }
}

// * Borrow the game's `#item_tooltip` node to show the slot3 tooltip (the mod draws nothing new = 100% identical appearance).
//   WARNING ownership rule: on frames where the game uses its own tooltip (hovering slot0~2 -> the game sets visible=true)
//     we never touch it. We borrow it **only on frames the game does not use**, and restore it when our hover ends.
unsafe fn drive_slot3_tooltip(ui: &Node, gv: usize, hover: Option<(u64, u32, f32, f32, f32, f32)>) {
    let Some(tip) = find_node(ui, "item_tooltip") else {
        return;
    };
    let Some((team, pos, sx, sy, sw, sh)) = hover else {
        // Our hover ended -> take down only what we raised (never touch what the game raised)
        if TIP_OWNED.swap(false, Ordering::Relaxed) {
            node_set_visible(tip, false);
        }
        return;
    };
    // NO old bug (2026-07-30 user report "the tooltip of the last hovered item stays up"):
    //   with `if !TIP_OWNED && tip.visible { return; }` as the yield rule, on a frame where the game had finished a slot0~2 hover but
    //   **had not taken the tooltip down yet** (it does so the next frame), moving the mouse to slot3 saw the leftover
    //   visible=true tooltip and returned -> **the previous item's tooltip stayed on screen with no content filled in**.
    //   => the yield decision was changed from "before filling content" to **"is the game actually hovering one of its own slots this frame"**.
    //   If any of slot0~2 is hovered it is the game's turn and we keep our hands off.
    if game_hovering_own_slot(ui) {
        if TIP_OWNED.swap(false, Ordering::Relaxed) { /* if it was ours, let it go (the game overwrites it) */
        }
        return;
    }
    // That player's items[3] -> the item object
    let Some(pv) = find_player_view(gv, team, pos) else {
        return;
    };
    let it_ptr = rd_u64(pv + PV_OFF_ITEMS_PTR) as usize;
    let it_len = rd_u64(pv + PV_OFF_ITEMS_LEN);
    if it_len < 4 || it_ptr < 0x10000 || !readable(it_ptr + 24, 8) {
        return;
    }
    let Some((data, vt)) = item_obj_at(gv, rd_u64(it_ptr + 3 * 8)) else {
        return;
    };
    // ** Call the game's own tooltip show function (contract confirmed by full RE 2026-07-30).
    //   Name, tier, price, stats, effect text, i18n, size, position and clamping are **all handled by the game** => mod items come out right automatically.
    //   NO three abandoned attempts (do not retry):
    //     (1) mistaking vtable +0x50 for name(&String) and dereferencing it -> crash (it is really a bool).
    //     (2) calling the same function but with **arguments shifted by one** (p1 <- arg4) -> instant death. Correct is p1=arg5 / p2=arg6 / root=arg4.
    //     (3) writing the labels directly + parsing the bundle files -> blank text (the path used `bundle_unpacked` while the game only has `_full`)
    //       + size/position not refreshed (the game writes the 4 authored blocks together with the rect).
    let (p1, p2) = (
        TIP_P1.load(Ordering::Relaxed),
        TIP_P2.load(Ordering::Relaxed),
    );
    if p1 < 0x10000 || p2 < 0x10000 {
        return;
    }
    // WARNING precondition: all 8 children must exist (if even one is missing the game panics on unwrap -> abort).
    let Some(d) = find_node(tip, "data") else {
        return;
    };
    let ok = find_node(tip, "bg").is_some()
        && find_node(d, "name").is_some()
        && find_node(d, "tier").is_some()
        && find_node(d, "price").is_some()
        && find_node(d, "desc").is_some()
        && find_node(d, "bar").is_some()
        && find_node(d, "slot")
            .and_then(|s| find_node(s, "icon"))
            .is_some();
    if !ok {
        return;
    }
    let base = exe_base_addr();
    if base == 0 {
        return;
    }
    let f = base + RVA_TIP_SHOW;
    if !code_ptr_ok(f) {
        return;
    }
    // Anchor (game rule): blue = right-aligned to the slot, 12px below / red = left of the slot, 12px above.
    //   authored w/h = tip+0x74 / tip+0x7c.
    let tn = tip as *const Node as usize;
    let aw = if readable(tn + 0x74, 4) {
        *((tn + 0x74) as *const f32)
    } else {
        274.0
    };
    let ah = if readable(tn + 0x7c, 4) {
        *((tn + 0x7c) as *const f32)
    } else {
        250.0
    };
    let (ax, ay) = if team == 0 {
        (sx + sw - aw, sy + sh + 12.0)
    } else {
        (sx, sy - ah - 12.0)
    };
    let clamp: [f32; 4] = [0.0, 0.0, 1920.0, 1080.0];
    type TipShow = unsafe extern "win64" fn(
        usize,
        usize,
        usize,
        usize, // p1, p2, p3 (measurement vtable constant), node
        usize,
        usize, // item_data, item_vtable  (borrowed only - never dropped)
        f32,
        f32,
        f32,
        f32, // x, y, pivot_x, pivot_y
        *const [f32; 4],
    ); // clamp rect
    let g: TipShow = core::mem::transmute(f);
    g(
        p1,
        p2,
        base + RVA_TIP_MEASURE_VT,
        tn,
        data,
        vt,
        ax,
        ay,
        0.0,
        0.0,
        &clamp,
    );
    // visible=1 is set by the function itself.
    TIP_OWNED.store(true, Ordering::Relaxed);
    TIP_SHOWN.fetch_add(1, Ordering::Relaxed);
}
// Is the game hovering one of its own slots (slot0~2) this frame? If so the tooltip is the game's turn.
//   * Do not decide this from the tooltip node's visible - the game does not take it down on the same frame the hover ends,
//     which produced the "previous item's tooltip stays visible" bug (see the drive_slot3_tooltip comment above).
unsafe fn game_hovering_own_slot(ui: &Node) -> bool {
    let roots: [Option<&Node>; 2] = [
        find_node(ui, "player_info"),
        find_node(ui, "wide_data").and_then(|w| find_node(w, "player_info")),
    ];
    for root in roots.iter().flatten() {
        for lane in LANES.iter() {
            let Some(ln) = find_node(root, lane) else {
                continue;
            };
            for side in ["blue_player", "red_player"].iter() {
                let Some(sp) = find_node(ln, side) else {
                    continue;
                };
                for k in 0..3 {
                    let Some(sl) = find_node(sp, &format!("slot{}", k)) else {
                        continue;
                    };
                    if node_focus(sl) == 1 || node_focus(sl) == 2 {
                        return true;
                    }
                    if let Some(bg) = find_node(sl, "bg") {
                        if node_focus(bg) == 1 || node_focus(bg) == 2 {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
// Find the address of the (team,pos) entry in the player_view hash map (linear bucket scan).
unsafe fn find_player_view(gv: usize, team: u64, pos: u32) -> Option<usize> {
    if !readable(gv + GV_OFF_PV_CTRL, 32) {
        return None;
    }
    let ctrl = rd_u64(gv + GV_OFF_PV_CTRL) as usize;
    let mask = rd_u64(gv + GV_OFF_PV_MASK) as usize;
    if ctrl < 0x10000 || mask > 0x1000 {
        return None;
    }
    for i in 0..=mask {
        if !readable(ctrl + i, 1) {
            break;
        }
        if *((ctrl + i) as *const u8) & 0x80 != 0 {
            continue;
        }
        let e = ctrl.wrapping_sub((i + 1) * PV_STRIDE);
        if e < 0x10000 || !readable(e, PV_STRIDE) {
            continue;
        }
        if rd_u64(e + PV_OFF_TEAM) == team && (rd_u64(e + PV_OFF_POS) & 0xffff_ffff) as u32 == pos {
            return Some(e);
        }
    }
    None
}

fn handle_comptest_screen(ui: &Node) {
    // * 07-21 switch: we now fully manage comp-test personal tactics in both 3-slot and 4-slot mode (user decision).
    //   In 3-slot mode ui_inject declares 3 at the vanilla coordinates (146/296/446, w140), in 4-slot mode 4 at the
    //   compressed ones (146/258/370/482, w104), and this code wires up the options/selection. The natives are hidden.
    // The `CTD_*` counters that used to be bumped here are gone. Nothing ever
    // read them — no diagnostic dump mentions them — and one of them cost a
    // `find_node(ui, "blue0")`, a full depth-first walk of the entire UI tree,
    // on every frame the game was running, purely to increment a number no code
    // could observe. `CTD_CHAMP` below went with them.
    let bnode = find_node(ui, "builds");
    // Detecting that the personal tactics tab is active: visibility of the row container (#builds).
    let active = bnode.map(|n| n.visible).unwrap_or(false);
    if !active {
        if CT_OPEN.swap(false, Ordering::Relaxed) {
            CT_INJECTED.store(false, Ordering::Relaxed);
        }
        return;
    }
    CT_OPEN.store(true, Ordering::Relaxed);
    // * Option labels are computed once per screen (file I/O cache).
    if !CT_INJECTED.swap(true, Ordering::Relaxed) {
        let o = compute_options();
        *OPTS_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = o;
    }
    let opts = OPTS_CACHE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if opts.is_empty() {
        return;
    }
    let refs: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
    // ** Timing (measured 07-21): on the first frame after entering the screen the game has not filled the champion in yet (champion_icon=None).
    //   With a "inject once on entry" approach everything is laid down as Auto and, with no champion readable, nothing is saved.
    //   -> switched to "re-seed a row whenever its observed champion changes" (covers both first appearance and swaps).
    let mut champs = CT_CHAMPS.lock().unwrap_or_else(|e| e.into_inner());
    if champs.len() < 10 {
        champs.resize(10, String::new());
    }
    let mut last = CT_LAST.lock().unwrap_or_else(|e| e.into_inner());
    let mut changed = false;
    // * Collect the per-side champion composition and publish it to the buy detour (ct_scope_for decides the side from it).
    let mut ct_blue: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ct_red: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (ri, rid) in CT_ROWS.iter().enumerate() {
        let Some(row) = find_node(ui, rid) else {
            continue;
        };
        let Some(c) = ct_row_champ(row) else {
            champs[ri].clear();
            continue;
        }; // no champion placed = not our business
           // * Rows 0~4 = blue / 5~9 = red (the order CT_ROWS is defined in). Selections are read and written with this scope =>
           //   the same champion on both sides is designated independently (previously they merged into one).
        let scope = if ri < 5 { Scope::CtBlue } else { Scope::CtRed };
        if ri < 5 {
            ct_blue.insert(c.clone());
        } else {
            ct_red.insert(c.clone());
        }
        let ns = slot_count();
        if champs[ri] != c {
            // Champion first appeared or was swapped -> re-seed all slots from that champion's stored values. SEL=0 (delegate) shows as Auto.
            //   Lookup is scope-first -> if absent, fall back to the plain (personal tactics) selection = inherit the existing designation.
            let mut ok = 0;
            for si in 0..ns {
                let sel = sel_get(scope, &c, si as u8);
                let sel = (sel as usize).min(opts.len().saturating_sub(1)) as u64;
                if unsafe { nat_dd_set_options(row, uinj::CT_DD_IDS[si], &refs, sel) } {
                    unsafe {
                        set_dd_max_height(row, uinj::CT_DD_IDS[si], MAX_ITEMS_HEIGHT);
                    }
                    last[ri * ITEM_SLOTS + si] = sel as i64;
                    ok += 1;
                }
            }
            if ok > 0 {
                champs[ri] = c;
                CTD_SET.fetch_add(ok, Ordering::Relaxed);
            }
            continue; // skip polling on a re-seeded frame (so our own write is not mistaken for a user selection)
        }
        // Poll the selection -> update SEL_BY_CHAMP (all slots)
        for si in 0..ns {
            if let Some(cur) = unsafe { nat_dd_selected(row, uinj::CT_DD_IDS[si]) } {
                let k = ri * ITEM_SLOTS + si;
                if cur as i64 != last[k] {
                    last[k] = cur as i64;
                    // * Store under the scoped key => applies only to this side and never leaks into normal matches.
                    //   When set back to Auto (0): a value left under the plain key would resurrect via fallback, so
                    //   record an **explicit Auto (SEL_AUTO)** to cut the fallback (the plain key is left untouched).
                    with_sel(|m| {
                        let k = (scoped_key(scope, &c), si as u8);
                        if cur == 0 {
                            if m.contains_key(&(c.clone(), si as u8)) {
                                m.insert(k, SEL_AUTO);
                            } else {
                                m.remove(&k);
                            }
                        } else {
                            m.insert(k, cur as u8);
                        }
                    });
                    SEL_DIRTY.store(true, Ordering::Relaxed);
                    changed = true;
                    let label = opts
                        .get(cur)
                        .cloned()
                        .unwrap_or_else(|| format!("idx{}", cur));
                    let sidetag = if scope == Scope::CtBlue {
                        "blue"
                    } else {
                        "red"
                    };
                }
            }
        }
    }
    publish_ct_roster(ct_blue, ct_red);
    if changed {
        drop(last);
        drop(champs);
        with_sel(|m| save_sel(m));
        update_override_snapshot();
    }
}
fn handle_tactics_screen(ui: &Node) {
    let personal = find_node(ui, "personal");
    let active = personal.map(|n| n.visible).unwrap_or(false);

    if !active {
        if SCREEN_OPEN.swap(false, Ordering::Relaxed) {
            OPTS_INJECTED.store(false, Ordering::Relaxed);
        }
        return;
    }
    SCREEN_OPEN.store(true, Ordering::Relaxed);

    // * NOP the personal_tactics -> dropdown revert (setter) once, so mod item (7+) selections persist.
    unsafe {
        nop_revert_setter();
    }
    // * Register the VEH (shared by safe_read/safe_write) - idempotent, once.
    seh_install();

    // * Re-apply max_items_height every frame (guarantees timing independence) + a one-shot diagnostic (verify the write took + dump the surroundings).
    for rid in ROW_IDS {
        let Some(row) = find_node(ui, rid) else {
            continue;
        };
        for si in 0..slot_count() {
            unsafe {
                set_dd_max_height(row, slot_dd_id(si), MAX_ITEMS_HEIGHT);
            }
        }
    }

    // Option injection (once per screen entry). Option labels are computed once on entry (file I/O cache).
    //   Initial display per slot = SEL_BY_CHAMP[(that row's champion, slot)] (champion-keyed, persisted). Absent -> 0 (auto).
    if !OPTS_INJECTED.swap(true, Ordering::Relaxed) {
        // * Recommendations must be applied before options are computed so the SEL lookups below see the new values (shown on the same frame).
        //   OPTS_INJECTED is reset whenever the screen closes, so the hash is re-checked on every re-entry.
        apply_recommendations();
        let opts = compute_options();
        *OPTS_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = opts.clone();
        let refs: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
        let mut last = LAST_SEL.lock().unwrap_or_else(|e| e.into_inner());
        let pt_sz = PT_SNAPSHOT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|m| m.len())
            .unwrap_or(0);
        let mut diag = format!(
            "[{}ms] option injection (row -> champion mapping) PT_SNAPSHOT={} opts={}
",
            now_ms(),
            pt_sz,
            opts.len()
        );
        for ri in 0..MAX_ROWS {
            let Some(row) = find_node(ui, &format!("row{}", ri)) else {
                continue;
            };
            let champ = row_champ(row);
            diag.push_str(&format!("  row{} champ={:?}\n", ri, champ));
            for si in 0..slot_count() {
                let iid = slot_dd_id(si);
                // Display priority: user selection (SEL_BY_CHAMP) > game personal_tactics (PT_SNAPSHOT vanilla) > Auto.
                //   -> even the vanilla display our NOP broke is restored exactly from personal_tactics.
                let (sel_v, pt_v) = if let Some(c) = champ.as_ref() {
                    (
                        with_sel(|m| m.get(&(c.clone(), si as u8)).copied()),
                        PT_SNAPSHOT
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .as_ref()
                            .and_then(|m| m.get(c))
                            .and_then(|b| b.get(si).copied()),
                    )
                } else {
                    (None, None)
                };
                // * SEL=0 (delegate/spurious) is not an override -> show the delegate (PT) value. Only SEL>=1 wins as a user pick.
                //   SEL_AUTO (= comp-test-only explicit Auto) cannot appear under a plain key, but exclude it defensively.
                let cur = sel_v
                    .filter(|&v| v >= 1 && v != SEL_AUTO)
                    .or(pt_v)
                    .unwrap_or(0);
                diag.push_str(&format!(
                    "    slot{}: SEL={:?} PT={:?} → cur={}\n",
                    si, sel_v, pt_v, cur
                ));
                let cur = (cur as usize).min(opts.len().saturating_sub(1)) as u64;
                if unsafe { nat_dd_set_options(row, iid, &refs, cur) } {
                    last[ri * ITEM_SLOTS + si] = cur as i64;
                    unsafe {
                        set_dd_max_height(row, iid, MAX_ITEMS_HEIGHT);
                    }
                }
            }
        }
        update_override_snapshot(); // refresh the injection snapshot on screen entry
        log_override();
    } else {
        // Poll the selection: update SEL_BY_CHAMP (champion-keyed) + persist + log, only for changed slots.
        let opts = OPTS_CACHE.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mut last = LAST_SEL.lock().unwrap_or_else(|e| e.into_inner());
        let mut changed = false;
        for ri in 0..MAX_ROWS {
            let Some(row) = find_node(ui, &format!("row{}", ri)) else {
                continue;
            };
            let Some(champ) = row_champ(row) else {
                continue;
            };
            for si in 0..slot_count() {
                if let Some(cur) = unsafe { nat_dd_selected(row, slot_dd_id(si)) } {
                    let k = ri * ITEM_SLOTS + si;
                    if cur as i64 != last[k] {
                        last[k] = cur as i64;
                        // cur 0 (delegate) = remove the entry -> fall back to delegate. >=1 = store the user override.
                        with_sel(|m| {
                            if cur == 0 {
                                m.remove(&(champ.clone(), si as u8));
                            } else {
                                m.insert((champ.clone(), si as u8), cur as u8);
                            }
                        });
                        SEL_DIRTY.store(true, Ordering::Relaxed); // * invalidate the designated-champion snapshot (rebuilt on the next buy)
                        changed = true;
                        let label = opts
                            .get(cur)
                            .cloned()
                            .unwrap_or_else(|| format!("idx{}", cur));
                        let modtag = if cur >= 7 {
                            mod_final_opts()
                                .get(cur - 7)
                                .map(|(id, k)| format!(" [mod item id={} {}]", id, k))
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                    }
                }
            }
        }
        if changed {
            with_sel(|m| save_sel(m));
            update_override_snapshot();
            log_override();
        }
    }
}
// Log of the current OVERRIDE (injection target) map - for verification.
fn log_override() {
    let map = build_override_map();
    let mut s = format!(
        "[{}ms] OVERRIDE (champ,slot) -> mod_id  ({} entries)
",
        now_ms(),
        map.len()
    );
    let mut v: Vec<_> = map.iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    for ((c, slot), id) in v {
        s.push_str(&format!("  {} slot{} → id {}\n", c, slot, id));
    }
}

// (champion key, slot) -> injected value. Consumed by the c6 detour.
//   * 2026-07-04 extension: vanilla picks are included too (there were signs that the game's dropdown -> personal_tactics commit
//   does not happen in a modded environment -> the mod forces every pick directly).
//   Value encoding: 0 = delegate (Auto), 1~6 = vanilla category (force the tactics byte -> the game's jump table handles it),
//              30+ = mod item game ID (write into the build buffer + zero the tactics byte).
fn build_override_map() -> HashMap<(String, u8), u64> {
    let finals = mod_final_opts(); // (id, key), order = option idx-7
    let mut out = HashMap::new();
    // -- (1) Merge the delegate ("tfm2.gg auto item selection") baseline ------------------------
    //   Lay down the category directions (1~6) that tfm2_meta_item_delegate wrote into
    //   champion_personal_tactics (Team+0x348) onto slots 0/1/2. PT_SNAPSHOT = the latest capture of that map.
    //   The sim c6c430 reads Team+0x348 directly (ghidra-re case a), but we also fold it into the c6 injection so it
    //   applies regardless of server-side Team copies or timing. Values 1~6 -> c6 converts them to the game's
    //   representative items (VANILLA_FINAL {4,24,9,14,19,29}) = bit-identical to the game's jump table (idempotent, harmless).
    //   WARNING: delegate covers only slots 0~2 (3 slots). slot3 (the 4th) stays on compute_auto_4th_id.
    if let Some(snap) = PT_SNAPSHOT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        for (champ, bytes) in snap.iter() {
            for slot in 0u8..3 {
                let b = bytes[slot as usize];
                if (1..=6).contains(&b) {
                    out.insert((champ.clone(), slot), b as u64);
                }
            }
        }
    }
    // -- (2) The user selection (SEL_BY_CHAMP) overrides the delegate baseline -------------------
    //   idx 0 (delegate) = no explicit override -> fall back to delegate/auto (prevents a spurious polled 0 from clobbering).
    with_sel(|m| {
        for ((champ, slot), &idx) in m.iter() {
            // * Comp-test side-scoped keys (@b:/@r:) are not targets of this map (2026-07-30):
            //   this override (the c6 / personal_tactics path) is per-champion and cannot express a side, and
            //   comp-test injection is handled scope-aware by the buy path. Adding them would only pile up entries for nonexistent champions.
            if is_scoped(champ) {
                continue;
            }
            if idx >= 7 {
                if let Some((id, _)) = finals.get(idx as usize - 7) {
                    out.insert((champ.clone(), *slot), *id); // mod item game ID (30+)
                }
            } else if idx >= 1 {
                out.insert((champ.clone(), *slot), idx as u64); // 1~6 = vanilla category
            }
            // idx 0 (delegate/Auto) = the user did not explicitly override this slot -> keep the delegate baseline from (1)
            //   (with no delegate value it is absent from out = c6 does not intervene = the game's network decides freely).
        }
    });
    out
}

// Vanilla category (1~6) -> final item game ID. Same conversion as the game's c6 jump table (cat1=AD .. cat6=HP).
//   WARNING churn: may move if the game's item tree changes (currently 0.4.14). Matches the constants in the game's jump table (0x143441cf4 etc.).
const VANILLA_FINAL: [u64; 6] = [4, 24, 9, 14, 19, 29];

// ===========================================================================
//  Phase 2c - live match build injection (mid-function detour in the FUN_140c6c430 candidate loop)
// ===========================================================================
// Lock-free snapshot read by the detour: (champ bytes, slot, mod_id). Rebuilt (leaked) and swapped when the selection changes.
//   The sim is parallel (rayon) -> to avoid locks this is an AtomicPtr snapshot (immutable, never freed = no UAF).
type OvEntry = (Vec<u8>, u8, u64);
static OVERRIDE_SNAPSHOT: AtomicPtr<Vec<OvEntry>> = AtomicPtr::new(core::ptr::null_mut());
static SNAP_SIG: AtomicU64 = AtomicU64::new(u64::MAX); // signature of the previous snapshot (change detection)
fn update_override_snapshot() {
    let map = build_override_map();
    let mut v: Vec<OvEntry> = map
        .into_iter()
        .map(|((c, s), id)| (c.into_bytes(), s, id))
        .collect();
    v.sort(); // deterministic order -> stable signature
              // * Safe to call every frame: if the content is unchanged, skip the rebuild/leak (delegate writes every frame but is usually unchanged).
    let mut sig: u64 = 0xcbf29ce484222325;
    for (c, s, id) in &v {
        for &b in c {
            sig = (sig ^ b as u64).wrapping_mul(0x100000001b3);
        }
        sig = (sig ^ *s as u64).wrapping_mul(0x100000001b3);
        sig = (sig ^ *id).wrapping_mul(0x100000001b3);
    }
    if sig == SNAP_SIG.swap(sig, Ordering::Relaxed) {
        return;
    }
    let boxed = Box::into_raw(Box::new(v));
    // The old snapshot is leaked (a detour on another thread may be reading it -> never free). Only happens on change = bounded.
    OVERRIDE_SNAPSHOT.store(boxed, Ordering::Release);
}

const TRAMPOLINE_DEBUG_PASSTHROUGH: bool = false; // * diagnostic: stub = original instructions + return only (no capture/call)

static PLAYER_TEAM_ID: AtomicU64 = AtomicU64::new(u64::MAX); // u64::MAX = not captured (scope not applied = fallback)
                                                             // ═══════════════════════════════════════════════════════════════════════════
                                                             //  ** My-team detection v15 (07-19, ported from the ai_adjust team_gate pattern) - no scene tag9 needed.
                                                             //    db.player_team_id() -> db.team(tid).last_starting (the 5 starters' athlete_ids) -> publish as a HashSet.
                                                             //    On the sim side, read athlete+0x810 (athlete_id) and test membership = my team.
                                                             //    => Also valid at spawn (SelectLineup, before tag9) -> removes the "my team = 0" bottleneck of the v14 spawn commit hook.
                                                             //    WARNING A2 static conclusion: the sim layer has no team_id at all (0 getters across all 78 provider vtable slots).
                                                             //      Scanning for team_id/match_id in the sim is a dead end (do not retry). athlete_id membership is the only path.
                                                             //    WARNING offset: 0.5.1 = +0x810. (The old 0x698 is 0.4.x STALE - still present in the ai_adjust source, but that is a separate TODO.
                                                             //      0x6a8 is not athlete_id either (measured all zeros; a mislabel). Use only 0x810.)
                                                             // ═══════════════════════════════════════════════════════════════════════════
                                                             // * Verified on 0.5.3 (2026-07-29): the 3-consecutive-store idiom of the athlete ctor 0x22cb050 -> **0xed32b0** is instruction-identical
                                                             //   (`mov [rsi+0x810],reg` / `+0x818,0` / `+0x820,rax`, reg <- rdx = arg2) => **+0x810 kept**.
                                                             //   Cross-checks = the roster walk 0x1740380 `add rbx,0x8d0` -> `mov r12,[rbx+0x810]` / VIEW signature 0xee9070
                                                             //   (`[rcx+0x840]` array, `[rcx+0x848]` count, `imul rcx,r9,0x8d0`) => **stride 0x8d0 kept too**.
                                                             //   Whole athlete layout confirmed unchanged: champ String +0x418/0x420/0x428 - items Vec +0x448/0x450/0x458
                                                             //   - build Vec +0x490/0x498/0x4a0 - id +0x810 - team +0x820 - gold +0x888 - position (dword) +0x8b0 - copy size 0x8b8.
                                                             // 0.5.4 = 0x800 (0.5.3 was 0x810). The roster walk that reads it is the SAME function either side
                                                             // (0x1740300 -> 0x17ce980, 286 bytes both) and reads it at the SAME two positions, +0x2d and +0x97.
const O_ATHLETE_ID: usize = 0x800;
static MY_ATHLETES: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_PREV: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_N: AtomicU64 = AtomicU64::new(0); // published starter count (0 = not obtained)
static ROSTER_TICK: AtomicU64 = AtomicU64::new(0);
static SPAWN_AID_OK: AtomicU64 = AtomicU64::new(0); // diagnostic: athlete_id (+0x810) valid at spawn (!=0, !=MAX)
static SPAWN_AID_ZERO: AtomicU64 = AtomicU64::new(0); // diagnostic: aid=0 at spawn (= not filled in yet at spawn time -> this path is unusable)
static SP4_NOBUILD: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: build Vec (+0x498/+0x4a0) invalid
static SP4_NOCAT: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: catalog (Game+0x1fc8 Vec) invalid
static SP4_NOIDX: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: failed to obtain the designated item index (scan returned None)
static SP4_RANGE: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: t >= cat_len (out of range)
static SP4_BLEN: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: sample of the observed build len
static SP4_CATLEN: AtomicU64 = AtomicU64::new(0); // (4) diagnostic: sample of the observed catalog len
static SPAWN_AID_SAMPLE: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
// Publish my team's starting roster (after the swap the previous copy is released lazily - a sim thread may be reading it, so never free immediately).
// * Diagnostic (2026-07-30): the actual published starter athlete_ids. Lets us decide "why is this player considered my team"
//   without guessing (especially when a sentinel such as aid=0 slips in and causes broad false positives).
static MY_ATH_IDS: [AtomicU64; 8] = [const { AtomicU64::new(u64::MAX) }; 8];
fn publish_my_athletes(set: std::collections::HashSet<u64>) {
    {
        let mut v: Vec<u64> = set.iter().copied().collect();
        v.sort_unstable();
        for k in 0..8 {
            MY_ATH_IDS[k].store(v.get(k).copied().unwrap_or(u64::MAX), Ordering::Relaxed);
        }
    }
    MY_ATH_N.store(set.len() as u64, Ordering::Relaxed);
    let boxed = Box::into_raw(Box::new(set));
    let old = MY_ATHLETES.swap(boxed, Ordering::AcqRel);
    let stale = MY_ATH_PREV.swap(old, Ordering::AcqRel);
    if !stale.is_null() {
        unsafe {
            drop(Box::from_raw(stale));
        }
    }
}
// Is this athlete_id one of my starters? If the roster is not obtained yet (before visiting the management screen), None = undecided (the caller decides).
#[inline]
unsafe fn is_my_athlete(athlete: usize) -> Option<bool> {
    let p = MY_ATHLETES.load(Ordering::Acquire);
    if p.is_null() || (*p).is_empty() {
        return None;
    }
    let aid = safe_read_u64(athlete + O_ATHLETE_ID)?;
    // NO **the `aid==0` block was added and then removed (2026-07-30)** - the history:
    //   (1) suspecting `pid=0` / `MY_ATHLETES=[0,1,2,3,4]` to be a db misreport, aid=0 matching was blocked, but
    //   (2) measurement confirmed **a save where team id 0 and player id 0 genuinely exist** (even after playing normal matches,
    //     0 observations of a non-zero pid / in background buys aid 1~4 appeared with a different champion each match = real players of my team).
    //   => keeping the block is a pure loss that **silently drops the designation of 1 of the 5 starters (20%)**.
    //   The original purpose - guarding against a not-yet-filled athlete (+0x810=0) - **cannot be distinguished at all** in a save
    //   where athlete_id 0 really exists, so it is not a problem to block here (the real cause, the pid misjudgement, was
    //   solved by ignoring 0 in comp-test context + the team-0 acceptance rule; see the mod's implementation notes, section 12).
    Some((*p).contains(&aid))
}
// * buy-path team gate: a sim athlete has no global team_id path, only side (+0x820, 0/1) (ghidra-re). Which side the player is on
//   = decided by majority vote over the side holding more user-designated/PT champions. Reset per match (before_management_tick). Enemy team = skip designation.
static PLAYER_SIDE: AtomicU64 = AtomicU64::new(u64::MAX); // 0/1, u64::MAX = undecided (fallback = apply)
static D_WROTE: AtomicU64 = AtomicU64::new(0); // an actual build[si] write happened

fn is_skill_key(k: &str) -> bool {
    k.contains("_skill")
        || k.contains("_passive")
        || k.contains("_ult")
        || k.contains("_slow")
        || k.contains("_stack")
        || k.contains("_buff")
        || k.contains("_curse")
        || k.contains("_road")
        || k.contains("move_speed")
        || k.contains("_aura")
        || k.contains("_mark")
}

// -- 4th slot (slot3) build buffer extension diagnostics/control --
//   The candidate build element c6 reads: [elem+8] = inner ptr, [elem+0x10] = len. [elem+0] presumed cap (confirmed by diagnostics).
//   Writing slot3 needs the inner Vec len >= 4 (the extractor only builds 3) -> extend len to 4 here when cap allows.
const EXTEND_BUILD: bool = false; // extending the candidate build is useless because the extractor discards slot3 -> OFF
                                  // * 0.5.3 regression diagnostic (2026-07-29 - to isolate "items 1~3 get bought but only the 4th does not").
                                  //   Inside the detour (a parallel rayon hot path) only **atomic counters** are touched; file output happens in post_update (main thread).
                                  //   [0] = reached the 4th-item path [1] = build_len != 3 [2] = build_cap != 3 [3] = ptr/writable failure
                                  //   [4] = failed to obtain the target index (t4=None) [5] = realloc failure [6] = * success (build[3] written)
                                  //   Diagnosis = which counter consumed the reach count. Set back to false once the cause is confirmed.
                                  // OFF in production. Gates the BE_* counters — which live in the `buy_item`
                                  // detour, i.e. the hottest path in the mod, running for every buy in every
                                  // parallel background sim — and the `build_ext_diag.txt` report they feed.
                                  //
                                  // Flip to `true` to get that file back; it is written every 300 frames through
                                  // `fs::write`, independent of `LOG_ENABLED`. It was what diagnosed the whole
                                  // post-merge chain, and it answers questions nothing else can:
                                  //   * is the 4th path reached at all, and if it bails, at which step;
                                  //   * `owned>=4` observed — distinguishes "not bought" from "bought, not drawn";
                                  //   * hook install state, VEH state, UI root address, mod item source.
                                  // * OFF again 2026-08-05: it verified the 4th-item parity fix (enemy builds now extend),
                                  //   confirmed in game. Flip back on to get `build_ext_diag.txt` — `BE_CNT[6]` (build[3]
                                  //   writes) and "owned>=4 observed" are what tell "the build was extended" apart from
                                  //   "extended and never bought". Costs a file write every ~5s on the main thread.
const BUILD_EXT_DIAG: bool = false; // * was OFF 2026-08-04: it identified the root-scan budget bug (see `ui_root::ATTEMPTS`) and that fix is confirmed in game
                                    // * Purchase order diagnostic (2026-07-30): write a snapshot of my team's build[] array to a file once per (champ, owned).
const BUY_ORDER_DIAG: bool = false;
// * For diagnosing comp-test injection failure - record the measured launcher retaddr list to a file (set false once the cause is confirmed).
// * Cause identified and fixed (comp-test injection = the missing team gate bypass; all 9 launcher retaddrs confirmed) -> OFF in production.
//   Set true to re-investigate = the measured list is written to launcher_retaddr.txt (it was decisive in tracking the cause down).
static BUY_ORDER_SEEN: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
static BUY_ORDER_BUF: Mutex<String> = Mutex::new(String::new());
static BE_CNT: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
static BE_LAST: AtomicU64 = AtomicU64::new(0); // last observed (build_len<<32)|cap
static BE_LAST_T: AtomicU64 = AtomicU64::new(0); // last recorded build[3] target index
static BE_TICK: AtomicU64 = AtomicU64::new(0); // post_update dump throttle
static BE_MAX_OWNED: AtomicU64 = AtomicU64::new(0); // max observed owned (item count) = evidence of real purchases
                                                    // ** 0.5.4 (2026-08-04): found by its documented body rather than an exe2exe signature (no old exe - see
                                                    //   `tools/rederive.py`). `mov rdi,r9 / mov rsi,rcx / cmp r8,0x11` is **1 hit in .text**, at +0x11 inside fn
                                                    //   0x29a7640. The body is __rust_realloc outright: `cmp r8,0x11 / jae` splits the over-aligned path, the
                                                    //   align<=16 path tail-jmps to HeapReAlloc(heap, 0, ptr, size), and the over-aligned path allocs (0x29bb920),
                                                    //   memcpys, then frees. Argument contract (rcx=ptr, rdx=old, r8=align, r9=new) is unchanged.
const RVA_REALLOC: usize = 0x29a7640; // 0.5.4 (0.5.3 was 0x28e3b10). History for 0.5.3 follows. (0.5.2 was 0x25c4dd0). The real __rust_realloc. (rcx=ptr, rdx=old, r8=align, r9=new) -> rax. A 112B masked signature from the old exe gave exactly 1 hit in the new exe + instruction-for-instruction identical body (mov rdi,r9 / mov rsi,rcx / cmp r8,0x11 / jae).
type ReallocFn = unsafe extern "win64" fn(usize, usize, usize, usize) -> usize;
static EXE_BASE_CACHE: AtomicUsize = AtomicUsize::new(0);
fn exe_base_addr() -> usize {
    let b = EXE_BASE_CACHE.load(Ordering::Relaxed);
    if b != 0 {
        return b;
    }
    let v = unsafe { GetModuleHandleW(core::ptr::null()) as usize };
    EXE_BASE_CACHE.store(v, Ordering::Relaxed);
    v
}

// === Pipeline firing map (count-only entry probes) - measures which setup stage runs in a spectated match ===
//  0 = score-many (0x100d150), 1 = megafunc/team gate (0x1447850), 2 = roster gen (0x11b77a0). Identical prologue (8 push, 55..).
static FIRE: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static NN_ID_NAME: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);
// Catalog index -> item name (evt[0x50] shadow-call). The inverse of scan_recipe_safe_index.
unsafe fn catalog_name_at(ctx: usize, idx: u64) -> Option<String> {
    if ctx < 0x10000 || !readable(ctx, 0x38) {
        return None;
    }
    let coll = rd_u64(ctx + 0x30) as usize;
    if coll < 0x10000 || !readable(coll, 0x18) {
        return None;
    }
    let data = rd_u64(coll + 8) as usize;
    let len = rd_u64(coll + 0x10);
    if idx >= len || data < 0x10000 || !readable(data + (idx as usize) * 16, 16) {
        return None;
    }
    let e = data + (idx as usize) * 16;
    let edata = rd_u64(e) as usize;
    let evt = rd_u64(e + 8) as usize;
    if edata < 0x10000 || evt < 0x10000 || !readable(evt, 0x60) {
        return None;
    }
    let namefn = rd_u64(evt + 0x58) as usize;
    if !code_ptr_ok(namefn) {
        return None;
    }
    let f: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(namefn);
    let nobj = f(edata);
    if nobj < 0x10000 || !readable(nobj, 0x18) {
        return None;
    }
    let chars = rd_u64(nobj + 8) as usize;
    let nlen = rd_u64(nobj + 0x10) as usize;
    if chars < 0x10000 || nlen == 0 || nlen > 64 || !readable(chars, nlen) {
        return None;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(chars as *const u8, nlen)).into_owned())
}

// === Match start launcher hook (0.5.1 RE) - deterministic capture of the rendered match seed ===
//   launcher 0x20588a0 (out=rcx, flag=dl, seed=r8, r9) <- called by the client render scene builder 0x722ca0 (the caller identifies rendering).
//   If retaddr rva is in [0x722ca0, 0x732ca0) it is a rendered match -> LIVE_SEED = seed (r8). The buy hook gates on sim_seed == LIVE_SEED.
// ** 0.5.4 re-derivation (2026-08-04, `tools/rederive.py frames` + `calls`; no old exe, see that file's header).
//   Found by frame size: this opens `push*8; mov eax,imm32; call __chkstk`, and the imm is a fingerprint.
//   Of 492 chkstk functions, **frame 0x25168 is 0x60 off the 0.5.3 launcher's 0x25108 and the next nearest is
//   0x17d0 away** — an isolated outlier, the same kind of drift as 0.5.2->0.5.3 (0x165c8 -> 0x25108).
//   Confirmed by the pair relationship, which is checkable inside one binary: it calls the seedctor candidate
//   (0x14e16d0) three times, and at 0x13b5598 the call is preceded by `mov rdx,r12` where `mov r12,r8`
//   at 0x13b5411 is the entry saving the seed => **rdx = the saved r8 = seed**, exactly the recorded contract.
//   It also calls the confirmed heap allocator 0x29bb920 repeatedly, which cross-checks that derivation too.
const CL_LAUNCHER_RVA: usize = 0x13b53d0; // 0.5.4 (0.5.3 was 0xeb8810). History for 0.5.3 follows. (0.5.2 was 0x1d96870). Evidence: (1) identical prologue idiom (8 push + mov eax,frame + call chkstk + lea rbp,[rsp+0x80] + xmm spills + [rbp+X]=-2) (2) **9 callers = the same count as the old exe** (3) the render scene builder (0x997740) calls it twice (4) internally it calls seedctor (0x12b9ab0) with rdx = the saved r8 (seed) = line-for-line correspondence with the old exe. The r8=seed entry contract still holds (mov r12,r8).
const CL_LAUNCHER_PROLOGUE: [u8; 17] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0xb8, 0x68, 0x51, 0x02,
    0x00,
]; // 0.5.4: 8 push + mov eax,0x25168 (0.5.3 was 0x25108, 0.5.2 0x165c8)
static CLAUNCH_INSTALLED: AtomicU64 = AtomicU64::new(0);
static LAUNCH_N: AtomicU64 = AtomicU64::new(0);
static LAUNCH_RENDER_N: AtomicU64 = AtomicU64::new(0);
static LAUNCH_RENDER_RA: AtomicU64 = AtomicU64::new(0); // the retaddr rva judged to be rendering
                                                        // * Is the current match a comp test? Comp test is a sandbox where the user composes both blue and red themselves, so
                                                        //   there is no notion of "my team" -> bypass the team gate and apply to both sides for designated champions.
static COMPTEST_MATCH: AtomicBool = AtomicBool::new(false);
static LAUNCH_ERR_N: AtomicU64 = AtomicU64::new(0); // launcher install failure log count (<=3)
static CLAUNCH_STUB: AtomicU64 = AtomicU64::new(0); // address of our launcher stub (for re-validating the entry point)
static LAUNCH_WAIT: AtomicU64 = AtomicU64::new(0); // frames spent waiting for serpen to install
                                                   // WARNING minimal detour: the launcher has a 91KB chkstk frame and fires for every 30~40 background matches -> no format!/fs/locks/catch_unwind (stack overflow).
                                                   //   The body does raw reads and atomics only (no panic source -> catch_unwind unnecessary).
unsafe extern "C" fn cap_launcher(saved: *mut u64, _e: usize) -> u64 {
    // WARNING keep the minimal-detour constraint - the probe too uses only rdtsc + global atomics (no rec_tl = TLS lazy-init path).
    if saved.is_null() {
        return 0;
    }
    let seed = *saved.add(2); // r8 = arg3 = seed
    let retaddr = *saved.add(10); // call-site retaddr (above the stub's 10 pushes)
    let base = GetModuleHandleW(core::ptr::null()) as u64;
    if base == 0 || retaddr < base {
        return 0;
    }
    let rva = retaddr - base;
    LAUNCH_N.fetch_add(1, Ordering::Relaxed);
    // * Caller within the client render scene builder range 0x722ca0 -> rendered match seed
    // * serpen canonical (CURRENT_MATCH_DETECT.md, verified in game): on-screen match call sites = exactly 0x72f507 (path A) and 0x733e9f (path B). 0x2061132 = background.
    // * Comp test (comp_test) added (07-21, ghidra-re confirmed): retaddr 0xc884fa (call site 0xc884f5, function 0xc831b0).
    //   Its reach path is unique (dispatch arm 31 -> 0x75fe90 -> 0xc831b0) so it does not mix with background. The other 3 observed are all background:
    //   0x13dd5a0 = solo_rank / 0x1659d55 = server::worker / 0x2061137 = tick driver -> never add these.
    //   The r8=seed passing form is the same as normal spectating, so the capture logic is reused as-is.
    // * 0.5.2 remap (exe2exe call-site re-enumeration 2026-07-22): the render scene builder container 0x722ca0 -> 0x74d510 (mnemonic 0.9928),
    //   its 2 launcher call retaddrs = 0x72f507 -> 0x759c36 / 0x733e9f -> 0x75e5cf.
    //   comptest container 0xc831b0 -> 0xd405c0 (the remaining pair of the 9/9 launcher-caller bijection; its single caller 0x75fe90 -> 0x78a5c0 is isomorphic)
    //   retaddr 0xc884fa -> 0xd40a63. TODO comptest is tentative only (container size shrank 0x5b8f -> 0xce1 = a refactor; ghidra-re confirmation recommended).
    // * 0.5.3 remap (2026-07-29, re-enumerated from the measured call sites of launcher 0xeb8810):
    //   render scene builder container 0x74d510 -> 0x997740 (caller count and size fingerprints match), its 2 launcher calls
    //   retaddr = 0x759c36 -> **0x9a3287** / 0x75e5cf -> **0x9a7b03** (both measured inside the container).
    //   comptest container 0xd405c0 -> 0x1925ab0 (size 0xce1 -> 0xf5a; single-caller fingerprint matches) retaddr 0xd40a63 -> **0x1925f12**.
    //   TODO comptest is tentative as in 0.5.2 (matched only down to the single-caller chain, not verified in game).
    // * The nature of all 9 launcher callers = fully determined (2026-07-30 full RE, panic Location file/line + packet dispatch arm reachability):
    //   0x9a3287  = spectate (arm75 SpectateGameStart)   * on screen
    //   0x9a7b03  = my match (arm30 GameStart)           * on screen
    //   0x1925f12 = comp test main match (arm31 CompTestStarted, data.rs:1545)  * on screen, both sides user-composed
    //   0x18f718e = comp test **record replay** (training_ui.rs:4351)           * on screen, both sides user-composed
    //   0x229ad94 = replay (pause_ui.rs:2332 - the value serpen uses)
    //   0x220acb (state.rs app state machine) / 0x195c5be (server\worker.rs) / 0x20dac9c (solo_rank.rs)
    //   0x2256a6d (solo_rank_ui.rs) = **background sim -> never add these**
    let is_comptest = rva == 0x1925f12 || rva == 0x18f718e;
    if (rva == 0x9a3287 || rva == 0x9a7b03 || is_comptest) && seed != 0 {
        let prev = LIVE_SEED.swap(seed, Ordering::Relaxed);
        if prev != seed {
            RENDER_PROVIDER.store(0, Ordering::Relaxed);
        } // new match seed -> the ctor right after re-captures the provider
        COMPTEST_MATCH.store(is_comptest, Ordering::Relaxed);
        LAUNCH_RENDER_N.fetch_add(1, Ordering::Relaxed);
        LAUNCH_RENDER_RA.store(rva, Ordering::Relaxed);
    }
    0
}
// * Hook install path counters (2026-07-22 diagnostic): "hook retry" was measured at 189us per frame = 470k cycles -
//   far too large for an early-return path. This distinguishes whether we actually reinstall (VirtualAlloc + VirtualProtect xN) every frame.
//   If a real install happens once per frame it means stub leakage + a mutual re-chaining cycle with serpen (as in the draft_overlay hang).
static HK_L_CALLS: AtomicU64 = AtomicU64::new(0); // install_launcher_hook calls
static HK_L_OURS: AtomicU64 = AtomicU64::new(0); // entry point confirmed to be our stub -> immediate return (normal path)
static HK_L_WAIT: AtomicU64 = AtomicU64::new(0); // returned while waiting for serpen
static HK_L_INSTALL: AtomicU64 = AtomicU64::new(0); // * actually entered install_detour_generic
static HK_L_B0: AtomicU64 = AtomicU64::new(0); // first byte of the last observed entry point
static HK_L_TGT: AtomicU64 = AtomicU64::new(0); // last observed movabs target
static HK_S_INSTALL: AtomicU64 = AtomicU64::new(0); // seed-ctor actually entered install
static HK_L_TICK: AtomicU64 = AtomicU64::new(0);
static HK_L_SKIP: AtomicU64 = AtomicU64::new(0); // frames skipped by the throttle
fn install_launcher_hook() {
    HK_L_CALLS.fetch_add(1, Ordering::Relaxed);
    // * Cost optimization (2026-07-22 perf measurement - this function cost **at least 106us** every frame, the single largest
    //   real main-thread expense. It is the minimum, not the average, that is 106us, so it is real work and not preemption noise):
    //   (1) `GetModuleHandleW` (loader lock) called directly every frame -> **the cached `exe_base_addr()`**
    //      (every other path used the cache; only this one called the raw API)
    //   (2) removed `readable()` = **VirtualQuery** (address-space lock) -> read the entry point with the VEH-protected `safe_read_u64`.
    //      The address is already validated at install time and a fault is caught by the VEH, so the double check was unnecessary.
    //   (3) the post-install re-validation (self-heal in case another mod overwrote our hook) has no reason to run every frame ->
    //      **every 60 frames (~1s)**. Self-healing within a second is plenty even if overwritten (this is a match-start event, so there is slack).
    //   (4) 2026-08-07: the throttle covered state 1 (installed, re-validating) only, so a
    //      FAILED install (2) still paid the full cost every frame — the case a game update
    //      puts every RVA in. Any non-zero state is now throttled; 0 is the untried first
    //      frame, which still runs immediately.
    if CLAUNCH_INSTALLED.load(Ordering::Relaxed) != 0 {
        if HK_L_TICK.fetch_add(1, Ordering::Relaxed) % 60 != 0 {
            HK_L_SKIP.fetch_add(1, Ordering::Relaxed);
            return;
        }
    }
    // * Coexisting with serpen (chain hooking): if serpen hooks launcher 0x20588a0 first (entry point = movabs+jmp) we chain behind it.
    //   WARNING installing first would let serpen overwrite us and orphan our hook -> wait for serpen (= a foreign movabs entry point) to appear (up to 240 frames),
    //     and if the original prologue is still there after that, assume serpen is absent and install standalone. Re-validate every frame (entry point != our stub -> re-chain, so we self-heal even if serpen overwrites us later).
    let base = exe_base_addr(); // * the cached copy (was: GetModuleHandleW every frame)
    if base == 0 {
        return;
    }
    let fn_addr = base + CL_LAUNCHER_RVA;
    // * Check the entry point with a VEH-protected read instead of VirtualQuery (was: readable() every frame).
    let Some(w0) = (unsafe { safe_read_u64(fn_addr) }) else {
        return;
    };
    let b0 = (w0 & 0xff) as u8;
    let b1 = ((w0 >> 8) & 0xff) as u8;
    let cur_tgt: usize = if b0 == 0x48 && b1 == 0xb8 {
        match unsafe { safe_read_u64(fn_addr + 2) } {
            Some(t) => t as usize,
            None => return,
        } // movabs imm64 = fn+2..+10
    } else {
        0
    };
    let our = CLAUNCH_STUB.load(Ordering::Relaxed) as usize;
    HK_L_B0.store(b0 as u64, Ordering::Relaxed);
    HK_L_TGT.store(cur_tgt as u64, Ordering::Relaxed);
    if our != 0 && cur_tgt == our {
        CLAUNCH_INSTALLED.store(1, Ordering::Relaxed);
        HK_L_OURS.fetch_add(1, Ordering::Relaxed);
        return;
    } // entry point = our stub -> fine
    let is_foreign = b0 == 0x48 && cur_tgt >= 0x10000 && cur_tgt != our; // a foreign hook (serpen etc.) is present
    let waited = LAUNCH_WAIT.fetch_add(1, Ordering::Relaxed);
    if !is_foreign && b0 != 0x48 && waited < 240 {
        HK_L_WAIT.fetch_add(1, Ordering::Relaxed);
        return;
    } // original prologue and still waiting -> wait for serpen to install
      // Install (or re-chain). install_detour_generic chains automatically when it detects a foreign hook.
    HK_L_INSTALL.fetch_add(1, Ordering::Relaxed);
    let r = unsafe {
        install_detour_generic(
            CL_LAUNCHER_RVA,
            12,
            cap_launcher as usize,
            &CL_LAUNCHER_PROLOGUE,
        )
    };
    match r {
        Ok(stub) => {
            CLAUNCH_STUB.store(stub as u64, Ordering::Relaxed);
            CLAUNCH_INSTALLED.store(1, Ordering::Relaxed);
        }
        Err(e) => {
            CLAUNCH_INSTALLED.store(2, Ordering::Relaxed);
            if LAUNCH_ERR_N.fetch_add(1, Ordering::Relaxed) < 3 {}
        }
    }
}

// === seed-ctor hook (0.5.1 RE) - deterministic capture of the rendered sim's provider pointer (seed values cannot be compared -> pointer identity) ===
//   ghidra-re confirmed: FUN_1421d03e0 (rcx = provider (this), rdx = seed (= launcher r8, bit-identical, no conversion)) stores seed at provider+0xeab8.
//   But +0xeab8 is updated on every random draw = RNG running state -> comparing it at buy time is impossible in principle.
//   Alternative: when the ctor is entered with rdx == LIVE_SEED (the rendered initial seed captured by launcher), record that provider (rcx) as RENDER_PROVIDER.
//   At buy time: provider == RENDER_PROVIDER -> definitely the rendered sim (address comparison, independent of mutable fields).
//   WARNING **legacy comment correction (0.5.3, 07-29)**: the current buy gate uses **r9 (arg4) = provider**, not `*(game_p6+0x1dc0)`
//     (see `*saved.add(3)` in the buy hook below - the old RE conclusion that [rsp+0x30] was the buy-list container, not the provider).
//     The only live code reading `Game+0x1dc0` is cap_spawn, and that is gated OFF. Game+0x1dc0/+0x1dc8 themselves are **confirmed still present** in 0.5.3
//     (launcher 0xeb9646 `mov [rsi+0x1dc0],rax; mov [rsi+0x1dc8],rax` + vtable slot +0x20 being `mov rax,[rcx+0xeaf8]` = consistent with the seed offset).
//   TODO **unverified**: "r9 = provider" cannot be confirmed statically because the buy body overwrites arg4 immediately - as in 0.5.2 it is **established only by in-game seed matching**.
//     If it is wrong in 0.5.3 the symptom is not a crash but a silent "spectated match not recognized" (detectable via the is_live hit counter).
//   launcher calls the ctor synchronously -> LIVE_SEED is guaranteed to be set first. Background sims do not use the render seed -> no match (contamination excluded).
// ** 0.5.4 (2026-08-04): frame 0x11ba8, **0x50 off the 0.5.3 seedctor's 0x11b58 with the next nearest 0x500 away**,
//   and it is the function the launcher calls with rdx = its saved seed (see CL_LAUNCHER_RVA above). Entry shape
//   is unchanged: 8 push (12B) + mov eax,frame + call chkstk, so SEEDCTOR_PROLOGUE needs no edit.
const SEEDCTOR_RVA: usize = 0x14e16d0; // 0.5.4 (0.5.3 was 0x12b9ab0). History for 0.5.3 follows. (0.5.2 was 0x22c1da0). The 12B prologue is completely identical (8 push); the chkstk frame went 0x11b58 -> 0x11b98; confirmed via the call inside launcher (0xeb8810) with rdx = the saved r8 (seed). WARNING: the seed store offset moved from provider+0xeab8 to **+0xeaf8** (measured at 0x12ba92d).
                                       // * 0.5.3: the seed store offset inside the provider struct moved (0.5.2 +0xeab8 -> 0.5.3 +0xeaf8).
                                       //   Measured = `mov [reg+0xeaf8], rdx` inside seedctor @0x12ba92d (the old exe has 0xeab8 in the same place).
                                       //   WARNING keep it in a single constant - updating only this on each patch carries the whole is_live gate along.
                                       // 0.5.4 = 0xeb28 (0.5.3 was 0xeaf8, 0.5.2 0xeab8). Measured, not guessed: seedctor spills its `rdx` (the seed)
                                       // to [rbp+0x11a48] at entry (0x14e1714), reloads it at 0x14e2596 and stores it to **[rsi+0xeb28]** at 0x14e259d.
                                       // The writes that follow (+0xeb30 = 0, +0xeb58 = 0, +0xeb59 = the bool arg) are the same field cluster.
const O_PROVIDER_SEED: usize = 0xeb28;
const SEEDCTOR_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // ghidra-re confirmed: 8 push (12B) + mov eax,0x11b58 + call chkstk (same pattern as launcher)
const SEEDCTOR_ORIG_LEN: usize = 12; // relocate the 8 pushes only (excluding the chkstk call). The jmp lands on fn+12 = mov eax -> the frame is set up correctly
static SEEDCTOR_INSTALLED: AtomicU64 = AtomicU64::new(0);
static RENDER_PROVIDER: AtomicU64 = AtomicU64::new(0); // * rendered sim provider pointer (the primary is_live gate)
static LIVE_SEED: AtomicU64 = AtomicU64::new(0); // * my match's seed (captured from r8 in the launcher hook). The v13 value-comparison key.
static PROV_HIT: AtomicU64 = AtomicU64::new(0); // is_live (v13 provider/seed match) firings
static VT_OK: AtomicU64 = AtomicU64::new(0); // of those, firings via seed value comparison
static INGAME_NOW: AtomicBool = AtomicBool::new(false); // "spectating right now" flag set by post_update
static BUY_WROTE_FIRE: AtomicU64 = AtomicU64::new(0); // successful build[si] writes
static SEEDCTOR_N: AtomicU64 = AtomicU64::new(0); // total ctor firings
static SEEDCTOR_MATCH_N: AtomicU64 = AtomicU64::new(0); // rdx == LIVE_SEED hits (rendered provider captured)
unsafe extern "C" fn cap_seed_ctor(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let provider = *saved; // saved+0 = rcx = arg1 = provider(this)
    let seed = *saved.add(1); // saved+1 = rdx = arg2 = seed(=launcher r8)
    SEEDCTOR_N.fetch_add(1, Ordering::Relaxed);
    let ls = LIVE_SEED.load(Ordering::Relaxed);
    if ls != 0 && seed == ls && provider >= 0x10000 && provider < 0x0000_8000_0000_0000 {
        RENDER_PROVIDER.store(provider as u64, Ordering::Relaxed);
        SEEDCTOR_MATCH_N.fetch_add(1, Ordering::Relaxed);
    }
    0
}
fn install_seed_ctor_hook() {
    let state = SEEDCTOR_INSTALLED.load(Ordering::Relaxed);
    if state == 1 {
        return;
    } // * skip only on 1 = success (0/2 = retry)
    // A failed attempt (2) backs off instead of re-running every frame; see
    // `install_retry_due`. State 0 is the untried first frame and is not delayed.
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if state == 2 && !install_retry_due(&RETRY) {
        return;
    }
    HK_S_INSTALL.fetch_add(1, Ordering::Relaxed);
    let r = unsafe {
        install_detour_generic(
            SEEDCTOR_RVA,
            SEEDCTOR_ORIG_LEN,
            cap_seed_ctor as usize,
            &SEEDCTOR_PROLOGUE,
        )
    };
    SEEDCTOR_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════════
//  ** v14 spawn commit hook (ghidra-re, confirmed on 0.5.1) - a "intervene once at build creation" design.
//    Every build creation path (live worker 0x164f040 / league 0xf63f80 / others) converges on the single choke point
//    athlete ctor -> wrapper -> spawn FUN_142060280. Planting only the build[] targets here lets the
//    buy resolver build up to them naturally (components -> combines) -> no per-buy intervention needed.
//    Arguments: rcx = Game (-> provider = *(Game+0x1dc0)), rdx = athlete (the final build), r8 = descriptor.
//    Once per athlete, a single call site, and after personal tactics is applied = our injection is the final winner.
//    WARNING the rdx athlete is a stack copy (0x8b8) - writing here propagates via the later memcpy all the way into the provider Vec (RE confirmed).
// ═══════════════════════════════════════════════════════════════════════════
// * 0.5.2 (2026-07-22 exe2exe): 0x2060280 = skeleton NO MATCH (= logic changed). Re-pinned via the call target at the same offset +0x8c
//   in the caller container 0x20565e0 -> 0x1d94640 (mnemonic 1.0000) = 0x1d9e0e0. The function shrank 0x714 -> 0x51f, and **the prologue went from 8 pushes to 7** (41 55 = push r13 is gone).
//   => prologue constants and ORIG_LEN updated + the entry patch (12B movabs+jmp) is longer than the push block (10B), so the mov eax must be relocated too
//     -> a rax-preserving tail (r11 jump) is required = install_detour_r11.
//   WARNING since the function's logic changed, the argument contract (rcx=Game, rdx=athlete) is unconfirmed -> **gate OFF** (re-enable after ghidra-re re-confirmation).
//     No functionality is lost: the 07-19 measurements showed build[] injection reaching 8/8 = the buy path alone is sufficient.
// * 0.5.3 re-pin done (2026-07-29, ghidra-re): 0x1d9e0e0 -> **0xebfe50** (~0xec0302). Caller container 0x1d94640 -> 0xeb6480 (the +0x91 call @0xeb6511).
//   Body instructions confirmed 1:1 (`[rcx+0x1dc0]`/`[rcx+0x1dc8]` -> `call [r15+0x160]`, `[rsi+0x1dd0]`/`[rsi+0x1dd8]` -> `call [rax+0x30]`).
//   WARNING **two changes are mandatory before re-enabling** - the constants below are unused today because the gate is OFF:
//     (1) prologue: 7 push + mov eax + chkstk -> **8 push (12B) + sub rsp,0xf8** (no chkstk) => ORIG_LEN=12 and no rax preservation needed (generic works).
//     (2) argument contract: r8 = &descriptor -> **r8/r9 = the descriptor's two-word pair** (the caller switched to calling the builder indirectly through the global function pointer 0x144531340).
//        rcx=Game and rdx=athlete stack copy (0x8b8) are unchanged. 15 direct callers = it remains a single choke point.
const SPAWN_RVA: usize = 0xebfe50; // 0.5.3 (0.5.2 was 0x1d9e0e0, 0.5.1 was 0x2060280). WARNING: SPAWN_INJECT_ENABLED=false, so no detour is installed = no effect.
const SPAWN_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // 0.5.3: 8 push (12B) + sub rsp,0xf8 (0.5.2 was 7 push + mov eax,0x4d20)
const SPAWN_ORIG_LEN: usize = 12; // 0.5.3: relocate the 8 pushes only (12B = exactly an instruction boundary) => install_detour_r11 is unnecessary on re-enable (generic suffices).
const SPAWN_INJECT_ENABLED: bool = false; // * keep the gate OFF (0.5.3 confirmed the **argument contract really did change** = (2) above - review before wiring it up). History for 0.5.2: gate OFF (logic change unconfirmed) - 0.5.1 had true. ~~resumed (07-19)~~ the sealing reason "no catalog at spawn time" turned out to be an offset error.
                                          //   The old 0x1fe8/0x1ff0 = a neighbouring empty Vec (always len=0) -> the real catalog is Game+0x1fd0/+0x1fd8 (ghidra-re confirmed).
                                          //   The v15 team decision (athlete_id membership) is verified (aid valid 10/10, my team 5/10 correct) -> (4) injection expected to complete.
static SPAWN_INSTALLED: AtomicU64 = AtomicU64::new(0);
static SPAWN_N: AtomicU64 = AtomicU64::new(0); // total hook firings
static SPAWN_LIVE_N: AtomicU64 = AtomicU64::new(0); // athletes judged to be in a rendered match
static SPAWN_PLAYER_N: AtomicU64 = AtomicU64::new(0); // of those, my team (injection targets)
static SPAWN_WROTE: AtomicU64 = AtomicU64::new(0); // actual build[] writes
static SPAWN_NOSIDE: AtomicU64 = AtomicU64::new(0); // skipped because the side was undecided (= covered by the buy path)
unsafe extern "C" fn cap_spawn(saved: *mut u64, _e: usize) -> u64 {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !SPAWN_INJECT_ENABLED || saved.is_null() {
            return;
        }
        SPAWN_N.fetch_add(1, Ordering::Relaxed);
        let game = *saved as usize; // rcx = Game
        let athlete = *saved.add(1) as usize; // rdx = athlete (stack copy, the final build)
        if game < 0x10000 || athlete < 0x10000 {
            return;
        }
        // -- (1) Rendered (spectated) match gate: provider = *(Game+0x1dc0) - offset confirmed unchanged in 0.5.3 (07-29). WARNING the buy hook uses r9 (a separate path) --
        let provider = match safe_read_u64(game + 0x1dc0) {
            Some(p) => p,
            None => return,
        };
        if provider < 0x10000 || provider >= 0x0000_8000_0000_0000 {
            return;
        }
        let lseed = LIVE_SEED.load(Ordering::Relaxed);
        let seed_ok =
            lseed != 0 && safe_read_u64(provider as usize + O_PROVIDER_SEED) == Some(lseed);
        let rp = RENDER_PROVIDER.load(Ordering::Relaxed);
        if !(seed_ok || (rp != 0 && provider == rp)) {
            return;
        }
        SPAWN_LIVE_N.fetch_add(1, Ordering::Relaxed);
        // * Diagnostic (v15 prerequisite check): is athlete_id (+0x810) already filled in at spawn time? If it is 0 this path is impossible.
        if readable(athlete + O_ATHLETE_ID, 8) {
            let aid = rd_u64(athlete + O_ATHLETE_ID);
            if aid == 0 || aid == u64::MAX {
                SPAWN_AID_ZERO.fetch_add(1, Ordering::Relaxed);
            } else {
                SPAWN_AID_OK.fetch_add(1, Ordering::Relaxed);
                for k in 0..4 {
                    if SPAWN_AID_SAMPLE[k].load(Ordering::Relaxed) == aid {
                        break;
                    }
                    if SPAWN_AID_SAMPLE[k]
                        .compare_exchange(0, aid, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }
        // -- (2) Is this a designated champion? --
        if !readable(athlete, 0x8a8) {
            return;
        }
        let cptr = rd_u64(athlete + 0x410) as usize;
        let clen = rd_u64(athlete + 0x418) as usize;
        if cptr < 0x10000 || clen == 0 || clen > 48 || !readable(cptr, clen) {
            return;
        }
        let champ_cow =
            String::from_utf8_lossy(std::slice::from_raw_parts(cptr as *const u8, clen));
        let champ: &str = champ_cow.as_ref();
        if !is_champ_designated(champ) {
            return;
        }
        // -- (3) Is it my team (v15): athlete_id (+0x810) in my starting roster = no scene tag9 needed -> holds at spawn time.
        //     WARNING because static analysis A2 established the sim has no team_id, this membership test is the only deterministic path.
        //     If the roster is not obtained yet (before visiting the management screen), stay undecided -> skip injection (avoiding enemy-team contamination > coverage). The buy path covers it.
        //     Also run the scene side as a fallback (for early frames where the roster exists but aid is not filled in).
        let mine = is_my_athlete(athlete);
        let ok = match mine {
            Some(true) => true,
            Some(false) => return, // definitely another team = do not inject
            None => {
                // roster/aid unavailable -> scene fallback (if any)
                let side = if readable(athlete + 0x810, 8) {
                    rd_u64(athlete + 0x810)
                } else {
                    u64::MAX
                };
                match scene_player_side() {
                    Some(ps) => side == ps,
                    None => false,
                }
            }
        };
        if !ok {
            SPAWN_NOSIDE.fetch_add(1, Ordering::Relaxed);
            return;
        }
        SPAWN_PLAYER_N.fetch_add(1, Ordering::Relaxed);
        // -- (4) Inject the build[] targets --
        // ** Catalog offset correction (07-19 ghidra-re confirmed): the old 0x1fe8/0x1ff0 were a **neighbouring empty Vec** 0x18 off
        //   (the Game ctor initializes it cap=0 / ptr=8 (dangling) / len=0, and there is no push site anywhere in the exe -> always len=0.
        //    That is exactly what the measured catlen=0 was - it was never a spawn timing problem).
        //   The real catalog = Game+0x1fc8{cap} / +0x1fd0{ptr} / +0x1fd8{len}, stride 0x10 {elem_ptr, vtable}.
        //   * Same index space: the ctx builder 0x1420571C8 puts ctx+0x30 = &(Game+0x1fc8) => it is the same heap buffer
        //    that buy indexes into = usable directly in build[] (no mapping table needed).
        //   * Ordering guaranteed: Game creation (catalog builder 0x21c0750) precedes spawn (in all 21 wrapper call sites).
        let cat_base = rd_u64(game + 0x1fd0) as usize;
        let cat_len = rd_u64(game + 0x1fd8);
        let bptr = rd_u64(athlete + 0x488) as usize;
        let blen = rd_u64(athlete + 0x490);
        SP4_BLEN.store(blen, Ordering::Relaxed);
        SP4_CATLEN.store(cat_len, Ordering::Relaxed);
        if bptr < 0x10000 || blen == 0 || blen > 8 || !writable(bptr, (blen as usize) * 8) {
            SP4_NOBUILD.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if cat_base < 0x10000 || cat_len == 0 || cat_len > 100000 {
            SP4_NOCAT.fetch_add(1, Ordering::Relaxed); // vanilla designations need no scan, so keep going
        }
        for si in 0u8..3 {
            if (si as u64) >= blen {
                break;
            }
            // * Scope = fixed to Plain: at spawn time there is no comp-test side information (and this hook is
            //   sealed with SPAWN_INJECT_ENABLED=false anyway). Per-side comp-test designations are handled by the buy path.
            let idx: Option<u64> = if let Some(vid) = slotN_vanilla_id(Scope::Plain, champ, si) {
                Some(vid) // vanilla: id == catalog index
            } else if let Some(mk) = slotN_item_key(Scope::Plain, champ, si) {
                scan_catalog_index(cat_base, cat_len, mk.as_bytes()) // mod item: name scan + recipe validation
            } else {
                continue;
            };
            let Some(t) = idx else {
                SP4_NOIDX.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            if cat_len > 0 && t < cat_len {
                if rd_u64(bptr + (si as usize) * 8) != t {
                    wr_u64(bptr + (si as usize) * 8, t);
                    SPAWN_WROTE.fetch_add(1, Ordering::Relaxed);
                }
            } else {
                SP4_RANGE.fetch_add(1, Ordering::Relaxed);
            }
        }
    }));
    0 // the install_detour_generic stub does not use the return value (this is an observe/modify hook)
}
fn install_spawn_hook() {
    if !SPAWN_INJECT_ENABLED {
        return;
    } // * when sealed, do not install the detour at all (a no-op hook = pure risk)
    let state = SPAWN_INSTALLED.load(Ordering::Relaxed);
    if state == 1 {
        return;
    }
    // See `install_retry_due`.
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if state == 2 && !install_retry_due(&RETRY) {
        return;
    }
    // * 0.5.2: a rax-preserving tail is mandatory (the relocated region contains mov eax,0x4d20 -> the chkstk right after uses that value as the frame size).
    let r = unsafe {
        install_detour_r11(
            SPAWN_RVA,
            SPAWN_ORIG_LEN,
            cap_spawn as usize,
            &SPAWN_PROLOGUE,
        )
    };
    SPAWN_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
}

static VIEW_OK: AtomicU64 = AtomicU64::new(0); // successful view captures
static VIEWSCAN_DONE: AtomicBool = AtomicBool::new(false); // one-shot gate for the detailed failure dump
                                                           // ** Reverse-search diagnostic: derive the db view offset at runtime from a buy athlete (a confirmed element of the spectated roster). Not a heuristic = deterministic.
                                                           // ** Thread identity gate check (07-11 RE priority 1): hypothesis that spectating (re-sim) = main thread, background sim = rayon workers.
                                                           //   Compare the post_update (main thread) tid with the buy hook (sim thread) tid -> if they differ, spectating can be detected with no offsets at all.
static CP_INSTALLED: AtomicU64 = AtomicU64::new(0);
const CP_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];
const BS_INJECT_TEST: bool = false; // input injection = heap DB corruption crash -> OFF. Switched to return hooking // * overwrite the build-score item key (on the stack) with "dagger" to tell whether it affects the real build

// ===========================================================================
//  player-state array probe - find the array that feeds the top item bar display (GamePlayerState array)
//  by scanning, and pin the items Vec offset. (GameViewSystem+0x840 array, stride 0x8d0)
//  champion@+0x420, team@+0x820, position@+0x8b0. items = somewhere between +0x420 and +0x820.
// ===========================================================================
const PS_PROBE_ENABLED: bool = false; // production: playerstate diagnostics OFF
static PS_DONE: AtomicBool = AtomicBool::new(false);
static PS_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

// * Hooking the sim driver FUN_14204f810: rdx = p2 = the match input data. Find the athlete item build here, before precomputation.
const SIM_PROBE_ENABLED: bool = false; // production: sim driver diagnostic hook OFF
const SIM_RVA: usize = 0x223d1b0; // WARNING STALE for 0.5.2/0.5.3 (exe2exe NO MATCH = logic changed; harmless because SIM_PROBE_ENABLED=false) // 0.5.0_3 (0.5.0_2 was 0x204f810; the 47-instruction anchor matches, diff = stack slot displacements only = codegen churn, not a structural change). SIM_PROBE_ENABLED=false (OFF)
const SIM_ORIG_LEN: usize = 12; // push rbp/r15/r14/r13/r12/rsi/rdi/rbx (8 of them, position independent)
const SIM_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];
static SIM_INSTALLED: AtomicBool = AtomicBool::new(false);
static SIM_DUMPED: AtomicBool = AtomicBool::new(false);
// From a heap ptr v, look for a String such as a champion name / item key nearby (v+0..v+0x28).
unsafe fn find_str_near(v: usize) -> Option<(usize, String)> {
    let mut o = 0usize;
    while o <= 0x28 {
        if let Some(st) = read_str_try(v + o) {
            if st.len() >= 3 {
                return Some((o, st));
            }
        }
        o += 8;
    }
    None
}
unsafe fn dump_region(label: &str, base: usize) -> String {
    let mut s = format!("  {} = {:#x} (+0x0..+0x400):\n", label, base);
    if base <= 0x10000 {
        return s;
    }
    let mut oo = 0usize;
    while oo < 0x400 {
        let v = safe_read_u64(base + oo).unwrap_or(0);
        if looks_heap(v) {
            let mut note = String::new();
            if let Some(st) = read_str_try(v as usize) {
                note = format!(" →Str'{}'", st);
            } else if let Some((o2, st)) = find_str_near(v as usize) {
                note = format!(" →+{:#x}Str'{}'", o2, st);
            } else {
                note.push_str(" →[");
                for j in 0..4 {
                    let e = safe_read_u64(v as usize + j * 8).unwrap_or(0);
                    note.push_str(&format!("{:#x} ", e));
                }
                note.push(']');
            }
            s.push_str(&format!("    +{:#x} = {:#018x}{}\n", oo, v, note));
        }
        oo += 8;
    }
    s
}
// * Display-source confirmation test: overwrite every champion's +0x410 (Vec<u64> of 3) with a recognizable item ID and see whether the bar changes.
const DISPLAY_TEST: bool = false; // +0x410 = a copy of the build plan, not the bar -> OFF
const DISPLAY_TEST_ID: u64 = 29; // a recognizable vanilla final item ID (all 3 slots get this)
static DTEST_LOGGED: AtomicBool = AtomicBool::new(false);
// * Capturing the view (GameViewSystem) pointer: FUN_1422360c0 mid-function 0x22360cc (rcx = view). view+0x840 = array / +0x848 = count.
//   (0.5.0: function start 0x22360c0, was 0x1e84d50; mid 0x22360cc, was 0x1e84d5c.)
static VIEW_PTR: AtomicU64 = AtomicU64::new(0);
// WARNING WARNING not migrated for 0.5.0_3 (STALE): 0x22360cc -> mask-sig MULTI (a monomorphic family of roster getters; candidates 0x19b77cc/787c/792c/79dc/..., stride 0xb0). No string-xref available = cannot be pinned statically -> follow-up via ghidra-re.
//   * Risk: the whole family shares VIEW_PROLOGUE (14B) -> pre-validation cannot prevent installing on the wrong one. With AUTO4_FORWARD_SCORE enabled the wrong getter could be hooked -> keep AUTO4 disabled until ghidra-re re-pins it.
const VIEW_RVA: usize = 0x20ae1ac; // WARNING STALE for 0.5.2/0.5.3 (not migrated; harmless because VIEW_HOOK_ENABLED=false) // 0.5.0_3 (0.5.0_2 was 0x22360cc, sig-xref UNIQUE: mov rax,[rcx+0x840]; imul rcx,r9,0x8d0). VIEW_HOOK_ENABLED=false (OFF)
const VIEW_ORIG_LEN: usize = 14; // mov rax,[rcx+0x840](7) + imul rcx,r9,0x8d0(7)
                                 // 0.5.0: mov rax,[rcx+0x840] = 48 8B 81 40 08 00 00 / imul rcx,r9,0x8d0 = 49 69 C9 D0 08 00 00
const VIEW_PROLOGUE: [u8; 14] = [
    0x48, 0x8b, 0x81, 0x40, 0x08, 0x00, 0x00, 0x49, 0x69, 0xc9, 0xd0, 0x08, 0x00, 0x00,
];
static VIEW_INSTALLED: AtomicBool = AtomicBool::new(false);
const VIEW_HOOK_ENABLED: bool = false; // * hooking a hot render function = crash -> OFF. Replaced by scanning.
                                       // Try both game String layouts: {len,ptr,cap} or {ptr,len,cap}. Return it if it is an ASCII key/name.
unsafe fn read_str_try(addr: usize) -> Option<String> {
    if !readable(addr, 24) {
        return None;
    }
    let q0 = safe_read_u64(addr)? as usize;
    let q8 = safe_read_u64(addr + 8)? as usize;
    for &(ptr, len) in &[(q8, q0), (q0, q8)] {
        // (len,ptr)=len@0,ptr@8 / (ptr,len)=ptr@0,len@8
        if ptr <= 0x10000 || ptr >= (1usize << 48) || len < 2 || len > 48 {
            continue;
        }
        let mut b = Vec::new();
        if !safe_read_bytes(ptr, len, &mut b) {
            continue;
        }
        if b.iter().all(|&x| x == b'_' || x.is_ascii_alphanumeric())
            && (b[0] as char).is_ascii_alphabetic()
        {
            return String::from_utf8(b).ok();
        }
    }
    None
}
// Detect item keys found by the push probe (used to identify the displayed items Vec).
fn is_known_item_key(k: &str) -> bool {
    const ITEMS: [&str; 20] = [
        "dagger",
        "ironsword",
        "vital_orb",
        "arcane_crystal",
        "steel_armor",
        "mystic_cloak",
        "soldiers_longsword",
        "wind_dagger",
        "spirit_crystal",
        "hardened_heart",
        "nashors_tooth",
        "ring_of_reincarnation",
        "ruinous_blade",
        "souls_edge",
        "dusk_raven",
        "staff_of_rapture",
        "twin_stormblade",
        "angels_fang",
        "thunderclaw",
        "spirit_visage",
    ];
    ITEMS.contains(&k)
        || k.starts_with("radiant_")
        || k.contains("_blade")
        || k.contains("sword")
        || k.contains("_armor")
        || k.contains("_plate")
}
// Validate a roster element by the position of its champion String. * 0.5.0_3: champ name @ +0x420 (consistent with ath_champ_name).
//   WARNING looking only at the legacy +0x388~0x3b0 offsets fails to recognize a 0.5.0 athlete -> find_view_by_scan fails -> LIVE_ARR=0 (the team gate collapses).
unsafe fn valid_ps_elem(elem: usize) -> bool {
    if read_str_try(elem + 0x420).is_some() {
        return true;
    } // the correct 0.5.0_3 position
    let mut o = 0x388usize; // fallback (for older versions / layout variants)
    while o <= 0x3b0 {
        if read_str_try(elem + o).is_some() {
            return true;
        }
        o += 8;
    }
    false
}
static CAP_MATCH_DONE: AtomicBool = AtomicBool::new(false);
static CAP_MPID: AtomicU64 = AtomicU64::new(0);
static CAP_MTID: AtomicU64 = AtomicU64::new(0);
static INJ_LOG: Mutex<Vec<(Vec<u8>, u8, u64)>> = Mutex::new(Vec::new());

/// Frames between retries of a hook install that has not succeeded.
const INSTALL_RETRY_FRAMES: u64 = 60;

/// Whether an installer that is not in the success state should try again this
/// frame.
///
/// A *failed* install is not free, and it used to run on every frame forever.
/// `install_detour_generic` takes the loader lock (module base) and the
/// address-space lock (`readable`) before it can even look at the prologue —
/// the same two calls that were measured at >=106us per frame and taken out of
/// `install_launcher_hook` on 2026-07-22. Only that one installer got the
/// treatment; the others kept retrying every frame, and their early-out is
/// `== 1`, so anything that fails pays full price forever.
///
/// That is the *expected* state after a game update, not an edge case: every RVA
/// in this module is version-specific, so one that has not been re-derived yet
/// fails the prologue check on every frame of every scene, main thread. Which
/// makes the whole mod feel slow while nothing looks broken.
fn install_retry_due(tick: &AtomicU64) -> bool {
    tick.fetch_add(1, Ordering::Relaxed) % INSTALL_RETRY_FRAMES == 0
}

unsafe fn install_detour_generic(
    rva: usize,
    orig_len: usize,
    cap_fn: usize,
    prologue: &[u8],
) -> Result<usize, &'static str> {
    // Cached: the raw `GetModuleHandleW` takes the loader lock, which is half of
    // what made a retrying install cost 106us a frame.
    let base = exe_base_addr();
    if base == 0 {
        return Err("module 0");
    }
    let fn_addr = base + rva;
    if !readable(fn_addr, orig_len + 4) {
        return Err("unreadable");
    }
    // * Chain hooking: if the entry point already holds a foreign mod's hook (movabs rax,tgt; jmp rax = 48 b8 .. ff e0), chain to that foreign stub instead of the original.
    //   When serpen or another mod hooked the same function first (e.g. launcher 0x20588a0) the prologue is overwritten, so skip prologue validation.
    let mut cur = [0u8; 12];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, cur.as_mut_ptr(), 12);
    let foreign_tgt: usize =
        if cur[0] == 0x48 && cur[1] == 0xb8 && cur[10] == 0xff && cur[11] == 0xe0 {
            usize::from_le_bytes(cur[2..10].try_into().unwrap())
        } else {
            0
        };
    let chained = foreign_tgt >= 0x10000;
    // Prologue validation (guards against a wrong RVA) - skipped when chaining (a foreign hook has overwritten the original prologue).
    if !chained {
        for i in 0..prologue.len() {
            if *((fn_addr + i) as *const u8) != prologue[i] {
                return Err("prologue mismatch");
            }
        }
    }
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let stub = VirtualAlloc(0, 256, MEM_CR, RWX);
    if stub == 0 {
        return Err("VirtualAlloc");
    }
    let ret_addr = fn_addr + orig_len;
    // * Bisection: with passthrough=true, register saving and the cap_fn call are both skipped - original instructions + return only.
    //   If that does not crash, the patch/relocation is fine -> the problem is in saving/calling. If it crashes, the patch/orig is the problem.
    if TRAMPOLINE_DEBUG_PASSTHROUGH {
        let mut s: Vec<u8> = Vec::new();
        let mut orig = vec![0u8; orig_len];
        core::ptr::copy_nonoverlapping(fn_addr as *const u8, orig.as_mut_ptr(), orig_len);
        s.extend_from_slice(&orig);
        s.extend_from_slice(&[0x48, 0xb8]);
        s.extend_from_slice(&ret_addr.to_le_bytes());
        s.extend_from_slice(&[0xff, 0xe0]);
        core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
        let mut patch = vec![0x90u8; orig_len];
        patch[0] = 0x48;
        patch[1] = 0xb8;
        patch[2..10].copy_from_slice(&stub.to_le_bytes());
        patch[10] = 0xff;
        patch[11] = 0xe0;
        let mut old: u32 = 0;
        if VirtualProtect(fn_addr, orig_len, RWX, &mut old) == 0 {
            return Err("VirtualProtect");
        }
        core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, orig_len);
        VirtualProtect(fn_addr, orig_len, old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), fn_addr, orig_len);
        return Ok(stub);
    }
    let mut s: Vec<u8> = Vec::new();
    // WARNING do not capture entry_rsp (mov r10,rsp) - the hooked original instructions save r10, so r10 must be preserved.
    //   cap_fn's second argument (entry_rsp) is unused -> rdx is left alone (the original rdx passes through; cap_fn ignores it).
    // push r12 rsi rdi rbx r11 r10 r9 r8 rdx rcx  (rcx last = saved+0; r12 = saved+0x48; r10/r9 keep their originals)
    //   * r12 was added because cap_fn accesses r12 (the personal_tactics match entry, for arming a watchpoint).
    s.extend_from_slice(&[
        0x41, 0x54, 0x56, 0x57, 0x53, 0x41, 0x53, 0x41, 0x52, 0x41, 0x51, 0x41, 0x50, 0x52, 0x51,
    ]);
    s.extend_from_slice(&[0x48, 0x89, 0xe1]); // mov rcx, rsp (saved=arg1)
    s.extend_from_slice(&[0x48, 0x89, 0xe3]); // mov rbx, rsp (alignment-restore holder, preserved across cap_fn)
    s.extend_from_slice(&[0x48, 0x83, 0xe4, 0xf0]); // and rsp, -16 (16-byte alignment fix for a mid-function entry)
    s.extend_from_slice(&[0x48, 0x83, 0xec, 0x20]); // sub rsp, 0x20 (shadow)
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&cap_fn.to_le_bytes()); // movabs rax, cap_fn
    s.extend_from_slice(&[0xff, 0xd0]); // call rax
    s.extend_from_slice(&[0x48, 0x89, 0xdc]); // mov rsp, rbx (restore alignment)
                                              // pop rcx rdx r8 r9 r10 r11 rbx rdi rsi r12  (reverse of the pushes)
    s.extend_from_slice(&[
        0x59, 0x5a, 0x41, 0x58, 0x41, 0x59, 0x41, 0x5a, 0x41, 0x5b, 0x5b, 0x5f, 0x5e, 0x41, 0x5c,
    ]);
    if chained {
        // * Chaining: do not run the original prologue -> jump to the foreign mod's stub (cur = movabs rax,foreign_tgt; jmp rax).
        //   The foreign stub handles its own capture + the original prologue + returning to fn+0xc. Clobbering rax is harmless (the original mov eax resets it).
        s.extend_from_slice(&cur); // = 48 b8 <foreign_tgt> ff e0
    } else {
        let mut orig = vec![0u8; orig_len]; // copy the original (position-independent) instructions to execute
        core::ptr::copy_nonoverlapping(fn_addr as *const u8, orig.as_mut_ptr(), orig_len);
        s.extend_from_slice(&orig);
        s.extend_from_slice(&[0x48, 0xb8]);
        s.extend_from_slice(&ret_addr.to_le_bytes()); // movabs rax, ret_addr
        s.extend_from_slice(&[0xff, 0xe0]); // jmp rax
    }
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    // Patch: movabs rax, stub; jmp rax (12B) + NOP padding
    let mut patch = vec![0x90u8; orig_len];
    patch[0] = 0x48;
    patch[1] = 0xb8;
    patch[2..10].copy_from_slice(&stub.to_le_bytes());
    patch[10] = 0xff;
    patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, orig_len, RWX, &mut old) == 0 {
        return Err("VirtualProtect");
    }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, orig_len);
    VirtualProtect(fn_addr, orig_len, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, orig_len);
    Ok(stub)
}

// * The rax-preserving variant of install_detour_generic (0.5.2 SPAWN only).
//   The generic tail is `movabs rax, ret_addr; jmp rax`, so if the relocated region contains `mov eax,imm` (a chkstk frame size)
//   that value is overwritten and chkstk runs away -> here the tail becomes `movabs r11, ret_addr; jmp r11` to preserve rax.
//   (r11 = an x64 volatile scratch register with no meaningful value at function entry = safe to clobber.)
//   The chain-hooking branch is unsupported (SPAWN is not a hook shared with other mods) - Err on detecting a foreign hook.
unsafe fn install_detour_r11(
    rva: usize,
    orig_len: usize,
    cap_fn: usize,
    prologue: &[u8],
) -> Result<usize, &'static str> {
    // Cached — see `install_detour_generic`.
    let base = exe_base_addr();
    if base == 0 {
        return Err("module 0");
    }
    if orig_len < 12 {
        return Err("orig_len<12");
    } // the entry patch is 12B (movabs+jmp)
    let fn_addr = base + rva;
    if !readable(fn_addr, orig_len + 4) {
        return Err("unreadable");
    }
    if *(fn_addr as *const u8) == 0x48 && *((fn_addr + 1) as *const u8) == 0xb8 {
        return Err("foreign hook");
    }
    for i in 0..prologue.len() {
        if *((fn_addr + i) as *const u8) != prologue[i] {
            return Err("prologue mismatch");
        }
    }
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let stub = VirtualAlloc(0, 256, MEM_CR, RWX);
    if stub == 0 {
        return Err("VirtualAlloc");
    }
    let ret_addr = fn_addr + orig_len;
    let mut s: Vec<u8> = Vec::new();
    // push r12 rsi rdi rbx r11 r10 r9 r8 rdx rcx (same layout as generic = compatible saved indices)
    s.extend_from_slice(&[
        0x41, 0x54, 0x56, 0x57, 0x53, 0x41, 0x53, 0x41, 0x52, 0x41, 0x51, 0x41, 0x50, 0x52, 0x51,
    ]);
    s.extend_from_slice(&[0x48, 0x89, 0xe1]); // mov rcx, rsp
    s.extend_from_slice(&[0x48, 0x89, 0xe3]); // mov rbx, rsp
    s.extend_from_slice(&[0x48, 0x83, 0xe4, 0xf0]); // and rsp, -16
    s.extend_from_slice(&[0x48, 0x83, 0xec, 0x20]); // sub rsp, 0x20
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&cap_fn.to_le_bytes());
    s.extend_from_slice(&[0xff, 0xd0]); // call rax
    s.extend_from_slice(&[0x48, 0x89, 0xdc]); // mov rsp, rbx
    s.extend_from_slice(&[
        0x59, 0x5a, 0x41, 0x58, 0x41, 0x59, 0x41, 0x5a, 0x41, 0x5b, 0x5b, 0x5f, 0x5e, 0x41, 0x5c,
    ]);
    let mut orig = vec![0u8; orig_len];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, orig.as_mut_ptr(), orig_len);
    s.extend_from_slice(&orig); // re-execute the original instructions (this is where rax = frame size is set)
    s.extend_from_slice(&[0x49, 0xbb]);
    s.extend_from_slice(&ret_addr.to_le_bytes()); // movabs r11, ret_addr
    s.extend_from_slice(&[0x41, 0xff, 0xe3]); // jmp r11  (rax preserved)
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    let mut patch = vec![0x90u8; orig_len];
    patch[0] = 0x48;
    patch[1] = 0xb8;
    patch[2..10].copy_from_slice(&stub.to_le_bytes());
    patch[10] = 0xff;
    patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, orig_len, RWX, &mut old) == 0 {
        return Err("VirtualProtect");
    }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, orig_len);
    VirtualProtect(fn_addr, orig_len, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, orig_len);
    Ok(stub)
}

// ===========================================================================
//  SDK lifecycle
// ===========================================================================

// (Was `impl ModExtension for ItemTacticsExt`. Driven from the host mod's
// `StableExtension::post_update` — see `driver` and `src/lib.rs`.)
//
// Two parameters changed shape in the move to the stable ABI:
//   * `scene: &mut Scene` -> `in_game: bool` plus the `StableClient` itself,
//     which answers what `Scene::InGame { data }.db()` used to;
//   * `ui: &mut GameUI` -> `driver::ui_root()`, the root `Node` captured by the
//     UI mega-function detour. The `ui` binding is now the root node itself, so
//     the field access that used to reach it is gone from every call below.
// `_assets` and `_dt` were unused and are gone.
fn tactics_post_update(client: &StableClient<'_>, in_game: bool) {
    {
        // The hook retry block below is what installs `cap_game_view`, and
        // `cap_game_view` is what publishes `TIP_ROOT` — so the root must be
        // fetched AFTER it, never as an early guard at the top of the function.
        // Guarding first would mean the hook is never installed, the root is
        // never captured, and every frame returns early forever.
        {
            install_launcher_hook();
            install_seed_ctor_hook();
            install_spawn_hook();
            install_game_view_hook();
        }
        // Validated `GameUI.root`, or 0 until the UI exists. NOT `TIP_ROOT` —
        // see `ui_root` for why that pointer crashed the game.
        let ui_root_ptr = ui_root::resolve().unwrap_or(0);
        // * 0.5.3 regression diagnostic dump (build extension path) - file write every 300 frames (~5s), main thread only.
        if BUILD_EXT_DIAG {
            let n = BE_TICK.fetch_add(1, Ordering::Relaxed);
            if n % 300 == 0 {
                let c: Vec<u64> = BE_CNT.iter().map(|a| a.load(Ordering::Relaxed)).collect();
                let last = BE_LAST.load(Ordering::Relaxed);
                let mut s = format!(
                    "[build extension path diagnostic]\n\
                     reached (entered the 4th-item path) = {}\n\
                     |- skipped, build_len != 3    = {}\n\
                     |- skipped, build_cap != 3    = {}\n\
                     |- ptr/writable failed        = {}\n\
                     |- could not get target index = {}\n\
                     |- realloc failed             = {}\n\
                     \\- SUCCESS (build[3] write)   = {}\n\
                     ACTUALLY BOUGHT: owned>=4 observed = {} times / max owned observed = {}\n\
                       (0 means it really is not bought; non-zero means it IS bought and only the in-match icon is missing)\n\
                     last observed: build_len={} build_cap={} / last target index={}\n\
                     [slot3 icon] set OK={} skipped={} / GameView={:#x}(hits {}) view-model owns a 4th={} players\n\
                     [slot UI surgery] {}\n\
                     note: mode(slot_count)={} - MY_ATHLETES={} - LIVE_SEED={:#x} - buy write successes={}\n",
                    c[0], c[1], c[2], c[3], c[4], c[5], c[6],
                    c[7], BE_MAX_OWNED.load(Ordering::Relaxed),
                    last >> 32, last & 0xffff_ffff, BE_LAST_T.load(Ordering::Relaxed),
                    SLOT3_ICON_N.load(Ordering::Relaxed), SLOT3_ICON_MISS.load(Ordering::Relaxed),
                    GAME_VIEW.load(Ordering::Relaxed), GV_HITS.load(Ordering::Relaxed),
                    SLOT3_PV_N.load(Ordering::Relaxed),
                    SLOTUI_MSG.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| "(not run)".into()),
                    slot_count(), MY_ATH_N.load(Ordering::Relaxed),
                    LIVE_SEED.load(Ordering::Relaxed), BUY_WROTE_FIRE.load(Ordering::Relaxed));
                // Hook install state. Added while diagnosing "never buys a 4th
                // item": every one of these reports through `append_log`, which
                // `LOG_ENABLED = false` discards, so a hook that failed to
                // install and a hook that installed and never fired produced
                // exactly the same evidence — none. `BUY_PROBE_INSTALLED = 2`
                // in particular means the `buy_item` detour is absent, which
                // makes every counter above read 0 no matter what else is right.
                let state = |v: u64| match v {
                    0 => "not attempted",
                    1 => "OK",
                    2 => "FAILED (signature mismatch)",
                    _ => "FAILED (install error)",
                };
                let buy_note = BUY_INSTALL_NOTE
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                s.push_str(&format!(
                    "\n-- hook install state --\n  \
                     buy_item detour (install_replace_4th) : {}\n  \
                     launcher (LIVE_SEED source)           : {}  fired={} render={} seed={:#x}\n  \
                     seed_ctor                             : {}  provider={:#x}\n  \
                     spawn                                 : {} (SPAWN_INJECT_ENABLED={SPAWN_INJECT_ENABLED}, so \"not attempted\" is expected)\n  \
                     VEH registered (gates every safe_read): {}\n  \
                     game_view (TIP_ROOT/GameView)         : {}\n  \
                     launcher install path: calls={} ours={} waited={} real_installs={} throttled={} last_b0={:#04x} last_movabs={:#x}\n  \
                     Database (from riot item-build hook)  : {:#x}   mod items={}  finals={}\n  \
                     mod item source: {}\n",
                    state(BUY_PROBE_INSTALLED.load(Ordering::Relaxed)),
                    state(CLAUNCH_INSTALLED.load(Ordering::Relaxed)),
                    LAUNCH_N.load(Ordering::Relaxed), LAUNCH_RENDER_N.load(Ordering::Relaxed),
                    LIVE_SEED.load(Ordering::Relaxed),
                    state(SEEDCTOR_INSTALLED.load(Ordering::Relaxed)),
                    RENDER_PROVIDER.load(Ordering::Relaxed),
                    state(SPAWN_INSTALLED.load(Ordering::Relaxed)),
                    if SEH_INSTALLED.load(Ordering::Relaxed) { "OK" } else { "NO - every safe_read fails" },
                    state(GV_HOOK_INSTALLED.load(Ordering::Relaxed)),
                    HK_L_CALLS.load(Ordering::Relaxed), HK_L_OURS.load(Ordering::Relaxed),
                    HK_L_WAIT.load(Ordering::Relaxed), HK_L_INSTALL.load(Ordering::Relaxed),
                    HK_L_SKIP.load(Ordering::Relaxed),
                    HK_L_B0.load(Ordering::Relaxed), HK_L_TGT.load(Ordering::Relaxed),
                    driver::db_addr(),
                    MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner()).len(),
                    MOD_FINALS.lock().unwrap_or_else(|e| e.into_inner()).len(),
                    {
                        let note = CATALOG_NOTE.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        if note.is_empty() { "NOT POPULATED (4th item can only be vanilla)".to_string() } else { note }
                    },
                ));
                if !buy_note.is_empty() {
                    s.push_str(&format!("  buy_item detail: {buy_note}\n"));
                }
                // What the Builds editor and the item-build hook's
                // `usable = build.len().min(picker_slots())` are working from.
                // `driver::picker_slots` reports 3 unless `tactics_init`
                // returned true, so this differing from `mode` above means the
                // version gate failed and no patch is in: the symptom on screen
                // is a 3-slot layout, not a 4th slot nothing ever fills.
                s.push_str(&format!("  picker slots: {}\n", driver::picker_slots()));
                // Whether the loader hook ever delivered the 4-slot templates.
                // These used to arrive through `mod.override_info` as well,
                // which cannot miss; the hook can, if a template is loaded
                // before `uinj::install` runs and is then served from cache.
                let (inst, pi, wide, strat) = uinj::inject_state();
                s.push_str(&format!(
                    "  ui_inject: installed={inst} player_info={pi} wide={wide} strategy={strat}\n"
                ));
                s.push_str(&format!("  {}\n", ui_root::report()));
                if let Some(d) = mod_dir() {
                    let _ = fs::write(d.join("build_ext_diag.txt"), s);
                }
            }
        }
        // (the every-frame hook retry that used to sit here now runs at the top
        //  of the function, because it is what publishes `TIP_ROOT`)
        if !in_game {
            INGAME_NOW.store(false, Ordering::Relaxed);
        }
        if UI_INJECT_ENABLED {
            unsafe {
                let _ = uinj::install();
            }
        } // strategy screen dropdown injection hook (mode 3 = item0m/1m/2m, mode 4 = + item3/slot3). Idempotent.
          // NOTE the UI-root gate is NOT here. It used to be, and that broke the
          // fourth item: the block below publishes `MY_ATHLETES`, which is the
          // team gate's only remaining input now that the `SCENE_SIDE` fast path
          // is off. Gating it made `is_my_athlete` return `None` forever, so the
          // gate closed on the safe side and nothing was ever injected. This block
          // needs the `client`, not the node tree — only `force_blue_slot_spacing`
          // inside it touches the tree, and it is gated individually.
          // * Capture the player team id (for team scoping) + the personal_tactics snapshot (for restoring the display).
          //   WARNING the strategy screen may not be InGame, so the #personal visible gate was removed -> fill it in ahead of time on the management screen.
          //   Throttled to every 20 frames (cuts the cost of walking the HashMap).
          // (was `if let Scene::InGame { data } = scene`)
          //
          // `data.db()` returned `mod_api::ClientDatabase` — the *client* scene's
          // database, a different object from the `game_core::Database` that
          // `probe_db` works on, and not something a stable-ABI mod can be handed.
          // The three things this block read off it are read from the stable
          // client instead; `stable_team_ids` and `stable_personal_tactics` are
          // the JSON-record equivalents of `team.last_starting` and
          // `team.champion_personal_tactics`.
        if let (true, Some(pid)) = (in_game, client.player_team_id()) {
            // * During a match player_team_id() returns 0/-1 -> store only when in the valid range (1~9999), otherwise keep the last valid value.
            //   My team id is constant during a session, so the value captured on the management/pre-match screen is used during the match too.
            // * pid=0 is valid too (the team id space starts at 0 - measured: db.team(0)=Some, 5 PT entries). Only -1 (u64::MAX) is invalid.
            // ** 2026-07-30 defect fix - prevent pid **regression**.
            //   The old comment judged "pid=0 is valid too (the team id space starts at 0, db.team(0)=Some)", but measurement showed
            //   the same save alternating between **105 and 0** depending on the moment (via the management screen = 105; straight into comp test
            //   right after starting the game = 0). Trusting the 0 publishes team(0).last_starting=[0,1,2,3,4] as my team and
            //   breaks the team gate => **once a valid non-zero pid has been seen, never fall back to 0.**
            //   (0 itself is not forbidden, since a save whose real team id is 0 may exist - 0 is used until a non-zero is seen.)
            // ** 2026-07-30 measurement addendum - **do not update pid during a comp-test match.**
            //   pid is only read under `Scene::InGame` (on the management screen this block does not run at all, so there is no
            //   chance to correct it), and comp test is also InGame while that screen has no notion of team membership, so
            //   `player_team_id()` **returns 0**. Publishing that 0 makes team(0).last_starting=[0,1,2,3,4] my team.
            //   => ignore 0 reports during comp test and keep the value captured in a normal match.
            let in_comptest = COMPTEST_MATCH.load(Ordering::Relaxed);
            let pu = pid as u64;
            // * Diagnostic: pid observation history. ** Confirmed by measurement (2026-07-30) - **from the user's point of view comp test is a
            //   background brief-sim, but under the SDK `Scene` enum it is `InGame`** (proved by LIVE_DB != 0, i.e. this block did run),
            //   and `player_team_id()` in that context returns **0**. That is the source of the pid=0 publications.
            // * Second extension (2026-07-30 measurement): `COMPTEST_MATCH` is only true **after the comp-test sim starts** (after the launcher
            //   fires), so 0 reports from the window **between entering the comp-test screen and the sim starting** leaked through and got published
            //   (measured: of 2416 observations of 0, only 1592 were blocked and the remaining 824 came from that window). => while the comp-test popup
            //   is open (`CT_OPEN`) treat it as the same context and ignore 0 as well.
            let ct_ctx = in_comptest || CT_OPEN.load(Ordering::Relaxed);
            if pu == 0 {
                PID_OBS_ZERO.fetch_add(1, Ordering::Relaxed);
                if ct_ctx {
                    PID_SKIP_CT.fetch_add(1, Ordering::Relaxed);
                } else {
                    PID_ZERO_CLEAN.fetch_add(1, Ordering::Relaxed);
                } // an observation of 0 unrelated to comp test
            } else if pu != u64::MAX && pu < 10000 {
                PID_OBS_NONZERO.fetch_add(1, Ordering::Relaxed);
            }
            if pu != u64::MAX && pu < 10000 && !(pu == 0 && ct_ctx) {
                if pu != 0 {
                    PLAYER_TEAM_ID.store(pu, Ordering::Relaxed);
                    PID_NONZERO_SEEN.store(1, Ordering::Relaxed);
                } else if PID_NONZERO_SEEN.load(Ordering::Relaxed) == 0 {
                    PLAYER_TEAM_ID.store(0, Ordering::Relaxed);
                }
                PID_EVER_VALID.store(1, Ordering::Relaxed);
            }
            // ** v15: publish my team's starting roster (5 athlete_ids) - the material for the spawn hook's scene-free team decision.
            //   Refreshed on the ROSTER_POLL period (transfers and lineup changes are picked up automatically). Once obtained, a low rate is plenty.
            {
                const ROSTER_POLL: u64 = 120; // frames
                let n = ROSTER_TICK.fetch_add(1, Ordering::Relaxed);
                let known = PLAYER_TEAM_ID.load(Ordering::Relaxed);
                if n % ROSTER_POLL == 0 && known != u64::MAX && known < 10000 {
                    // (was `db.team(known).last_starting` / `.champion_personal_tactics`)
                    let my = stable_last_starting(client, known as usize);
                    let pt_n = stable_personal_tactics(client, known as usize).len();
                    MY_PT_N.store(pt_n as u64, Ordering::Relaxed);
                    // NO **PT-count cross-check abandoned (refuted by measurement 2026-07-30)**: based on an old note that "my team has dozens of PT entries
                    //   while an AI team has only a few (team(0) has 5)", `pt_n >= 20` was used, but measurement showed
                    //   **team(0) PT = 95**, which passes the threshold meaninglessly => the PT count has no discriminating power.
                    //   (pt_n is kept for diagnostic display only.)
                    // * Replacement rule: `pid=0` is **treated as undetermined and withheld by default** (withheld = is_my_athlete returns None
                    //   = the team gate closes on the safe side). But if 0 has been observed **long enough (600 ticks, ~10s) in an InGame unrelated to
                    //   comp test**, accept it as a genuine team-id-0 save and publish.
                    //   => it is never published in a comp-test-only session, and playing a normal match captures the real pid.
                    let trust = known != 0 || PID_ZERO_CLEAN.load(Ordering::Relaxed) >= 600;
                    if !my.is_empty() && trust {
                        publish_my_athletes(my);
                    } else if !trust {
                        MY_TRUST_SKIP.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            // ** lean (07-18): spectate identification = launcher (LIVE_SEED) + seed-ctor (RENDER_PROVIDER) + buy r9 comparison (v13).
            //   The old db scan (v10), P6 probe and link scan are all gone. Only the scene side (my-team decision) and LIVE_DB/PID remain here.
            if !DIAG_BUY_OFF {
                INGAME_NOW.store(true, Ordering::Relaxed);
                {
                    let pu = PLAYER_TEAM_ID.load(Ordering::Relaxed);
                    if pu != u64::MAX && pu < 10000 {
                        LIVE_PID.store(pu, Ordering::Relaxed);
                    }
                }
                // MERGE GAP — the direct scene read is off.
                //
                // `LIVE_DB` was the `ClientDatabase` pointer, and `quick_scene_side`
                // reads the live scene's team ids straight out of it (+0x1338 tag,
                // +0x17A0/+0x17C0 team tags, +0x1900 is_team1_blue) to decide which
                // sim side is the player's. A stable-ABI mod is never handed that
                // pointer, and unlike the `Database` there is no argument anywhere
                // in this mod that leaks it, so `LIVE_DB` stays 0 and `SCENE_SIDE`
                // stays undetermined.
                //
                // That is a documented, supported state rather than a break:
                // `scene_player_side()` returning `None` means "use the roster
                // fallback", and the roster (`MY_ATHLETES`, published just above
                // from the stable record API) is what the team gate then uses. The
                // cost is the fast path — the spawn hook's early side decision,
                // which existed to cover the owned=0 injection window.
                //
                // Restoring it needs the `ClientDatabase` address from somewhere:
                // another detour argument, or a fixed offset off the `App` pointer
                // that `cap_game_view` already captures.
            }
            // * Force blue slot/stat x (+0x84, 4 states) every frame: the game resets blue_player to vanilla 50px spacing, and we overwrite that with 42px spacing + left alignment.
            //   WARNING compact (player_info) only - wide_player_info (fullscreen) has no reset and is correct from the .ui (34px spacing, different kda/cs)
            //   Applying it to the whole ui.root would overwrite wide with the compact values (42/242/290) and break it. Restricted to the player_info subtree.
            if ITEM_MODE.load(Ordering::Relaxed) == 4
                && UI_TREE_WALK_ENABLED
                && ui_root_ptr > 0x10000
            {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                    let ui: &Node = &*(ui_root_ptr as *const Node);
                    if let Some(pi) = find_node(ui, "player_info") {
                        force_blue_slot_spacing(pi);
                    }
                }));
            }
            // * Source of the delegate (tfm2.gg auto-selection) baseline = champion_personal_tactics.
            //   Refreshed every frame (lightweight, ~52 entries) -> display/injection always current. Only the log is throttled to every 20 frames.
            //   After refreshing, OVERRIDE_SNAPSHOT is rebuilt (signature guard -> no leak when unchanged) ->
            //   so the delegate direction reaches the c6 injection even without opening the strategy screen.
            // * Throttle (2026-07-22 perf measurement): this block cost **at least 174us** every InGame frame - the comment above claiming
            //   "lightweight, ~52 entries" was wrong (per-champion String clones + HashMap rebuild + update_override_snapshot's
            //   map build / sort / FNV, every frame). The delegate (champion_personal_tactics) only changes **through user action**,
            //   so there is no reason to rebuild it every frame -> every 20 frames (~0.3s). No perceptible difference in display/injection responsiveness.
            static PT_REBUILD: AtomicU64 = AtomicU64::new(0);
            if PT_REBUILD.fetch_add(1, Ordering::Relaxed) % 20 == 0 {
                {
                    // WARNING `t` here is the team (bound above), so the probe uses a separate variable name
                    // (was `db.team(pid).champion_personal_tactics`, a
                    //  `HashMap<String, [u8; 3]>` read field-by-field)
                    let snap = stable_personal_tactics(client, pid);
                    *PT_SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(snap);
                    update_override_snapshot();
                }
            }
        }
        // Everything from here down walks the live UI node tree. This is the
        // only thing `UI_TREE_WALK_ENABLED` is meant to cover — see its doc
        // comment for why it is off and what that costs.
        if !UI_TREE_WALK_ENABLED || ui_root_ptr <= 0x10000 {
            return;
        }
        let ui: &mut Node = unsafe { &mut *(ui_root_ptr as *mut Node) };
        {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handle_tactics_screen(ui);
            }));
        }
        {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handle_comptest_screen(ui);
            }));
        }
        // * In-match 4th slot icon (direct node writing - no game code modification)
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handle_ingame_slot3(ui);
        }));
        // * Hide native item0/1/2 (the mod-owned item0m/1m/2m overlay replaces them). Only on the personal tactics screen. Common to modes 3 and 4 (the overlay exists in both).
        {
            let _ =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hide_native_item_dds(ui)));
        }
        // * Comp test: mod-owned dropdowns replace them, so hide native item0/1/2 (idempotent - no write if already false).
        {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if find_node(ui, "builds").map(|n| n.visible).unwrap_or(false) {
                    hide_comptest_native_dds(ui);
                }
            }));
        }
    }
}

// Server side: access the Database and fill the mod item registry once.
// (Was `impl ModServerExtension for ItemTacticsServerExt`. Driven from the host
// mod's `StableServerExtension` — see `driver` and `src/lib.rs`.)
fn tactics_on_server_start() {
    probe_db();
    install_replace_4th();
    install_launcher_hook();
    install_seed_ctor_hook();
    install_spawn_hook();
} // resolver = common to modes 3 and 4 (slot 0/1/2 designation) + the v13 identification hooks (launcher seed + seed-ctor provider)

fn tactics_before_management_tick() {
    // * Reset the team gate cache between matches (management screen) -> re-scan the roster next match (in case addresses are reused).
    //   The management tick does not run during a match sim, so there is no race with the sim thread's decisions.
    PLAYER_SIDE.store(u64::MAX, Ordering::Relaxed);
    SIDE_CACHE.lock().unwrap_or_else(|e| e.into_inner()).clear();
    probe_db();
    install_replace_4th(); // resolver = common to modes 3 and 4 (idempotent)
}
static NETSCAN_DONE: AtomicBool = AtomicBool::new(false);
/// Whether `a` looks like the item recommendation network: header
/// `16384 / 16384 / 1` **and** a weight pointer that is actually readable.
///
/// * Signature hardened for 0.5.1 (ghidra-re): a lookalike matching only the header (16384/16384/1) at db+0xd30 has a dangling weight ptr at +0x8
///   -> AV when dereferenced at +0x44a inside forward. Adding a readable check on the weight ptr rejects the fake and passes only the real net (db+0x1558).
///
/// Lifted out of `probe_db` (was a local closure) so `driver::record_item_net`
/// can apply the same test to the agent the host's item-build detour is handed.
unsafe fn itemnet_header_ok(a: usize) -> bool {
    readable(a, 0x20)
        && rd_u64(a) == 16384
        && rd_u64(a + 0x10) == 16384
        && rd_u64(a + 0x18) == 1
        && {
            let w = rd_u64(a + 0x8) as usize;
            w >= 0x10000 && readable(w, 16384 * 4)
        }
}

// ===========================================================================
//  Stable-ABI replacements for the two `ClientDatabase` reads
// ===========================================================================
// `Scene::InGame { data }.db()` gave a `mod_api::ClientDatabase`, whose `team()`
// returned a struct these two fields were read straight off. The stable client
// exposes the same management records as JSON documents instead, so the shape
// of the answer is unchanged and only the route to it differs.
//
// Both are called from throttled paths (the roster every 120 frames, the
// tactics snapshot every 20), which is what makes a JSON round-trip per call
// acceptable where a field read was before.

/// Athlete ids of `team_id`'s starting five — was `team.last_starting`.
///
/// That field is `[Option<usize>; 5]`, so the JSON has nulls in it for an
/// incomplete lineup; those slots are skipped exactly as the `if let Some(aid)`
/// did. An empty set means "could not read it", which the caller already
/// handles by not publishing (`!my.is_empty()`).
fn stable_last_starting(
    client: &StableClient<'_>,
    team_id: usize,
) -> std::collections::HashSet<u64> {
    let mut out = std::collections::HashSet::new();
    let Some(json) = client.record_get_json(RecordKindV1::Team, team_id, "last_starting") else {
        return out;
    };
    let Some(JsonValue::Arr(slots)) = JsonParser::new(&json).parse_value() else {
        return out;
    };
    for slot in slots {
        if let JsonValue::Num(id) = slot {
            if id >= 0.0 {
                out.insert(id as u64);
            }
        }
    }
    out
}

/// Per-champion vanilla item categories — was `team.champion_personal_tactics`,
/// a `HashMap<String, [u8; 3]>`.
///
/// Three entries per champion, one per vanilla item slot. A champion whose
/// array is short or malformed is dropped rather than padded: this map is the
/// baseline the delegate/injection compares against, and inventing a zero there
/// would read as "the user chose category 0".
fn stable_personal_tactics(client: &StableClient<'_>, team_id: usize) -> HashMap<String, [u8; 3]> {
    let mut out = HashMap::new();
    let Some(json) =
        client.record_get_json(RecordKindV1::Team, team_id, "champion_personal_tactics")
    else {
        return out;
    };
    let Some(JsonValue::Obj(entries)) = JsonParser::new(&json).parse_value() else {
        return out;
    };
    for (champion, value) in entries {
        let JsonValue::Arr(categories) = value else {
            continue;
        };
        if categories.len() < 3 {
            continue;
        }
        let mut slots = [0u8; 3];
        let mut ok = true;
        for (slot, category) in slots.iter_mut().zip(categories.iter()) {
            match category {
                JsonValue::Num(n) if *n >= 0.0 && *n <= u8::MAX as f64 => *slot = *n as u8,
                _ => ok = false,
            }
        }
        if ok {
            out.insert(champion, slots);
        }
    }
    out
}

/// Was `probe_db(ctx: &mut ServerModContext)`, which read the `Database` base as
/// `&ctx.database.champion_patch_statistics - 0x16698`. A stable-ABI mod never
/// sees that object, so the base now arrives from `driver::db_addr()` and this
/// no-ops until the host's item-build detour has settled it.
fn probe_db() {
    let db = driver::db_addr();
    if db == 0 {
        return;
    }
    // -- Item neural network probe + self-validation (16384/16384/1) --
    //   * Measured on 0.5.0_3: db+0xd30 (moved -0x70 from the old 0xda0; the netscan diagnostic hit it). Sequential candidates + a window scan fallback (patch-robust).
    if ITEM_NET_ADDR.load(Ordering::Relaxed) == 0 {
        unsafe {
            let sig_ok = |a: usize| itemnet_header_ok(a);
            let mut found = 0usize;
            for &off in &[0x1558usize, 0xd30, 0xda0] {
                // * 0.5.1: prefer the game's real net = GameData+0x1558 (ghidra-re confirmed, identical in both versions). db == the GameData base.
                if sig_ok(db + off) {
                    found = db + off;
                    break;
                }
            }
            if found == 0 {
                // automatic window search (self-heals if it moves again in a future patch)
                let mut o = 0usize;
                while o < 0x18000 {
                    let a = db + o;
                    if sig_ok(a) {
                        found = a;
                        break;
                    }
                    o += 8;
                }
            }
            if found != 0 {
                ITEM_NET_ADDR.store(found as u64, Ordering::Relaxed);
            } else {
                let net = db + 0xda0;
                // * Diagnostic (regardless of LOG): +0xda0 failed -> scan a wide window from db for the net signature (16384/*/16384/1) to find the real offset.
                //   + also dump the forward RVA prologue (to distinguish itemnet_addr_valid failure causes). Only once.
                // The `cps` (champion_patch_statistics @ db+0x16698) figure is gone
                // from this dump: it was how the classic build *derived* db, and the
                // merged build derives db from the item network instead — so there
                // is no second, independent address left to cross-check against.
                if !NETSCAN_DONE.swap(true, Ordering::Relaxed) {
                    let mut out = format!("db={:#x} (from item-build hook agent - 0x1558)\n net@+0xda0={:#x} sig=({},{},{}) readable={}\n",
                        db, net,
                        if readable(net,0x20){rd_u64(net) as i64}else{-1}, if readable(net,0x20){rd_u64(net+0x10) as i64}else{-1},
                        if readable(net,0x20){rd_u64(net+0x18) as i64}else{-1}, readable(net,0x20));
                    // Scan 0..0x18000 from db: rd(O)==16384 && rd(O+0x10)==16384 && rd(O+0x18)==1
                    let mut hits = 0;
                    let mut o = 0usize;
                    while o < 0x18000 && hits < 8 {
                        let a = db + o;
                        if readable(a, 0x20)
                            && rd_u64(a) == 16384
                            && rd_u64(a + 0x10) == 16384
                            && rd_u64(a + 0x18) == 1
                        {
                            out.push_str(&format!(" ★HIT db+{:#x} (abs={:#x})\n", o, a));
                            hits += 1;
                        }
                        o += 8;
                    }
                    if hits == 0 {
                        out.push_str(" (scan found nothing - suspect the db base itself, or a changed signature)
");
                    }
                    // forward RVA prologue
                    let fa = exe_base_addr() + ITEMNET_FORWARD_RVA;
                    if readable(fa, 12) {
                        let pb: Vec<String> = (0..12)
                            .map(|i| format!("{:02x}", *((fa + i) as *const u8)))
                            .collect();
                        out.push_str(&format!(
                            " fwd RVA={:#x} prologue={} (expected 55415741...)
",
                            ITEMNET_FORWARD_RVA,
                            pb.join(" ")
                        ));
                    } else {
                        out.push_str(" fwd RVA unreadable\n");
                    }
                    if TRACE_FILES {
                        if let Some(d) = mod_dir() {
                            let _ = fs::create_dir_all(&d);
                            let _ = fs::write(d.join("4items_netscan.txt"), out);
                        }
                    }
                }
            }
        }
    }
    if MODITEMS_DONE.load(Ordering::Relaxed) {
        return;
    }
    unsafe {
        dump_mod_items(db);
    }
    driver::mark_db_probed();
}

// == athlete -> champion mapping probe (scanning buy_item's r8 = athlete) =====================
// ** 0.5.4 re-derivation (2026-08-04) - `tools/rederive.py sig`, no old exe available (see that file's header).
//   The mod relocates 19B of this entry, so its exact opening was already known and became the search key:
//   `41 57 41 56 56 57 53 48 83 EC 50` (5 push + sub 0x50) + `48 8B 84 24 A8 00 00 00` (mov rax,[rsp+0xa8] = arg6).
//   **Exactly 1 hit in .text: 0xe767e0, a .pdata function start (size 230).**
//   Every term of the documented argument contract is visible in its first 40 bytes:
//     mov rax,[rsp+0xa8]          -> arg6 = Game            (what the detour reads as rsp_entry+0x30)
//     cmp qword [r8+0x490],0      -> **r8 = athlete**, +0x490 = its build Vec (the recorded athlete layout)
//     mov r15,[rax+0x30]          -> Game+0x30 = catalog
//     mov rsi,[r15+8] / rdi,[r15+0x10] -> catalog Vec ptr/len
//     shl rax,4; mov rcx,[rsi+rax]; mov rax,[rsi+rax+8] -> the 16B element {elem_ptr@0, vtable@8}
//     call [rax+0x70]             -> vtable dispatch, same family as the recorded name@0x50 / recipe@0x68
//   The 19B relocation boundary is unchanged: the instruction after the mov starts at 0xe767f3 = entry+19.
const RVA_BUY_ITEM: usize = 0xe767e0; // 0.5.4 (0.5.3 was 0xd0c680). History for 0.5.3 follows.(0.5.2 was 0x211e070). **The first 24B of the entry are byte-identical** (a single unique hit in the whole exe) + the body is instruction-for-instruction isomorphic + the argument contract is unchanged (r8=athlete, [rsp_entry+0x30]=Game, Game+0x30=catalog). orig_len=19 is unchanged too (11B < 12B -> the next clean boundary is the 8B mov rax,[rsp+0xa8]). WARNING 0.5.3 change: the call path became a vtable (+0x78) thunk 0xd22340 instead of a direct call, but **since we hook the function entry, every call is still caught**. History for 0.5.2 follows. (0.5.1 was 0x1f01090; exe2exe skeleton UNIQUE, the 24B prologue completely identical = body unchanged, delta +0x21cfe0.) History for 0.5.1 follows: the function was heavily reworked (8 push/sub 0x38 -> 5 push/sub 0x50, with build/name comparison split out into the subfunction 0x1f00920) so mask-sig was NONE, but it was confirmed by the unchanged argument contract (r8=athlete, p6=Game@rsp_entry+0x30, Game+0x30=catalog). Cross-checked against the buy driver FUN_142234430 (successor to the old FUN_1420e76e0) + the vtable slot.
const BUY_PROLOGUE: [u8; 12] = [
    0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xEC, 0x50, 0x48,
]; // first 12B of the new 0.5.1 prologue: push r15/r14/rsi/rdi/rbx; sub rsp,0x50; (11B = a clean boundary) + the first byte of the following mov (0x48...). Trampoline relocation = 19B (next clean boundary = + mov rax,[rsp+0xa8])
static BUY_PROBE_INSTALLED: AtomicU64 = AtomicU64::new(0);
static CHAMP_SCAN: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static SCAN_DIAG_DONE: AtomicBool = AtomicBool::new(false); // * one-shot gate for the 0.5.1 scan diagnostic

// install_detour (trampoline): saved = push rcx rdx r8 r9 r10 r11 -> r8 = saved.add(3). cap_fn(rcx=saved, rdx=entry_rsp).
unsafe fn install_detour(
    rva: usize,
    orig_len: usize,
    cap_fn: usize,
) -> Result<usize, &'static str> {
    let mbase = exe_base_addr();
    if mbase == 0 {
        return Err("module 0");
    }
    let fn_addr = mbase + rva;
    if !readable(fn_addr, orig_len + 4) {
        return Err("fn unreadable");
    }
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let stub = VirtualAlloc(0, 256, MEM_CR, RWX);
    if stub == 0 {
        return Err("VirtualAlloc");
    }
    let ret_addr = fn_addr + orig_len;
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(&[0x49, 0x89, 0xe2]);
    s.extend_from_slice(&[0x51, 0x52, 0x41, 0x50, 0x41, 0x51, 0x41, 0x52, 0x41, 0x53]);
    s.extend_from_slice(&[0x48, 0x89, 0xe1]);
    s.extend_from_slice(&[0x4c, 0x89, 0xd2]);
    s.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&cap_fn.to_le_bytes());
    s.extend_from_slice(&[0xff, 0xd0]);
    s.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]);
    s.extend_from_slice(&[0x41, 0x5b, 0x41, 0x5a, 0x41, 0x59, 0x41, 0x58, 0x5a, 0x59]);
    let mut orig = vec![0u8; orig_len];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, orig.as_mut_ptr(), orig_len);
    s.extend_from_slice(&orig);
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&ret_addr.to_le_bytes());
    s.extend_from_slice(&[0xff, 0xe0]);
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    let mut patch = vec![0x90u8; orig_len];
    patch[0] = 0x48;
    patch[1] = 0xb8;
    patch[2..10].copy_from_slice(&stub.to_le_bytes());
    patch[10] = 0xff;
    patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, orig_len, RWX, &mut old) == 0 {
        return Err("VirtualProtect");
    }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, orig_len);
    VirtualProtect(fn_addr, orig_len, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, orig_len);
    Ok(stub)
}

#[inline]
unsafe fn rd_u64(p: usize) -> u64 {
    std::ptr::read_unaligned(p as *const u64)
}
#[inline]
unsafe fn wr_u64(p: usize, v: u64) {
    std::ptr::write_unaligned(p as *mut u64, v);
}

// -- Direct call into the item neural network forward (ported from the verified scrim version) --
//   forward(net, ctx=&[u64;11], build_ptr, build_len, flag=0) -> f32 sigmoid score.
//   ctx: [0..5] = our team's champ ids / [5..10] = the opponents' / [10] = position (0~4; forward panics above 4).
// 0.5.4 (2026-08-04): exe2exe `match`, 1 hit at 320 and 640 bytes, size 1609.
const ITEMNET_FORWARD_RVA: usize = 0x145a680; // 0.5.4 (0.5.3 was 0x10587e0). History for 0.5.3 follows. (0.5.2 was 0x1b9cce0). The first 24B of the entry are identical + all 5 feature-name strings match (self_item/champ_pos_build/lane_counter/synergy/global_counter) + the net layout is unchanged (net+0x8 = weight ptr, +0x10 = 16384 bound, +0x18 = 1) => the mod's per-call re-validation logic stays valid as-is. History for 0.5.2 follows. (0.5.1 was 0x1bc82e0; exe2exe UNIQUE, identical prologue.) History for 0.5.1 follows: (0.5.0_3 was 0x1b78420, mask-sig UNIQUE PROL-OK push8 554157415641554154565753). WARNING it was OFF via AUTO4_FORWARD_SCORE=false (an AV at +0x44a inside forward on 0.5.1; see the flag comment above). A matching prologue does not imply identical internals.
type ItemNetFn = unsafe extern "C" fn(usize, usize, *const u64, u64, u8) -> f32;
static ITEM_NET_ADDR: AtomicU64 = AtomicU64::new(0);
static ITEMNET_VALID: AtomicU64 = AtomicU64::new(0); // 0 = unchecked, 1 = valid, 2 = invalid
unsafe fn itemnet_addr_valid() -> bool {
    match ITEMNET_VALID.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let fa = exe_base_addr() + ITEMNET_FORWARD_RVA;
    let expect = [
        0x55u8, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
    ]; // push8
    let mut ok = readable(fa, 12);
    if ok {
        for i in 0..12 {
            if *((fa + i) as *const u8) != expect[i] {
                ok = false;
                break;
            }
        }
    }
    ITEMNET_VALID.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
    ok
}
unsafe fn itemnet_forward(net: usize, ctx: &[u64; 11], build: &[u64]) -> f32 {
    // Only reached from AUTO4 scoring in a spectated match (never a background sim) -> low frequency, so global atomic counters suffice.
    if net == 0 || !itemnet_addr_valid() {
        return f32::MIN;
    }
    // ** Per-call weight re-validation (07-17, to eliminate crashes): net was sig_ok'd only once at detection, but if the weight ptr inside net (net+0x8)
    //   goes stale on a session switch / background sim reload, forward dereferences that stale ptr internally (at +0x81) -> AV (0xc0000005). Right before every call
    //   we re-check the header (16384/16384/1) + weight ptr readability -> if stale, skip the call (f32::MIN = candidate rejected -> fallback). The shadow-call crash condition is cut off.
    if !(readable(net, 0x20)
        && rd_u64(net) == 16384
        && rd_u64(net + 0x10) == 16384
        && rd_u64(net + 0x18) == 1
        && {
            let w = rd_u64(net + 0x8) as usize;
            w >= 0x10000 && readable(w, 16384 * 4)
        })
    {
        AUTO4_NET_STALE.fetch_add(1, Ordering::Relaxed);
        return f32::MIN;
    }
    let func: ItemNetFn = core::mem::transmute(exe_base_addr() + ITEMNET_FORWARD_RVA);
    let out = func(
        net,
        ctx.as_ptr() as usize,
        build.as_ptr(),
        build.len() as u64,
        0,
    ); // * includes the real cost of the game-function shadow-CALL (this site *is* that cost)
    out
}
static AUTO4_NET_STALE: AtomicU64 = AtomicU64::new(0); // * per-call net-stale detections (skips)
const CHAMP_SHEET: [&str; 61] = [
    "swordman",
    "monk",
    "mod_champions",
    "fighter",
    "knight",
    "archer",
    "soldier",
    "priest",
    "pythoness",
    "pyromancer",
    "ice_mage",
    "ninja",
    "magic_knight",
    "berserker",
    "executioner",
    "lancer",
    "ogre",
    "dual_blader",
    "cavalry_knight",
    "gunner",
    "pole_warrior",
    "jiangshi",
    "gambler",
    "hammerer",
    "demon",
    "vampire",
    "spirit_caller",
    "boomerang_hunter",
    "inquisitor",
    "shield_bearer",
    "whip_master",
    "werewolf",
    "dokkaebi",
    "necromancer",
    "bard",
    "barrier_magician",
    "chef",
    "clown",
    "dancer",
    "dark_mage",
    "exorcist",
    "ghost",
    "illusionist",
    "lightning_mage",
    "plague_doctor",
    "poison_dart_hunter",
    "shadowmancer",
    "taoist",
    "siege_breaker",
    "android",
    "druid",
    "prisoner",
    "bomber",
    "voodoo_shaman",
    "white_mage",
    "wind_mage",
    "enchanter",
    "hitman",
    "guardian_spirit",
    "hunter",
    "circus_blade",
];
fn champ_id_of(name: &str) -> Option<usize> {
    CHAMP_SHEET.iter().position(|&c| c == name)
}
const SHADOW_CALL_NAMES: bool = true; // name of a ctx+0x20 element = calling vtable[0x50] (AV risk, hence the gate)
static MAX_OWNED4: AtomicU64 = AtomicU64::new(0);
static BUY4_LOGGED: AtomicBool = AtomicBool::new(false);
static CHAMP_AT3: Mutex<Vec<String>> = Mutex::new(Vec::new()); // diagnostic: champions that reached owned==3
static CHAMP_AT4: Mutex<Vec<String>> = Mutex::new(Vec::new()); // diagnostic: champions that reached owned>=4 (4th item tier/price)
static BUILD3_AT: Mutex<Vec<String>> = Mutex::new(Vec::new()); // diagnostic: source of the build[3] target (neural/manual/vanilla), once per champion
                                                               // * AUTO 4th: the 4th item id of the beam captured by c6 (per champion). Forced at buy time for auto champions.
static BEAM4TH: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
fn beam4_get(champ: &str) -> Option<u64> {
    BEAM4TH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(champ).copied())
}
fn beam4_set(champ: String, id: u64) {
    let mut g = BEAM4TH.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    if m.len() < 64 {
        m.insert(champ, id);
    }
}
// Item game id -> name key (0~29 = vanilla, 30+ = mod items). Used to scan names in the ctx+0x20 collection.
fn item_id_to_key(id: u64) -> Option<String> {
    if (id as usize) < VANILLA_KEYS.len() {
        return Some(VANILLA_KEYS[id as usize].to_string());
    }
    let reg = MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    reg.get((id as usize).checked_sub(30)?).cloned()
}
// * AUTO 4th = pick the highest-scoring final item via forward (neural recommendation). false = only capture beam4.
// * 0.5.0: ON - roster offsets RE-confirmed (SimState+0x840 stride 0x8d0, team@+0x820, pos@+0x8b0, champ@+0x420,
//   net@Database+0xda0). compute_auto_4th_id / build_lineup_ctx back in service = neural automatic 4th selection.
const AUTO4_FORWARD_SCORE: bool = true; // * Re-enabled (07-17): the crash cause was that the weight ptr net+0x8 goes stale after detection (session switch) and was never re-validated. itemnet_forward now re-checks net+0x8 readability on every call, so a stale net is skipped -> fallback (no crash). Feature kept + crash condition cut. ~~false (an attempt to drop the shadow-call)~~
                                        // forward scoring at c6 (personal tactics application) time - abandoned (never fires for enemy/background). AUTO is handled at buy time (compute_auto_4th_id).
const AUTO4_C6_SCORE: bool = false;
// * 0.5.0 build extension: RVA_REALLOC (the real function 0x25a56c0) confirmed -> ON. Real purchases via the buy build Vec 3->4 are back.
const BUILD_EXTEND_ENABLED: bool = true;
// * 0.5.0 ui_inject (#item3 dropdown + #slot3 node): loader hook RVAs (LOADER 0x4d8fb0 / PARSER 0x2493b90 /
//   ALLOC 0x25a5620) confirmed -> ON. Strategy-screen 4th dropdown / in-match slot3 node injection are back.
const UI_INJECT_ENABLED: bool = true; // * 0.5.0 fix: player_info/wide .ui rewritten on a 0.5.0 base with 4 slots -> re-enabled (isolated test)
                                      // * Diagnostic: OFF gate for the slot UI patch (bounds + helper) - bisecting the crash when returning to the title (demo battle).
                                      // * 0.5.0: helper RVA_SLOT_HELPER (0xdc2390) confirmed -> OFF (= patch_slot_ui back in service). In-match slot3 icon display.
                                      // ** 0.5.3 (2026-07-29) forced OFF - this feature alone cannot be ported (crash prevention). Two-part evidence:
                                      //   (1) the helper function itself **disappeared**: the 0.5.2 RVA_SLOT_HELPER (0xc5cd80) is **fully inlined** into the UI mega-function 0xa5c1e0
                                      //      in 0.5.3 (0 "blue_pla"/"red_play" movabs in the new exe .text, 0 call sites).
                                      //      The 4 inlined blocks (75B each) store 3 (ptr,len) pairs directly into rbp+0x10d20/+0x10d30/+0x10d40.
                                      //   (2) merely raising the bound is **impossible** too: the slots for a 4th entry, rbp+0x10d50/+0x10d58, are in 0.5.3 **already used by
                                      //      other locals** (measured 40 and 27 references respectively, e.g. 0xa62f9f mov [rbp+0x10d50],0 / 0xa6339f cmp rdi,[rbp+0x10d50]).
                                      //      Changing only cmp 0x30 -> 0x40 would make the loop read those locals as a string (ptr,len) = a guaranteed crash.
                                      //      There is no frame headroom either (the rbp limit is +0x10f88 and above that are xmm spills).
                                      //   => resuming would require trampolines on the inlined blocks + relocating the array base disp32 = a separate redesign project.
                                      //      What is lost while OFF is only the in-match 4th item **icon display** (purchasing, stat application and AI recommendation all still work).
                                      // ** Resumed (2026-07-30, user instruction): (1) and (2) above mean that "the 0.5.2 approach (replace the helper + only raise the bound)" is impossible;
                                      //   we judged that **frame extension + array relocation** surgery can work and re-enabled it. Detailed design and safeguards = the `patch_slot_ui` comment.
                                      //   Two switches to roll it back on trouble: set this to true (skip everything) or `SLOT_UI_SURGERY=false`.
                                      // ** 2026-07-30 final summary - this switch is exclusively for the **old byte-patch approach (SLOT_BOUNDS bound extension + SLOT_HELPER replace)**.
                                      //   That approach failed on 0.5.3 (see `SLOT_UI_SURGERY=false`), and the 4th icon is now **verified in game** via
                                      //   **direct view-model reading** (`handle_ingame_slot3` - GameView -> player_view -> items[3] -> node write).
                                      //   => leaving this true (= skip the old path entirely) is correct. The icon feature's on/off switch is `SLOT3_ICON_ENABLED`.
                                      //   WARNING do not confuse them: "DIAG_SLOT_UI_OFF=true" does not mean the icon is off (only the old path is off).
const DIAG_SLOT_UI_OFF: bool = true; // keep the old byte-patch path sealed (it failed). The icon works separately via direct view-model reading. History for 0.5.1~0.5.2 follows: the 4 SLOT_BOUNDS sites (0x4b4d40/50b0/5790/5b00) and SLOT_HELPER (0xd81b30) were all confirmed correct by ghidra-re's OLD<->NEW byte comparison of the mask-sig picks (HIGH confidence; the +0x8fb0 idiom in 4 places, "blue_pla" movabs). Not a misidentification -> ON.
                                     // * Performance cache (0.5.1): the result of compute_auto_4th_id. Key = (champ, build3, lineup ctx). For the same match, champion and build
                                     //   the neural 4th recommendation is always the same -> removes the 51-forward recomputation on repeated owned==3 buys (eases the load spike when gold is low).
                                     //   Since ctx is part of the key there is no "ignores the lineup = wrong answer" concern. Parallel matches have different ctx, so keys do not collide.
static AUTO4_RESULT: Mutex<Option<HashMap<(String, u64, u64, u64, [u64; 11]), Option<u64>>>> =
    Mutex::new(None);
static AUTO_CANDS: Mutex<Option<std::sync::Arc<Vec<u64>>>> = Mutex::new(None);
fn auto_cands() -> std::sync::Arc<Vec<u64>> {
    {
        let g = AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = g.as_ref() {
            return v.clone();
        } // Arc clone = refcount only (no data copy)
    }
    let mut v: Vec<u64> = VANILLA_FINAL.to_vec();
    for (id, _) in mod_final_opts() {
        v.push(id);
    }
    let arc = std::sync::Arc::new(v);
    *AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(arc.clone());
    arc
}

// -- Restore the match's real lineup ctx from the roster array (SimState+0x840, stride 0x8d0) --
//   athlete = an array element. team = +0x820 (0/1), champion name = +0x420. Parallel matches use separate arrays, so an
//   athlete pointer belongs to exactly one match = no collisions (no back pointer needed, RE confirmed).
// 0.5.4 = 0x8c0 (0.5.3 was 0x8d0). `imul r,r,stride`: 15 hits/0 on 0.5.3, 0/16 on 0.5.4.
const ATH_STRIDE: usize = 0x8c0;
// Validate an athlete + return (team, champ_id). Strong validation (team in {0,1} + a real champion name) determines the array bounds automatically.
unsafe fn athlete_lineup_at(p: usize) -> Option<(u64, u64)> {
    if p < 0x10000 {
        return None;
    }
    let team = safe_read_u64(p + 0x820)?;
    if team > 1 {
        return None;
    }
    let nptr = safe_read_u64(p + 0x420)? as usize; // 0.5.0 champion name ptr (was 0x398)
    let nlen = safe_read_u64(p + 0x428)? as usize; // 0.5.0 champion name len (was 0x3a0)
    if nptr < 0x10000 || nlen == 0 || nlen > 48 {
        return None;
    }
    let mut buf = Vec::new();
    if !safe_read_bytes(nptr, nlen, &mut buf) {
        return None;
    }
    let name = String::from_utf8_lossy(&buf).into_owned();
    let cid = champ_id_of(&name)? as u64;
    Some((team, cid))
}
// * Read an athlete's champion name (+0x420 ptr / +0x428 len, confirmed on 0.5.0_3). For SEL/PT matching.
unsafe fn ath_champ_name(p: usize) -> Option<String> {
    if p < 0x10000 {
        return None;
    }
    let nptr = safe_read_u64(p + 0x420)? as usize;
    let nlen = safe_read_u64(p + 0x428)? as usize;
    if nptr < 0x10000 || nlen == 0 || nlen > 48 {
        return None;
    }
    let mut buf = Vec::new();
    if !safe_read_bytes(nptr, nlen, &mut buf) {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}
// * Validate an athlete + return (side 0/1, champ name). WARNING it uses neither champ_id_of nor a name charset = **mod champions fully included**
//   Criteria = side (+0x820) in {0,1} + **position (+0x8b0) in 0..4 (a structural filter, independent of champion type = mod champions included)** + a readable name (len 2~48).
//   WARNING lesson: using the name charset (the old ascii filter) for bounds detection (1) excludes mod champions with non-identifier names and (2) removing it entirely leaves only side,
//     which misjudges adjacent structural memory -> bounds over-extend -> the count collapses (a regression where nothing was injected at all). position<5 gives precise bounds and mod-champion compatibility at once.
//   False positives are harmless since they fail SEL/PT membership (count) anyway. (build_lineup_ctx uses lane<5 as well.)
unsafe fn ath_side_champ(p: usize) -> Option<(u64, String)> {
    let side = safe_read_u64(p + 0x820)?;
    if side > 1 {
        return None;
    }
    let pos = safe_read_u64(p + 0x8b0)? & 0xffff_ffff; // lane 0~4
    if pos >= 5 {
        return None;
    }
    let nm = ath_champ_name(p)?; // len 1..=48, readable
    if nm.len() < 2 {
        return None;
    }
    Some((side, nm))
}
// ** Deterministic team gate (global majority vote abandoned): scan only "the roster array of that match" which this athlete belongs to
//   and decide the player side (0/1) immediately and deterministically. The side with more user-designated (SEL) or PT champions = player.
//   If both are 0 (= the player is not in this match) or tied, return None -> no injection (prevents copying onto the enemy / misjudgement).
//   Per-match arrays are independent (no contamination from parallel background matches, RE confirmed) -> eliminates the startup-gap / contamination / inversion problems of global voting at the root.
//   Cached by base pointer (avoids re-scanning on every buy). Reset = cleared in before_management_tick.
static SIDE_CACHE: Mutex<Vec<(usize, i8)>> = Mutex::new(Vec::new()); // (roster_base, side: 0/1, -1=none)
unsafe fn player_side_for_match(athlete: usize) -> Option<u64> {
    // base for the cache key (roughly the array start): walk back up to 9 slots. Only for cache hits; counting is done by the fixed window below.
    let mut base = athlete;
    for _ in 0..9 {
        let c = base.wrapping_sub(ATH_STRIDE);
        if ath_side_champ(c).is_some() {
            base = c;
        } else {
            break;
        }
    }
    // Cache lookup - * only a decided side (0/1) is cached. None is not cached (re-decide = self-heal).
    {
        let g = SIDE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&(_, s)) = g.iter().find(|&&(b, _)| b == base) {
            if s >= 0 {
                return Some(s as u64);
            }
        }
    }
    // * Count player-designated champions per side over a fixed +/-9 slot window around the athlete (* SEL only).
    //   A fixed window always covers the whole 10-player roster from any athlete + has no walk truncation.
    //   WARNING PT_SNAPSHOT is excluded from the vote (07-10): the PT map = the team's whole personal tactics, ~52 champions -> nearly every player on both teams matches
    //   -> a 5:5 tie -> undecided -> the champ_designated safety net fired on enemy designated champions too = the cause of "the enemy follows my tactics".
    //   Counting only SEL (champions the user actually designated) sharply reduces ties and prevents misattribution to the enemy side.
    let (mut c0, mut c1) = (0u32, 0u32);
    for k in -9i64..=9 {
        let a = athlete.wrapping_add((k.wrapping_mul(ATH_STRIDE as i64)) as usize);
        if let Some((team, nm)) = ath_side_champ(a) {
            // * Match ignoring the scope prefix (2026-07-30): champions with only comp-test designations must be counted too.
            let is_p = with_sel(|m| m.keys().any(|(c, _)| strip_scope(c) == nm.as_str()));
            if is_p {
                if team == 0 {
                    c0 += 1;
                } else {
                    c1 += 1;
                }
            }
        }
    }
    let side: i8 = if c0 > c1 {
        0
    } else if c1 > c0 {
        1
    } else {
        -1
    };
    // * Cache only when decided (bounded). None (not participating / a transient glitch) is not cached -> re-decided on the next buy.
    if side >= 0 {
        let mut g = SIDE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if !g.iter().any(|&(b, _)| b == base) {
            if g.len() >= 64 {
                g.remove(0);
            }
            g.push((base, side));
        }
    }
    if side < 0 {
        None
    } else {
        Some(side as u64)
    }
}
// buy athlete -> (that match's real ctx[11], view roster count@+0x848). Position = athlete+0x8b0.
//   view = base - 0x840. count==3 marks a demo/title live sim (the context where forward crashes).
unsafe fn build_lineup_ctx(p: usize) -> Option<([u64; 11], u64)> {
    let (my_team, _) = athlete_lineup_at(p)?;
    // Array bounds: scan by stride forwards and backwards from p (while athletes stay valid, <=9 each way).
    let mut base = p;
    for _ in 0..9 {
        let c = base.wrapping_sub(ATH_STRIDE);
        if athlete_lineup_at(c).is_some() {
            base = c;
        } else {
            break;
        }
    }
    let mut end = p;
    for _ in 0..9 {
        let c = end.wrapping_add(ATH_STRIDE);
        if athlete_lineup_at(c).is_some() {
            end = c;
        } else {
            break;
        }
    }
    let mut ctx = [9999u64; 11];
    let mut a = base;
    while a <= end {
        if let Some((team, cid)) = athlete_lineup_at(a) {
            let lane = (safe_read_u64(a + 0x8b0).unwrap_or(9) & 0xffff_ffff) as usize; // the real position (0~4)
            if lane < 5 {
                if team == my_team {
                    ctx[lane] = cid;
                } else {
                    ctx[5 + lane] = cid;
                }
            }
        }
        a = a.wrapping_add(ATH_STRIDE);
    }
    let pos = ((safe_read_u64(p + 0x8b0).unwrap_or(0) & 0xffff_ffff) as usize).min(4);
    ctx[10] = pos as u64; // ctx[pos] = my champion (self-consistent)
    let vcount = safe_read_u64(base.wrapping_sub(0x840) + 0x848).unwrap_or(0);
    Some((ctx, vcount))
}
// * AUTO 4th (universal - every player, every match): at buy time (owned==3), score build[0..3] with the neural forward and
//   append each final-item candidate as the 4th; the highest score = the network's chosen 4th. Independent of c6 firing (covers enemy/background too).
//   ctx = the match's real lineup restored from the roster array (our 5 + their 5 + pos). Simple fallback on failure.
unsafe fn compute_auto_4th_id(athlete: usize, champ: &str) -> Option<u64> {
    if !AUTO4_FORWARD_SCORE {
        return None;
    }
    let net = ITEM_NET_ADDR.load(Ordering::Relaxed) as usize;
    if net == 0 || !itemnet_addr_valid() {
        return None;
    }
    let ptr = rd_u64(athlete + 0x488) as usize; // 0.5.0 build ptr (was 0x410)
    if ptr < 0x10000 || !readable(ptr, 24) {
        return None;
    }
    let b0 = rd_u64(ptr);
    let b1 = rd_u64(ptr + 8);
    let b2 = rd_u64(ptr + 16);
    if b0 >= 0x10000 || b1 >= 0x10000 || b2 >= 0x10000 {
        return None;
    }
    // * Mod champions (unknown cid): running forward with a garbage ctx (cid=0, opponents 9999) gives everyone the same answer (fixed at 116) -> skip that and
    //   use the variety fallback (champ-hash spread). A complete fix = the game's champion registry name -> id (follow-up).
    let cid = match champ_id_of(champ) {
        Some(c) => c as u64,
        // Mod champion: no cid, so forward would score everyone identically -> variety fallback.
        None => return None,
    };
    // * Restore the match's real lineup ctx from the roster array (the 5 real opponents = global_counter stays meaningful). Simple fallback on failure.
    let (ctx, real, vcount) = match build_lineup_ctx(athlete) {
        Some((c, vc)) => (c, true, vc),
        None => {
            let mut c = [0u64; 11];
            for k in 5..10 {
                c[k] = 9999;
            } // opponents unknown
            c[0] = cid;
            c[10] = 0;
            (c, false, 0)
        }
    };
    // * In a demo/title live sim (view roster count==3) calling the game's forward crashes -> fall back to the heuristic 4th.
    //   In a real sim (background league etc., count != 3) use forward for the neural 4th. (DIAG_FWD_OFF = emergency global heuristic.)
    if DIAG_FWD_OFF || vcount == 3 {
        let pick = auto_cands()
            .iter()
            .copied()
            .find(|&c| c != b0 && c != b1 && c != b2);
        return pick;
    }
    // * Performance cache lookup: identical (champ, build, lineup) needs no forward sweep recomputation.
    let ckey = (champ.to_string(), b0, b1, b2, ctx);
    {
        let mut g = AUTO4_RESULT.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(HashMap::new);
        if let Some(&cached) = m.get(&ckey) {
            return cached;
        }
    }
    let cands = auto_cands();
    let mut best: Option<u64> = None;
    let mut best_s = f32::MIN;
    for &cand in cands.iter() {
        if cand == b0 || cand == b1 || cand == b2 {
            continue;
        } // exclude duplicates
        let s = itemnet_forward(net, &ctx, &[b0, b1, b2, cand]);
        if s > best_s {
            best_s = s;
            best = Some(cand);
        }
    }
    // * Store in the cache (cap 8192; simply cleared when exceeded - allows for many parallel matches)
    {
        let mut g = AUTO4_RESULT.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(HashMap::new);
        if m.len() >= 8192 {
            m.clear();
        }
        m.insert(ckey.clone(), best);
    }
    best
}

// Designated item key for a champion's 4th slot (SEL slot3). idx 0 = auto (None), 1~6 = vanilla category final items,
//   7+ = mod items. The returned key is used to scan the clone source collection (96 entries, containing both vanilla and mod names).
// Manually designated item key for slot si (0~3). 0 = auto (None), 1~6 = vanilla category final items, 7+ = mod items.
/// The host mod's build editor (`item-builds.json`), which owns item
/// designation now that the tactics dropdowns offer only stat categories.
///
/// Consulted ahead of `SEL` for all four slots. This is the whole point of
/// routing it through here: everything below is reached from the buy detour,
/// *after* `is_my_athlete` has said the athlete is the player's, so a build set
/// in the editor can only ever be applied to the player's own players. The
/// route hook cannot make that distinction — it is handed one team per call with
/// nothing to say which (see `hook::detour`).
fn pinned_key(champ: &str, si: u8) -> Option<String> {
    crate::build_config::pinned_key(champ, si as usize)
}

fn slotN_item_key(scope: Scope, champ: &str, si: u8) -> Option<String> {
    if let Some(key) = pinned_key(champ, si) {
        return Some(key);
    }
    let idx = sel_get(scope, champ, si);
    if idx == 0 {
        return None;
    } // auto -> force nothing
    if idx <= 6 {
        // vanilla category -> that category's final item name
        return VANILLA_KEYS
            .get(VANILLA_FINAL[(idx - 1) as usize] as usize)
            .map(|k| k.to_string());
    }
    mod_final_opts()
        .get(idx as usize - 7)
        .map(|(_, k)| k.clone()) // mod item
}
fn slot3_item_key(scope: Scope, champ: &str) -> Option<String> {
    slotN_item_key(scope, champ, 3)
}
// Item id of the slot3 manual designation (for the build[3] target). 0 = auto (None), 1~6 = vanilla category finals, 7+ = mod items.
#[allow(dead_code)]
fn slot3_item_id(scope: Scope, champ: &str) -> Option<u64> {
    let idx = sel_get(scope, champ, 3);
    if idx == 0 {
        return None;
    }
    if idx <= 6 {
        return Some(VANILLA_FINAL[(idx - 1) as usize]);
    }
    mod_final_opts().get(idx as usize - 7).map(|(id, _)| *id)
}
// * build[3] index for vanilla designations (idx 1~6) only. For vanilla, id == catalog index, so no scan is needed (works even if the 0.5.0 scan is broken).
//   Mod items (7+) return None (id != index -> a name scan is required).
fn slotN_vanilla_id(scope: Scope, champ: &str, si: u8) -> Option<u64> {
    // A pinned slot answers here or nowhere: returning a `SEL` id for a slot the
    // editor has pinned would let the old designation win, because the caller
    // tries this before `slotN_item_key`.
    if let Some(raw) = crate::build_config::pinned_key_raw(champ, si as usize) {
        // Vanilla items only, and by the verbatim key: for those the id *is* the
        // catalog index, so the name scan is not needed. A mod item returns
        // `None` and goes down the scan path, which is what `slotN_item_key` is
        // for.
        return VANILLA_KEYS
            .iter()
            .position(|key| *key == raw)
            .map(|id| id as u64);
    }
    let idx = sel_get(scope, champ, si);
    if (1..=6).contains(&idx) {
        Some(VANILLA_FINAL[(idx - 1) as usize])
    } else {
        None
    }
}
fn slot3_vanilla_id(scope: Scope, champ: &str) -> Option<u64> {
    slotN_vanilla_id(scope, champ, 3)
}
// * How the 4th is acquired: true = plant only the target in build[3] and let the game build up naturally from t1 (paying full gold). false = force-inject the final item immediately.
const AUTO4_NATURAL: bool = true; // * natural build-up (user decision): plant only the target in build[3] and let the game build up from t1 at full price. Higher starting gold is expected to raise the completion rate.

// * The 4th target = scan the catalog (ctx+0x20) by name to get the index + validate the recipe. (Mod items need a name scan because id != index.)
//   catalog = the same array the resolver indexes (RE confirmed). element{elem_ptr@0, vtable@8}, name = vtable[0x50],
//   has_recipe = calling vtable[0x68] (!=0 = has a recipe). Without a recipe the game panics in FUN_141d5ab40 -> always validate before use.
//   Returns = the catalog index of a valid final item that has a recipe (usable directly in build[3]). None otherwise (vanilla fallback).
// * For the spawn hook (v14): a scan that takes the catalog base/len directly (Game+0x1fd0/+0x1fd8). Cache key = base.
//   Same index space as the buy path (the ctx+0x30 collection) - a build[] value *is* this index, so the resolver consumes it as-is.
unsafe fn scan_catalog_index(base: usize, len: u64, want: &[u8]) -> Option<u64> {
    if want.is_empty() || base < 0x10000 || len == 0 || len > 100000 {
        return None;
    }
    {
        let g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(outer) = g.as_ref() {
            if let Some(m) = outer.get(&base) {
                if let Some(&v) = m.get(want) {
                    return if v >= 0 { Some(v as u64) } else { None };
                }
            }
        }
    }
    let res = scan_recipe_safe_in(base, len, want);
    {
        let mut g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(HashMap::new());
        }
        if let Some(outer) = g.as_mut() {
            if !outer.contains_key(&base) && outer.len() >= 16 {
                outer.clear();
            }
            let m = outer.entry(base).or_insert_with(HashMap::new);
            if m.len() < 256 {
                m.insert(want.to_vec(), res.map(|i| i as i64).unwrap_or(-1));
            }
        }
    }
    res
}
unsafe fn scan_recipe_safe_index(ctx: usize, want: &[u8]) -> Option<u64> {
    if want.is_empty() || ctx < 0x10000 || !readable(ctx, 0x28) {
        return None;
    }
    let coll = rd_u64(ctx + 0x30) as usize; // * 0.5.0: the catalog collection offset moved ctx+0x20 -> +0x30 (RE confirmed, the only change)
    if coll < 0x10000 || !readable(coll, 0x18) {
        return None;
    }
    let data = rd_u64(coll + 8) as usize;
    let len = rd_u64(coll + 0x10);
    scan_recipe_safe_in(data, len, want)
}
// Shared scan core: find the index in the catalog array (element{elem_ptr@0, vtable@8}, stride 0x10) whose name matches and which has a recipe.
unsafe fn scan_recipe_safe_in(data: usize, len: u64, want: &[u8]) -> Option<u64> {
    if data < 0x10000 || len == 0 || len > 100000 || !readable(data, (len as usize) * 16) {
        return None;
    }
    let do_diag = false;
    let mut dbg = if do_diag {
        format!(
            "[{}ms] scan want='{}' data={:#x} len={}\n",
            now_ms(),
            String::from_utf8_lossy(want),
            data,
            len
        )
    } else {
        String::new()
    };
    let mut names_ok = 0u64;
    let mut i = 0u64;
    while i < len {
        let e = data + (i as usize) * 16;
        let edata = rd_u64(e) as usize;
        let evt = rd_u64(e + 8) as usize;
        if edata >= 0x10000 && evt >= 0x10000 && readable(evt, 0x78) {
            let namefn = rd_u64(evt + 0x58) as usize;
            if code_ptr_ok(namefn) {
                let f: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(namefn);
                let nobj = f(edata);
                if nobj >= 0x10000 && readable(nobj, 0x18) {
                    let chars = rd_u64(nobj + 8) as usize;
                    let nlen = rd_u64(nobj + 0x10) as usize;
                    if chars >= 0x10000 && nlen > 0 && nlen <= 64 && readable(chars, nlen) {
                        let nm = std::slice::from_raw_parts(chars as *const u8, nlen);
                        if do_diag {
                            names_ok += 1;
                            if names_ok <= 12 || nm.starts_with(b"radiant") {
                                dbg.push_str(&format!(
                                    "  [{}] '{}'\n",
                                    i,
                                    String::from_utf8_lossy(nm)
                                ));
                            }
                        }
                        if nm == want {
                            // * Recipe validation: calling vtable[0x68] must return !=0 for natural build-up to be safe (0 = a base item -> panic).
                            let recfn = rd_u64(evt + 0x70) as usize; // 0.5.1: the next_tier/recipe getter slot moved +0x68 -> +0x70 (ghidra-re)
                            if code_ptr_ok(recfn) {
                                let rf: unsafe extern "win64" fn(usize) -> usize =
                                    core::mem::transmute(recfn);
                                if rf(edata) != 0 {
                                    return Some(i);
                                }
                            }
                            return None; // the name matches but there is no recipe -> fall back
                        }
                    }
                }
            }
        }
        i += 1;
    }
    if do_diag {
        dbg.push_str(&format!(
            "  name extraction succeeded {}/{} - want not found
",
            names_ok, len
        ));
    }
    None
}

// * Performance: scan cache (name -> index). Reduces the 96-element shadow-call scan to once per name. Value -1 = not found / no recipe.
//   * Multi-collection (keyed by coll base): parallel background sims using different ctx collections do not thrash. Collection cap 16.
static SCAN_CACHE: Mutex<Option<HashMap<usize, HashMap<Vec<u8>, i64>>>> = Mutex::new(None);
unsafe fn scan_idx_cached(ctx: usize, want: &[u8]) -> Option<u64> {
    if ctx < 0x10000 || !readable(ctx, 0x28) {
        return None;
    }
    let coll = rd_u64(ctx + 0x30) as usize; // * 0.5.0: the catalog collection offset moved ctx+0x20 -> +0x30 (RE confirmed, the only change)
    {
        let g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(outer) = g.as_ref() {
            if let Some(m) = outer.get(&coll) {
                if let Some(&v) = m.get(want) {
                    return if v >= 0 { Some(v as u64) } else { None };
                }
            }
        }
    }
    let res = scan_recipe_safe_index(ctx, want);
    {
        let mut g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(HashMap::new());
        }
        if let Some(outer) = g.as_mut() {
            if !outer.contains_key(&coll) && outer.len() >= 16 {
                outer.clear();
            } // reset when too many collections (memory cap)
            let m = outer.entry(coll).or_insert_with(HashMap::new);
            if m.len() < 256 {
                m.insert(want.to_vec(), res.map(|i| i as i64).unwrap_or(-1));
            }
        }
    }
    res
}
// * buy_item replace-detour: when owned==3 and the champion designates a mod item as its 4th, scan the clone source collection (ctx+0x20)
//   by name (vtable[0x50]) -> return that mod item's index i as rax=1/rdx=i -> run_tick_ext clones/pushes it
//   -> the 4th = the mod item. (Anything else / no match = passthrough = the original, normal 3 purchases.)
const DIAG_SCAN_OFF: bool = false; // * diagnostic #4: realloc proven innocent -> scanning resumed
const DIAG_FWD_OFF: bool = false; // false = run forward when count != 3 (a real sim). true = emergency global heuristic.
                                  // * Live injection gate for slots 0/1/2: write the designated index into build[0/1/2] (the same build-Vec target mechanism as slot 3).
const SLOT012_INJECT_ENABLED: bool = true;

// ** fix B (2026-07-27): spectate == final. The is_live early exit was removed -> inject in background matches too, with the team scope = is_my_athlete (+0x810).
//   My players get designated items / everyone else gets the network, identically in background and spectated sims -> they converge. Being id-based, AI-vs-AI matches have my=0 = no designation = zero statistical contamination.
//   WARNING false = restores the old behaviour (is_live gate, no background injection). Kept for an immediate rollback on trouble.
const FIXB: bool = true;

/// Whether this athlete's build `Vec` still has to be grown from 3 to 4.
///
/// Read through `safe_read_u64` (the VEH, no syscall) rather than `readable`
/// (`VirtualQuery`, a kernel call), because this runs on the buy hot path *ahead
/// of* the background early exit — the one place where a syscall per call was
/// measured at 75% of the mod's whole cost. Two protected reads of an address
/// that is about to be read anyway is the budget here.
///
/// Answers `false` for everyone in 3-slot mode, and `false` for good once the
/// extension has run (`len` becomes 4), so no athlete keeps the exit open.
///
/// This deliberately mirrors the `build_len == 3 && cap == 3` condition the
/// extension itself tests further down. If those two ever disagree the symptom is
/// silent — the gate opens for an athlete the extension then declines — so they
/// are worth changing together.
unsafe fn needs_build_extension(athlete: usize) -> bool {
    if slot_count() != 4 || !BUILD_EXTEND_ENABLED {
        return false;
    }
    // 0.5.4 build Vec: cap@+0x480, ptr@+0x488, len@+0x490.
    matches!(
        (
            safe_read_u64(athlete + 0x480),
            safe_read_u64(athlete + 0x490)
        ),
        (Some(3), Some(3))
    )
}

unsafe extern "C" fn buy_replace_ctx(saved: *mut u64, rsp_entry: usize) -> u64 {
    // * Hot path (parallel rayon workers) - global atomic counters would make the measurement itself expensive through cache-line contention,
    //   so thread_local accumulation (rec_tl) is used. T_BUY_ALL = the whole detour (including catch_unwind),
    //   T_BUY_EARLY = the background-sim early exit portion (contained in ALL, so it is double counted - subtract when interpreting).
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> u64 {
        if saved.is_null() {
            return 0;
        } // * mode=3 passes through here too (slot 0/1/2 designation injection). Only the 4th-item logic is gated on mode=4 below.
        let athlete = *saved.add(2) as usize; // r8
        if athlete < 0x10000 {
            return 0;
        }
        // *** Early-exit reordering (2026-07-22, established by perf measurement): there used to be a `readable(athlete,0x4a8)` ahead of this, and
        //   since readable() is a **VirtualQuery kernel call**, every buy call (6.89M in 130.7s, 53k/s) entered the kernel.
        //   => the buy early exit averaged 3.6us = 75% of the mod's total cost (25.9 core-seconds). athlete fields are only touched after passing the
        //   is_live gate, so **all checks and diagnostics moved behind the gate** (VirtualQuery 6.89M -> about 80k = 1.2%).
        //   The 07-18 lean comment claimed "we exit immediately after 2 memory reads", but a kernel call remained in front of it and
        //   nullified that intent. (The LOG_ENABLED diagnostic block moved along with it - its purpose, tracking the 4th item's tier in a spectated match, is unchanged.)
        // *** Hot-path early exit (07-18 lean): the spectated (rendered) match test comes first - about 94% of all buy calls are background league sims,
        //   so they exit here after 2 memory reads. (Structurally, the old order of extracting the champion name and doing a hash lookup first was the main cost.)
        // * Spectate identification v13 (confirmed working 07-18): buy r9 (saved[3]) = the provider (the 0xeb08 sim object).
        //   provider+O_PROVIDER_SEED (0.5.3 = 0xeaf8, 0.5.2 = 0xeab8) = the match seed (a constant value, verified in game by serpen) == LIVE_SEED (captured by the launcher hook) -> an on-screen match.
        //   Secondary: pointer identity with RENDER_PROVIDER (captured by the seed-ctor hook matching rdx == LIVE_SEED).
        //   WARNING [rsp+0x30] = the buy-list container (not the provider) - the reason the old gates (v5~v11) failed. r9 is the right one (RE confirmed).
        let lseed = LIVE_SEED.load(Ordering::Relaxed);
        let provider_now = *saved.add(3); // r9 = param_4 = provider
        let seed_r9 = if provider_now >= 0x10000 && provider_now < 0x0000_8000_0000_0000 {
            safe_read_u64(provider_now as usize + O_PROVIDER_SEED).unwrap_or(0)
        } else {
            0
        };
        let seed_match_r9 = lseed != 0 && seed_r9 == lseed;
        let rp = RENDER_PROVIDER.load(Ordering::Relaxed);
        let is_live = seed_match_r9 || (rp != 0 && provider_now != 0 && provider_now == rp);
        if !is_live && !FIXB {
            // (FIXB=false, old behaviour) background league sim = passthrough with no injection.
            return 0;
        }
        // * fix B: with FIXB=true, background sims are injected too (team scope = is_my_athlete). Only the is_live-specific counters stay gated.
        if is_live {
            PROV_HIT.fetch_add(1, Ordering::Relaxed);
            if seed_match_r9 {
                VT_OK.fetch_add(1, Ordering::Relaxed);
            }
        }
        // ** fix B performance (2026-07-27): in background buys only my players (is_my_athlete) are injection targets -> a background buy by anyone else
        //   passes through immediately after a cheap VEH read (+0x810) + HashSet lookup, before the expensive readable (= VirtualQuery kernel call).
        //   This restores the background early exit that 07-22 removed, in a way compatible with the fix (~94% of background buys exit here). None (roster unavailable) =
        //   no injection = early exit (identical to the old behaviour). Spectated matches (is_live) always pass through (they need the by_scene decision).
        //
        // ** 4th-item parity fix (2026-08-05) — `&& !needs_build_extension`.
        //   The gate above is about *designation* scope: only my players get an item
        //   pinned, which is right. But the build **extension** (the Vec 3 -> 4 that
        //   makes a 4th item possible at all) sits below it, so this exit denied it to
        //   everyone else — and every match except the player's own is a background sim.
        //   The result was a league where only the player's five athletes ever built a
        //   4th item, which is the reported "the opposing team never buys a 4th item".
        //
        //   Letting an athlete through *only while its build still needs growing* keeps
        //   the measurement this exit was built for: `needs_build_extension` is two
        //   VEH reads and no syscall, it is false for everyone in 3-slot mode, and it
        //   goes false for good once the Vec is 4 — so each athlete passes at most once
        //   per match and every later buy exits exactly as cheaply as before.
        //
        //   Scope is unaffected: `is_player` is still what gates the designation, so a
        //   non-player athlete reaching the extension takes the network/vanilla fallback
        //   the code below already had for it, and `note_my_champion` is still only
        //   called under `is_player`.
        if FIXB
            && !is_live
            && !matches!(is_my_athlete(athlete), Some(true))
            && !needs_build_extension(athlete)
        {
            return 0;
        }
        // -- From here on, only spectated-match buys (a small minority) and background buys by my 5 players get through --
        // * The athlete validity check (VirtualQuery) happens once, here - see the reordering comment above.
        if !readable(athlete, 0x498) {
            return 0;
        } // 0.5.0: covers build len@+0x4a0+8
        let owned = rd_u64(athlete + 0x448); // 0.5.0 owned (was 0x3d0)
                                             // * 0.5.3 regression diagnostic, stage 2: measure "we planted the target" and "it was actually bought" separately.
                                             //   The build[3] injection was confirmed to succeed (31 times) => the remaining question is whether the game ends up owning a 4th.
                                             //   Count the maximum owned (item count) and how often it reaches 4+. If owned>=4 is 0 it really was not bought;
                                             //   if it is non-zero it is bought but simply not shown on screen (no icon) = the expected consequence of the DIAG_SLOT_UI_OFF sealing.
        if BUILD_EXT_DIAG {
            if owned >= 4 {
                BE_CNT[7].fetch_add(1, Ordering::Relaxed);
            }
            let mx = BE_MAX_OWNED.load(Ordering::Relaxed);
            if owned > mx && owned <= 16 {
                BE_MAX_OWNED.store(owned, Ordering::Relaxed);
            }
        }
        // * Only handle target (designated) champions - everything else passes through (build untouched).
        let cptr = rd_u64(athlete + 0x410) as usize; // 0.5.0 champ name ptr (was 0x398, derived +0x88)
        let clen = rd_u64(athlete + 0x418) as usize; // 0.5.0 champ name len (was 0x3a0)
        if cptr < 0x10000 || clen == 0 || clen > 48 || !readable(cptr, clen) {
            return 0;
        }
        // * Performance: borrow via Cow (no heap allocation for valid UTF-8).
        let champ_cow =
            String::from_utf8_lossy(std::slice::from_raw_parts(cptr as *const u8, clen));
        let champ: &str = champ_cow.as_ref();
        // (`let champ_designated = is_champ_designated(champ)` used to sit here.
        //  Nothing read it — it was the safety net described at the `by_scene`
        //  comment below, and the team gate replaced it — so every buy that got
        //  this far paid two global mutex acquisitions, and sometimes a rebuild of
        //  the designated-champion `HashSet`, to compute a value it dropped.
        //  `is_champ_designated` itself is still used by the spawn path.)
        let side = if readable(athlete + 0x810, 8) {
            rd_u64(athlete + 0x810)
        } else {
            u64::MAX
        };
        // * Deciding the side: prefer the direct scene read (SCENE_SIDE, refreshed on the main thread) -> if undecided, decide on the spot from LIVE_DB (protects the owned=0 injection window).
        //   Undecided = no injection (prevents enemy/background contamination - the fallback vote is definitively abandoned).
        let scene_ps = scene_player_side().or_else(|| {
            if !is_live {
                return None;
            }
            let db = LIVE_DB.load(Ordering::Relaxed) as usize;
            let pid = LIVE_PID.load(Ordering::Relaxed);
            if db == 0 {
                return None;
            }
            let r = quick_scene_side(db, pid);
            if let Some(s) = r {
                SCENE_SIDE.store(s, Ordering::Relaxed);
            }
            r
        });
        let by_scene: bool = if is_live {
            match scene_ps {
                Some(ps) => side == ps,
                None => match player_side_for_match(athlete) {
                    Some(ps) => side == ps,
                    None => false,
                },
            }
        } else {
            false
        };
        // * Comp test: both sides are user-composed, so bypass the scene side gate (apply to any designated champion).
        // ** fix B: team scope = athlete_id membership (is_my_athlete, +0x810). The same decision in background and spectated sims -> convergence.
        //   If MY_ATHLETES is not published yet (before spectating) None = false = network. Only my players are designated; AI vs AI = my 0 = no designation.
        // ** Comp-test regression fix (2026-07-30 user report "item injection doesn't work in comp test"):
        //   the FIXB (= athlete_id membership) path **was missing the comp-test bypass**.
        //   Comp test is a sandbox where the user composes both sides, so those players are **not in** `MY_ATHLETES`
        //   (= db.team(pid).last_starting = my team's starters) => is_my_athlete = false
        //   => designated item injection was silently skipped. (The old FIXB=false path had the COMPTEST_MATCH bypass, but that
        //    condition disappeared when switching to FIXB=true = an omission introduced with fix B, not by a migration.)
        //   => for a match judged to be comp test (launcher retaddr measured = 0x1925f12), **apply designations to both sides**.
        //     Bypassing the team gate on a screen that has no notion of "my team" was the original design intent (see the comment at line 2409).
        // *** 2026-07-30 second fix - **blocking background contamination** (`&& is_live`):
        //   `COMPTEST_MATCH` is a **sticky global flag** updated only when an on-screen match launcher comes around again
        //   (the background sim call sites 0x220acb / 0x195c5be / 0x20dac9c / 0x2256a6d do not update it).
        //   => after one comp test, every buy in the background sims of subsequent schedule advances became is_player=true and
        //     **designated items were injected into every player on both teams of background matches** (until a spectate/own match started).
        //   The cause was that when the first fix hoisted the bypass to the top branch, the **is_live AND of the old condition
        //   `is_live && (by_scene || COMPTEST_MATCH)` fell away with it**. Comp-test main matches and record replays are both
        //   on-screen matches (the launcher plants LIVE_SEED), so filtering by is_live keeps the feature intact.
        let is_comptest_live = COMPTEST_MATCH.load(Ordering::Relaxed) && is_live;
        let is_player = if is_comptest_live {
            true // comp test = both sides user-composed -> bypass the team gate
        } else if FIXB {
            matches!(is_my_athlete(athlete), Some(true))
        } else {
            is_live && by_scene
        };
        // This branch is the only place in the mod that knows a champion belongs
        // to the player, so it is where the host half's build editor learns the
        // player's lineup — `hook::detour` cannot tell the teams apart on its
        // own. Comp test is excluded: it makes `is_player` true for *both*
        // sides, so noting from there would publish the opponent's champions as
        // the player's and invert the gate.
        if is_player && !is_comptest_live {
            crate::my_team::note_my_champion(champ);
        }
        // ** Deciding the SEL scope (2026-07-30): in comp test, read the designation under that player's side (blue/red) scope.
        //   Outside comp test it is Scope::Plain = exactly the old lookup => league/spectate/background behaviour unchanged.
        //   This removes both "the same champion on both sides merges into one designation" and "comp-test designations
        //   leaking into normal matches" at the same time (per-side keys + scoped lookup).
        let scope = if is_comptest_live {
            ct_scope_for(champ, side)
        } else {
            Scope::Plain
        };
        // * Slot 0/1/2 designations (mod or vanilla) -> set the build Vec target to that catalog index (the live buy path, same as slot 3).
        //   Only for slots not yet bought (owned <= si) -> the game builds up naturally towards that index. Vanilla = id, mod items = name scan (with recipe validation).
        if SLOT012_INJECT_ENABLED && is_player {
            let ctx012 = rd_u64(rsp_entry + 0x30) as usize;
            let bptr = rd_u64(athlete + 0x488) as usize; // 0.5.0 build ptr
            let blen = rd_u64(athlete + 0x490); // 0.5.0 build len
            if ctx012 >= 0x10000
                && bptr >= 0x10000
                && blen >= 1
                && blen <= 8
                && readable(bptr, (blen as usize) * 8)
            {
                for si in 0u8..3 {
                    if (si as u64) >= blen {
                        break;
                    } // build has no such slot
                    if owned > si as u64 {
                        continue;
                    } // slot already purchased -> too late
                    let idx: Option<u64> = if let Some(vid) = slotN_vanilla_id(scope, champ, si) {
                        Some(vid) // vanilla: id == catalog index (no scan needed)
                    } else if let Some(mk) = slotN_item_key(scope, champ, si) {
                        scan_idx_cached(ctx012, mk.as_bytes()) // mod item: name scan + recipe validation
                    } else {
                        None
                    };
                    if let Some(t) = idx {
                        // * Idempotence guard (07-19): skip the write if the target value is already there. Measured, the vast majority of 53,890 writes
                        //   were rewrites of the same value on the same athlete and slot -> a value comparison cut it to about 10 (removing the hot-path cost).
                        if rd_u64(bptr + (si as usize) * 8) == t {
                            continue;
                        }
                        if writable(bptr + (si as usize) * 8, 8) {
                            wr_u64(bptr + (si as usize) * 8, t);
                            BUY_WROTE_FIRE.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
        // ** Purchase order diagnostic (2026-07-30, investigating "it buys the 4th first"): record a snapshot of my players' build[] arrays
        //   once per (champ, owned) combination. What the game really targets is build[0..len], so recording which item each index is
        //   (catalog name) plus the current owned count shows directly **which build slot the game completes first**.
        //   This point is **after both** the build extension and the slot012 injection, so the final array is visible.
        if BUY_ORDER_DIAG && is_player {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let ctx = rd_u64(rsp_entry + 0x30) as usize;
                let bp = rd_u64(athlete + 0x488) as usize;
                let bl = rd_u64(athlete + 0x490);
                if ctx < 0x10000
                    || bp < 0x10000
                    || bl == 0
                    || bl > 8
                    || !readable(bp, (bl as usize) * 8)
                {
                    return;
                }
                let key = format!("{}#{}", champ, owned);
                let mut seen = BUY_ORDER_SEEN.lock().unwrap_or_else(|e| e.into_inner());
                let set = seen.get_or_insert_with(std::collections::HashSet::new);
                if set.contains(&key) || set.len() > 200 {
                    return;
                }
                set.insert(key);
                let mut line = format!("{} owned={} build_len={} build=[", champ, owned, bl);
                for i in 0..bl as usize {
                    let idx = rd_u64(bp + i * 8);
                    let nm = catalog_name_at(ctx, idx).unwrap_or_else(|| "?".into());
                    line.push_str(&format!("{}={} ", idx, nm));
                }
                line.push_str("]\n");
                let mut buf = BUY_ORDER_BUF.lock().unwrap_or_else(|e| e.into_inner());
                buf.push_str(&line);
                if let Some(d) = mod_dir() {
                    let _ = fs::write(d.join("buy_order.txt"), buf.clone());
                }
            }));
        }
        // * mode=3 stops here (slot 0/1/2 designation injection only). The 4th item (build extension, network, forced purchase) is mode=4 only -> 3 slots keep vanilla behaviour.
        if slot_count() != 4 {
            return 0;
        }
        if !SHADOW_CALL_NAMES {
            return 0;
        }
        // * 0.5.0: build extension (calls __rust_realloc @0x25a56c0 below). RVA_REALLOC confirmed -> BUILD_EXTEND_ENABLED=true.
        //   Passthrough (the original 3 purchases) only when OFF. Currently ON = build Vec 3->4 and real purchases.
        if !BUILD_EXTEND_ENABLED {
            return 0;
        }
        // Realloc the build Vec 3->4 + build[3] = catalog index. At owned==3 the resolver targets build[3] and builds up from t1.
        //   * RE: a build Vec value is a "catalog index" (not an item id). build[0] = a valid index the game itself put there, with a recipe.
        //   Mechanism check: build[3] = a copy of build[0] (guaranteed valid). If that works, owned goes to 4. Then map the real 4th index.
        let mut build_len = rd_u64(athlete + 0x490); // 0.5.0 build len (was 0x418)
                                                     // * 0.5.3 regression diagnostic (2026-07-29): to isolate "only the 4th is not bought". Inside the detour, **counters only** (no file IO -
                                                     //   synchronous IO in a parallel rayon-worker detour = a runaway crash). Actual file output happens in post_update (main thread).
        if BUILD_EXT_DIAG {
            BE_CNT[0].fetch_add(1, Ordering::Relaxed); // reached the 4th-item path
            let cap_now = rd_u64(athlete + 0x480);
            BE_LAST.store(
                (build_len << 32) | (cap_now & 0xffff_ffff),
                Ordering::Relaxed,
            ); // last observed (len, cap)
            if build_len != 3 {
                BE_CNT[1].fetch_add(1, Ordering::Relaxed);
            }
            if cap_now != 3 {
                BE_CNT[2].fetch_add(1, Ordering::Relaxed);
            }
        }
        if build_len == 3 && rd_u64(athlete + 0x480) == 3 {
            // 0.5.0 build cap (was 0x408)
            let ptr = rd_u64(athlete + 0x488) as usize; // 0.5.0 build ptr (was 0x410)
            if !(ptr >= 0x10000 && readable(ptr, 24) && writable(athlete + 0x480, 0x18)) {
                if BUILD_EXT_DIAG {
                    BE_CNT[3].fetch_add(1, Ordering::Relaxed);
                } // ptr/writable failure
            }
            if ptr >= 0x10000 && readable(ptr, 24) && writable(athlete + 0x480, 0x18) {
                let (b0, b1, b2) = (rd_u64(ptr), rd_u64(ptr + 8), rd_u64(ptr + 16));
                // * build[3] = (1) manual personal-tactics designation -> (2) neural recommendation -> (3) a distinct vanilla fallback.
                //   (1) and (2) scan the catalog by item "name" for an index + recipe validation (mod item ids != index, so a name scan is mandatory).
                //   Picks without a recipe (base items) are discarded and fall back (using them panics in FUN_141d5ab40).
                let ctx = rd_u64(rsp_entry + 0x30) as usize;
                // (1) manual designation (personal tactics) first -> (2) the network (cached) -> each obtains an index via a (cached) name scan + recipe validation.
                // * Team gate: manual designations (vanilla/mod) apply to the player's team only. For the enemy, van = manual = None -> the network fallback.
                let manual = if is_player {
                    slot3_item_key(scope, champ)
                } else {
                    None
                };
                let van = if is_player {
                    slot3_vanilla_id(scope, champ)
                } else {
                    None
                };
                let picked = if let Some(vid) = van {
                    Some(vid) // * vanilla designation: id == catalog index -> no scan needed (robust, 0.5.0)
                } else if let Some(mk) = manual.as_ref() {
                    scan_idx_cached(ctx, mk.as_bytes()) // mod item: name scan (works thanks to the ctx+0x30 fix)
                } else {
                    // * Enemy team or no designation: a fresh network call (our 5 + their 5 + position ctx). Not cached (ignoring the lineup = wrong answer).
                    compute_auto_4th_id(athlete, champ)
                        .and_then(item_id_to_key)
                        .and_then(|k| scan_idx_cached(ctx, k.as_bytes()))
                };
                // (3) Fallback: a vanilla final item different from build[0..2] (recipe guaranteed; for vanilla, id == index for sure).
                //   * Attack-damage bias fix: the implementation always scanned from [0] = attack damage (id 4) -> when the network failed, every enemy 4th was attack damage.
                //   -> the starting point is now spread by an FNV hash of the champion name (deterministic per champion = replay safe, and categories are distributed evenly).
                let t4 = picked.or_else(|| {
                    let mut h: u64 = 0xcbf29ce484222325;
                    for &b in champ.as_bytes() {
                        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
                    }
                    let start = (h % 6) as usize;
                    (0..6)
                        .map(|k| VANILLA_FINAL[(start + k) % 6])
                        .find(|&v| v != b0 && v != b1 && v != b2)
                });
                if t4.is_none() && BUILD_EXT_DIAG {
                    BE_CNT[4].fetch_add(1, Ordering::Relaxed);
                } // failed to obtain the target index
                if let Some(t) = t4 {
                    let realloc: ReallocFn = core::mem::transmute(exe_base_addr() + RVA_REALLOC);
                    let np = realloc(ptr, 24, 8, 32);
                    if BUILD_EXT_DIAG && !(np >= 0x10000 && writable(np, 32)) {
                        BE_CNT[5].fetch_add(1, Ordering::Relaxed);
                    } // realloc failure
                    if np >= 0x10000 && writable(np, 32) {
                        wr_u64(np + 24, t); // * build[3] = the manual/neural index or the vanilla fallback
                        wr_u64(athlete + 0x488, np as u64);
                        wr_u64(athlete + 0x480, 4);
                        wr_u64(athlete + 0x490, 4); // 0.5.0 build ptr/cap/len
                        build_len = 4;
                        if BUILD_EXT_DIAG {
                            BE_CNT[6].fetch_add(1, Ordering::Relaxed);
                            BE_LAST_T.store(t, Ordering::Relaxed);
                        } // * success: build[3] written
                    }
                }
            }
        }
        // * AUTO4_NATURAL: plant only the build[3] target and force nothing -> the game builds up naturally from components (t1), paying full gold.
        if AUTO4_NATURAL {
            return 0;
        }
        if DIAG_SCAN_OFF {
            return 0;
        } // * diagnostic #4: do only the realloc and skip the scan/shadow-call/forward (isolating realloc)
        if owned != 3 || build_len < 4 {
            return 0;
        }
        // -- owned==3 confirmed -> decide the 4th item key --
        // Manual designation (vanilla/mod) first. Otherwise AUTO: the best 4th given build[0..3] via the neural forward (universal, everyone).
        let want_key = match slot3_item_key(scope, champ) {
            Some(k) => k,
            None => match compute_auto_4th_id(athlete, champ).and_then(item_id_to_key) {
                Some(k) => k,
                None => return 0, // no designation and no neural 4th -> passthrough (the normal 3 purchases)
            },
        };
        // ctx (the stack argument = [rsp_entry+0x30]) -> coll (+0x20) -> data (+8) / len (+0x10)
        //   (relative to rsp_entry: after the prologue [rsp+0xa8] = rsp_entry-0x78+0xa8 = +0x30. The old buy_replace used +0x30 as well.)
        let ctx = rd_u64(rsp_entry + 0x30) as usize;
        if ctx < 0x10000 || !readable(ctx, 0x28) {
            return 0;
        }
        let coll = rd_u64(ctx + 0x30) as usize; // * 0.5.0: the catalog collection offset moved ctx+0x20 -> +0x30 (RE confirmed, the only change)
        if coll < 0x10000 || !readable(coll, 0x18) {
            return 0;
        }
        let data = rd_u64(coll + 8) as usize;
        let len = rd_u64(coll + 0x10);
        if data < 0x10000 || len == 0 || len > 100000 || !readable(data, (len as usize) * 16) {
            return 0;
        }
        // Name scan: elem = {data[i*16] = edata, +8 = vtable}. name = vtable[0x50](edata) -> {chars@+8, len@+0x10}
        let mut found: Option<u64> = None;
        let mut names_log = String::new();
        let mut i = 0u64;
        while i < len {
            let e = data + (i as usize) * 16;
            let edata = rd_u64(e) as usize;
            let evt = rd_u64(e + 8) as usize;
            if edata >= 0x10000 && evt >= 0x10000 && readable(evt, 0x60) {
                let namefn = rd_u64(evt + 0x58) as usize;
                if namefn >= 0x10000 {
                    let f: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(namefn);
                    let nobj = f(edata);
                    if nobj >= 0x10000 && readable(nobj, 0x18) {
                        let chars = rd_u64(nobj + 8) as usize;
                        let nlen = rd_u64(nobj + 0x10) as usize;
                        if chars >= 0x10000 && nlen > 0 && nlen <= 64 && readable(chars, nlen) {
                            let nm = std::slice::from_raw_parts(chars as *const u8, nlen);
                            if !BUY4_LOGGED.load(Ordering::Relaxed) && names_log.len() < 3000 {
                                names_log.push_str(&String::from_utf8_lossy(nm));
                                names_log.push(' ');
                            }
                            if nm == want_key.as_bytes() {
                                found = Some(i);
                                break;
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        if !BUY4_LOGGED.swap(true, Ordering::Relaxed) {}
        let Some(rdx) = found else {
            return 0;
        };
        *saved.add(1) = rdx; // rdx = item index
        *saved.add(6) = 1; // rax = 1 (success)
        1
    }));
    r.unwrap_or(0)
}

// Install the buy_item replace-detour (stub: mov r10,rsp; push rax r11 r10 r9 r8 rdx rcx; cap_fn(rcx=saved, rdx=rsp_entry)).
unsafe fn install_replace_buy(
    rva: usize,
    orig_len: usize,
    cap_fn: usize,
) -> Result<usize, &'static str> {
    let mbase = exe_base_addr();
    if mbase == 0 {
        return Err("module 0");
    }
    let fn_addr = mbase + rva;
    if !readable(fn_addr, orig_len + 4) {
        return Err("fn unreadable");
    }
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let stub = VirtualAlloc(0, 256, MEM_CR, RWX);
    if stub == 0 {
        return Err("VirtualAlloc");
    }
    let ret_addr = fn_addr + orig_len;
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(&[0x49, 0x89, 0xe2]);
    s.extend_from_slice(&[
        0x50, 0x41, 0x53, 0x41, 0x52, 0x41, 0x51, 0x41, 0x50, 0x52, 0x51,
    ]);
    s.extend_from_slice(&[0x48, 0x89, 0xe1]);
    s.extend_from_slice(&[0x4c, 0x89, 0xd2]);
    s.extend_from_slice(&[0x48, 0x83, 0xec, 0x20]);
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&cap_fn.to_le_bytes());
    s.extend_from_slice(&[0xff, 0xd0]);
    s.extend_from_slice(&[0x48, 0x83, 0xc4, 0x20]);
    s.extend_from_slice(&[0x48, 0x85, 0xc0]);
    s.extend_from_slice(&[0x74, 0x0c]);
    s.extend_from_slice(&[
        0x59, 0x5a, 0x41, 0x58, 0x41, 0x59, 0x41, 0x5a, 0x41, 0x5b, 0x58, 0xc3,
    ]); // HANDLED: pop..ret
    s.extend_from_slice(&[
        0x59, 0x5a, 0x41, 0x58, 0x41, 0x59, 0x41, 0x5a, 0x41, 0x5b, 0x58,
    ]); // PASSTHROUGH: pop
    let mut orig = vec![0u8; orig_len];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, orig.as_mut_ptr(), orig_len);
    s.extend_from_slice(&orig);
    s.extend_from_slice(&[0xff, 0x25, 0x00, 0x00, 0x00, 0x00]);
    s.extend_from_slice(&ret_addr.to_le_bytes());
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    let mut patch = vec![0x90u8; orig_len];
    patch[0] = 0x48;
    patch[1] = 0xb8;
    patch[2..10].copy_from_slice(&stub.to_le_bytes());
    patch[10] = 0xff;
    patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, orig_len, RWX, &mut old) == 0 {
        return Err("VirtualProtect");
    }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, orig_len);
    VirtualProtect(fn_addr, orig_len, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, orig_len);
    Ok(stub)
}
// ===========================================================================
//  ** Direct scene reading = a deterministic team gate (no hooking; ghidra-re + crm anchors, confirmed on 0.5.0_3)
//  During a live match, the client scene (ClientScene::InGame, tag=9) keeps both teams' team_id + is_team1_blue in its match_info.
//  Read directly every frame in post_update (main thread) -> compare with player_team_id() -> determine the PLAYER SIDE (0/1).
//  Absolute db offsets (0.5.0_3, a uniform -0xA0 shift, triple-checked): scene tag (u32)@+0x1338 == 9 /
//  team1 tag(u64,Normal=0)@+0x17A0·id@+0x17A8 / team2 tag@+0x17C0·id@+0x17C8 / is_team1_blue(u8)@+0x1900.
//  is_team1_blue is updated to reflect the per-set side swap -> reading it always reflects the current set.
//  WARNING the old GameStart packet deserializer hook (0x3217f0) is a dead end (never fires in a single live process; crossbeam delivers directly) -> removed.
// ===========================================================================
const SCENE_GATE_ENABLED: bool = true; // * v5 (07-11): after confirming live (tid), decide the side by reading the scene directly -> ON. update_scene_side refreshes SCENE_SIDE every frame (main thread).
                                       // ** Confirmed (07-11, in game): the sim athlete+0x820 side is fixed at blue=0 / red=1. In a spectated match (my team blue), KT Aiming = meiling was
                                       //   dumped as sim side1 (red) -> confirming side0 = blue = my team. So the scene player <-> sim side mapping = blue is side0.
                                       //   WARNING this is a side-independent fixed mapping (not a constant inversion) - matching scene team_id <-> pid returns the correct sim side even when sides swap.
const SCENE_BLUE_IS_SIDE0: bool = true; // blue team = sim side0 (confirmed in game). update_scene_side matches pid with (s0,s1) = (blue,red).
static SCENE_SIDE: AtomicU64 = AtomicU64::new(u64::MAX); // 0/1 = the player's side in a live match, MAX = undetermined (not a match / not spectating)
static LIVE_DB: AtomicU64 = AtomicU64::new(0); // * v6: the absolute db address stored by the InGame post_update (for the spawn hook's early side decision)
static LIVE_PID: AtomicU64 = AtomicU64::new(u64::MAX); // * v6: the stored PLAYER_TEAM_ID
static SPAWN_SCENE_OK: AtomicU64 = AtomicU64::new(0); // diagnostic: successful early side decisions in the spawn hook
static SPAWN_NO_DB: AtomicU64 = AtomicU64::new(0); // diagnostic: no LIVE_DB at spawn time (spawn before InGame)
                                                   // * v6 lightweight side-only decision (called from the spawn hook = a sim thread; VEH-safe reads only, no file I/O or locks).
                                                   //   scene tag9 + team_id Normal + is_team1_blue + pid matching -> player side (0/1). Same offsets as update_scene_side.
unsafe fn quick_scene_side(db: usize, pid: u64) -> Option<u64> {
    if db < 0x10000 || pid == u64::MAX {
        return None;
    }
    if safe_read_u64(db + 0x1338).map(|v| v & 0xffff_ffff) != Some(9) {
        return None;
    }
    let t1_tag = safe_read_u64(db + 0x17A0)?;
    let t2_tag = safe_read_u64(db + 0x17C0)?;
    if t1_tag != 0 || t2_tag != 0 {
        return None;
    } // Normal (team_id) only
    let t1 = safe_read_u64(db + 0x17A8)?;
    let t2 = safe_read_u64(db + 0x17C8)?;
    let blue_b = safe_read_u64(db + 0x1900)? & 0xff;
    let t1_blue = blue_b != 0;
    let (blue, red) = if t1_blue { (t1, t2) } else { (t2, t1) };
    let (s0, s1) = if SCENE_BLUE_IS_SIDE0 {
        (blue, red)
    } else {
        (red, blue)
    };
    if s0 == pid {
        Some(0)
    } else if s1 == pid {
        Some(1)
    } else {
        None
    }
}
static PID_EVER_VALID: AtomicU64 = AtomicU64::new(0); // has player_team_id() ever returned a valid value (1~9999)?
                                                      // * 2026-07-30: have we ever seen a valid **non-zero** pid? If 1, ignore later reports of 0 (prevents pid regression - measurement showed
                                                      //   the same save alternating between 105 and 0 depending on the moment, and trusting the 0 breaks the team gate).
static PID_NONZERO_SEEN: AtomicU64 = AtomicU64::new(0);
static MY_PT_N: AtomicU64 = AtomicU64::new(0); // number of champion_personal_tactics entries of the team to publish (for validating "my team")
static MY_TRUST_SKIP: AtomicU64 = AtomicU64::new(0); // times MY_ATHLETES publication was withheld due to pid=0 + insufficient PT
static PID_OBS_ZERO: AtomicU64 = AtomicU64::new(0); // observations where player_team_id() returned 0
static PID_OBS_NONZERO: AtomicU64 = AtomicU64::new(0); // observations of a valid non-zero value
static PID_SKIP_CT: AtomicU64 = AtomicU64::new(0); // times a 0 report was ignored in comp-test context (during the sim or with the popup open)
static PID_ZERO_CLEAN: AtomicU64 = AtomicU64::new(0); // * times pid=0 was observed in an InGame unrelated to comp test
                                                      //   (>=600 accepts it as "a save whose real team id is 0" = MY_ATHLETES publication allowed)
static SCENE_DIAG_LAST: AtomicU64 = AtomicU64::new(u64::MAX); // diagnostic state fingerprint (rewrite the file only when it changes)
static LINK_SCAN_DONE: AtomicBool = AtomicBool::new(true); // pointer scan abandoned (closed after confirming the match_id was a coincidental hit)
static BUY_SIMS: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new()); // SimStates (base-0x840) + champ0 seen in the buy hook (while deciding the scene)
                                                                       // === FLOW diagnostic: scene tag transitions + SimState birth / tag9 activity timeline (the whole flow of one match) ===
static CUR_TAG: AtomicU64 = AtomicU64::new(u64::MAX);
static FLOW_SIMS: Mutex<Vec<(usize, u64, bool)>> = Mutex::new(Vec::new()); // (sim, first_tag, tag9_logged)
fn scene_player_side() -> Option<u64> {
    if !SCENE_GATE_ENABLED {
        return None;
    } // when OFF, use the roster fallback
    match SCENE_SIDE.load(Ordering::Relaxed) {
        v @ 0..=1 => Some(v),
        _ => None,
    }
}
// * Raw scene values (for diagnostic dumps) - refreshed every frame by update_scene_side.
static SCENE_T1: AtomicU64 = AtomicU64::new(u64::MAX);
static SCENE_T2: AtomicU64 = AtomicU64::new(u64::MAX);
static SCENE_BLUEB: AtomicU64 = AtomicU64::new(u64::MAX);
// ** Typed hedge (07-11): SDK db.replay_view -> match_replays -> blue/red_team_id (canonical side0=blue / side1=red, MatchReplayData).
//   Cross-checked against the direct scene read (SCENE_T1/T2/BLUEB) = verifies "do the two sources give exactly the same team_id". Behind the DIAG_ENABLED gate.
//   WARNING MatchReplayData describes a finished/recorded match, so it may be unrecorded while live (MRD=MAX) -> the comparison is only valid after the match is recorded.
const DIAG_BUY_OFF: bool = false; // master switch for buy injection (true = injection/identification OFF)
fn install_replace_4th() {
    if DIAG_BUY_OFF {
        return;
    }
    if BUY_PROBE_INSTALLED.load(Ordering::Relaxed) != 0 {
        return;
    }
    let base = unsafe { GetModuleHandleW(core::ptr::null()) } as usize;
    if base == 0 {
        return;
    }
    let fn_addr = base + RVA_BUY_ITEM;
    let ok = unsafe { readable(fn_addr, 12) }
        && (0..12).all(|i| unsafe { *((fn_addr + i) as *const u8) } == BUY_PROLOGUE[i]);
    if !ok {
        // State 2 = signature moved (re-derive RVA_BUY_ITEM/BUY_PROLOGUE);
        // state 3 below = signature matched but the trampoline install failed.
        // Different causes, different fixes, so they are not merged — and the
        // observed bytes are recorded because that is what re-deriving needs.
        BUY_PROBE_INSTALLED.store(2, Ordering::Relaxed);
        let seen: Vec<String> = (0..12)
            .map(|i| match unsafe { safe_read_u64(fn_addr + i) } {
                Some(w) => format!("{:02x}", (w & 0xff) as u8),
                None => "??".to_string(),
            })
            .collect();
        let expected: Vec<String> = BUY_PROLOGUE.iter().map(|b| format!("{b:02x}")).collect();
        *BUY_INSTALL_NOTE.lock().unwrap_or_else(|e| e.into_inner()) = format!(
            "buy_item prologue mismatch at rva={RVA_BUY_ITEM:#x}\n    expected {}\n    saw      {}",
            expected.join(" "),
            seen.join(" ")
        );
        return;
    }
    // orig_len=19: the new 0.5.1 prologue, 5 push (7) + sub rsp,0x50 (4) = 11B, cannot cover the 12B jmp patch -> relocate to the next clean boundary, 11 + mov rax,[rsp+0xa8] (8) = 19B.
    match unsafe { install_replace_buy(RVA_BUY_ITEM, 19, buy_replace_ctx as usize) } {
        Ok(_) => BUY_PROBE_INSTALLED.store(1, Ordering::Relaxed),
        Err(e) => {
            BUY_PROBE_INSTALLED.store(3, Ordering::Relaxed);
            *BUY_INSTALL_NOTE.lock().unwrap_or_else(|e| e.into_inner()) =
                format!("buy_item prologue matched but install_replace_buy failed: {e}");
        }
    }
}

/// Why `install_replace_4th` gave up, for the diagnostic report. Empty until it
/// fails. Held as a string rather than logged because `append_log` is gated on
/// `LOG_ENABLED`, which is off in production — and this is exactly the failure
/// that makes every other counter read zero.
static BUY_INSTALL_NOTE: Mutex<String> = Mutex::new(String::new());

// owned 3-cap patch: change the imm8 3 -> 4 of `cmp qword[rax+0x3d0], 3` inside run_tick_ext (which skips stat application above 3 items).
//   So the 4th item's stats apply. (0.4.14 + hotfix RVA, [[tfm2-item-slot-count]] patch (1))
unsafe fn patch_owned_cap() -> String {
    let base = exe_base_addr();
    // 0.5.0: cmp qword[rsi+0x458],3 = 48 83 BE 58 04 00 00 03 (0.4.14 was cmp [rax+0x3d0],3). imm8 @ sig+7.
    // 0.5.2 (2026-07-22): the container (0x2234430 -> 0x233e9d0) was refactored and the **register went RSI -> R15** (48 83 be -> 49 83 bf).
    //   The disp (struct offset 0x458) and imm (3) are unchanged. The form `cmp qword[reg+0x458],3` occurs exactly once in the whole new exe.
    // 0.5.3 (2026-07-29): the register went back R15 -> **RSI** (49 83 bf -> 48 83 be). disp 0x458 and imm 3 unchanged.
    //   The form `cmp qword[reg+0x458],3` occurs **exactly once** in the whole new exe .text (verified by byte scan) = misidentification impossible.
    let sig = base + 0x1420b29; // 0.5.4 (0.5.3 was 0xf24a39). (0.5.2 was 0x2341440). Container 0x233e9d0 -> 0xf21fe0.
    let imm = base + 0x1420b30; // the cmp's imm8 (= sig+7)
                                // 0.5.4: the athlete's items-Vec len moved 0x458 -> 0x448, so the disp changed with it. Still RSI, still
                                //   `cmp qword[rsi+<items len>],3`, and still **exactly one** occurrence in the whole .text (byte-scanned).
    let expect = [0x48u8, 0x83, 0xbe, 0x48, 0x04, 0x00, 0x00, 0x03];
    if !readable(sig, 8) {
        return "owned_cap: unreadable".into();
    }
    for i in 0..8 {
        if *((sig + i) as *const u8) != expect[i] {
            return format!(
                "owned_cap: sig mismatch @+{} = {:#04x}",
                i,
                *((sig + i) as *const u8)
            );
        }
    }
    const RWX: u32 = 0x40;
    let mut old = 0u32;
    if VirtualProtect(imm, 1, RWX, &mut old) == 0 {
        return "owned_cap: VirtualProtect fail".into();
    }
    *(imm as *mut u8) = 0x04;
    VirtualProtect(imm, 1, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), imm, 1);
    "owned_cap: patched 3->4".into()
}

// * Slot 4 natural purchase gate patch: the owned>2-only has_recipe gate inside the resolver FUN_142052dd0
//   `0x142052e76 jbe` (0x76) -> `jmp` (0xEB). owned>2 now takes the same path as owned<=2 (slots 1~3) = natural
//   build-up from components is allowed. No effect on owned<=2 (the jbe skipped there anyway, same destination). With build_len<4 gate (1)
//   stops first, so slot=3 and other mods are unaffected. (ghidra-re confirmed, 0.4.14 + hotfix)
unsafe fn patch_gate3() -> String {
    let base = exe_base_addr();
    // 0.5.0: jbe @ 0x1e4bd36 (was 0x2052e76). sig start = jbe-9. 76 -> EB (JMP) disables the owned>2 gate.
    let sig = base + 0xe76b1e; // 0.5.4 (0.5.3 was 0xd0c9be). (0.5.2 was 0x211e428): resolver container 0x211e150 -> **0xd0c770** (called directly by buy 0xd0c680). The spill slot moved rsp+0x78 -> **rsp+0x40**, and the form `cmp qword[rsp+0x40],2; jbe` occurs **exactly once** in the whole new exe (verified by byte scan). History for 0.5.2: (0.5.1 was 0x1f01448): resolver container 0x1f01170 -> 0x211e150 (skeleton UNIQUE, +0x21cfe0), same offset +0x2d8, the 7B signature byte-identical (BYTE-OK). History for 0.5.1: (0.5.0_3 was 0x1fb8cdd, ghidra-re HIGH re-ID). Inside the resolver's successor FUN_141f01170. owned_count spilled to [rsp+0x78] so the sequence was rewritten as 'cmp qword[rsp+0x78],2; jbe' (previously 'mov rsi,[rsp+0x40]; jbe').
    let jbe = base + 0xe76b24; // the 0.5.4 jbe opcode byte (= sig+6, verified; 0.5.2 was 0x211e42e). owned<=2 -> jump, >2 -> fall through (the extra has_recipe check).
    let expect = [0x48u8, 0x83, 0x7c, 0x24, 0x40, 0x02, 0x76]; // 0.5.3: cmp qword[rsp+0x40],2 ; jbe (0.5.2 was rsp+0x78)
    if !readable(sig, 7) {
        return "gate3: unreadable".into();
    }
    for i in 0..7 {
        if *((sig + i) as *const u8) != expect[i] {
            return format!(
                "gate3: sig mismatch @+{} = {:#04x}",
                i,
                *((sig + i) as *const u8)
            );
        }
    }
    const RWX: u32 = 0x40;
    let mut old = 0u32;
    if VirtualProtect(jbe, 1, RWX, &mut old) == 0 {
        return "gate3: VirtualProtect fail".into();
    }
    *(jbe as *mut u8) = 0xEB;
    VirtualProtect(jbe, 1, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), jbe, 1);
    "gate3: patched jbe->jmp (owned>2 gate neutralised)".into()
}

// * AI auto-recommended 4th: raise the beam depth limit literal 2 -> 3 (0,1,2,3 = 4 iterations -> beam computes a 4-item build).
//   Two sites (entry guard 0x19f14a5, back edge 0x19f1a11), both `cmp r8d,2` (41 83 f8 02) imm8 02 -> 03. Both are required.
//   (extractor RE: the slot write is only a personal_tactics override, and the 4th stays auto so the beam value is kept -> raising the depth is enough.)
// * Diagnostic (title-return crash bisection #2): beam_depth OFF. The AUTO 4th is decided by forward at buy time (compute_auto_4th_id),
//   so beam_depth (2->3) is legacy - turning it off means the beam does not build 4-item builds (removing the suspicion of an internal buffer OOB). Buying 4 items still works.
const AUTO4_BEAM_DEPTH: bool = false;
unsafe fn patch_beam_depth() -> String {
    let base = exe_base_addr();
    let mut msgs = Vec::new();
    // WARNING STALE for 0.5.2/0.5.3: these two addresses already mismatched their signatures back on 0.5.1 (= patch skipped, fail-safe) and AUTO4_BEAM_DEPTH=false means they never run.
    for (name, rva) in [("A", 0x19f14a5usize), ("B", 0x19f1a11usize)] {
        let addr = base + rva;
        if !readable(addr, 4) {
            msgs.push(format!("{}:unreadable", name));
            continue;
        }
        let b = [
            *(addr as *const u8),
            *((addr + 1) as *const u8),
            *((addr + 2) as *const u8),
            *((addr + 3) as *const u8),
        ];
        if b != [0x41, 0x83, 0xf8, 0x02] {
            // cmp r8d, 2
            msgs.push(format!(
                "{}:mismatch[{:02x} {:02x} {:02x} {:02x}]",
                name, b[0], b[1], b[2], b[3]
            ));
            continue;
        }
        const RWX: u32 = 0x40;
        let mut old = 0u32;
        if VirtualProtect(addr + 3, 1, RWX, &mut old) == 0 {
            msgs.push(format!("{}:vprot", name));
            continue;
        }
        *((addr + 3) as *mut u8) = 0x03;
        VirtualProtect(addr + 3, 1, old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), addr + 3, 1);
        msgs.push(format!("{}:OK(02→03)", name));
    }
    format!("beam_depth: {}", msgs.join(", "))
}

// * Disabling the candidate gate: make FUN_141a35490 (the recipe/count filter) always-true (mov al,1; ret) ->
//   4th candidates survive even for a completed 3-item build -> the beam produces 4-item builds (the network picks the best 4th).
//   Single call site (0x142145ce0); side effects are confined to beam candidate expansion (RE checked).
const CAND_GATE_ON: bool = false; // * OFF: always-true breaks beam build generation (measured beam4 -> 0)
const CAND_GATE_RVA: usize = 0x1a3b280; // WARNING STALE for 0.5.2/0.5.3 (exe2exe NO MATCH; harmless because CAND_GATE_ON=false) // 0.5.0_3 (0.5.0_2 was 0x1a35490; monomorphic triple, anchor region 0x1a3bxxx confirmed). CAND_GATE_ON=false (OFF, prologue self-guard)
unsafe fn patch_cand_gate() -> String {
    let base = exe_base_addr();
    let addr = base + CAND_GATE_RVA;
    if !readable(addr, 3) {
        return "cand_gate: unreadable".into();
    }
    let b0 = *(addr as *const u8);
    if b0 == 0xB0 {
        return "cand_gate: already".into();
    }
    // Prologue sanity: is the function's first byte a common prologue (push/sub/mov, i.e. 0x40~0x57 / 0x48 etc.)? If not, abort.
    if !(b0 == 0x48
        || b0 == 0x40
        || b0 == 0x55
        || b0 == 0x53
        || b0 == 0x56
        || b0 == 0x57
        || (0x41..=0x41).contains(&b0))
    {
        return format!("cand_gate: prologue?[{:02x}] abort", b0);
    }
    const RWX: u32 = 0x40;
    let mut old = 0u32;
    if VirtualProtect(addr, 3, RWX, &mut old) == 0 {
        return "cand_gate: vprot".into();
    }
    *(addr as *mut u8) = 0xB0; // mov al, 1
    *((addr + 1) as *mut u8) = 0x01;
    *((addr + 2) as *mut u8) = 0xC3; // ret
    VirtualProtect(addr, 3, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), addr, 3);
    format!("cand_gate: patched always-true (was {:02x})", b0)
}

// == In-match UI 4th slot icon display (slot loop bound patch + slot path helper) ==========
const RVA_SLOT_HELPER: usize = 0xc5cd80; // WARNING **no 0.5.3 equivalent (the function vanished = inlined into the mega-function 0xa5c1e0)**. Unused and harmless because DIAG_SLOT_UI_OFF=true. The value is the 0.5.2 one, kept for history. Conditions for resuming = see the DIAG_SLOT_UI_OFF comment above.
const BLUE_SLOTS: [&[u8]; 4] = [
    b"blue_player.slot0",
    b"blue_player.slot1",
    b"blue_player.slot2",
    b"blue_player.slot3",
];
const RED_SLOTS: [&[u8]; 4] = [
    b"red_player.slot0",
    b"red_player.slot1",
    b"red_player.slot2",
    b"red_player.slot3",
];
// The 4 slot icon loop bounds (blue/red x windowed/fullscreen; cmp reg,0x30 -> 0x40, imm@+3).
// cmp start address, imm (0x30 -> 0x40) @ +3. (The old 0.5.0_2 values 0x54b760/0x54bad0/0x54c1b0/0x54c520 were all misidentifications.)
// Confirmed on 0.5.0_3 (ghidra-re re-search): inside the UI render mega-function 0x414800..0x42b4c5 there are exactly these 4 `cmp reg,0x30`
//   = blue/red x windowed/fullscreen, 4 loops. patch_slot_ui pre-validates the signature -> skips on mismatch (fail-safe, no crash).
// 0.5.1 (0.5.0_3 was 0x4186d0/0x418a40/0x419120/0x419490, all mask-sig UNIQUE win=0x60).
// 0.5.2 (2026-07-22 exe2exe): the container UI mega-function 0x4b0e70 -> 0x4e07f0 (skeleton UNIQUE, +0x2f980), remapped at the same offsets
//   (+0x3ed0/+0x4240/+0x4920/+0x4c90) - all 4 confirmed byte-identical between old and new addresses (BYTE-OK). 0.5.1 was 0x4b4d40/0x4b50b0/0x4b5790/0x4b5b00.
// * 0.5.3 re-pin done (2026-07-29; an exhaustive search for `cmp reg,0x30` inside container 0x4e07f0 -> **0xa5c1e0** = exactly 4, all RBX):
//   0xa63166 / 0xa638df / 0xa64486 / 0xa64c16, all measured as `48 83 fb 30`. The imm position is still +3.
//   WARNING but DIAG_SLOT_UI_OFF=true means they are **not applied** (comment (2) above = the 4th entry's slot collides with another local -> crash).
//   The values are kept only to avoid re-investigating when a redesign starts.
const SLOT_BOUNDS: [(usize, [u8; 4]); 4] = [
    (0xa63166, [0x48, 0x83, 0xfb, 0x30]), // 0.5.3 blue (windowed)   - 0.5.2 was 0x4e46c0 cmp r14
    (0xa638df, [0x48, 0x83, 0xfb, 0x30]), // 0.5.3 red (windowed)    - 0.5.2 was 0x4e4a30 cmp r15
    (0xa64486, [0x48, 0x83, 0xfb, 0x30]), // 0.5.3 blue (fullscreen) - 0.5.2 was 0x4e5110 cmp r14
    (0xa64c16, [0x48, 0x83, 0xfb, 0x30]), // 0.5.3 red (fullscreen)  - 0.5.2 was 0x4e5480 cmp r14
];
unsafe extern "C" fn fill_slots(buf: *mut u64, len: u64) -> *mut u64 {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if buf.is_null() {
            return;
        }
        let slots: &[&[u8]; 4] = if len == 11 {
            &BLUE_SLOTS
        } else if len == 10 {
            &RED_SLOTS
        } else {
            return;
        };
        for i in 0..4 {
            *buf.add(i * 2) = slots[i].as_ptr() as u64;
            *buf.add(i * 2 + 1) = slots[i].len() as u64;
        }
    }));
    buf
}
unsafe fn install_helper_replace(rva: usize, cap_fn: usize) -> Result<usize, &'static str> {
    let mbase = exe_base_addr();
    let fn_addr = mbase + rva;
    if !readable(fn_addr, 16) {
        return Err("unreadable");
    }
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let stub = VirtualAlloc(0, 64, MEM_CR, RWX);
    if stub == 0 {
        return Err("VirtualAlloc");
    }
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]); // sub rsp,0x28
    s.extend_from_slice(&[0x4c, 0x89, 0xc2]); // mov rdx, r8 (len)
    s.extend_from_slice(&[0x48, 0xb8]);
    s.extend_from_slice(&cap_fn.to_le_bytes());
    s.extend_from_slice(&[0xff, 0xd0]); // call rax (rcx=buf)
    s.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]); // add rsp,0x28
    s.extend_from_slice(&[0xc3]);
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    let mut patch = [0u8; 12];
    patch[0] = 0x48;
    patch[1] = 0xb8;
    patch[2..10].copy_from_slice(&stub.to_le_bytes());
    patch[10] = 0xff;
    patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, 12, RWX, &mut old) == 0 {
        return Err("VirtualProtect");
    }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, 12);
    VirtualProtect(fn_addr, 12, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, 12);
    Ok(stub)
}
static SLOTUI_DONE: AtomicBool = AtomicBool::new(false);
// ═══════════════════════════════════════════════════════════════════════════
//  ** 0.5.3 slot UI restoration - the "frame extension + array relocation" surgery (2026-07-30)
// ═══════════════════════════════════════════════════════════════════════════
//  Why the 0.5.2 approach (replace the helper + raise the bound 0x30 -> 0x40) does not work:
//    (1) the slot-name helper (formerly 0xc5cd80) is **inlined into** the UI mega-function 0xa5c1e0 in 0.5.3 and no longer exists.
//    (2) the game **reuses the 4th entry's slots rbp+0x10d50/0x10d58 for other locals** (67 references across the function,
//       4 of them **inside** the loops: 0xa6339f `cmp rdi,[rbp+0x10d50]` etc.) => raising only the bound would dereference that integer
//       as a string (ptr,len) = a guaranteed crash.
//  The solution (this function): relocate the array itself to **a new region at the top of the frame (rbp+0x10f80, 64B)**.
//    A) prologue `mov eax,0x11008` -> `0x11048`, **extending the stack frame by +0x40** (keeping the chkstk argument and 16B alignment).
//       => rbp drops by 0x40, so **every intra-frame reference follows automatically** (all rbp+disp are relative).
//    B) Only the 13 references to the **caller's stack** (the 5th argument = entry rsp+0x28 = rbp+0x10ff0) are absolute and must be
//       corrected to `+0x11030`. * Measured beforehand: **zero** `[rsp+X]` references with X >= 0x100 =
//       everything rsp-relative is shadow space/locals, so frame extension is safe (this check is the premise of the surgery).
//    C) Replace the 4 inlined init blocks (75B each, writing only 3 pairs) **wholesale with stubs** -> the stub writes **all 4 pairs** at the new base.
//       The stub is a pure sequence of movs that clobbers only rax (no calls, no stack use) - rax is dead on entry to an init block
//       (its first instruction is `lea rax,..`) and dead at the return point too (the next definition is `mov rax,[rbp+rbx+...]`) => safe.
//    D) Point the disp32 of the 8 loop-indexing sites at the new base, and change the 4 bounds 0x30 -> 0x40.
//  WARNING all-or-nothing: **validate every site's signature first** and if even one mismatches, **touch nothing**
//    (a partial patch = a bigger frame with the array still in the old place -> instant death). This is a hot render function with no undo, so this rule is the lifeline.
// NO **failed in game 2026-07-30 -> immediately OFF**: reproduced a freeze right after a match starts. Even with all 84/84 sites passing signature
//   validation, that means **something still breaks runtime consistency** (see the failure record below). Do not re-enable before the cause is identified.
const SLOT_UI_SURGERY: bool = false; // * master switch for the 0.5.3 slot UI surgery - OFF after failure (only the icon is missing; everything else is fine)
const UI_MEGA_PROLOGUE_IMM: usize = 0xa5c1ed; // position of the imm32 in `mov eax,imm32`
const UI_FRAME_OLD: u32 = 0x11008;
const UI_FRAME_NEW: u32 = 0x11048; // +0x40 (keeps 16B alignment)
const UI_ARG5_OLD: u32 = 0x10ff0; // the 5th argument = entry rsp+0x28
const UI_ARG5_NEW: u32 = 0x11030; // a 0x40 bigger frame drops rbp by 0x40, hence +0x40
const UI_SLOT_BASE_OLD: u32 = 0x10d20; // old array base (shared with other variables by the game)
const UI_SLOT_BASE_NEW: u32 = 0x10f80; // new array base (above the xmm15 spill at 0x10f70, 64B, exclusively ours)
                                       // **68** references to the 5th argument - all `48|4c 8b <modrm> f0 0f 01 00` (mov r64,[rbp+0x10ff0]), length 7, disp32 @ +3.
                                       //   WARNING **an exhaustive count is mandatory**: we first miscounted 13 (mistaking a truncated output list for the whole thing). Missing even one makes
                                       //   that instruction read the wrong place in the caller's stack after the frame extension = **instant death**. Measured = these 68 are all of them, and
                                       //   there is no other disp at or above 0x10f88 (the frame limit). The REX byte is not only 0x48 but also **0x4c** (targeting r8/r11), so
                                       //   the validation below accepts both.
const UI_ARG5_SITES: [usize; 68] = [
    0xa5c3a6, 0xa5c57f, 0xa5ca03, 0xa5cbfc, 0xa5d416, 0xa5d44d, 0xa5d621, 0xa62821, 0xa62ad5,
    0xa62b3f, 0xa62eab, 0xa641b1, 0xa64280, 0xa65274, 0xa652f0, 0xa65355, 0xa6550e, 0xa6638e,
    0xa66430, 0xa665d4, 0xa665f7, 0xa667e4, 0xa6681e, 0xa66d06, 0xa67367, 0xa675ee, 0xa68777,
    0xa68e07, 0xa6908e, 0xa6a5eb, 0xa6abd1, 0xa6afbf, 0xa6b3ad, 0xa6b45c, 0xa6b5d8, 0xa6c404,
    0xa6c4b4, 0xa6c65f, 0xa6c682, 0xa6c860, 0xa6c89a, 0xa6cd7b, 0xa6d3cb, 0xa6d645, 0xa6e7ce,
    0xa6ee57, 0xa6f0ca, 0xa7069b, 0xa70c91, 0xa7107f, 0xa7146d, 0xa7151c, 0xa72e71, 0xa73092,
    0xa732b3, 0xa734d4, 0xa7369d, 0xa737fe, 0xa73a34, 0xa73cda, 0xa740de, 0xa74e7f, 0xa7529d,
    0xa754ef, 0xa7558f, 0xa7562b, 0xa75a8c, 0xa77a62,
];
// The 4 inlined init blocks: (start, length, is_blue) - blue len=0x11 ("blue_player.slotN"), red len=0x10
const UI_INIT_BLOCKS: [(usize, usize, bool); 4] = [
    (0xa630c2, 0x4b, true),  // blue (windowed)
    (0xa6384a, 0x4b, false), // red  (windowed)
    (0xa643e3, 0x4b, true),  // blue (fullscreen)
    (0xa64b6a, 0x4b, false), // red  (fullscreen)
];
// The 8 loop-indexing sites - `48 8b 84 1d <disp32>` (rax) / `48 8b 8c 1d <disp32>` (rcx), disp32 @ +4
const UI_LOOP_SITES: [(usize, u32); 8] = [
    (0xa63173, 0x10d20),
    (0xa63182, 0x10d28),
    (0xa638ec, 0x10d20),
    (0xa638fb, 0x10d28),
    (0xa64493, 0x10d20),
    (0xa644a2, 0x10d28),
    (0xa64c23, 0x10d20),
    (0xa64c32, 0x10d28),
];
static SLOTUI_STUBS: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
static SLOTUI_MSG: Mutex<Option<String>> = Mutex::new(None); // surgery result (exposed in the diagnostic dump - regardless of LOG_ENABLED)

// Byte-signature check for one site.
unsafe fn sig_at(addr: usize, want: &[u8]) -> bool {
    if !readable(addr, want.len()) {
        return false;
    }
    (0..want.len()).all(|i| *((addr + i) as *const u8) == want[i])
}
unsafe fn write_bytes(addr: usize, data: &[u8]) -> bool {
    const RWX: u32 = 0x40;
    let mut old = 0u32;
    if VirtualProtect(addr, data.len(), RWX, &mut old) == 0 {
        return false;
    }
    core::ptr::copy_nonoverlapping(data.as_ptr(), addr as *mut u8, data.len());
    VirtualProtect(addr, data.len(), old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), addr, data.len());
    true
}
// Init-block replacement stub: write the 4 (ptr,len) pairs at the new base and jump back to the end of the block. Uses only rax.
unsafe fn build_slot_stub(blue: bool, ret_addr: usize) -> Option<usize> {
    const MEM_CR: u32 = 0x1000 | 0x2000;
    const RWX: u32 = 0x40;
    let mem = VirtualAlloc(0, 256, MEM_CR, RWX);
    if mem == 0 {
        return None;
    }
    let names: &[&[u8]; 4] = if blue { &BLUE_SLOTS } else { &RED_SLOTS };
    let mut s: Vec<u8> = Vec::with_capacity(160);
    for i in 0..4 {
        let d_ptr = UI_SLOT_BASE_NEW + (i as u32) * 0x10;
        let d_len = d_ptr + 8;
        s.extend_from_slice(&[0x48, 0xb8]); // movabs rax, <str ptr>
        s.extend_from_slice(&(names[i].as_ptr() as u64).to_le_bytes());
        s.extend_from_slice(&[0x48, 0x89, 0x85]); // mov [rbp+d_ptr], rax
        s.extend_from_slice(&d_ptr.to_le_bytes());
        s.extend_from_slice(&[0x48, 0xc7, 0x85]); // mov qword [rbp+d_len], imm32
        s.extend_from_slice(&d_len.to_le_bytes());
        s.extend_from_slice(&(names[i].len() as u32).to_le_bytes());
    }
    s.extend_from_slice(&[0x48, 0xb8]); // movabs rax, ret
    s.extend_from_slice(&(ret_addr as u64).to_le_bytes());
    s.extend_from_slice(&[0xff, 0xe0]); // jmp rax
    if s.len() > 256 {
        return None;
    }
    core::ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, s.len());
    Some(mem)
}
unsafe fn patch_slot_ui() -> String {
    let r = patch_slot_ui_inner();
    *SLOTUI_MSG.lock().unwrap_or_else(|e| e.into_inner()) = Some(r.clone());
    r
}
unsafe fn patch_slot_ui_inner() -> String {
    if !SLOT_UI_SURGERY {
        return "slot_ui: surgery OFF".into();
    }
    if SLOTUI_DONE.swap(true, Ordering::Relaxed) {
        return "slot_ui: already".into();
    }
    let base = exe_base_addr();
    if base == 0 {
        return "slot_ui: no base".into();
    }

    // -- (1) Pre-validate every site (all-or-nothing) ------------------------------
    // frame imm32
    if !sig_at(base + UI_MEGA_PROLOGUE_IMM, &UI_FRAME_OLD.to_le_bytes()) {
        return "slot_ui: ABORT(frame imm mismatch) - not applied".into();
    }
    // the 13 references to the 5th argument: `48 8b ?? f0 0f 01 00`
    for &r in UI_ARG5_SITES.iter() {
        let a = base + r;
        // REX = 0x48 or 0x4c (targeting r8/r11), opcode 0x8b, disp32 @ +3
        let rex_ok = readable(a, 2)
            && matches!(*(a as *const u8), 0x48 | 0x4c)
            && *((a + 1) as *const u8) == 0x8b;
        if !(rex_ok && sig_at(a + 3, &UI_ARG5_OLD.to_le_bytes())) {
            return format!("slot_ui: ABORT(arg5 {:#x} mismatch) - not applied", r);
        }
    }
    // the 8 loop-indexing sites: `48 8b <modrm> 1d <disp32>` (rbp+rbx indexing), disp32 @ +4
    for &(r, d) in UI_LOOP_SITES.iter() {
        let a = base + r;
        if !(sig_at(a, &[0x48, 0x8b]) && sig_at(a + 3, &[0x1d]) && sig_at(a + 4, &d.to_le_bytes()))
        {
            return format!("slot_ui: ABORT(loop {:#x} mismatch) - not applied", r);
        }
    }
    // the 4 loop bounds
    for (r, sig) in SLOT_BOUNDS.iter() {
        if !sig_at(base + r, sig) {
            return format!("slot_ui: ABORT(bound {:#x} mismatch) - not applied", r);
        }
    }
    // the 4 init blocks: is the first instruction `lea rax,[rip+..]` (48 8d 05)?
    for &(r, _l, _b) in UI_INIT_BLOCKS.iter() {
        if !sig_at(base + r, &[0x48, 0x8d, 0x05]) {
            return format!("slot_ui: ABORT(init {:#x} mismatch) - not applied", r);
        }
    }
    // Prepare the 4 stubs (abort if any fails - the game's code is still untouched at this point)
    let mut stubs = [0usize; 4];
    for (i, &(r, l, blue)) in UI_INIT_BLOCKS.iter().enumerate() {
        match build_slot_stub(blue, base + r + l) {
            Some(m) => {
                stubs[i] = m;
                SLOTUI_STUBS[i].store(m, Ordering::Relaxed);
            }
            None => return "slot_ui: ABORT(stub alloc) - not applied".into(),
        }
    }

    // -- (2) Apply (from here on everything must succeed for consistency) ------------
    let mut done = 0;
    // loop indexing -> the new base
    for &(r, d) in UI_LOOP_SITES.iter() {
        let nd = UI_SLOT_BASE_NEW + (d - UI_SLOT_BASE_OLD);
        if write_bytes(base + r + 4, &nd.to_le_bytes()) {
            done += 1;
        }
    }
    // bounds 0x30 -> 0x40
    for (r, _sig) in SLOT_BOUNDS.iter() {
        if write_bytes(base + r + 3, &[0x40]) {
            done += 1;
        }
    }
    // fix up the 5th-argument references
    for &r in UI_ARG5_SITES.iter() {
        if write_bytes(base + r + 3, &UI_ARG5_NEW.to_le_bytes()) {
            done += 1;
        }
    }
    // init blocks -> jump to the stub (12B) + nop the rest
    for (i, &(r, l, _b)) in UI_INIT_BLOCKS.iter().enumerate() {
        let mut patch = vec![0x90u8; l];
        patch[0] = 0x48;
        patch[1] = 0xb8;
        patch[2..10].copy_from_slice(&stubs[i].to_le_bytes());
        patch[10] = 0xff;
        patch[11] = 0xe0;
        if write_bytes(base + r, &patch) {
            done += 1;
        }
    }
    // * The frame extension goes **last** (if it went first, every old-array reference would be wrong from that instant)
    let frame_ok = write_bytes(base + UI_MEGA_PROLOGUE_IMM, &UI_FRAME_NEW.to_le_bytes());
    format!(
        "slot_ui: surgery applied to {}/84 sites + frame extension {} (base {:#x} -> {:#x})",
        done,
        if frame_ok { "OK" } else { "FAIL★" },
        UI_SLOT_BASE_OLD,
        UI_SLOT_BASE_NEW
    )
}

// ═══════════════════════════════════════════════════════════════════════════
//  ** Game version gate - 0.5.3 only. On any other version **every feature disables itself automatically**.
// ═══════════════════════════════════════════════════════════════════════════
//  Why: this mod depends on 12 hardcoded RVAs + 2 byte patches + many struct offsets.
//  Once the game is patched to 0.5.4 all those addresses are wrong and we would **hook/patch the wrong code**
//  (hooks with prologue validation simply fail to install, but the weakly validated places risk crashes and data corruption).
//  => check the version at init and, on a mismatch, install **not a single** hook or patch.
//
//  Two-part decision (both must pass to enable):
//   (1) exe file size - 0.5.3 = 74,970,624B. It reliably differs per version and costs nothing to read.
//   (2) measured entry prologues of 3 key hooks - catches a repackage that happens to have the same size but different code.
//  WARNING a loose check (size only) could misbehave on a hotfix, so we look at the prologues too.
const GAME_EXE_SIZE_054: u64 = 75_936_256;
static VERSION_OK: AtomicBool = AtomicBool::new(false);
static VERSION_MSG: Mutex<String> = Mutex::new(String::new());
/// Decide whether this is 0.5.3. Called once from init; the result is stored in VERSION_OK.
fn check_game_version() -> bool {
    let mut why = String::new();
    // (1) exe size
    let size_ok = match exe_path().and_then(|p| fs::metadata(p).ok()) {
        Some(m) => {
            let sz = m.len();
            if sz == GAME_EXE_SIZE_054 {
                true
            } else {
                why = format!(
                    "exe size mismatch: {}B (0.5.4 = {}B)",
                    sz, GAME_EXE_SIZE_054
                );
                false
            }
        }
        None => {
            why = "could not read the exe path or its metadata".into();
            false
        }
    };
    // (2) entry prologues of the key hooks (caught here even if the size matches but the code differs)
    //  WARNING WARNING **a chain-hooking exception is mandatory** (real incident 2026-07-30): for functions like launcher it is **normal** for
    //    **another mod (serpen) to have hooked first**, leaving the entry overwritten with `48 b8 <tgt> ... ff e0` (movabs+jmp).
    //    Misjudging that as a "version mismatch" disabled the whole mod (user report: "the 4-slot mod suddenly stopped working").
    //    => accept an entry that is in **foreign-hook form** as passing, and only byte-compare when it is the original prologue.
    //  WARNING the same form also appears when we ourselves already installed (re-init / hot reload).
    let proto_ok = if size_ok {
        let base = exe_base_addr();
        if base == 0 {
            why = "module base 0".into();
            false
        } else {
            // * Only check places with **no cross-mod shared hooking**.
            //   launcher (CL_LAUNCHER_RVA) is a chain-hooking point shared with serpen etc., so its entry may be
            //   overwritten by someone else's hook and its state depends on init order => **unsuitable as version evidence**.
            //   buy/seedctor are exclusive to this mod, so at init time they always hold the original prologue.
            let checks: [(&str, usize, &[u8]); 2] = [
                ("BUY", RVA_BUY_ITEM, &BUY_PROLOGUE),
                ("SEEDCTOR", SEEDCTOR_RVA, &SEEDCTOR_PROLOGUE),
            ];
            let mut ok = true;
            for (nm, rva, want) in checks.iter() {
                let a = base + rva;
                if !unsafe { readable(a, want.len().max(12)) } {
                    why = format!("{}: could not read the entry point @{:#x}", nm, rva);
                    ok = false;
                    break;
                }
                // An already-hooked entry (movabs rax,imm64 ; jmp rax) = normal (ours or another mod's) -> check passes
                let hooked = unsafe {
                    *(a as *const u8) == 0x48
                        && *((a + 1) as *const u8) == 0xb8
                        && *((a + 10) as *const u8) == 0xff
                        && *((a + 11) as *const u8) == 0xe0
                };
                if hooked {
                    continue;
                }
                let hit = (0..want.len()).all(|i| unsafe { *((a + i) as *const u8) } == want[i]);
                if !hit {
                    why = format!("{}: prologue mismatch @{:#x}", nm, rva);
                    ok = false;
                    break;
                }
            }
            ok
        }
    } else {
        false
    };
    let ok = size_ok && proto_ok;
    *VERSION_MSG.lock().unwrap_or_else(|e| e.into_inner()) = if ok {
        "0.5.3 confirmed - active".to_string()
    } else {
        format!("version mismatch -> this half is fully disabled ({})", why)
    };
    VERSION_OK.store(ok, Ordering::Relaxed);
    ok
}
/// Whether the gate passed (queried from runtime hook/patch entry points).
#[inline]
fn version_ok() -> bool {
    VERSION_OK.load(Ordering::Relaxed)
}

/// Was `init(_ctx: &GameCtx) -> ModRegistration` + `declare_mod!(init)`.
///
/// Returns whether the tactics half is active. The host mod registers its own
/// extensions unconditionally and consults this before routing anything here,
/// which reproduces the old "return a bare `ModRegistration`" behaviour: on a
/// version mismatch not one hook or patch is installed.
///
/// The version gate matters more than it used to. The host mod's `mod.mod_info`
/// says `base >= 0.5.3` (its stable-ABI half keeps working across updates),
/// while everything here is hardcoded RVAs, byte patches and struct offsets for
/// exactly 0.5.3 — so the loader will happily attach this DLL on 0.5.4 and this
/// gate is the only thing standing between that and a corrupted game.
fn tactics_init() -> bool {
    // Register the VEH before anything else. `safe_copy` returns `false` on
    // entry while `SEH_INSTALLED` is false, so until this runs EVERY protected
    // read in this module fails — `safe_read_u64`, `safe_read_bytes`, all of it.
    //
    // Upstream called this from only two places: `dump_mod_items`, and
    // `handle_tactics_screen`. The second one is what actually did the work,
    // because it ran every frame — the VEH was registered as a side effect of
    // the tactics screen handler existing. Disabling the UI tree walk
    // (`UI_TREE_WALK_ENABLED`) removed that, and `dump_mod_items` cannot cover
    // for it: it needs a `Database` address that only arrives once the host's
    // item-build detour has fired.
    //
    // The symptom was total and silent. `install_launcher_hook` returned at its
    // first `safe_read_u64` on all 18,902 calls, so `LIVE_SEED` stayed 0 and no
    // buy was ever classified as live; `is_my_athlete` could not read
    // `athlete+0x810`, so the buy detour early-exited before touching a build.
    // Every counter read 0 and every hook reported healthy.
    //
    // Idempotent (`SEH_INSTALLED.swap`), so init is simply the correct place.
    seh_install();

    // ** Version gate: if this is not 0.5.3, install **no hooks or patches at all** and return an empty registration.
    //   (It depends on hardcoded RVAs, byte patches and struct offsets, so other versions risk misbehaviour.)
    if !check_game_version() {
        let msg = VERSION_MSG
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // Written once so the user can tell why this half is disabled. Silenced
        // with the other trace files — after a game update the symptom is "the
        // 4th item quietly stopped working" with nothing on disk saying why, so
        // `TRACE_FILES` is the first thing to flip if that ever happens.
        if TRACE_FILES {
            if let Some(d) = mod_dir() {
                let _ = fs::create_dir_all(&d);
                let _ = fs::write(
                    d.join("version_gate.txt"),
                    format!(
                        "{}

This half of the mod (the 4th item slot) requires game version 0.5.3 exactly.
If the game has updated, please wait for a mod update. The rest of the mod is unaffected.
",
                        msg
                    ),
                );
            }
        }
        // Register only, attaching **not a single** extension, hook or patch = completely disabled.
        return false;
    }
    let mode = load_mode();
    // Byte-patch results always leave a trace, regardless of `LOG_ENABLED` —
    // the same rule `load_mode` already follows, for the same reason.
    //
    // Both patches validate their target byte-for-byte and **skip silently** on
    // a mismatch, and `patch_gate3` skipping means the 4th item is never bought
    // at all (0 purchases). Routing that through `append_log`, which
    // `LOG_ENABLED = false` turns off in production, makes "the signature moved"
    // and "the feature works" produce identical evidence: no file either way.
    let mut patch_report = format!("mode = {mode} slots\n");
    if mode == 4 {
        let r = unsafe { patch_owned_cap() };
        patch_report.push_str(&format!("patch_owned_cap : {r}\n"));
        let rg = unsafe { patch_gate3() }; // * disable the slot-4 natural purchase gate
        patch_report.push_str(&format!("patch_gate3     : {rg}\n"));
        // * Diagnostic (title-return crash bisection): slot UI patches (bound 0x30 -> 0x40 + full replace of helper 0xbbbd60) OFF.
        //   If the crash disappears the UI patch is the cause (-> helper trampoline / context gate). If it persists it is on the sim side.
        if !DIAG_SLOT_UI_OFF {
            let rs = unsafe { patch_slot_ui() }; // in-match 4th slot icon
            patch_report.push_str(&format!("patch_slot_ui   : {rs}\n"));
        } else {
            patch_report.push_str("patch_slot_ui   : SKIP (DIAG_SLOT_UI_OFF)\n");
        }
        if AUTO4_BEAM_DEPTH {
            let rb = unsafe { patch_beam_depth() };
            patch_report.push_str(&format!("patch_beam_depth: {rb}\n"));
            // * The cand_gate patch actually breaks beam build generation (beam4 drops to 0) -> disabled.
            if CAND_GATE_ON {
                let rc = unsafe { patch_cand_gate() };
                patch_report.push_str(&format!("patch_cand_gate : {rc}\n"));
            }
        }
    } else {
        patch_report.push_str("(3-slot mode: no byte patches applied)\n");
    }
    if TRACE_FILES {
        if let Some(d) = mod_dir() {
            let _ = fs::create_dir_all(&d);
            let _ = fs::write(d.join("4items_patches.txt"), &patch_report);
        }
    }
    // Set the log path for uinj (item3/slot3 UI injection). (MODE4/IN_MATCH_UI come from load_mode and the defaults.)
    true
}
