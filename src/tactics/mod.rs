//! The native half of the mod: what the stable mod API cannot express.
//!
//! - `own_team_only`: the buy and spawn detours hold the athlete pointer, so
//!   they can restrict the player's pins to the player's own athletes.
//! - The 5th and 6th item slots: the buy detour grows every build past the
//!   game's four, and four byte patches let the engine buy and draw them.
//!
//! Everything here is pinned to one exact game build (`check_game_version`):
//! hardcoded addresses, byte patches and struct offsets, re-derived every game
//! update.
#![allow(dead_code, unused_imports, unused_variables)]

use mod_api_stable::{RecordKindV1, StableClient};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;

pub mod driver;

const TRACE_FILES: bool = false;

// ===========================================================================
//  WinAPI FFI
// ===========================================================================
type HMODULE = isize;
type DWORD = u32;
type BOOL = i32;
// Declared elsewhere in the crate with pointer types; on 64-bit Windows both
// pass the same way.
#[allow(clashing_extern_declarations)]
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
// Whether `p` is on a committed executable page. A readable but non-executable
// page still faults under DEP, which the VEH cannot catch.
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

// Per-thread SEH state, so guarded reads on the sim workers never contend: the
// VEH runs on the faulting thread and reads its own copy. `Cell`, `const` init
// and no `Drop` keep the handler free of allocation, locks and panics; slot 1
// is unused.
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
        // `try_with`: pass the fault on if this thread's TLS is being torn
        // down, since the handler must not panic.
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
        *g.add(7) += 1; // fault counter
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
// Where this half's diagnostic files live: beside the host DLL, which may be a
// Workshop folder outside the game directory.
fn mod_dir() -> Option<PathBuf> {
    crate::config::dll_dir()
}

// ===========================================================================
//  Mod item registry (filled once, from the catalog the item-build detour is handed)
// ===========================================================================
static MOD_REGISTRY: Mutex<Vec<String>> = Mutex::new(Vec::new()); // idx i -> key (game ID = 30+i)
static MOD_FINALS: Mutex<Vec<u64>> = Mutex::new(Vec::new()); // mod item IDs whose next_tier is empty
static MODITEMS_DONE: AtomicBool = AtomicBool::new(false);

// The game's 30 item keys in id order (0..29); ids from 30 on are
// `MOD_REGISTRY`'s.
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

// Whether the mod-item registry is built, so the hook can skip assembling the
// catalog argument (two allocations per item, for every team of every match).
pub(crate) fn item_catalog_recorded() -> bool {
    MODITEMS_DONE.load(Ordering::Relaxed)
}

// Fills `MOD_REGISTRY`/`MOD_FINALS` from the catalog the item-build detour is
// handed, `(key, next_tier)` per entry. Ids are `0..30` vanilla and `30 + i`
// for `MOD_REGISTRY[i]`, used only inside this module (injection resolves keys
// by name), so catalog order is fine.
fn record_item_catalog(catalog: Vec<(String, Vec<String>)>) {
    if catalog.is_empty() || MODITEMS_DONE.swap(true, Ordering::Relaxed) {
        return;
    }

    // Pass 1: every key something upgrades into, vanilla included, since a mod
    // item can be a vanilla component's upgrade.
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
        // Pass 2: a final has no next tier and something builds into it, which
        // rules out lone components. Upgraded boots pass too, but Smart Builds
        // places those.
        if next_tier.is_empty()
            && built_into.contains(key.as_str())
            && !crate::smart_builds::is_boots(key)
        {
            finals.push(id);
        }
        registry.push(key.clone());
    }

    *MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner()) = registry;
    *MOD_FINALS.lock().unwrap_or_else(|e| e.into_inner()) = finals;
    // `auto_cands` memoizes on first call, so drop any list built before the
    // registry.
    *AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// Every mod final item as (game id, key).
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

// Vanilla category (1-6) -> its final's game id, as the game's c6 jump table
// maps them (cat1 = AD .. cat6 = HP).
const VANILLA_FINAL: [u64; 6] = [4, 24, 9, 14, 19, 29];

// ===========================================================================
//  Live match build injection
// ===========================================================================

const TRAMPOLINE_DEBUG_PASSTHROUGH: bool = false; // diagnostic: the stub only runs the original instructions

static PLAYER_TEAM_ID: AtomicU64 = AtomicU64::new(u64::MAX); // u64::MAX = not captured
const O_ATHLETE_ID: usize = 0x7f0; // 0.6.3 (0.6.2: 0x9f0)

// The athlete fields this module reads, named so a layout migration finds every
// reader. Re-derived each update off the spawn function (`SPAWN_RVA`) and the
// buy resolver; the struct moves by different amounts in different places, so
// never shift every field by one delta.
//
// A candidate reader whose offsets did not move between builds is walking
// another struct. Since 0.6.3 the roster is a `Vec` of pointers to athletes, so
// there is no stride.
const O_ATHLETE_CHAMP_PTR: usize = 0x2e0; // 0.6.3 (0.6.2: 0x4e0)
const O_ATHLETE_CHAMP_LEN: usize = 0x2e8; // 0.6.3 (0.6.2: 0x4e8)
const O_ATHLETE_TEAM: usize = 0x800; // 0.6.3 (0.6.2: 0xa00)
const O_ATHLETE_POS: usize = 0x890; // dword. 0.6.3 (0.6.2: 0xa90)
// The owned-item count, the build `Vec` (`{cap, ptr, len}`), the size of the
// athlete the spawn callers copy, and the `Game` fields the spawn detour reads.
const O_ATHLETE_ITEMS_LEN: usize = 0x318; // 0.6.3 (0.6.2: 0x518)
// The items `Vec`'s pointer, the word before its length. Elements are 16 bytes,
// an item and its vtable, like a catalog entry.
const O_ATHLETE_ITEMS_PTR: usize = O_ATHLETE_ITEMS_LEN - 8;
const O_ATHLETE_BUILD_CAP: usize = 0x350; // 0.6.3 (0.6.2: 0x550)
const O_ATHLETE_BUILD_PTR: usize = 0x358; // 0.6.3 (0.6.2: 0x558)
const O_ATHLETE_BUILD_LEN: usize = 0x360; // 0.6.3 (0.6.2: 0x560)
const ATHLETE_COPY_SIZE: usize = 0x898; // 0.6.3 (0.6.2: 0xa98)
const O_GAME_PROVIDER: usize = 0x1ba0; // 0.6.3 (0.6.2: 0x1dc0)
const O_GAME_CATALOG_PTR: usize = 0x1da0; // 0.6.3 (0.6.2: 0x1fd0)
const O_GAME_CATALOG_LEN: usize = 0x1da8; // 0.6.3 (0.6.2: 0x1fd8)
static MY_ATHLETES: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_PREV: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_N: AtomicU64 = AtomicU64::new(0); // published starter count (0 = not obtained)
static ROSTER_TICK: AtomicU64 = AtomicU64::new(0);
// Publishes my team's starting roster. The previous set is freed one swap
// later, since a sim thread may still be reading it.
fn publish_my_athletes(set: std::collections::HashSet<u64>) {
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
// The lane `athlete` plays this match, from `O_ATHLETE_POS` (the game's
// `Position`, Top = 0); `None` when it is not one of the five.
unsafe fn athlete_lane(athlete: usize) -> Option<crate::build_config::Role> {
    let pos = (safe_read_u64(athlete + O_ATHLETE_POS)? & 0xffff_ffff) as usize;
    (pos < 5).then(|| crate::build_config::Role::from_lane_code(pos))
}
// Whether this athlete is on my team (contracted, or in the last starting
// five); `None` while the roster is unknown, before the management screen is
// visited.
#[inline]
unsafe fn is_my_athlete(athlete: usize) -> Option<bool> {
    let p = MY_ATHLETES.load(Ordering::Acquire);
    if p.is_null() || (*p).is_empty() {
        return None;
    }
    let aid = safe_read_u64(athlete + O_ATHLETE_ID)?;
    // Athlete id 0 is a real player in some saves, so it must not be filtered
    // out here.
    Some((*p).contains(&aid))
}
// `__rust_realloc` (rcx = ptr, rdx = old, r8 = align, r9 = new) -> rax. When no
// signature pairs, find it by its body: `mov rdi,r9 / mov rsi,rcx / cmp
// r8,0x11` is unique in .text and tail-jumps to HeapReAlloc.
const RVA_REALLOC: usize = 0x2fd35b0; // 0.6.4 (0.6.3: 0x2f855a0); exe2exe strict unique, 174 bytes
type ReallocFn = unsafe extern "win64" fn(usize, usize, usize, usize) -> usize;
// The first 12 bytes of `RVA_REALLOC`, checked before every call, so a stale
// address declines (the build keeps the game's four) instead of crashing as it
// did on 2026-09-16.
const REALLOC_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x28,
];

// Whether `RVA_REALLOC` still starts with [`REALLOC_PROLOGUE`]. Read once and
// cached: the image does not change under a running game.
fn realloc_ok() -> bool {
    static STATE: AtomicU8 = AtomicU8::new(0); // 0 unknown, 1 ok, 2 mismatch
    match STATE.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let addr = exe_base_addr() + RVA_REALLOC;
    let mut bytes = Vec::new();
    let ok = unsafe { safe_read_bytes(addr, REALLOC_PROLOGUE.len(), &mut bytes) }
        && bytes[..] == REALLOC_PROLOGUE[..];
    STATE.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
    ok
}
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

// Catalog index -> item name, through the entry's name getter (vtable +0x58).
// Reads are VEH-guarded rather than `readable` (a `VirtualQuery` syscall each),
// and the getter is proven code once ([`name_getter_ok`]).
unsafe fn catalog_name_at(ctx: usize, idx: u64) -> Option<String> {
    if ctx < 0x10000 {
        return None;
    }
    let coll = safe_read_u64(ctx + 0x30)? as usize;
    if coll < 0x10000 {
        return None;
    }
    catalog_name_in(
        safe_read_u64(coll + 8)? as usize,
        safe_read_u64(coll + 0x10)?,
        idx,
    )
}

// [`catalog_name_at`] on an already-read catalog array (`data`/`len`), so the
// index cache can re-check an index without a context.
unsafe fn catalog_name_in(data: usize, len: u64, idx: u64) -> Option<String> {
    if idx >= len || data < 0x10000 {
        return None;
    }
    let e = data + (idx as usize) * 16;
    let edata = safe_read_u64(e)? as usize;
    let evt = safe_read_u64(e + 8)? as usize;
    if edata < 0x10000 || evt < 0x10000 {
        return None;
    }
    let namefn = safe_read_u64(evt + 0x58)? as usize;
    if !name_getter_ok(namefn) {
        return None;
    }
    let f: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(namefn);
    let nobj = f(edata);
    if nobj < 0x10000 {
        return None;
    }
    let chars = safe_read_u64(nobj + 8)? as usize;
    let nlen = safe_read_u64(nobj + 0x10)? as usize;
    if chars < 0x10000 || nlen == 0 || nlen > 64 {
        return None;
    }
    let mut name = Vec::new();
    if !safe_read_bytes(chars, nlen, &mut name) {
        return None;
    }
    Some(String::from_utf8_lossy(&name).into_owned())
}

// Match-start launcher hook: captures the rendered match's seed (r8) as
// `LIVE_SEED`, which gates the buy hook. Re-derived each update as the
// seedctor's caller: it calls the seedctor three times with rdx = the seed it
// saved at entry (`mov r12,r8`), and its chkstk frame size is a fingerprint
// (`tools/rederive.py frames`).
const CL_LAUNCHER_RVA: usize = 0x16d3140; // 0.6.4 (0.6.3: 0x13f3950); calls the seedctor at +0x19a/+0x26d/+0x344, `mov r12,r8` saves the seed at +0x41
const CL_LAUNCHER_PROLOGUE: [u8; 17] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0xb8, 0x98, 0xed, 0x01,
    0x00,
]; // 0.6.4: 8 push + mov eax,0x1ed98, the frame size, which moves most updates
static CLAUNCH_INSTALLED: AtomicU64 = AtomicU64::new(0);
// Whether the current match is a comp test, where the user builds both sides:
// there is no "my team", so the team gate is bypassed.
static COMPTEST_MATCH: AtomicBool = AtomicBool::new(false);
static CLAUNCH_STUB: AtomicU64 = AtomicU64::new(0); // address of our launcher stub (for re-validating the entry point)
static LAUNCH_WAIT: AtomicU64 = AtomicU64::new(0); // frames spent waiting for serpen to install
unsafe extern "C" fn cap_launcher(saved: *mut u64, _e: usize) -> u64 {
    // Minimal detour: the launcher has a 91KB chkstk frame and fires for every
    // background match, so this body does raw reads and atomics only (no
    // format!, fs, locks or TLS lazy-init).
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
    // The caller's return address says which match this is: spectate, my match,
    // a comp test or its record replay are on screen. Every other caller is a
    // background sim and must never be added; these are 0.5.3's addresses and
    // were never re-derived (see `build_config::training_match`).
    let is_comptest = rva == 0x1925f12 || rva == 0x18f718e;
    if (rva == 0x9a3287 || rva == 0x9a7b03 || is_comptest) && seed != 0 {
        let prev = LIVE_SEED.swap(seed, Ordering::Relaxed);
        if prev != seed {
            RENDER_PROVIDER.store(0, Ordering::Relaxed);
        } // new match seed -> the ctor right after re-captures the provider
        COMPTEST_MATCH.store(is_comptest, Ordering::Relaxed);
    }
    0
}
static HK_L_TICK: AtomicU64 = AtomicU64::new(0);
fn install_launcher_hook() {
    // Runs every frame, so it uses the cached exe base and VEH-guarded reads,
    // and once tried re-checks only every 60 frames (~1 s).
    if CLAUNCH_INSTALLED.load(Ordering::Relaxed) != 0 {
        if HK_L_TICK.fetch_add(1, Ordering::Relaxed) % 60 != 0 {
            return;
        }
    }
    // Coexists with serpen: when its hook (a movabs entry) is present we chain
    // behind it, so we wait up to 240 frames for it before installing alone.
    // The later re-checks re-chain if anyone overwrites ours.
    let base = exe_base_addr();
    if base == 0 {
        return;
    }
    let fn_addr = base + CL_LAUNCHER_RVA;
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
    if our != 0 && cur_tgt == our {
        CLAUNCH_INSTALLED.store(1, Ordering::Relaxed);
        return;
    } // entry point = our stub -> fine
    let is_foreign = b0 == 0x48 && cur_tgt >= 0x10000 && cur_tgt != our; // a foreign hook (serpen etc.) is present
    let waited = LAUNCH_WAIT.fetch_add(1, Ordering::Relaxed);
    if !is_foreign && b0 != 0x48 && waited < 240 {
        return;
    }
    // Install, or re-chain behind a foreign hook (`install_detour_generic`
    // detects it).
    let r = unsafe {
        install_detour_generic(
            CL_LAUNCHER_RVA,
            12,
            cap_launcher as *const () as usize,
            &CL_LAUNCHER_PROLOGUE,
        )
    };
    match r {
        Ok(stub) => {
            CLAUNCH_STUB.store(stub as u64, Ordering::Relaxed);
            CLAUNCH_INSTALLED.store(1, Ordering::Relaxed);
        }
        Err(_) => CLAUNCH_INSTALLED.store(2, Ordering::Relaxed),
    }
}

// Seed-ctor hook: the provider constructed with `LIVE_SEED` (rcx = provider,
// rdx = seed) is the rendered sim, recorded as `RENDER_PROVIDER`. The seed
// field itself is the RNG's running state, so providers are compared by address
// instead.
const SEEDCTOR_RVA: usize = 0x16dd950; // 0.6.4 (0.6.3: 0x13fddb0); holds the image-unique seed store `mov [rsi+0xec88],rax`, and the launcher calls it three times
const O_PROVIDER_SEED: usize = 0xec88; // 0.6.4 (0.6.2: 0xec90): `mov [rsi+0xec88],rax` at seedctor +0x686
const SEEDCTOR_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // 8 push; the chkstk call after them stays in place
const SEEDCTOR_ORIG_LEN: usize = 12; // the 8 pushes only; the jmp back lands on `mov eax,frame`
static SEEDCTOR_INSTALLED: AtomicU64 = AtomicU64::new(0);
static RENDER_PROVIDER: AtomicU64 = AtomicU64::new(0); // the rendered sim's provider (the primary is_live gate)
static LIVE_SEED: AtomicU64 = AtomicU64::new(0); // the rendered match's seed, from the launcher hook
unsafe extern "C" fn cap_seed_ctor(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let provider = *saved; // saved+0 = rcx = arg1 = provider(this)
    let seed = *saved.add(1); // saved+1 = rdx = arg2 = seed(=launcher r8)
    let ls = LIVE_SEED.load(Ordering::Relaxed);
    if ls != 0 && seed == ls && provider >= 0x10000 && provider < 0x0000_8000_0000_0000 {
        RENDER_PROVIDER.store(provider as u64, Ordering::Relaxed);
    }
    0
}
fn install_seed_ctor_hook() {
    let state = SEEDCTOR_INSTALLED.load(Ordering::Relaxed);
    if state == 1 {
        return;
    }
    // A failed attempt backs off ([`install_retry_due`]); the untried first
    // frame is not delayed.
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if state == 2 && !install_retry_due(&RETRY) {
        return;
    }
    let r = unsafe {
        install_detour_generic(
            SEEDCTOR_RVA,
            SEEDCTOR_ORIG_LEN,
            cap_seed_ctor as *const () as usize,
            &SEEDCTOR_PROLOGUE,
        )
    };
    SEEDCTOR_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
}

// Spawn hook: every build-creation path goes through this one function (rcx =
// Game, rdx = a stack copy of the athlete, copied into the provider
// afterwards), so build targets written here are bought up to like any other
// build.
//
// To re-derive it, look for the athlete gold store `mov [rdx+<gold>],rax` at
// +0x41 (8-push prologue, two direct callers that memcpy `ATHLETE_COPY_SIZE`
// bytes into rdx), or for its Game-field reads (0x1ba0..0x1bb8 in 0.6.3).
const SPAWN_RVA: usize = 0x16d6c60; // 0.6.4 (0.6.3: 0x13f7550); exe2exe strict unique at 1818 bytes; callers 0x16d62c0 +0x231 and 0x16d6650 +0x91
const SPAWN_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // 8 push (12B), then `sub rsp`
const SPAWN_ORIG_LEN: usize = 12; // the 8 pushes, an instruction boundary
const SPAWN_INJECT_ENABLED: bool = true; // the only path that can set slot 0 under `own_team_only`
static SPAWN_INSTALLED: AtomicU64 = AtomicU64::new(0);
unsafe extern "C" fn cap_spawn(saved: *mut u64, _e: usize) -> u64 {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _probe = crate::perf::Probe::sim(crate::perf::Section::SpawnDetour);
        // Only under `own_team_only`: otherwise `item_build_hook::decide_build`
        // has already set these slots on the stable API, and two writers would
        // fight over them.
        if !SPAWN_INJECT_ENABLED || saved.is_null() || !crate::build_config::own_team_only_enabled()
        {
            return;
        }
        let game = *saved as usize; // rcx = Game
        let athlete = *saved.add(1) as usize; // rdx = athlete (stack copy, the final build)
        if game < 0x10000 || athlete < 0x10000 {
            return;
        }
        // (1) The match's provider, watched or not: under `own_team_only` this
        // only writes to the player's own five.
        let provider = match safe_read_u64(game + O_GAME_PROVIDER) {
            Some(p) => p,
            None => return,
        };
        if provider < 0x10000 || provider >= 0x0000_8000_0000_0000 {
            return;
        }
        // (2) The champion.
        if !readable(athlete, ATHLETE_COPY_SIZE) {
            return;
        }
        let cptr = rd_u64(athlete + O_ATHLETE_CHAMP_PTR) as usize;
        let clen = rd_u64(athlete + O_ATHLETE_CHAMP_LEN) as usize;
        if cptr < 0x10000 || clen == 0 || clen > 48 || !readable(cptr, clen) {
            return;
        }
        let champ_cow =
            String::from_utf8_lossy(std::slice::from_raw_parts(cptr as *const u8, clen));
        let champ: &str = champ_cow.as_ref();
        // Every athlete, pins or not, while a lane or 5v5 test is on: this is
        // how the buy detour tells the test's match from the league fixtures
        // simulating alongside it.
        crate::build_config::note_test_spawn(
            safe_read_u64(provider as usize + O_PROVIDER_SEED).unwrap_or(0),
            champ,
        );
        // Before any pin is looked up: the lane every lookup below resolves in.
        crate::build_config::set_athlete_lane(athlete_lane(athlete));
        if !crate::build_config::has_pins(champ) {
            return;
        }
        // (3) Whether it is my team, by athlete id in my roster: the sim has no
        // team id, so membership is the only test.
        let mine = is_my_athlete(athlete);
        crate::own_team_log::line(|| {
            format!(
                "spawn: {} aid={:?} provider=0x{:x} seed=0x{:x} lane={:?} mine={:?} roster_n={}",
                champ,
                safe_read_u64(athlete + O_ATHLETE_ID),
                provider,
                safe_read_u64(provider as usize + O_PROVIDER_SEED).unwrap_or(0),
                athlete_lane(athlete),
                mine,
                MY_ATH_N.load(Ordering::Relaxed)
            )
        });
        // `Some(false)` is another team's athlete and `None` is undecided; both
        // are left alone, since putting the player's build on the enemy is
        // worse than missing slot 0. The roster comes from every athlete's
        // contract (`roster_scan_step`), so it is known from load.
        if mine != Some(true) {
            return;
        }
        // (4) Write the build targets. The catalog is the `Vec` at
        // `O_GAME_CATALOG_PTR`/`_LEN` (16-byte entries), the same index space
        // the buy context uses.
        let cat_base = rd_u64(game + O_GAME_CATALOG_PTR) as usize;
        let cat_len = rd_u64(game + O_GAME_CATALOG_LEN);
        let bptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
        let blen = rd_u64(athlete + O_ATHLETE_BUILD_LEN);
        if bptr < 0x10000 || blen == 0 || blen > 8 || !writable(bptr, (blen as usize) * 8) {
            return;
        }
        if cat_base < 0x10000 || cat_len == 0 || cat_len > 100000 {
            // vanilla designations need no scan, so keep going
        } else {
            let held = spawn_build_names(bptr, blen, cat_base, cat_len);
            // The stable hook gave this athlete the pin-free build both teams
            // get; swap in the pin-aware one it recorded before the pins are
            // written.
            spawn_paste_pinned_build(champ, bptr, blen, cat_base, cat_len);
            crate::own_team_log::line(|| {
                format!(
                    "spawn: {} held={:?} after_swap={:?} pin_row={:?}",
                    champ,
                    held,
                    spawn_build_names(bptr, blen, cat_base, cat_len),
                    crate::build_config::athlete_pin_row(champ)
                )
            });
        }
        // The pins for the game's own four slots.
        for si in 0..crate::build_config::game_slots() as u8 {
            if (si as u64) >= blen {
                break;
            }
            if crate::build_config::pinned_key_raw(champ, si as usize).is_none() {
                continue;
            }
            // By key, vanilla included: name scan + recipe validation.
            let idx =
                slot_n_catalog_index(champ, si, |key| scan_catalog_index(cat_base, cat_len, key));
            let Some(t) = idx else {
                continue;
            };
            if cat_len > 0 && t < cat_len {
                let here = rd_u64(bptr + (si as usize) * 8);
                if here != t {
                    // The engine may target the pinned item (or, for boots,
                    // another pair) in another slot. Nothing is bought yet, so
                    // swap rather than duplicate, unless the player pinned that
                    // slot too.
                    let pin_boots = spawn_is_boots(cat_base, cat_len, t);
                    let elsewhere = (0..blen as usize).filter(|&j| j != si as usize).find(|&j| {
                        let there = rd_u64(bptr + j * 8);
                        (there == t || (pin_boots && spawn_is_boots(cat_base, cat_len, there)))
                            && spawn_pin_at(champ, cat_base, cat_len, j) != Some(there)
                    });
                    if let Some(j) = elsewhere {
                        wr_u64(bptr + j * 8, here);
                    } else if !pin_boots && spawn_is_boots(cat_base, cat_len, here) {
                        // The pin covers Smart Builds' boots (rule 7): unless
                        // the player pinned a pair, move them where the rule
                        // would (`displaced_boots_slot`).
                        let player_boots = (0..crate::build_config::picker_slots()).any(|j| {
                            spawn_pin_at(champ, cat_base, cat_len, j)
                                .is_some_and(|pin| spawn_is_boots(cat_base, cat_len, pin))
                        });
                        let free = displaced_boots_slot(champ, si as usize, blen as usize, true);
                        if let (false, Some(j)) = (player_boots, free) {
                            wr_u64(bptr + j * 8, here);
                        }
                    }
                    wr_u64(bptr + (si as usize) * 8, t);
                }
            }
        }
        // Boots pinned in the 5th or 6th replace the engine's pair now, so the
        // build holds one pair, bought when the engine's would have been;
        // `pin_placed_by_engine` then drops the later pin.
        if cat_len > 0 {
            let planted = (blen as usize).min(crate::build_config::game_slots());
            let pinned_boots = (planted..crate::build_config::picker_slots())
                .filter_map(|j| spawn_pin_at(champ, cat_base, cat_len, j))
                .find(|&pin| spawn_is_boots(cat_base, cat_len, pin));
            if let Some(pin) = pinned_boots {
                for k in 0..blen as usize {
                    let there = rd_u64(bptr + k * 8);
                    if there != pin
                        && crate::build_config::pinned_key_raw(champ, k).is_none()
                        && spawn_is_boots(cat_base, cat_len, there)
                    {
                        wr_u64(bptr + k * 8, pin);
                    }
                }
            }
        }
        crate::own_team_log::line(|| {
            format!(
                "spawn: {} final={:?}",
                champ,
                spawn_build_names(bptr, blen, cat_base, cat_len)
            )
        });
    }));
    0 // the stub ignores the return value
}
// Where Smart Builds' boots in slot `si` go when a pin lands there: the next
// slot no pin holds, else `None` when an open 5th or 6th will get them as it
// grows (`extra_slot_boots`), else the first slot if unbought (`first_open`)
// and unpinned.
fn displaced_boots_slot(champ: &str, si: usize, len: usize, first_open: bool) -> Option<usize> {
    let open = |j: usize| crate::build_config::pinned_key_raw(champ, j).is_none();
    if let Some(j) = (si + 1..len).find(|&j| open(j)) {
        return Some(j);
    }
    if builds_grow_past_four() && (len..crate::build_config::picker_slots()).any(open) {
        return None;
    }
    (first_open && si != 0 && open(0)).then_some(0)
}

// Whether catalog entry `index` is a pair of boots, for `cap_spawn`, which
// holds the catalog base and length rather than a buy context.
unsafe fn spawn_is_boots(cat_base: usize, cat_len: u64, index: u64) -> bool {
    catalog_name_in(cat_base, cat_len, index)
        .is_some_and(|name| crate::smart_builds::is_boots(&name))
}

// The athlete's build as catalog names, for the `own_team_log` test log.
// `None` for an entry the catalog does not name.
unsafe fn spawn_build_names(
    bptr: usize,
    blen: u64,
    cat_base: usize,
    cat_len: u64,
) -> Vec<Option<String>> {
    (0..blen as usize)
        .map(|j| catalog_name_in(cat_base, cat_len, rd_u64(bptr + j * 8)))
        .collect()
}

// Swaps the pin-free build the stable hook gave this athlete for the pin-aware
// one it recorded under `own_team_only`
// (`item_build_hook::remember_pinned_build`). Writes nothing unless the athlete
// still holds exactly the recorded build and every pinned item resolves.
unsafe fn spawn_paste_pinned_build(
    champ: &str,
    bptr: usize,
    blen: u64,
    cat_base: usize,
    cat_len: u64,
) {
    let held: Option<Vec<String>> = (0..blen as usize)
        .map(|j| catalog_name_in(cat_base, cat_len, rd_u64(bptr + j * 8)))
        .collect();
    let Some(held) = held else {
        return;
    };
    let row = crate::build_config::athlete_pin_row(champ);
    let Some(pinned) = crate::build_config::pinned_build(champ, &row, &held) else {
        return;
    };
    let indices: Option<Vec<u64>> = pinned
        .iter()
        .map(|key| scan_catalog_index(cat_base, cat_len, key.as_bytes()))
        .collect();
    let Some(indices) = indices.filter(|indices| indices.len() == blen as usize) else {
        return;
    };
    for (j, index) in indices.into_iter().enumerate() {
        wr_u64(bptr + j * 8, index);
    }
}

// The player's pin for build slot `slot`, as a catalog index, for `cap_spawn`.
unsafe fn spawn_pin_at(champ: &str, cat_base: usize, cat_len: u64, slot: usize) -> Option<u64> {
    slot_n_catalog_index(champ, slot as u8, |key| {
        scan_catalog_index(cat_base, cat_len, key)
    })
}

fn install_spawn_hook() {
    if !SPAWN_INJECT_ENABLED {
        return;
    } // a sealed hook is not installed at all
    let state = SPAWN_INSTALLED.load(Ordering::Relaxed);
    if state == 1 {
        return;
    }
    // See `install_retry_due`.
    static RETRY: AtomicU64 = AtomicU64::new(0);
    if state == 2 && !install_retry_due(&RETRY) {
        return;
    }
    // The r11 tail preserves rax, which 0.5.2's relocated `mov eax,frame`
    // needed.
    let r = unsafe {
        install_detour_r11(
            SPAWN_RVA,
            SPAWN_ORIG_LEN,
            cap_spawn as *const () as usize,
            &SPAWN_PROLOGUE,
        )
    };
    SPAWN_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    crate::own_team_log::line(|| format!("spawn hook install @ {SPAWN_RVA:#x}: {r:?}"));
}

// Frames between retries of a hook install that has not succeeded.
const INSTALL_RETRY_FRAMES: u64 = 60;

// Whether a failed installer should retry this frame. A failed install takes
// the loader and address-space locks each time, and after a game update every
// stale RVA fails, so retries wait [`INSTALL_RETRY_FRAMES`] frames.
fn install_retry_due(tick: &AtomicU64) -> bool {
    tick.fetch_add(1, Ordering::Relaxed) % INSTALL_RETRY_FRAMES == 0
}

unsafe fn install_detour_generic(
    rva: usize,
    orig_len: usize,
    cap_fn: usize,
    prologue: &[u8],
) -> Result<usize, &'static str> {
    // Cached: the raw `GetModuleHandleW` takes the loader lock.
    let base = exe_base_addr();
    if base == 0 {
        return Err("module 0");
    }
    let fn_addr = base + rva;
    if !readable(fn_addr, orig_len + 4) {
        return Err("unreadable");
    }
    // Chain hooking: when the entry already holds a foreign hook (`movabs
    // rax,tgt; jmp rax`), chain to it and skip the prologue check.
    let mut cur = [0u8; 12];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, cur.as_mut_ptr(), 12);
    let foreign_tgt: usize =
        if cur[0] == 0x48 && cur[1] == 0xb8 && cur[10] == 0xff && cur[11] == 0xe0 {
            usize::from_le_bytes(cur[2..10].try_into().unwrap())
        } else {
            0
        };
    let chained = foreign_tgt >= 0x10000;
    // The prologue check against a wrong RVA, unless chaining.
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
    // Bisection aid: the original instructions and a return only, no capture.
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
    // Pushes r12 rsi rdi rbx r11 r10 r9 r8 rdx rcx, so rcx is saved+0 and r12
    // saved+0x48. Entry rsp is not captured into r10, which the relocated
    // instructions save.
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
        // Chaining: skip the original prologue and jump to the foreign stub,
        // which runs it and returns into the function.
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

// The rax-preserving variant of [`install_detour_generic`]: the tail jumps
// through r11, so a relocated `mov eax,frame` survives into chkstk. Foreign
// hooks are not chained.
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

// Every client frame, from `driver::post_update`.
fn tactics_post_update(client: &mut StableClient<'_>, in_game: bool) {
    {
        install_launcher_hook();
        install_seed_ctor_hook();
        install_spawn_hook();
        // The player's team id, for team scoping.
        if let (true, Some(pid)) = (in_game, client.player_team_id()) {
            // `player_team_id()` reads 0 or -1 at times, 0 throughout comp
            // tests, and a 0 publishes team 0's lineup as mine. So only 0..9999
            // is stored, a 0 never replaces a non-zero id, and 0s during tests
            // are ignored (`COMPTEST_MATCH` hangs on stale 0.5.3 retaddrs,
            // hence `build_config::training_match`).
            let in_comptest =
                COMPTEST_MATCH.load(Ordering::Relaxed) || crate::build_config::training_match();
            let pu = pid as u64;
            // Counts 0 reports outside tests, so a save whose team id really is
            // 0 is trusted after a while (`trust` below).
            let ct_ctx = in_comptest;
            if pu == 0 && !ct_ctx {
                // an observation of 0 unrelated to comp test
                PID_ZERO_CLEAN.fetch_add(1, Ordering::Relaxed);
            }
            if pu != u64::MAX && pu < 10000 && !(pu == 0 && ct_ctx) {
                if pu != 0 {
                    PLAYER_TEAM_ID.store(pu, Ordering::Relaxed);
                    PID_NONZERO_SEEN.store(1, Ordering::Relaxed);
                } else if PID_NONZERO_SEEN.load(Ordering::Relaxed) == 0 {
                    PLAYER_TEAM_ID.store(0, Ordering::Relaxed);
                }
            }
            // Publish my team's athlete ids for the spawn hook: everyone under
            // contract (`roster_scan_step`) plus the last starting five,
            // refreshed every `ROSTER_POLL` frames.
            {
                const ROSTER_POLL: u64 = 120; // frames
                let n = ROSTER_TICK.fetch_add(1, Ordering::Relaxed);
                let known = PLAYER_TEAM_ID.load(Ordering::Relaxed);
                let valid = known != u64::MAX && known < 10000;
                // Every frame: the contract scan reads a few records at a time.
                let scanned = valid && roster_scan_step(client, known as usize);
                if valid && (n % ROSTER_POLL == 0 || scanned) {
                    // The contracted roster, with the last starting five as a
                    // floor.
                    let starting = stable_last_starting(client, known as usize);
                    let roster = contracted_roster(known as usize);
                    let mut my = starting.clone();
                    my.extend(roster.iter().copied());
                    // A team id of 0 is withheld (the team gate stays closed)
                    // until 0 has been seen for 600 frames outside tests, so a
                    // test-only session never publishes team 0.
                    let trust = known != 0 || PID_ZERO_CLEAN.load(Ordering::Relaxed) >= 600;
                    crate::own_team_log::on_change("roster", {
                        let mut ids: Vec<u64> = starting.iter().copied().collect();
                        ids.sort_unstable();
                        format!(
                            "pid_raw={} team_id={} trust={} last_starting={:?} contracted_n={} published_n={} comptest={}",
                            pid,
                            known,
                            trust,
                            ids,
                            roster.len(),
                            MY_ATH_N.load(Ordering::Relaxed),
                            in_comptest
                        )
                    });
                    if !my.is_empty() && trust {
                        publish_my_athletes(my);
                    }
                }
            }
        }
    }
}

// Server start, from `driver::on_server_start`.
fn tactics_on_server_start() {
    // Session boundary: the item network belongs to the save that was just
    // left, and its picks were made for that save's matches.
    NETWORK_AGENT.store(0, Ordering::Relaxed);
    network_picks_forget();
    install_replace_4th();
    install_launcher_hook();
    install_seed_ctor_hook();
    install_spawn_hook();
}

fn tactics_before_management_tick() {
    install_replace_4th(); // idempotent
}

// ===========================================================================
//  Roster reads through the stable client
// ===========================================================================

// Athlete ids of `team_id`'s starting five; empty when unreadable, which the
// caller takes as nothing to publish.
fn stable_last_starting(
    client: &StableClient<'_>,
    team_id: usize,
) -> std::collections::HashSet<u64> {
    let mut out = std::collections::HashSet::new();
    let Some(json) = client.record_get_json(RecordKindV1::Team, team_id, "last_starting") else {
        return out;
    };
    let Ok(serde_json::Value::Array(slots)) = serde_json::from_str::<serde_json::Value>(&json) else {
        return out;
    };
    for id in slots.iter().filter_map(serde_json::Value::as_f64) {
        if id >= 0.0 {
            out.insert(id as u64);
        }
    }
    out
}

// ── The contracted roster ──────────────────────────────────────────────
//
// Everyone under contract with the player's team, read off each athlete
// record's `contract`. `last_starting` alone is empty until the team has
// played, so a new save's first match would spawn before anything was
// published.
//
// A read costs 0.1-0.2 ms, so a pass is spread over frames
// ([`ROSTER_SCAN_BATCH`]) and repeated only when the in-game day changes, at
// most every [`ROSTER_RESCAN_FRAMES`].

// Athlete records [`roster_scan_step`] reads per frame: under 3 ms a frame and
// about a second a pass (0.6.2).
const ROSTER_SCAN_BATCH: usize = 16;
// Frames (~10 s) a finished pass waits before a day change starts another.
const ROSTER_RESCAN_FRAMES: u64 = 600;

type GameDay = Option<(i32, u32, u32)>;

#[derive(Default)]
struct RosterScan {
    team: usize,
    // The athlete records this pass walks, and how far it has got.
    ids: Vec<usize>,
    next: usize,
    found: std::collections::HashSet<u64>,
    // Time spent reading, summed over the frames of this pass.
    work: std::time::Duration,
    frames: u64,
    // One contract fragment, logged once so the shape can be checked.
    sample: Option<String>,
    // The last finished pass, and what has happened since.
    done: Option<std::collections::HashSet<u64>>,
    day: GameDay,
    idle_frames: u64,
}

static ROSTER_SCAN: Mutex<Option<RosterScan>> = Mutex::new(None);

// Advances the contracted-roster scan for `team_id` by one frame. True on
// the frame a pass finishes, so the caller can publish at once rather than
// on its next poll.
fn roster_scan_step(client: &StableClient<'_>, team_id: usize) -> bool {
    let mut guard = ROSTER_SCAN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().map_or(true, |scan| scan.team != team_id) {
        *guard = Some(RosterScan {
            team: team_id,
            ..RosterScan::default()
        });
    }
    let Some(scan) = guard.as_mut() else {
        return false;
    };
    if scan.next >= scan.ids.len() {
        scan.idle_frames += 1;
        let day: GameDay = client.game_time().map(|(y, m, d, _, _)| (y, m, d));
        let due =
            scan.done.is_none() || (scan.day != day && scan.idle_frames >= ROSTER_RESCAN_FRAMES);
        if !due {
            return false;
        }
        let ids = client.record_ids(RecordKindV1::Athlete);
        if ids.is_empty() {
            return false; // the save is still loading
        }
        scan.ids = ids;
        scan.next = 0;
        scan.found.clear();
        scan.work = std::time::Duration::ZERO;
        scan.frames = 0;
        scan.day = day;
    }
    let started = std::time::Instant::now();
    let end = (scan.next + ROSTER_SCAN_BATCH).min(scan.ids.len());
    for &id in &scan.ids[scan.next..end] {
        let Some(json) = client.record_get_json(RecordKindV1::Athlete, id, "contract") else {
            continue;
        };
        let team = contract_team_id(&json);
        if scan.sample.is_none() && team.is_some() {
            scan.sample = Some(json.chars().take(200).collect());
        }
        if team == Some(team_id) {
            scan.found.insert(id as u64);
        }
    }
    scan.next = end;
    scan.work += started.elapsed();
    scan.frames += 1;
    if scan.next < scan.ids.len() {
        return false;
    }
    crate::own_team_log::line(|| {
        format!(
            "roster scan: team {team_id} day {:?}: {} of {} athletes under contract \
             ({:.1} ms over {} frames); sample contract: {}",
            scan.day,
            scan.found.len(),
            scan.ids.len(),
            scan.work.as_secs_f64() * 1000.0,
            scan.frames,
            scan.sample.as_deref().unwrap_or("(none)")
        )
    });
    scan.done = Some(std::mem::take(&mut scan.found));
    scan.idle_frames = 0;
    true
}

// The last finished pass of [`roster_scan_step`] for `team_id`; empty until
// one has finished.
fn contracted_roster(team_id: usize) -> std::collections::HashSet<u64> {
    let guard = ROSTER_SCAN.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .filter(|scan| scan.team == team_id)
        .and_then(|scan| scan.done.clone())
        .unwrap_or_default()
}

// The team a serialized `contract` is with: its first `team_id`, or `None`
// for a free agent (whose `requests` can hold other teams' ids).
fn contract_team_id(json: &str) -> Option<usize> {
    if json.contains("FreeAgent") {
        return None;
    }
    let key = "\"team_id\"";
    let rest = json[json.find(key)? + key.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// The buy hook on `buy_item`: r8 = athlete, r9 = provider, [rsp_entry+0x30] =
// Game, Game+0x30 = catalog, the index out in rdx. Since 0.6.3 it is reached
// through slot +0x80 of the `run_tick` vtable; not the 189-byte "anything to
// buy" test with the same `cmp qword [r8+<build len>],0` head, which takes the
// Game in r9.
const RVA_BUY_ITEM: usize = 0xf755b0; // 0.6.4 (0.6.3: 0xf3f510); exe2exe strict unique at 317 bytes, the run_tick vtable's +0x80 slot
const BUY_PROLOGUE: [u8; 12] = [
    0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53,
]; // 0.6.3: push r15/r14/r13/r12/rsi/rdi/rbp/rbx, 12B, all the trampoline relocates (`install_replace_4th`)
static BUY_PROBE_INSTALLED: AtomicU64 = AtomicU64::new(0);

#[inline]
unsafe fn rd_u64(p: usize) -> u64 {
    std::ptr::read_unaligned(p as *const u64)
}
#[inline]
unsafe fn wr_u64(p: usize, v: u64) {
    std::ptr::write_unaligned(p as *mut u64, v);
}

// The item network's scorer, called directly for the 5th and 6th item
// (`network_pick`): rcx net, rdx lineup ([rdx+lane*8] the champion, +0x28 on
// the five enemies with 9999 for nobody, [rdx+0x50] the lane, which panics
// above 4), r8/r9 the build slice, [rsp+0x20] a noise flag; xmm0 the score.
//
// Found as the scoring callee of the beam search the hook.rs target calls. A
// sibling function trains the weights, so a score is only good when it was
// asked for (`NETWORK_PICKS`).
const ITEMNET_FORWARD_RVA: usize = 0x1887ac0; // 0.6.4 (0.6.3: 0x1932130); exe2exe strict unique at 1609 bytes
// The eight pushes and `sub rsp, 0xd8`. The pushes alone open thousands of
// functions; the frame size is what makes a stale address fail this.
const ITEMNET_FORWARD_PROLOGUE: [u8; 19] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0xd8,
    0x00, 0x00, 0x00,
];
type ItemNetFn = unsafe extern "C" fn(usize, usize, *const u64, u64, u8) -> f32;
// The agent `hook::detour` was last handed, exactly as it came: the network
// the 5th and 6th item are scored with ([`network_pick`]), which proves it
// before every pick ([`network_ready`]).
static NETWORK_AGENT: AtomicU64 = AtomicU64::new(0);
// Most weights [`network_ready`] accepts. The network has 16384.
const NETWORK_WEIGHTS_MAX: usize = 1 << 20;
// Whether `net` can be handed to `itemnet_forward`: a readable weight array of
// the length the network states. The function bounds-checks every index against
// that count itself.
unsafe fn network_ready(net: usize) -> bool {
    if net < 0x10000 || !readable(net, 0x20) {
        return false;
    }
    let weights = rd_u64(net + 0x8) as usize;
    let count = rd_u64(net + 0x10) as usize;
    weights >= 0x10000 && (1..=NETWORK_WEIGHTS_MAX).contains(&count) && readable(weights, count * 4)
}
static ITEMNET_VALID: AtomicU64 = AtomicU64::new(0); // 0 = unchecked, 1 = valid, 2 = invalid
unsafe fn itemnet_addr_valid() -> bool {
    match ITEMNET_VALID.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let fa = exe_base_addr() + ITEMNET_FORWARD_RVA;
    let len = ITEMNET_FORWARD_PROLOGUE.len();
    let ok = readable(fa, len)
        && std::slice::from_raw_parts(fa as *const u8, len) == ITEMNET_FORWARD_PROLOGUE.as_slice();
    ITEMNET_VALID.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
    ok
}
const SHADOW_CALL_NAMES: bool = true; // gate on the name getter shadow-calls (an AV risk)

// A start offset spread deterministically by champion name (FNV-1a), so a
// candidate walk does not hand every champion the same answer and a replay
// matches the match.
fn champ_spread(champ: &str, modulo: usize) -> usize {
    if modulo == 0 {
        return 0;
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in champ.as_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    (h % modulo as u64) as usize
}

// The first free final that `matches`, from a champion-spread offset so the
// league does not converge on one stand-in. Free means unclaimed by earlier
// slots (`taken`) and in this match's catalog with a recipe.
unsafe fn pick_candidate(
    ctx: usize,
    wanted: u64,
    taken: &[u64],
    champ: &str,
    matches: impl Fn(&str) -> bool,
) -> Option<u64> {
    let candidates = auto_cands();
    let start = champ_spread(champ, candidates.len());
    for step in 0..candidates.len() {
        let id = candidates[(start + step) % candidates.len()];
        let Some(key) = item_id_to_key(id) else {
            continue;
        };
        if !matches(&key) {
            continue;
        }
        let Some(index) = scan_idx_cached(ctx, key.as_bytes()) else {
            continue;
        };
        if index != wanted && !taken.contains(&index) {
            return Some(index);
        }
    }
    None
}

// Item game id -> key (0..29 vanilla, 30+ mod items).
fn item_id_to_key(id: u64) -> Option<String> {
    if (id as usize) < VANILLA_KEYS.len() {
        return Some(VANILLA_KEYS[id as usize].to_string());
    }
    let reg = MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    reg.get((id as usize).checked_sub(30)?).cloned()
}
// Whether the buy build `Vec` is grown from the engine's four to
// `build_config::picker_slots()`. Every grow goes through `realloc_ok`, since a
// stale `RVA_REALLOC` crashes mid-match.
const BUILD_EXTEND_ENABLED: bool = true;
static AUTO_CANDS: Mutex<Option<std::sync::Arc<Vec<u64>>>> = Mutex::new(None);
fn auto_cands() -> std::sync::Arc<Vec<u64>> {
    {
        let g = AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = g.as_ref() {
            return v.clone();
        } // Arc clone = refcount only (no data copy)
    }
    let mut v: Vec<u64> = VANILLA_FINAL.to_vec();
    for (id, _) in mod_final_opts_all() {
        v.push(id);
    }
    let arc = std::sync::Arc::new(v);
    *AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(arc.clone());
    arc
}

// ---------------------------------------------------------------------------
//  Build-slot lookups
// ---------------------------------------------------------------------------
//
// The pins from `item-builds.json`. The 5th and 6th always come through here,
// since the stable hook cannot reach them; the game's four only under
// `own_team_only`.

// The pinned item key for one build slot, normalized (radiant + alias) the way
// the item catalog is keyed.
fn slot_n_item_key(champ: &str, si: u8) -> Option<String> {
    crate::build_config::pinned_key(champ, si as usize)
}

// Catalog index of a slot's pin, by key through `lookup` (a name scan of the
// live catalog), never by id: the catalog grows and reorders with mods and
// saves. The normalized key is tried first (`bloodthirster` finds
// `warlords_final_judgement`), then the key as written.
fn slot_n_catalog_index(champ: &str, si: u8, lookup: impl Fn(&[u8]) -> Option<u64>) -> Option<u64> {
    let raw = crate::build_config::pinned_key_raw(champ, si as usize)?;
    slot_n_item_key(champ, si)
        .and_then(|key| lookup(key.as_bytes()))
        .or_else(|| lookup(raw.as_bytes()))
}

// Catalog index of the final named `want` that has a recipe (the game panics
// building one without), in the catalog at `base`/`len`. The same index space
// as the buy context's, so a build slot takes it as-is; cached in `SCAN_CACHE`.
unsafe fn scan_catalog_index(base: usize, len: u64, want: &[u8]) -> Option<u64> {
    if want.is_empty() || base < 0x10000 || len == 0 || len > 100000 {
        return None;
    }
    if let Some(index) = cached_catalog_index(base, len, want) {
        return index;
    }
    let res = scan_recipe_safe_in(base, len, want);
    // Read outside the lock: it calls into the game.
    let last = catalog_name_in(base, len, len - 1);
    let mut g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let outer = g.get_or_insert_with(HashMap::new);
    if !outer.contains_key(&len) && outer.len() >= 16 {
        outer.clear(); // reset when too many catalogs (memory cap)
    }
    let cache = outer.entry(len).or_insert_with(|| CatalogCache {
        last,
        found: HashMap::new(),
    });
    if cache.found.len() < 256 {
        cache
            .found
            .insert(want.to_vec(), res.map(|i| i as i64).unwrap_or(-1));
    }
    res
}
// The scan behind [`scan_catalog_index`]: the first entry named `want`, if it
// has a recipe. Reads are VEH-guarded (`catalog_entry_named`), since
// `VirtualQuery` checks made a cold walk cost ~6 ms.
unsafe fn scan_recipe_safe_in(data: usize, len: u64, want: &[u8]) -> Option<u64> {
    if want.is_empty() || data < 0x10000 || len == 0 || len > 100000 {
        return None;
    }
    let i = (0..len).find(|&i| catalog_entry_named(data, len, i, want))?;
    // The recipe getter returns 0 for a base item, which the game would panic
    // building. Only the first entry with the name counts.
    let e = data + (i as usize) * 16;
    let edata = safe_read_u64(e)? as usize;
    let evt = safe_read_u64(e + 8)? as usize;
    let recfn = safe_read_u64(evt + 0x70)? as usize; // the recipe getter
    if !name_getter_ok(recfn) {
        return None;
    }
    let rf: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(recfn);
    (rf(edata) != 0).then_some(i)
}

// Name -> catalog index cache, keyed by catalog length, since every sim copy of
// a match builds its own catalog from the same item list. A hit is re-read by
// name before use and a miss trusted only while the catalog's last entry is
// unchanged; a failed check forgets that length's answers.
static SCAN_CACHE: Mutex<Option<HashMap<u64, CatalogCache>>> = Mutex::new(None);

// Cached lookups against every catalog of one length.
struct CatalogCache {
    // Name of the catalog's last entry when this cache was started — what a
    // cached miss is checked against.
    last: Option<String>,
    // Key -> catalog index, `-1` = not found / no recipe.
    found: HashMap<Vec<u8>, i64>,
}

// A cached answer for `want`, re-checked: `Some(Some(i))` a verified index,
// `Some(None)` a miss whose catalog still ends in the same entry, `None`
// nothing usable (scan). A failed check forgets the catalog's answers, so no
// stale miss outlives a stale hit.
unsafe fn cached_catalog_index(base: usize, len: u64, want: &[u8]) -> Option<Option<u64>> {
    let (cached, last) = {
        let g = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = g.as_ref()?.get(&len)?;
        (*cache.found.get(want)?, cache.last.clone())
    };
    let verified = if cached >= 0 {
        catalog_entry_named(base, len, cached as u64, want).then_some(Some(cached as u64))
    } else {
        last.is_some_and(|name| catalog_entry_named(base, len, len - 1, name.as_bytes()))
            .then_some(None)
    };
    if verified.is_none() {
        forget_catalog(len);
    }
    verified
}

// Whether catalog entry `idx` is named `want`. On the buy hot path, so it reads
// VEH-guarded and proves each name getter is code only once.
unsafe fn catalog_entry_named(data: usize, len: u64, idx: u64, want: &[u8]) -> bool {
    if idx >= len || data < 0x10000 {
        return false;
    }
    let e = data + (idx as usize) * 16;
    let (Some(edata), Some(evt)) = (safe_read_u64(e), safe_read_u64(e + 8)) else {
        return false;
    };
    let (edata, evt) = (edata as usize, evt as usize);
    if edata < 0x10000 || evt < 0x10000 {
        return false;
    }
    let Some(namefn) = safe_read_u64(evt + 0x58).map(|f| f as usize) else {
        return false;
    };
    if !name_getter_ok(namefn) {
        return false;
    }
    let f: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(namefn);
    let nobj = f(edata);
    if nobj < 0x10000 {
        return false;
    }
    let (Some(chars), Some(nlen)) = (safe_read_u64(nobj + 8), safe_read_u64(nobj + 0x10)) else {
        return false;
    };
    if chars < 0x10000 || nlen as usize != want.len() {
        return false;
    }
    let mut name = Vec::new();
    safe_read_bytes(chars as usize, want.len(), &mut name) && name == want
}

// Getter addresses already proven to be code. Sixteen, since the recipe getters
// share the ring with the name getters.
static NAME_GETTERS: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];
static NAME_GETTER_NEXT: AtomicUsize = AtomicUsize::new(0);

// Whether `namefn` (a name or recipe getter) is code, proving and remembering
// it on first sight since `code_ptr_ok` is a syscall. The low-address test
// comes first because an empty slot is 0.
unsafe fn name_getter_ok(namefn: usize) -> bool {
    if namefn < 0x10000 {
        return false;
    }
    if NAME_GETTERS
        .iter()
        .any(|g| g.load(Ordering::Relaxed) == namefn)
    {
        return true;
    }
    if !code_ptr_ok(namefn) {
        return false;
    }
    let slot = NAME_GETTER_NEXT.fetch_add(1, Ordering::Relaxed) % NAME_GETTERS.len();
    NAME_GETTERS[slot].store(namefn, Ordering::Relaxed);
    true
}

fn forget_catalog(len: u64) {
    if let Some(outer) = SCAN_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
    {
        outer.remove(&len);
    }
}

// VEH-guarded reads, not `readable`: the candidate searches call this once per
// candidate (see [`catalog_name_at`]).
unsafe fn scan_idx_cached(ctx: usize, want: &[u8]) -> Option<u64> {
    if ctx < 0x10000 {
        return None;
    }
    let coll = safe_read_u64(ctx + 0x30)? as usize; // the catalog collection
    if coll < 0x10000 {
        return None;
    }
    scan_catalog_index(
        safe_read_u64(coll + 8)? as usize,
        safe_read_u64(coll + 0x10)?,
        want,
    )
}

// What goes into build slot `si` when the `Vec` grows (4 and 5 are the 5th and
// 6th, 3 the 4th of a rare three-long build). A designated athlete
// (`buy_replace_ctx`'s team gate) gets its pin, everyone else and blank slots
// [`auto_extra_pick`], since the engine never plans these slots.
unsafe fn extra_slot_pick(
    ctx: usize,
    buyer: Buyer,
    champ: &str,
    si: usize,
    taken: &[u64],
    designate: bool,
) -> Option<u64> {
    designate
        .then(|| pinned_extra_slot(ctx, champ, si, taken))
        .flatten()
        .inspect(|_| picked_by("pin"))
        .or_else(|| {
            extra_slot_boots(ctx, champ, si, taken, designate).inspect(|_| picked_by("boots rule"))
        })
        .or_else(|| {
            let reserved = if designate {
                later_pins(ctx, champ, si + 1)
            } else {
                Vec::new()
            };
            // The player's first slot pinned: no jungle or World Atlas item
            // (Smart Builds rules 9 and 12).
            let first_pinned = designate
                && slot_n_catalog_index(champ, 0, |key| scan_idx_cached(ctx, key)).is_some();
            auto_extra_pick(ctx, buyer, champ, si, taken, &reserved, first_pinned)
        })
}

thread_local! {
    // How `extra_slot_pick` last filled a slot on this thread, for the Check
    // Tactics test log.
    static PICKED_BY: core::cell::Cell<&'static str> = const { core::cell::Cell::new("") };
}

fn picked_by(how: &'static str) {
    PICKED_BY.with(|picked| picked.set(how));
}

// Smart Builds' boots for the 5th or 6th slot, which the rule leaves here when
// pins hold slots 2-4 (`smart_builds::enforce`). Never over a pin or beside a
// pinned pair; the pair is the one the rule picked with the enemy lineup in
// view.
unsafe fn extra_slot_boots(
    ctx: usize,
    champ: &str,
    si: usize,
    taken: &[u64],
    designate: bool,
) -> Option<u64> {
    if !crate::build_config::smart_builds_enabled()
        || taken.iter().any(|&index| buy_is_boots(ctx, index))
    {
        return None;
    }
    if designate {
        let player_boots = (0..crate::build_config::picker_slots()).any(|j| {
            slot_n_catalog_index(champ, j as u8, |key| scan_idx_cached(ctx, key))
                .is_some_and(|pin| buy_is_boots(ctx, pin))
        });
        if player_boots || crate::build_config::pinned_key_raw(champ, si).is_some() {
            return None;
        }
    }
    let key = crate::build_config::rule_boots(champ).unwrap_or_else(|| {
        let role = crate::build_config::role_for_champion(champ);
        crate::smart_builds::boots_for(champ, role, &[]).to_string()
    });
    scan_idx_cached(ctx, key.as_bytes())
}

// The pinned item for build slot `si`, as a catalog index. Planted exactly as
// written: Smart Builds only ever rewrites the AI's picks, never the player's.
unsafe fn pinned_extra_slot(ctx: usize, champ: &str, si: usize, taken: &[u64]) -> Option<u64> {
    slot_n_catalog_index(champ, si as u8, |key| scan_idx_cached(ctx, key))
        .filter(|&t| !pin_placed_by_engine(ctx, champ, t, taken))
}

// Whether pinned item `t` already sits in an earlier slot because the engine
// put it there (slot 0 under `own_team_only`, or a blank slot), so planting it
// again would build it twice. A slot whose own pin is `t` is the player
// duplicating on purpose; boots count as placed when the engine holds any pair.
unsafe fn pin_placed_by_engine(ctx: usize, champ: &str, t: u64, taken: &[u64]) -> bool {
    let pin_boots = buy_is_boots(ctx, t);
    taken.iter().enumerate().any(|(j, &v)| {
        (v == t || (pin_boots && buy_is_boots(ctx, v)))
            && slot_n_catalog_index(champ, j as u8, |key| scan_idx_cached(ctx, key)) != Some(v)
    })
}

// Whether catalog index `index` is a pair of boots, on the buy path.
unsafe fn buy_is_boots(ctx: usize, index: u64) -> bool {
    catalog_name_at(ctx, index).is_some_and(|name| crate::smart_builds::is_boots(&name))
}

// The player's pins for build slots `from` onward, as catalog indices — slots
// not planted yet but already spoken for, which an automatic pick for an
// earlier slot must not duplicate or crowd out.
unsafe fn later_pins(ctx: usize, champ: &str, from: usize) -> Vec<u64> {
    (from..crate::build_config::picker_slots())
        .filter_map(|si| slot_n_catalog_index(champ, si as u8, |key| scan_idx_cached(ctx, key)))
        .collect()
}

// What the slots before this one have spent of the Smart Builds budgets; an
// index the scan cannot name adds nothing.
unsafe fn spent_budget(
    ctx: usize,
    fit: crate::smart_builds::Fit,
    taken: &[u64],
) -> crate::smart_builds::Budget {
    let keys: Vec<String> = taken
        .iter()
        .filter_map(|&index| catalog_name_at(ctx, index))
        .collect();
    crate::smart_builds::Budget::spent(keys.iter().map(String::as_str), fit)
}

// The athlete a build slot is being filled for, and the seed of its match.
#[derive(Clone, Copy)]
struct Buyer {
    athlete: usize,
    seed: u64,
}

// The automatic 5th and 6th item: the final the game's item network wants most
// on top of the build ([`network_pick`]), held to Smart Builds, with rule 17
// supports asked about their preferred items first. Without the network, the
// first final the rules accept from a champion-spread start, then any final;
// never one in `taken` or the later pins `reserved`, and no starter item when
// `first_pinned`.
unsafe fn auto_extra_pick(
    ctx: usize,
    buyer: Buyer,
    champ: &str,
    si: usize,
    taken: &[u64],
    reserved: &[u64],
    first_pinned: bool,
) -> Option<u64> {
    // `taken` stays the build in slot order, for the network; everything else
    // works from the whole build, pins still to come included.
    let spoken = [taken, reserved].concat();
    // What the champion may hold at all, toggle or not: the engine only offers
    // finals matching the champion's tags, and these slots are picked on its
    // behalf (as in `item_build_hook::score_item`).
    let fit = crate::smart_builds::fit(champ, crate::build_config::role_for_champion(champ));
    let suits = crate::smart_builds::Budget::empty(fit);
    // The build-wide rules too while the toggle is on (`None` when off); a
    // pinned first slot rules out the jungle and World Atlas items (rules 9 and
    // 12).
    let rule_fit = if first_pinned {
        fit.starter_slot_pinned()
    } else {
        fit
    };
    let budget =
        crate::build_config::smart_builds_enabled().then(|| spent_budget(ctx, rule_fit, &spoken));
    // Rule 6 too: these slots are past the first two, where an early item
    // goes (supports are exempt).
    let allowed = |candidate: &str| {
        suits.rejects(candidate).is_none()
            && budget.as_ref().is_none_or(|budget| {
                budget.rejects(candidate).is_none() && fit.fits_slot(candidate, si)
            })
    };
    // Rule 17: a support that looks to the heal, shield and buff items first
    // takes these slots from them while one is left, by the network among those
    // alone. Only with the toggle on, as a preference.
    let aid_first = |candidate: &str| fit.is_preferred_aid_item(candidate) && allowed(candidate);
    let preferred = if budget.is_some() && fit.prefers_aid_items() {
        network_pick(ctx, buyer, champ, si, taken, &spoken, &aid_first)
            .inspect(|_| picked_by("network, heal/shield/buff items first"))
            .or_else(|| {
                pick_candidate(ctx, u64::MAX, &spoken, champ, &aid_first)
                    .inspect(|_| picked_by("first heal/shield/buff item the rules allow"))
            })
    } else {
        None
    };
    preferred
        .or_else(|| {
            network_pick(ctx, buyer, champ, si, taken, &spoken, &allowed)
                .inspect(|_| picked_by("network"))
        })
        .or_else(|| {
            pick_candidate(ctx, u64::MAX, &spoken, champ, &allowed)
                .inspect(|_| picked_by("first the rules allow"))
        })
        // Last resort: any final, but while the toggle is on never a jungle
        // item off the jungle or a starter item behind a pinned first slot.
        .or_else(|| {
            pick_candidate(ctx, u64::MAX, &spoken, champ, |candidate| {
                budget.is_none()
                    || !(fit.off_role_jungle_item(candidate) || rule_fit.skips_starter(candidate))
            })
            .inspect(|_| picked_by("last resort, no rule held"))
        })
}

// What the item network's lineup table holds for a seat nobody is in.
const NET_NO_CHAMPION: u64 = 9999;

// The network's picks so far, `(seed, team, lane, slot)` -> item key. The
// network keeps training and a match is played more than once (in the
// background, then on screen), so a seat keeps its first answer or the watched
// match drifts from the recorded one.
static NETWORK_PICKS: Mutex<Option<HashMap<(u64, u64, u64, u64), String>>> = Mutex::new(None);

// Seats [`NETWORK_PICKS`] holds before it starts over: about eight hundred
// matches of ten athletes and two slots.
const NETWORK_PICKS_MAX: usize = 16384;

fn network_pick_recall(seat: (u64, u64, u64, u64)) -> Option<String> {
    let picks = NETWORK_PICKS.lock().unwrap_or_else(|e| e.into_inner());
    picks.as_ref()?.get(&seat).cloned()
}

fn network_pick_remember(seat: (u64, u64, u64, u64), item: String) {
    let mut guard = NETWORK_PICKS.lock().unwrap_or_else(|e| e.into_inner());
    let picks = guard.get_or_insert_with(HashMap::new);
    if picks.len() >= NETWORK_PICKS_MAX {
        picks.clear();
    }
    picks.entry(seat).or_insert(item);
}

// Drops every remembered pick: another save's matches reuse seeds.
fn network_picks_forget() {
    *NETWORK_PICKS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// The final the game's item network scores highest as this athlete's next item,
// among those `allowed` and not in `spoken`; `None` when it cannot be asked. A
// seat keeps its first answer ([`NETWORK_PICKS`]).
//
// One step of the engine's own beam search, but over Smart Builds' candidates,
// with only this champion in the lineup (the two enemy features sit out) and no
// boots in the build shown (the network has never seen a pair).
unsafe fn network_pick(
    ctx: usize,
    buyer: Buyer,
    champ: &str,
    si: usize,
    taken: &[u64],
    spoken: &[u64],
    allowed: &dyn Fn(&str) -> bool,
) -> Option<u64> {
    // The network indexes a five-entry table with the lane and panics past it.
    let lane = (safe_read_u64(buyer.athlete + O_ATHLETE_POS)? & 0xffff_ffff) as usize;
    if lane >= 5 {
        return None;
    }
    let team = safe_read_u64(buyer.athlete + O_ATHLETE_TEAM)?;
    let seat = (buyer.seed != 0).then_some((buyer.seed, team, lane as u64, si as u64));
    if let Some(item) = seat.and_then(network_pick_recall) {
        // Held to the rules and the build as they are now: a pin added since
        // may have taken the item, or the slot before it.
        if allowed(&item) {
            if let Some(index) = scan_idx_cached(ctx, item.as_bytes()) {
                if !spoken.contains(&index) {
                    return Some(index);
                }
            }
        }
    }

    let net = NETWORK_AGENT.load(Ordering::Relaxed) as usize;
    // Once per pick, not per candidate: `network_ready` is two `VirtualQuery`
    // calls, and a pick scores every final there is.
    if !itemnet_addr_valid() || !network_ready(net) {
        return None;
    }
    let champion = crate::build_config::champion_roster_index(champ)? as u64;
    let mut lineup = [NET_NO_CHAMPION; 11];
    lineup[lane] = champion;
    lineup[10] = lane as u64;
    let mut build: Vec<u64> = taken
        .iter()
        .copied()
        .filter(|&index| !buy_is_boots(ctx, index))
        .collect();
    build.push(0);
    let last = build.len() - 1;

    let forward: ItemNetFn = core::mem::transmute(exe_base_addr() + ITEMNET_FORWARD_RVA);
    let mut best: Option<(f32, u64, String)> = None;
    // From a start spread by champion, like every other walk of this list: a
    // network with nothing to say yet scores every build the same, and would
    // otherwise hand the whole league the first final in it.
    let candidates = auto_cands();
    let start = champ_spread(champ, candidates.len());
    for step in 0..candidates.len() {
        let Some(key) = item_id_to_key(candidates[(start + step) % candidates.len()]) else {
            continue;
        };
        if !allowed(&key) {
            continue;
        }
        let Some(index) = scan_idx_cached(ctx, key.as_bytes()) else {
            continue;
        };
        if spoken.contains(&index) {
            continue;
        }
        build[last] = index;
        let score = forward(
            net,
            lineup.as_ptr() as usize,
            build.as_ptr(),
            build.len() as u64,
            0,
        );
        // What the save's item balance patches have made of the item, as in
        // the stable hook's `score_item`: nothing while no patch has moved it.
        let score = score + crate::patches::leaning(&key) * crate::patches::LEAN_SCORE;
        // The first of equals wins, so a tie falls the same way every time.
        if !score.is_nan() && best.as_ref().is_none_or(|(top, _, _)| score > *top) {
            best = Some((score, index, key));
        }
    }
    let (_, index, key) = best?;
    if let Some(seat) = seat {
        network_pick_remember(seat, key);
    }
    Some(index)
}

// Whether [`retier_unstarted`] has capped a build this session. Until it
// has, no build can hold a legendary target and nothing needs putting back,
// so a session that never leaves All Radiants runs none of this.
static TIERS_RULED: AtomicBool = AtomicBool::new(false);

// Item Rules (`crate::item_tiers`): rewrites every slot target from `from` on
// to the form `form` gives, where the catalog has it. `from` is never below the
// owned count: lowering the target of a slot already upgraded past it would
// leave the athlete buying nothing for the rest of the match.
unsafe fn retier_unstarted(
    ctx: usize,
    athlete: usize,
    from: u64,
    form: impl Fn(&str) -> Option<String>,
) {
    let (Some(ptr), Some(len)) = (
        safe_read_u64(athlete + O_ATHLETE_BUILD_PTR),
        safe_read_u64(athlete + O_ATHLETE_BUILD_LEN),
    ) else {
        return;
    };
    let (ptr, len) = (ptr as usize, len as usize);
    if ptr < 0x10000 || len > BUY_MEMO_BUILD_MAX || !readable(ptr, len * 8) {
        return;
    }
    for si in (from as usize)..len {
        let index = rd_u64(ptr + si * 8);
        let Some(wanted) = catalog_name_at(ctx, index).and_then(|key| form(&key)) else {
            continue;
        };
        let Some(other) = scan_idx_cached(ctx, wanted.as_bytes()) else {
            continue;
        };
        if other != index && writable(ptr + si * 8, 8) {
            wr_u64(ptr + si * 8, other);
        }
    }
}

// Item Rules, No Radiants: holds the slot in hand (`owned - 1`) at its
// legendary too, unless the item in hand is already the radiant or unreadable.
// A component or the legendary itself in hand is finished by the lower target.
unsafe fn cap_slot_in_hand(ctx: usize, athlete: usize, owned: u64) {
    let Some(slot) = owned.checked_sub(1) else {
        return;
    };
    let (Some(ptr), Some(len), Some(items), Some(items_len)) = (
        safe_read_u64(athlete + O_ATHLETE_BUILD_PTR),
        safe_read_u64(athlete + O_ATHLETE_BUILD_LEN),
        safe_read_u64(athlete + O_ATHLETE_ITEMS_PTR),
        safe_read_u64(athlete + O_ATHLETE_ITEMS_LEN),
    ) else {
        return;
    };
    if (ptr as usize) < 0x10000 || slot >= len || len as usize > BUY_MEMO_BUILD_MAX {
        return;
    }
    // After the bounds: `slot` is under the build's length here.
    let at = ptr as usize + slot as usize * 8;
    if !readable(at, 8) {
        return;
    }
    let index = rd_u64(at);
    let Some(target) = catalog_name_at(ctx, index) else {
        return;
    };
    let Some(legendary) = crate::item_tiers::legendary_of(&target) else {
        return;
    };
    match catalog_name_in(items as usize, items_len, slot) {
        Some(in_hand) if in_hand != target => {}
        _ => return,
    }
    let Some(other) = scan_idx_cached(ctx, legendary.as_bytes()) else {
        return;
    };
    if other != index && writable(at, 8) {
        wr_u64(at, other);
    }
}

// The catalog index of the radiant over the item at `index`, where that
// item is a legendary with one; `index` itself otherwise. For reading a
// slot Item Rules holds at its legendary the way the build was decided.
unsafe fn radiant_index(ctx: usize, index: u64) -> u64 {
    catalog_name_at(ctx, index)
        .and_then(|key| crate::item_tiers::radiant_of(&key))
        .and_then(|radiant| scan_idx_cached(ctx, radiant.as_bytes()))
        .unwrap_or(index)
}

// Smart Builds rules 6 and 9 over a build just grown to six, which the stable
// hook's pass never saw (`smart_builds::sort_by_timing`). Slots up to the one
// being built (`owned`), the player's pins and the boots stay put.
unsafe fn reorder_unbought(
    ctx: usize,
    champ: &str,
    slots: &mut [u64],
    owned: u64,
    designate: bool,
) {
    let movable: Vec<usize> = (owned as usize + 1..slots.len())
        .filter(|&j| !(designate && crate::build_config::pinned_key_raw(champ, j).is_some()))
        .filter(|&j| !buy_is_boots(ctx, slots[j]))
        .collect();
    let mut picks: Vec<u64> = movable.iter().map(|&j| slots[j]).collect();
    // The fit too, so a jungler's role item (rule 9) stays ahead of the early
    // items this sort pulls forward, and a support's World Atlas item ahead of
    // everything (`smart_builds::buy_order`).
    let fit = crate::smart_builds::fit(champ, crate::build_config::role_for_champion(champ));
    crate::smart_builds::sort_by_timing(&mut picks, fit, |&index| catalog_name_at(ctx, index));
    for (&j, index) in movable.iter().zip(picks) {
        slots[j] = index;
    }
}

// Grows the athlete's build `Vec` to `slots.len()`, writing every slot from
// `old_len` on, and returns its new length (`old_len` if anything declined).
// Reallocates only when `cap` is short, and writes ptr/cap at once, since a
// successful realloc may have freed the old block.
unsafe fn grow_build(athlete: usize, ptr: usize, cap: u64, old_len: u64, slots: &[u64]) -> u64 {
    let new_len = slots.len();
    let old = old_len as usize;
    if new_len <= old {
        return old_len;
    }
    let np = if cap as usize >= new_len {
        ptr
    } else {
        let realloc: ReallocFn = core::mem::transmute(exe_base_addr() + RVA_REALLOC);
        let np = realloc(ptr, cap as usize * 8, 8, new_len * 8);
        if np < 0x10000 {
            return old_len;
        }
        wr_u64(athlete + O_ATHLETE_BUILD_PTR, np as u64);
        wr_u64(athlete + O_ATHLETE_BUILD_CAP, new_len as u64);
        np
    };
    if !writable(np + old * 8, (new_len - old) * 8) {
        return old_len;
    }
    for (i, &value) in slots.iter().enumerate().skip(old) {
        wr_u64(np + i * 8, value);
    }
    wr_u64(athlete + O_ATHLETE_BUILD_LEN, new_len as u64);
    new_len as u64
}

// Whether the buy detour grows builds past the game's four slots, so a 5th and
// 6th exist for Smart Builds' boots to wait for
// (`build_config::later_slot_open`). `grow`'s conditions in `buy_replace_ctx`,
// less the per-athlete ones, plus the detour being in.
pub(crate) fn builds_grow_past_four() -> bool {
    BUILD_EXTEND_ENABLED
        && BUY_PROBE_INSTALLED.load(Ordering::Relaxed) == 1
        && crate::build_config::picker_slots() > crate::build_config::game_slots()
        && realloc_ok()
}

// Whether this athlete's build `Vec` still has to grow to
// `build_config::picker_slots()`. On the buy hot path ahead of the background
// early exit, so it reads VEH-guarded; it mirrors `grow` in `buy_replace_ctx`,
// and the two must change together.
unsafe fn needs_build_extension(athlete: usize) -> bool {
    // `realloc_ok` too: an athlete whose Vec can never grow must not keep
    // taking the slow path on every buy. It is one cached atomic load.
    if !BUILD_EXTEND_ENABLED || !realloc_ok() {
        return false;
    }
    let target = crate::build_config::picker_slots() as u64;
    match (
        safe_read_u64(athlete + O_ATHLETE_BUILD_CAP),
        safe_read_u64(athlete + O_ATHLETE_BUILD_LEN),
    ) {
        (Some(cap), Some(len)) => len >= 3 && len < target && cap >= len,
        _ => false,
    }
}

// Fixed words at the front of [`BuyInputs`]; the build targets follow.
const BUY_INPUT_FIXED: usize = 13;
// Longest build [`BuyInputs`] holds. The detour grows builds to
// `build_config::picker_slots()` (6); a longer one is never memoized.
const BUY_MEMO_BUILD_MAX: usize = 8;

// Everything `buy_replace_ctx` decides an athlete's build from, read without
// a syscall. Two calls with equal inputs make the same decision.
#[derive(Clone, Copy, PartialEq, Eq)]
struct BuyInputs([u64; BUY_INPUT_FIXED + BUY_MEMO_BUILD_MAX]);

// Reads [`BuyInputs`] for `athlete`; `None` when any of it is unreadable or the
// build is too long, and the call takes the full path. Covers everything the
// full path decides from (not gold), the network's pick through its inputs.
unsafe fn buy_inputs(
    athlete: usize,
    rsp_entry: usize,
    seed: u64,
    is_live: bool,
) -> Option<BuyInputs> {
    let read = |offset: usize| safe_read_u64(athlete + offset);
    let ptr = read(O_ATHLETE_BUILD_PTR)?;
    let len = read(O_ATHLETE_BUILD_LEN)?;
    if len as usize > BUY_MEMO_BUILD_MAX {
        return None;
    }
    let mine = match is_my_athlete(athlete) {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    };
    let flags = is_live as u64
        | (COMPTEST_MATCH.load(Ordering::Relaxed) as u64) << 1
        | (crate::build_config::is_test_match(seed) as u64) << 2
        | mine << 3
        | (crate::build_config::own_team_only_enabled() as u64) << 5
        | (crate::build_config::smart_builds_enabled() as u64) << 6
        | (crate::item_tiers::rule().code() as u64) << 7;
    let fixed: [u64; BUY_INPUT_FIXED] = [
        seed,
        read(O_ATHLETE_ID)?,
        read(O_ATHLETE_CHAMP_PTR)?,
        read(O_ATHLETE_CHAMP_LEN)?,
        read(O_ATHLETE_TEAM)?,
        read(O_ATHLETE_POS)? & 0xffff_ffff,
        read(O_ATHLETE_ITEMS_LEN)?, // owned
        read(O_ATHLETE_BUILD_CAP)?, // build cap
        ptr,
        len,
        safe_read_u64(rsp_entry + 0x30)?, // ctx: the catalog every pick resolves in
        flags,
        crate::build_config::pins_generation(),
    ];
    let mut words = [0u64; BUY_INPUT_FIXED + BUY_MEMO_BUILD_MAX];
    words[..BUY_INPUT_FIXED].copy_from_slice(&fixed);
    for i in 0..len as usize {
        words[BUY_INPUT_FIXED + i] = safe_read_u64(ptr as usize + i * 8)?;
    }
    Some(BuyInputs(words))
}

// Athletes remembered per thread: a worker steps several fixtures in turn, and
// a miss only costs the full path.
const BUY_MEMO_SLOTS: usize = 64;

// Which athlete a [`BuyInputs`] belongs to: match seed, athlete id and build
// `Vec` address. Not the athlete pointer, which the game reuses for every
// athlete of a team.
fn buy_memo_key(inputs: &BuyInputs) -> [u64; 3] {
    [inputs.0[0], inputs.0[1], inputs.0[8]]
}

// Per athlete, the inputs of its last buy decision that changed nothing.
struct BuyMemo {
    entries: [([u64; 3], BuyInputs); BUY_MEMO_SLOTS],
    next: usize,
}

thread_local! {
    // `const` and no `Drop`: no lazy init or destructor registration on the
    // detour's threads, as with `SEH_T`.
    static BUY_MEMO: core::cell::RefCell<BuyMemo> = const {
        core::cell::RefCell::new(BuyMemo {
            entries: [([0; 3], BuyInputs([0; BUY_INPUT_FIXED + BUY_MEMO_BUILD_MAX])); BUY_MEMO_SLOTS],
            next: 0,
        })
    };
}

// Whether this athlete's last decision on this thread changed nothing and was
// made from exactly `inputs` -- so this one would change nothing either.
fn buy_memo_hit(inputs: &BuyInputs) -> bool {
    let key = buy_memo_key(inputs);
    BUY_MEMO
        .try_with(|memo| {
            memo.try_borrow().is_ok_and(|memo| {
                memo.entries
                    .iter()
                    .any(|(known_key, known)| *known_key == key && known == inputs)
            })
        })
        .unwrap_or(false)
}

// Remembers that a decision from `inputs` changed nothing for `athlete`,
// replacing whatever this thread held for it.
fn buy_memo_store(inputs: BuyInputs) {
    crate::perf::count(crate::perf::Section::BuyMemoStored);
    let key = buy_memo_key(&inputs);
    let _ = BUY_MEMO.try_with(|memo| {
        let Ok(mut memo) = memo.try_borrow_mut() else {
            return;
        };
        let known = memo
            .entries
            .iter()
            .position(|(known_key, _)| *known_key == key);
        let slot = known.unwrap_or(memo.next);
        if known.is_none() {
            memo.next = (slot + 1) % BUY_MEMO_SLOTS;
        }
        memo.entries[slot] = (key, inputs);
    });
}

// -- Control's shop picks -----------------------------------------------
//
// Control (`tft2_pov`) lets the player take over a champion with a shop that
// writes into the athlete's build `Vec`, and Control's pick beats a pin (user,
// 2026-10-13). A pick is a slot holding something other than what this detour
// left at its last full pass, remembered per sim copy under the buy memo's key;
// none of this runs unless Control is enabled.
const CONTROL_MOD_ID: &str = "tft2_pov";

// Whether Control is enabled this session, read once like the game's own list;
// unreadable means no.
fn control_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.parent()
                    .map(|game| game.join(crate::item_stats::toolbox_tab::MODS_JSON))
            })
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|document| {
                crate::item_stats::toolbox_tab::listed_enabled(&document, CONTROL_MOD_ID)
            })
            .unwrap_or(false)
    })
}

// One athlete's build as the last full pass left it, and the slots picked
// through Control since, a bit a slot.
struct LeftBuild {
    slots: Vec<u64>,
    picked: u64,
}

// Builds remembered at most, oldest dropped first. Under `own_team_only` only
// the player's athletes are tracked; with it off, every athlete a pin can
// reach is, a league day's worth of them.
const LEFT_BUILDS_KEPT: usize = 512;

struct LeftBuilds {
    by_key: HashMap<[u64; 3], LeftBuild>,
    order: std::collections::VecDeque<[u64; 3]>,
}

static LEFT_BUILDS: Mutex<Option<LeftBuilds>> = Mutex::new(None);

// An athlete's build `Vec` and its targets, read without a syscall.
unsafe fn read_build(athlete: usize) -> Option<(u64, Vec<u64>)> {
    let ptr = safe_read_u64(athlete + O_ATHLETE_BUILD_PTR)?;
    let len = safe_read_u64(athlete + O_ATHLETE_BUILD_LEN)?;
    if ptr < 0x10000 || len > 16 {
        return None;
    }
    let slots = (0..len as usize)
        .map(|j| safe_read_u64(ptr as usize + j * 8))
        .collect::<Option<Vec<u64>>>()?;
    Some((ptr, slots))
}

// The slots of `build` picked through Control: those found before, and any
// holding something else than the last full pass left there. An athlete
// that owns nothing yet is at the start of a sim copy, with nothing picked.
fn control_picks(key: [u64; 3], owned: u64, build: &[u64]) -> u64 {
    if owned == 0 {
        return 0;
    }
    let Ok(guard) = LEFT_BUILDS.lock() else {
        return 0;
    };
    let Some(left) = guard.as_ref().and_then(|builds| builds.by_key.get(&key)) else {
        return 0;
    };
    build
        .iter()
        .zip(&left.slots)
        .enumerate()
        .filter(|(_, (now, was))| now != was)
        .fold(left.picked, |picked, (slot, _)| picked | (1 << slot))
}

// Remembers what a full pass left in a build, with the picks found in it.
fn note_left_build(key: [u64; 3], slots: Vec<u64>, picked: u64) {
    let Ok(mut guard) = LEFT_BUILDS.lock() else {
        return;
    };
    let builds = guard.get_or_insert_with(|| LeftBuilds {
        by_key: HashMap::new(),
        order: std::collections::VecDeque::new(),
    });
    if builds
        .by_key
        .insert(key, LeftBuild { slots, picked })
        .is_none()
    {
        builds.order.push_back(key);
        while builds.order.len() > LEFT_BUILDS_KEPT {
            if let Some(old) = builds.order.pop_front() {
                builds.by_key.remove(&old);
            }
        }
    }
}

unsafe extern "C" fn buy_replace_ctx(saved: *mut u64, rsp_entry: usize) -> u64 {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> u64 {
        // Relabelled as the call gets further, so each exit is timed as the
        // kind of call it was: an early exit, a memo hit, or a full pass.
        let mut probe = crate::perf::Probe::sim(crate::perf::Section::BuyEarlyExit);
        if saved.is_null() {
            return 0;
        }
        let athlete = *saved.add(2) as usize; // r8
        if athlete < 0x10000 {
            return 0;
        }
        // Spectate check first, from two memory reads: r9 is the provider, and
        // its seed matching `LIVE_SEED` (or the provider being
        // `RENDER_PROVIDER`) marks the on-screen match. Most buy calls are
        // background sims and must exit before any syscall.
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
        // Background buys exit here unless the athlete is mine, a test's, or
        // still needs its build grown (`needs_build_extension`: two VEH reads,
        // true at most once per athlete). The same decision runs in background
        // and on-screen copies of a match, so they converge.
        if !is_live
            && !crate::build_config::is_test_match(seed_r9)
            && !matches!(is_my_athlete(athlete), Some(true))
            && !needs_build_extension(athlete)
        {
            return 0;
        }
        // Per-athlete memo: `run_tick` calls this every tick for every dead or
        // in-fountain athlete, and everything below is a function of
        // [`BuyInputs`]. A call whose inputs match the last call that changed
        // nothing passes through.
        probe.set(crate::perf::Section::BuyMemoHit);
        let memo_inputs = buy_inputs(athlete, rsp_entry, seed_r9, is_live);
        if memo_inputs.is_none() {
            crate::perf::count(crate::perf::Section::BuyUnmemoizable);
        }
        if memo_inputs.is_some_and(|inputs| buy_memo_hit(&inputs)) {
            return 0;
        }
        probe.set(crate::perf::Section::BuyFullPass);
        // Only on-screen buys and background buys by my athletes get here. The
        // build len is read only as proof that the raw reads below are in
        // bounds.
        let (Some(owned), Some(cptr), Some(clen), Some(_)) = (
            safe_read_u64(athlete + O_ATHLETE_ITEMS_LEN), // owned
            safe_read_u64(athlete + O_ATHLETE_CHAMP_PTR),
            safe_read_u64(athlete + O_ATHLETE_CHAMP_LEN),
            safe_read_u64(athlete + O_ATHLETE_BUILD_LEN),
        ) else {
            return 0;
        };
        let (cptr, clen) = (cptr as usize, clen as usize);
        let mut champ_bytes = Vec::new();
        if cptr < 0x10000
            || clen == 0
            || clen > 48
            || !safe_read_bytes(cptr, clen, &mut champ_bytes)
        {
            return 0;
        }
        let champ_cow = String::from_utf8_lossy(&champ_bytes);
        let champ: &str = champ_cow.as_ref();
        // Before any pin is looked up: the lane every lookup below resolves in.
        crate::build_config::set_athlete_lane(athlete_lane(athlete));
        // Both sides of a comp test are the player's, so it bypasses the team
        // gate, but only on screen: `COMPTEST_MATCH` is sticky, and background
        // sims after a comp test would otherwise give every athlete the
        // player's pins.
        let is_comptest_live = COMPTEST_MATCH.load(Ordering::Relaxed) && is_live;
        // Lane and 5v5 tests, by the route call's `mode` (the comp-test
        // retaddrs date from 0.5.3), and only the test's own match, since
        // league fixtures simulate alongside it
        // (`build_config::note_test_spawn`).
        let is_training = crate::build_config::is_test_match(seed_r9);
        let is_player = if is_comptest_live || is_training {
            true // a test: both sides are the player's
        } else {
            matches!(is_my_athlete(athlete), Some(true))
        };
        if crate::own_team_log::ENABLED && crate::build_config::has_pins(champ) {
            let ctx = rd_u64(rsp_entry + 0x30) as usize;
            let bptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
            let blen = rd_u64(athlete + O_ATHLETE_BUILD_LEN);
            let build: Vec<Option<String>> = if ctx >= 0x10000
                && bptr >= 0x10000
                && blen <= 8
                && readable(bptr, blen as usize * 8)
            {
                (0..blen as usize)
                    .map(|j| catalog_name_at(ctx, rd_u64(bptr + j * 8)))
                    .collect()
            } else {
                Vec::new()
            };
            let aid = safe_read_u64(athlete + O_ATHLETE_ID);
            crate::own_team_log::on_change(
                &format!("buy {champ} aid={aid:?} provider=0x{provider_now:x}"),
                format!(
                    "owned={} is_player={} mine={:?} training={} live={} provider=0x{:x} seed=0x{:x} build={:?}",
                    owned,
                    is_player,
                    is_my_athlete(athlete),
                    is_training,
                    is_live,
                    provider_now,
                    seed_r9,
                    build
                ),
            );
        }
        // Item Rules (`crate::item_tiers`): everything below decides on radiant
        // targets, so slots an earlier pass held at legendary are put back
        // first and capped again after the growth. Only unstarted slots
        // ([`retier_unstarted`]); a started one is read as its radiant where
        // the build is copied out.
        let tiers_rule = crate::item_tiers::rule();
        let tiers_ctx = rd_u64(rsp_entry + 0x30) as usize;
        let tiers_put_back = tiers_rule != crate::item_tiers::Rule::AllRadiants
            || TIERS_RULED.load(Ordering::Relaxed);
        // The editor's scope toggle, read once for the four slots below and the
        // 5th and 6th.
        let own_team_only = crate::build_config::own_team_only_enabled();
        // The slots picked through Control's shop, which no pin takes back
        // ([`control_picks`]); tracked for the athletes the pins reach.
        let aid = safe_read_u64(athlete + O_ATHLETE_ID).unwrap_or(0);
        let track = (is_player || !own_team_only) && control_enabled();
        let picked = if track {
            read_build(athlete).map_or(0, |(ptr, build)| {
                control_picks([seed_r9, aid, ptr], owned, &build)
            })
        } else {
            0
        };
        // Item Rules' put-back, after Control's check: that compares the build
        // as this pass found it with what the last one left, capped slots and
        // all, and this is the pass's first write.
        if tiers_put_back {
            retier_unstarted(tiers_ctx, athlete, owned, crate::item_tiers::radiant_of);
        }
        // The game's four slots under `own_team_only`: the player's pins
        // written straight into the build `Vec`, gated on `is_player`. The
        // stable hook (`item_build_hook::decide_build`) sets these slots
        // otherwise but cannot tell sides apart, so exactly one of the two
        // runs.
        //
        // Slot 0 is already bought by the first buy decision, which is why
        // `cap_spawn` (`SPAWN_INJECT_ENABLED`) plants it at spawn.
        if own_team_only && is_player {
            let ctx012 = rd_u64(rsp_entry + 0x30) as usize;
            let bptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
            let blen = rd_u64(athlete + O_ATHLETE_BUILD_LEN);
            if ctx012 >= 0x10000
                && bptr >= 0x10000
                && blen >= 1
                && blen <= 8
                && readable(bptr, (blen as usize) * 8)
            {
                for si in 0..crate::build_config::game_slots() as u8 {
                    if (si as u64) >= blen {
                        break;
                    } // build has no such slot
                    if owned > si as u64 {
                        continue;
                    } // slot already purchased -> too late
                    if picked & (1 << si) != 0 {
                        continue;
                    } // picked through Control -> the player's choice stands
                    let idx = slot_n_catalog_index(champ, si, |key| scan_idx_cached(ctx012, key));
                    if let Some(t) = idx {
                        // Skip a write of the value already there, which most
                        // writes were.
                        if rd_u64(bptr + (si as usize) * 8) == t {
                            continue;
                        }
                        // Never plant a second copy of an item the build
                        // already targets (the engine chose unpinned slots
                        // blind to the pins). An earlier copy is bought or
                        // being built, so this slot keeps the engine's item; a
                        // later one swaps. Boots clash with any pair, and a
                        // slot pinned to what it holds is a deliberate
                        // duplicate.
                        let pin_at = |j: usize| {
                            slot_n_catalog_index(champ, j as u8, |key| scan_idx_cached(ctx012, key))
                        };
                        let pin_boots = buy_is_boots(ctx012, t);
                        let elsewhere =
                            (0..blen as usize).filter(|&j| j != si as usize).find(|&j| {
                                let there = rd_u64(bptr + j * 8);
                                (there == t || (pin_boots && buy_is_boots(ctx012, there)))
                                    && pin_at(j) != Some(there)
                            });
                        if let Some(j) = elsewhere {
                            // A later slot picked through Control holding the
                            // pinned item honours the pin too, by the player's
                            // hand, and is not moved.
                            if j < si as usize || picked & (1 << j) != 0 {
                                continue;
                            }
                            if writable(bptr, (blen as usize) * 8) {
                                wr_u64(bptr + j * 8, rd_u64(bptr + (si as usize) * 8));
                                wr_u64(bptr + (si as usize) * 8, t);
                            }
                            continue;
                        }
                        // The pin covers Smart Builds' boots (rule 7): move
                        // them as `cap_spawn` does (`displaced_boots_slot`)
                        // unless the player pinned a pair. Later slots are
                        // unbought; the first only while nothing is.
                        let here = rd_u64(bptr + (si as usize) * 8);
                        if !pin_boots
                            && buy_is_boots(ctx012, here)
                            && !(0..crate::build_config::picker_slots())
                                .any(|j| pin_at(j).is_some_and(|pin| buy_is_boots(ctx012, pin)))
                        {
                            let free =
                                displaced_boots_slot(champ, si as usize, blen as usize, owned == 0)
                                    .filter(|&j| picked & (1 << j) == 0);
                            if let Some(j) = free {
                                if writable(bptr + j * 8, 8) {
                                    wr_u64(bptr + j * 8, here);
                                }
                            }
                        }
                        if writable(bptr + (si as usize) * 8, 8) {
                            wr_u64(bptr + (si as usize) * 8, t);
                        }
                    }
                }
            }
        }
        // How long this athlete's build should end up: the game's four, and
        // the 5th and 6th this half adds.
        let target = crate::build_config::picker_slots() as u64;
        if target < 4 {
            return 0;
        }
        if !SHADOW_CALL_NAMES {
            return 0;
        }
        // The slots the engine never plans: `in_place` rewrites a changed pin
        // where it stands, and `grow` extends a short `Vec` (to the mod's six,
        // or a 3-long build's 4th) and fills every new slot before `len` moves.
        // `grow` is the only path to `RVA_REALLOC`, behind `realloc_ok`; keep
        // it in step with `needs_build_extension`.
        let mut build_len = rd_u64(athlete + O_ATHLETE_BUILD_LEN);
        let cap_now = rd_u64(athlete + O_ATHLETE_BUILD_CAP);
        let in_place = build_len >= 4 && cap_now >= 4;
        let grow = BUILD_EXTEND_ENABLED
            && build_len >= 3
            && build_len < target
            && cap_now >= build_len
            && realloc_ok();
        if in_place || grow {
            let ptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
            // Every live slot is read below, and in place writes build[4..].
            let live = build_len.min(16) as usize;
            let ok = ptr >= 0x10000
                && readable(ptr, live * 8)
                && writable(athlete + O_ATHLETE_BUILD_CAP, 0x18)
                && (!in_place || writable(ptr, live * 8));
            if ok {
                let ctx = rd_u64(rsp_entry + 0x30) as usize;
                // The pins' team gate: with `own_team_only` off a pin applies
                // to whoever plays the champion, on both teams; with it on,
                // only my athletes get theirs.
                let designate = !own_team_only || is_player;
                let smart = crate::build_config::smart_builds_enabled();
                // The whole build as it will stand. A started slot Item Rules
                // holds at legendary is read as its radiant, so later pins and
                // picks still see it.
                let mut slots: Vec<u64> = (0..live)
                    .map(|i| {
                        let index = rd_u64(ptr + i * 8);
                        if tiers_put_back && (i as u64) < owned {
                            radiant_index(ctx, index)
                        } else {
                            index
                        }
                    })
                    .collect();
                // Grown slots 5 and 6 take only a changed pin, while unbought;
                // their automatic picks stand.
                if designate {
                    for si in 4..slots.len().min(target as usize) {
                        if owned > si as u64 || picked & (1 << si) != 0 {
                            continue;
                        }
                        let Some(t) = pinned_extra_slot(ctx, champ, si, &slots[..si]) else {
                            continue;
                        };
                        if slots[si] != t && writable(ptr + si * 8, 8) {
                            wr_u64(ptr + si * 8, t);
                            slots[si] = t;
                        }
                    }
                }
                if grow {
                    // Each new slot's item and how it was come by, for the
                    // Check Tactics panel's test log.
                    let mut picks: Vec<(u64, &'static str)> = Vec::new();
                    while (slots.len() as u64) < target {
                        let si = slots.len();
                        let buyer = Buyer {
                            athlete,
                            seed: seed_r9,
                        };
                        match extra_slot_pick(ctx, buyer, champ, si, &slots, designate) {
                            Some(t) => {
                                picks.push((t, PICKED_BY.with(|picked| picked.get())));
                                slots.push(t);
                            }
                            None => break,
                        }
                    }
                    if slots.len() as u64 > build_len {
                        let old_len = build_len as usize;
                        // Not over a Control pick, which stays where it was put.
                        if smart && picked == 0 {
                            reorder_unbought(ctx, champ, &mut slots, owned, designate);
                        }
                        build_len = grow_build(athlete, ptr, cap_now, build_len, &slots);
                        if crate::own_team_log::ENABLED && crate::build_config::has_pins(champ) {
                            crate::own_team_log::line(|| {
                                format!(
                                    "grown: {} aid={:?} owned={} designate={} build={:?}",
                                    champ,
                                    safe_read_u64(athlete + O_ATHLETE_ID),
                                    owned,
                                    designate,
                                    slots
                                        .iter()
                                        .take(build_len as usize)
                                        .map(|&index| catalog_name_at(ctx, index))
                                        .collect::<Vec<_>>()
                                )
                            });
                        }
                        // `grow_build` writes only the new slots, and rule 6
                        // may have moved old ones: write those back once the
                        // growth held (the `Vec` may have moved).
                        let new_ptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
                        if build_len as usize == slots.len()
                            && new_ptr >= 0x10000
                            && writable(new_ptr, old_len * 8)
                        {
                            for (j, &index) in slots.iter().enumerate().take(old_len) {
                                // Rule 6 moves nothing at or before the slot
                                // in hand, and a started slot's target is
                                // not this copy's to restore (Item Rules).
                                if (j as u64) < owned {
                                    continue;
                                }
                                if rd_u64(new_ptr + j * 8) != index {
                                    wr_u64(new_ptr + j * 8, index);
                                }
                            }
                        }
                        // For the Check Tactics panel: the whole grown build,
                        // once per athlete, in the order rule 6 left it.
                        if build_len as usize == slots.len() {
                            let keys = slots
                                .iter()
                                .map(|&index| catalog_name_at(ctx, index))
                                .collect::<Option<Vec<String>>>();
                            let lane = safe_read_u64(athlete + O_ATHLETE_POS)
                                .map(|pos| (pos & 0xffff_ffff) as usize);
                            let side = safe_read_u64(athlete + O_ATHLETE_TEAM);
                            let picks = picks
                                .iter()
                                .filter_map(|&(index, how)| {
                                    Some((catalog_name_at(ctx, index)?, how))
                                })
                                .collect();
                            if let (Some(keys), Some(lane), Some(side)) = (keys, lane, side) {
                                crate::match_builds::note_grown(
                                    seed_r9,
                                    side,
                                    lane,
                                    safe_read_u64(athlete + O_ATHLETE_ID),
                                    champ,
                                    keys,
                                    picks,
                                    designate,
                                );
                            }
                        }
                    }
                }
            }
        }
        // Item Rules, the other half: First Item Radiant caps every unstarted
        // slot after the first, No Radiants every unstarted slot plus the one
        // in hand while safe (`cap_slot_in_hand`). A pass that only put back
        // and capped again is remembered like one that wrote nothing.
        match tiers_rule {
            crate::item_tiers::Rule::AllRadiants => {}
            crate::item_tiers::Rule::FirstItemRadiant => {
                TIERS_RULED.store(true, Ordering::Relaxed);
                retier_unstarted(
                    tiers_ctx,
                    athlete,
                    owned.max(1),
                    crate::item_tiers::legendary_of,
                );
            }
            crate::item_tiers::Rule::NoRadiants => {
                TIERS_RULED.store(true, Ordering::Relaxed);
                retier_unstarted(tiers_ctx, athlete, owned, crate::item_tiers::legendary_of);
                cap_slot_in_hand(tiers_ctx, athlete, owned);
            }
        }

        // What this pass left, for the next one to tell Control's picks by.
        // After growth, under the `Vec` it grew into.
        if track {
            if let Some((ptr, build)) = read_build(athlete) {
                note_left_build([seed_r9, aid, ptr], build, picked);
            }
        }
        // The memo's other half: inputs unchanged by the whole pass mean it
        // wrote nothing, so they are remembered; a pass that wrote runs in full
        // again next call.
        if let Some(inputs) = memo_inputs {
            if buy_inputs(athlete, rsp_entry, seed_r9, is_live) == Some(inputs) {
                buy_memo_store(inputs);
            }
        }
        // Only build targets are planted here; nothing is bought on the
        // engine's behalf, so the original always runs.
        0
    }));
    r.unwrap_or(0)
}

// Installs the buy_item detour. The stub does `mov r10,rsp; push rax r11 r10 r9
// r8 rdx rcx` and calls `cap_fn(rcx = saved, rdx = rsp_entry)`.
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
// Whether a non-zero team id has been seen; after that, 0 reports are ignored
// (one save alternated between 105 and 0).
static PID_NONZERO_SEEN: AtomicU64 = AtomicU64::new(0);
static PID_ZERO_CLEAN: AtomicU64 = AtomicU64::new(0); // 0 reports seen outside tests; 600 trusts a real team 0
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
        BUY_PROBE_INSTALLED.store(2, Ordering::Relaxed);
        return;
    }
    // 12 bytes since 0.6.3: the 8 pushes, exactly the jmp patch (through 0.6.2
    // it was 19).
    match unsafe { install_replace_buy(RVA_BUY_ITEM, 12, buy_replace_ctx as *const () as usize) } {
        Ok(_) => BUY_PROBE_INSTALLED.store(1, Ordering::Relaxed),
        Err(_) => BUY_PROBE_INSTALLED.store(3, Ordering::Relaxed),
    }
}

// ===========================================================================
//  5th and 6th item slots
// ===========================================================================
//
// The game stops at four in four places, each byte-patched: the resolver's
// final gate (`patch_final_gate`), the tick's finals cap
// (`patch_tick_finals_cap`), and the in-match and result item rows
// (`patch_row_floor`, `patch_result_row_floor`). The build `Vec` is grown by
// `buy_replace_ctx` instead (`grow_build`).
//
// Every write address is derived from a signature unique in .text, never
// pinned separately: a mismatch skips the patch (in `4items_patches.txt`
// with `TRACE_FILES` on) and the engine stops at four as before.
const EXTRA_SLOT_PATCHES: bool = true;

// Checks `expect` at `sig`, then writes `writes`, both as (offset, byte).
// Already-patched bytes count as a match, so a second init is a no-op.
unsafe fn patch_bytes(
    name: &str,
    sig: usize,
    expect: &[(usize, u8)],
    writes: &[(usize, u8)],
) -> String {
    let span = expect
        .iter()
        .chain(writes)
        .map(|&(off, _)| off + 1)
        .max()
        .unwrap_or(0);
    if !readable(sig, span) {
        return format!("{name}: unreadable");
    }
    for &(off, want) in expect {
        let got = *((sig + off) as *const u8);
        let already = writes.iter().any(|&(w, value)| w == off && value == got);
        if got != want && !already {
            return format!("{name}: sig mismatch @+{off} = {got:#04x} (want {want:#04x})");
        }
    }
    const RWX: u32 = 0x40;
    for &(off, value) in writes {
        let at = sig + off;
        let mut old = 0u32;
        if VirtualProtect(at, 1, RWX, &mut old) == 0 {
            return format!("{name}: VirtualProtect fail @+{off}");
        }
        *(at as *mut u8) = value;
        VirtualProtect(at, 1, old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), at, 1);
    }
    format!("{name}: patched")
}

// The resolver's "four finals complete" gate (buy_item's second callee): `cmp
// rax,3 / jbe <price check>`, so a 5th final is never started, and `jbe` ->
// `jmp` treats four and up like three. Since 0.6.4 the count is a call; re-find
// it by `cmp rax,3 / jbe / cmp r14,rsi / jae` ahead of the `[vt+0x70]` test.
unsafe fn patch_final_gate() -> String {
    let sig = exe_base_addr() + 0xe9385e; // 0.6.4: resolver 0xe933d0 +0x48e (0.6.3: 0xea216a)
    const EXPECT: [(usize, u8); 11] = [
        (0, 0x48),
        (1, 0x83),
        (2, 0xf8),
        (3, 0x03), //  cmp rax, 3
        (4, 0x76),
        (5, 0x21), //  jbe +0x21   <- opcode at +4
        (6, 0x49),
        (7, 0x39),
        (8, 0xf6), //  cmp r14, rsi
        (9, 0x0f),
        (10, 0x83), // jae rel32
    ];
    patch_bytes("final_gate", sig, &EXPECT, &[(4, 0xEB)])
}

// The tick's own finals cap: `run_tick` recounts owned finals after an approved
// purchase and skips it at `cmp rax,3 / ja`. Raising the imm to `slots - 1`
// moves the cap rather than removing it; re-find it by the 13-byte form with
// the rel32 masked, and re-read the trailing frame slot, which moves every
// update.
unsafe fn patch_tick_finals_cap(slots: u8) -> String {
    let sig = exe_base_addr() + 0x1746803; // 0.6.4: run_tick 0x1740e00 +0x5a03 (0.6.3: 0x146baf3)
    const EXPECT: [(usize, u8); 17] = [
        (0, 0x48),
        (1, 0x83),
        (2, 0xf8),
        (3, 0x03), //                cmp rax, 3   <- imm at +3
        (4, 0x0f),
        (5, 0x87),
        (6, 0x4a),
        (7, 0x02),
        (8, 0x00),
        (9, 0x00), // ja rel32
        (10, 0x4c),
        (11, 0x8b),
        (12, 0x8d),
        (13, 0xa8),
        (14, 0x42),
        (15, 0x00),
        (16, 0x00), // mov r9, [rbp+0x42a8] (0.6.3: 0x42f0)
    ];
    patch_bytes("tick_finals_cap", sig, &EXPECT, &[(3, slots - 1)])
}

// In-match row floor: `cmp rax,5 / mov ecx,4 / cmovae rcx,rax` is `max(4, most
// items owned)`, and both immediates move so it stays `max(rax, slots)`.
// Nothing else reads the count, and the fill loop already draws slots past a
// player's items empty.
//
// Pinned by a masked form (the two `[rbp+disp32]` byte loads ahead of it),
// since the bare `max` idiom has dozens of hits; that is also how to re-find
// it.
unsafe fn patch_row_floor(slots: u8) -> String {
    let sig = exe_base_addr() + 0xbfb061; // 0.6.4: ingame mega-function 0xbf45d0 +0x6a91 (0.6.3: 0xba33f1)
    const EXPECT: [(usize, u8); 18] = [
        (0, 0x8a),
        (1, 0x9d), //                           mov bl, [rbp+disp32]
        (6, 0x44),
        (7, 0x8a),
        (8, 0xb5), //                mov r14b, [rbp+disp32]
        (13, 0x48),
        (14, 0x83),
        (15, 0xf8),
        (16, 0x05), // cmp rax, 5     <- imm at +16
        (17, 0xb9),
        (18, 0x04),
        (19, 0x00),
        (20, 0x00),
        (21, 0x00), // mov ecx, 4 <- imm at +18
        (22, 0x48),
        (23, 0x0f),
        (24, 0x43),
        (25, 0xc8), // cmovae rcx, rax
    ];
    patch_bytes("row_floor", sig, &EXPECT, &[(16, slots + 1), (18, slots)])
}

// Match-result row floor, the same change on that screen's own count (`cmp
// rdx,5 / mov ecx,4 / cmovb rdx,rcx / test al,1 / cmove rdx,rcx`). One count
// serves all ten rows, so surplus slots are already drawn empty; the 19-byte
// form is unique in .text.
unsafe fn patch_result_row_floor(slots: u8) -> String {
    let sig = exe_base_addr() + 0xcf376e; // 0.6.4: match-result screen builder 0xcf12f0 +0x247e (0.6.3: 0x910f8a)
    const EXPECT: [(usize, u8); 19] = [
        (0, 0x48),
        (1, 0x83),
        (2, 0xfa),
        (3, 0x05), //      cmp rdx, 5   <- imm at +3
        (4, 0xb9),
        (5, 0x04),
        (6, 0x00),
        (7, 0x00),
        (8, 0x00), // mov ecx, 4 <- imm at +5
        (9, 0x48),
        (10, 0x0f),
        (11, 0x42),
        (12, 0xd1), //   cmovb rdx, rcx
        (13, 0xa8),
        (14, 0x01), //                          test al, 1
        (15, 0x48),
        (16, 0x0f),
        (17, 0x44),
        (18, 0xd1), //  cmove rdx, rcx
    ];
    patch_bytes(
        "result_row_floor",
        sig,
        &EXPECT,
        &[(3, slots + 1), (5, slots)],
    )
}

// ===========================================================================
//  Server item settings: the mod items, lifted out for a settings write
// ===========================================================================
//
// `setting_set_json` on the item settings rebuilds `mod_items` from JSON and
// drops the old entries, and with them each entry's mod item object
// (`O_MOD_ITEM_OBJECT`), whose absence the game reads as inactive. So the
// `Vec` is emptied for the write and its three words put back after, entries
// untouched.
//
// The constants are checked against the server's own list before anything is
// written, and used only behind the version gate
// (`driver::lift_server_mod_items`).

// Where the server call's host state keeps its `Database` (the vtable's
// `setting_set_json` loads it with `mov rcx,[rdi+0x10]`).
const O_SERVER_STATE_DATABASE: usize = 0x10;

// The Database's `mod_items` `Vec` (cap, ptr, len), found where the settings
// handler drops the old list.
const O_DATABASE_MOD_ITEMS: usize = 0x166e8;

// Size of one `ModItemEntry` (0.6.3: `lea rsi,[rcx+0x1a8]` steps that drop
// loop). Its key, a `String`, is the first field: capacity, pointer, length.
const MOD_ITEM_ENTRY_SIZE: usize = 0x1a8;

// Where an entry holds the mod's item object: data pointer, then vtable.
const O_MOD_ITEM_OBJECT: usize = 0x190;

// An empty `Vec<ModItemEntry>`: no capacity, the dangling pointer of an
// 8-aligned type, no length. What `Vec::new()` is, and what the host's own
// code leaves where it deserializes an empty list.
const EMPTY_VEC: [u64; 3] = [0, 8, 0];

// The server's mod items, out of its item settings until this is dropped.
pub(crate) struct ModItemsLift {
    // Address of the `Vec`'s three words in the Database.
    header: usize,
    // What they held.
    taken: [u64; 3],
}

impl Drop for ModItemsLift {
    // Puts the list back whatever the write came to: accepted or refused, these
    // three words now hold an empty list that owns nothing.
    fn drop(&mut self) {
        unsafe {
            for (word, &value) in self.taken.iter().enumerate() {
                wr_u64(self.header + word * 8, value);
            }
        }
    }
}

// Empties the server's `mod_items` and returns what puts it back. Only when the
// memory there is exactly the list `keys` reports (same length, every key in
// place); `None` otherwise, and the caller must not write.
//
// # Safety
//
// Only from inside a server hook, on its thread, where the host itself replaces
// these settings.
unsafe fn lift_server_mod_items(state: usize, keys: &[String]) -> Option<ModItemsLift> {
    if keys.is_empty() {
        return None;
    }
    let database = safe_read_u64(state.checked_add(O_SERVER_STATE_DATABASE)?)? as usize;
    let header = database.checked_add(O_DATABASE_MOD_ITEMS)?;
    let capacity = safe_read_u64(header)?;
    let entries = safe_read_u64(header + 8)? as usize;
    let len = safe_read_u64(header + 16)?;
    if len != keys.len() as u64 || capacity < len {
        return None;
    }
    let mut spelled = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let entry = entries.checked_add(index.checked_mul(MOD_ITEM_ENTRY_SIZE)?)?;
        let text = safe_read_u64(entry + 8)? as usize;
        let text_len = safe_read_u64(entry + 16)? as usize;
        // The object's two words must be there to read as well, or this is
        // not an array of entries this size.
        safe_read_u64(entry + O_MOD_ITEM_OBJECT + 8)?;
        if text_len != key.len()
            || !safe_read_bytes(text, text_len, &mut spelled)
            || spelled != key.as_bytes()
        {
            return None;
        }
    }
    if !writable(header, EMPTY_VEC.len() * 8) {
        return None;
    }
    for (word, &value) in EMPTY_VEC.iter().enumerate() {
        wr_u64(header + word * 8, value);
    }
    Some(ModItemsLift {
        header,
        taken: [capacity, entries as u64, len],
    })
}

// ═══════════════════════════════════════════════════════════════════════════
//  Game version gate: on any other build every feature disables itself
// ═══════════════════════════════════════════════════════════════════════════
//
// This half depends on hardcoded RVAs, byte patches and struct offsets, so on
// a mismatch not one hook or patch is installed. Two checks: the exe's size,
// and the entry prologues of hooks no other mod shares.
const GAME_EXE_SIZE_064: u64 = 87_228_416; // 0.6.4 (0.6.3: 86_804_992)
static VERSION_MSG: Mutex<String> = Mutex::new(String::new());
// Decides whether this is the one game build this half is pinned to. Called once from init.
fn check_game_version() -> bool {
    let mut why = String::new();
    // (1) exe size
    let size_ok = match exe_path().and_then(|p| fs::metadata(p).ok()) {
        Some(m) => {
            let sz = m.len();
            if sz == GAME_EXE_SIZE_064 {
                true
            } else {
                why = format!(
                    "exe size mismatch: {}B (0.6.4 = {}B)",
                    sz, GAME_EXE_SIZE_064
                );
                false
            }
        }
        None => {
            why = "could not read the exe path or its metadata".into();
            false
        }
    };
    // (2) Entry prologues. An entry already in hook form (`movabs rax; jmp
    // rax`) passes: another mod's, or ours after a re-init.
    let proto_ok = if size_ok {
        let base = exe_base_addr();
        if base == 0 {
            why = "module base 0".into();
            false
        } else {
            // Only hooks no other mod shares: serpen may chain onto the
            // launcher first, so its entry is no evidence.
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
        "0.6.4 confirmed - active".to_string()
    } else {
        format!("version mismatch -> this half is fully disabled ({})", why)
    };
    ok
}

// Whether the tactics half is active; the host consults this before routing
// anything here. The gate is all that stands between a game update and patching
// the wrong code, since the host mod itself loads on any version.
fn tactics_init() -> bool {
    // The VEH first: until it is registered every guarded read fails, which
    // once left every hook silently inert.
    seh_install();

    let version_ok = check_game_version();
    crate::own_team_log::line(|| {
        format!(
            "version gate: {}",
            VERSION_MSG.lock().unwrap_or_else(|e| e.into_inner())
        )
    });
    if !version_ok {
        let msg = VERSION_MSG
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // Written once so the user can tell why this half is off, with the
        // other trace files (`TRACE_FILES`).
        if TRACE_FILES {
            if let Some(d) = mod_dir() {
                let _ = fs::create_dir_all(&d);
                let _ = fs::write(
                    d.join("version_gate.txt"),
                    format!(
                        "{}

This half of the mod (own-team-only builds and the 5th and 6th item slots) works only on the
game version it was made for. If the game has updated, please wait for a mod update. The rest
of the mod is unaffected.
",
                        msg
                    ),
                );
            }
        }
        return false;
    }
    // The patch report: every patch skips silently on a mismatch, so without it
    // "the signature moved" and "it works" look the same. Written when
    // `TRACE_FILES` is on.
    let mut patch_report = String::new();
    // * 5th and 6th item slots.
    if EXTRA_SLOT_PATCHES && crate::build_config::picker_slots() > 4 {
        let slots = crate::build_config::picker_slots().min(15) as u8;
        let rg = unsafe { patch_final_gate() };
        patch_report.push_str(&format!("patch_final_gate: {rg}\n"));
        let rt = unsafe { patch_tick_finals_cap(slots) };
        patch_report.push_str(&format!("patch_tick_cap  : {rt}\n"));
        let rr = unsafe { patch_row_floor(slots) };
        patch_report.push_str(&format!("patch_row_floor : {rr}\n"));
        let rm = unsafe { patch_result_row_floor(slots) };
        patch_report.push_str(&format!("patch_result_flr: {rm}\n"));
    }
    if TRACE_FILES {
        if let Some(d) = mod_dir() {
            let _ = fs::create_dir_all(&d);
            let _ = fs::write(d.join("4items_patches.txt"), &patch_report);
        }
    }
    // Asked here, where a file read costs nothing, and not first on a sim
    // thread inside the buy detour.
    let _ = control_enabled();
    true
}
