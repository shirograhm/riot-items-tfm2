//! The native half of the mod: what the stable mod API cannot express.
//! ===========================================================================
//! - `own_team_only`: the buy and spawn detours hold the athlete pointer, so
//!   they can restrict the player's pins to the player's own athletes.
//! - The 5th and 6th item slots: the buy detour grows every build past the
//!   game's four, and four byte patches let the engine buy and draw them.
//!
//! Everything here is pinned to one exact game build (`check_game_version`):
//! hardcoded addresses, byte patches and struct offsets, re-derived every game
//! update. It began as the standalone mod `tfm2_item_tactics`, whose original
//! purpose, a fourth item slot, the game has shipped itself since 0.6.0; the
//! code for that was removed on 2026-10-07.
//! ===========================================================================
#![allow(dead_code, unused_imports, unused_variables)]

// The client half of what the classic `Scene`/`ClientDatabase` used to give.
use mod_api_stable::{RecordKindV1, StableClient};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;

pub mod driver;

// This half no longer touches the game's native **Personal** tactics tab.
// `crate::strategy_ui` replaces that tab outright with the mod's own `#builds`
// editor and hides `#personal` on entry, so the dropdown overlay
// (`item0m/1m/2m`/`item3`), the selection polling, the `SEL` store behind them
// and the code that hid the native dropdowns underneath were all driving a panel
// nobody could see. All of it is deleted, along with the comp-test screen's copy
// of the same machinery. `item-builds.json` is the single authority on builds.

/// Trace files this half drops in its own folder: `4items_patches.txt` at
/// every init, `version_gate.txt` when the version gate closes, and
/// `4items_netscan.txt` once if the item-network probe misses.
///
/// Off by user request (2026-08-04) — no `.txt` files in the mod folder.
///
/// They were unconditional because a config read that silently falls back to 4
/// slots, a byte patch that silently skips, and a version gate that silently
/// disables this half all look *exactly* like the feature working. With this
/// off there is no evidence of any of them.
const TRACE_FILES: bool = false;

/// **Bisect switch.** `false` makes `tactics_init` install nothing at all — no
/// detours, no byte patches, no per-frame UI work — exactly as a closed version
/// gate does, while leaving the rest of the mod (the stable-API item builds via
/// `crate::item_build_hook`, and `src/hooks/hook.rs`'s data tap) untouched.
///
/// Added 2026-08-19 to bisect a performance regression on game 0.5.6: days
/// advance, but very slowly. This half is the only part the 0.5.6 migration
/// switched back on, so flipping this to `false` answers "is it this half?" in
/// one rebuild. Leave it `true` in any shipped build.
const TACTICS_ENABLED: bool = true;

// ===========================================================================
//  WinAPI FFI
// ===========================================================================
type HMODULE = isize;
type DWORD = u32;
type BOOL = i32;
// The same kernel32 calls are declared again elsewhere in the crate with
// pointers where these take and return pointer-sized integers. On 64-bit
// Windows the two are passed the same way, so the declarations differ in
// spelling only and the lint that compares them has nothing to report.
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
/// Where this half's diagnostic files live.
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
//  Mod item registry (filled once, from the catalog the item-build detour is handed)
// ===========================================================================
static MOD_REGISTRY: Mutex<Vec<String>> = Mutex::new(Vec::new()); // idx i -> key (game ID = 30+i)
static MOD_FINALS: Mutex<Vec<u64>> = Mutex::new(Vec::new()); // mod item IDs whose next_tier is empty
static MODITEMS_DONE: AtomicBool = AtomicBool::new(false);

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
/// This replaced a scan of `Database + 0..0x60000` for something Vec-shaped
/// (`dump_mod_items`, removed 2026-10-07). That scan needed a correct
/// `Database` base, and the merged build derived one as
/// `item_network - 0x1558` — a value whose only self-check was circular
/// (`sig_ok(db + 0x1558)` is true by construction). It found 0 items, so the
/// candidate list was `VANILLA_FINAL` alone and an automatic pick could never
/// be a mod item.
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
        // Upgraded boots pass both tests too, but they are no build goal for
        // an automatic 4th-6th pick; Smart Builds places them.
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
    // `auto_cands` memoizes on first call and never reconsiders, so a list built
    // before this ran would pin the 4th item to vanilla for the whole session.
    *AUTO_CANDS.lock().unwrap_or_else(|e| e.into_inner()) = None;
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

// Vanilla category (1~6) -> final item game ID. Same conversion as the game's c6 jump table (cat1=AD .. cat6=HP).
//   WARNING churn: may move if the game's item tree changes (currently 0.4.14). Matches the constants in the game's jump table (0x143441cf4 etc.).
const VANILLA_FINAL: [u64; 6] = [4, 24, 9, 14, 19, 29];

// ===========================================================================
//  Phase 2c - live match build injection (mid-function detour in the FUN_140c6c430 candidate loop)
// ===========================================================================

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
                                                             // ═══ 0.5.5 athlete layout (2026-08-11) ═════════════════════════════════════════════════════════════════════
                                                             // The struct moved again, and **not by one uniform amount** — it grew 0x60 twice, in two different places, so
                                                             // nothing here may be obtained by shifting the 0.5.4 values by a single delta. Fields below 0x518 moved +0x60;
                                                             // fields from 0x800 up moved +0x120.
                                                             //
                                                             //   champ ptr  0x410 -> 0x470     items ptr 0x440 -> 0x4a0     build cap 0x480 -> 0x4e0     id     0x800 -> 0x920
                                                             //   champ len  0x418 -> 0x478     items len 0x448 -> 0x4a8     build ptr 0x488 -> 0x4e8     team   0x810 -> 0x930
                                                             //                                                              build len 0x490 -> 0x4f0     gold   0x878 -> 0x998
                                                             //   read guard 0x8a8 -> 0x9c8     stride    0x8c0 -> 0x9e0
                                                             //   position   0x8b0 -> 0x9c0  (dword, sits 8 below the read guard in both layouts)
                                                             // The position row was missing from this table until 2026-09-08, and the helpers that read it were still on
                                                             // 0x8b0 that whole time — see `O_ATHLETE_POS`, which is now the only place any of these are written.
                                                             //
                                                             // Every one of these comes from a function established to walk the athlete *by call contract* and then shown to
                                                             // be a clean recompile — same .pdata size, same instruction count, zero mnemonic mismatches — so the offsets are
                                                             // read out of a 1:1 correspondence rather than pattern-matched:
                                                             //
                                                             //   * `buy_item` (r8 = athlete) and its three callees give items ptr/len, build ptr/len and gold.
                                                             //   * the 286-byte roster walk (0x17ce980 -> 0x18ab160, identical at every instruction offset) gives id and,
                                                             //     from its `add rbx,imm`, the stride.
                                                             //   * two further functions (0xf059a0 -> 0xf16ef0, 0x13bd510 -> 0x14b4d40) read the champ String and the id
                                                             //     through the *same base register*, which is what makes their 0x410/0x418 the athlete's and not another
                                                             //     struct's; the second also gives team directly.
                                                             //   * build cap and the read guard are the only two taken by bracketing, and both are bracketed tightly by
                                                             //     fields with equal deltas on either side (0x448/0x488 both +0x60; 0x8a0/0x8c0 both +0x120), which offsets
                                                             //     cannot escape without reordering the struct.
                                                             //
                                                             // The trap this avoids is real: four plausible-looking candidates found by scanning for the 0x410/0x418/0x448
                                                             // displacements turned out to be walking unrelated structs — their offsets did not move at all between builds.
                                                             // ═══ 0.6.0_beta1 athlete layout (2026-08-27) ═══════════════════════════════════════════════════════════
                                                             // The struct moved again, and — as in 0.5.5 — **not by one uniform amount**: it grew in two places, so no
                                                             // value here may be obtained by shifting a 0.5.7 value by a single delta. Fields from the champ String up to
                                                             // ~0x548 moved **+0x30**; fields from the id up moved **+0x90**.
                                                             //
                                                             //   champ ptr  0x470 -> 0x4a0     items ptr 0x4a0 -> 0x4d0     build cap 0x4e0 -> 0x510     id     0x920 -> 0x9b0
                                                             //   champ len  0x478 -> 0x4a8     items len 0x4a8 -> 0x4d8     build ptr 0x4e8 -> 0x518     team   0x930 -> 0x9c0
                                                             //                                                              build len 0x4f0 -> 0x520     gold   0x998 -> 0xa28
                                                             //   position   0x9c0 -> 0xa50     read guard 0x9c8 -> 0xa58     stride    0x9e0 -> 0xa70
                                                             //
                                                             // Every one comes from a 1:1 instruction correspondence, never from pattern-shifting:
                                                             //   * `buy_item` (r8 = athlete) + its three callees — all four pair instruction-isomorphic at IDENTICAL size
                                                             //     (230/691/871/856) — give items ptr/len, build ptr/len and gold. NOTE they only pair under `--loose`:
                                                             //     the displacements are exactly what changed, which is itself the signal that the struct moved.
                                                             //   * the 286-byte roster walk (0x18c84b0 -> 0x1a41030, 74 insns both) gives id and, from `add rbx,imm`, the stride.
                                                             //   * champ ptr/len, team and position are confirmed by **three** independent athlete walkers that read them
                                                             //     through the same base register as the id — 0x18cffe0->0x1a48f10, 0x1076c40->0x118c260, 0xf126a0->0xff53b0 —
                                                             //     each pairing isomorphic. In all three the dword read straight after id+team is the position: 0x9c0 -> 0xa50.
                                                             //   * build cap and the read guard are the only two taken by bracketing, and both are bracketed tightly by
                                                             //     fields with equal deltas on either side (0x4a8/0x4e8 both +0x30; 0x9c0/0x9e0 both +0x90).
                                                             //
                                                             // The 0.5.5 trap repeated exactly: four plausible candidates found by scanning for the 0x470/0x478 displacements
                                                             // were walking unrelated structs and showed **zero** delta. A candidate whose offsets do not move is disproof.
                                                             // 0.5.6 and 0.5.7 = 0x920, unchanged and **measured each time**: the 286-byte roster walk pairs
                                                             // instruction-isomorphic with zero differing displacements, and in 0.5.7 it still reads the id as
                                                             // `mov r14,[rdx+0x920]` at 0x18c84dd, immediately before `add rbx,0x9e0` (= ATH_STRIDE).
                                                             // 0.5.8 = 0x920, unchanged and **measured again, not carried forward**: the 286-byte roster walk pairs
                                                             // clean (0x18c84b0 -> 0x191e560, zero differing displacements) and in the new body still reads the id as
                                                             // `mov r14,[rdx+0x920]` at 0x191e58d, immediately before `add rbx,0x9e0` (= ATH_STRIDE) at 0x191e5aa.
const O_ATHLETE_ID: usize = 0x7f0; // 0.6.3..0.7.0-beta, re-measured on 0.7.0-beta: the 313-byte roster walk pairs clean (0x1b72360 -> 0x1c87df0) and still reads `mov rax,[rdx] / mov r14,[rax+0x7f0]` (0.6.0-beta2..0.6.2 0x9f0, 0.6.0-beta was 0x9b0, 0.5.5..0.5.8 0x920, 0.5.4 0x800, 0.5.3 0x810)

/// The rest of the athlete fields this module reads, as constants rather than
/// literals — which is the whole point of them existing.
///
/// # Why they were added (2026-09-08)
///
/// The 0.5.5 layout migration above updated the buy detour, which spells these
/// offsets out inline, and **missed every helper below it**: `athlete_lineup_at`,
/// `ath_champ_name`, `ath_side_champ`, `build_lineup_ctx` and `valid_ps_elem`
/// were still reading the 0.5.4/0.5.3 layout — champion at `0x420/0x428`, team
/// at `0x820`, position at `0x8b0` — three game versions later. They fail
/// silently: `athlete_lineup_at` validates `team <= 1` against whatever now sits
/// at `0x820`, so the roster scan finds bogus bounds and `build_lineup_ctx`
/// hands `compute_auto_4th_id` a lineup of `9999`s. The auto 4th-item pick has
/// been scoring on that since 0.5.5. (All of them went on 2026-10-07, with the
/// automatic 4th pick and the probes they served.)
///
/// A named constant is what stops the next migration repeating it: one place to
/// change, and a grep for the name finds every reader.
///
/// # Measured, on the shipped 0.5.8 executable
///
/// One site carries all four fields — id, team, position, champion — in eight
/// instructions (`0x1818df0 +0xf1`, base `r13`):
///
/// ```text
///   movdqu xmm6, [r13+0x920]        id, as the 0x920/0x928 pair
///   mov    rbx,  [r13+0x930]        team
///   mov    eax,  dword [r13+0x9c0]  position
///   mov    rdi,  [r13+0x478]        champion name len
///   mov    r15,  [r13+0x470]        champion name ptr
/// ```
///
/// Across the whole image: of every function reading both `+0x920` and `+0x930`
/// through one non-stack base, `+0x9c0` is the *only* dword field any of them
/// also reads (9 sites in 9 functions; the runner-up has 1). `+0x470`/`+0x478`
/// dominate the champion String range at 11 and 14 sites. And the 0.5.5 note
/// above independently pins it: it recorded `read guard 0x8a8 -> 0x9c8`, and the
/// position sits 8 below the guard in both layouts (`0x8b0` under `0x8a8`,
/// `0x9c0` under `0x9c8`) — the guard was written down and the position beside
/// it was not, which is how it went missing.
///
/// The buy detour still writes these as literals; it is the hottest path in the
/// mod and its values are correct, so it was left alone.
///
/// # 0.6.0-beta (2026-09-08)
///
/// Carried forward from the 0.6.0 athlete layout derived on `game-beta`, whose
/// note pins the same three fields: champion `0x470 -> 0x4a0`, team
/// `0x930 -> 0x9c0`, position `0x9c0 -> 0xa50`, id `0x920 -> 0x9b0`, stride
/// `0x9e0 -> 0xa70`. The two halves of the struct move by different amounts
/// (`+0x30` low, `+0x90` high), which is why every field is listed rather than
/// shifted by one delta. Independently corroborated while re-deriving
/// `SPAWN_RVA`: that function writes the athlete's gold at `[rdx+0x998]` on
/// 0.5.8 and `[rdx+0xa28]` on 0.6.0, exactly the move the layout note records.
///
/// # 0.6.3 (2026-10-06)
///
/// The athlete shrank by **0x200 below the champion String**, so every field
/// this module reads moved by exactly that much, and the roster stopped being
/// an array of athletes: it is a `Vec` of pointers to them now (the 313-byte
/// roster walk `0x1b72360` reads `mov rax,[rdx] / mov r14,[rax+0x7f0]` and
/// steps 8), so there is no stride any more.
///
/// Nothing pairs exe2exe this time, strict or loose, so each value is read off
/// the 0.6.3 spawn function (`SPAWN_RVA`), which makes 0.6.2's reads in 0.6.2's
/// order: gold `[rdx+0xa68]` -> `[rdx+0x868]`, then through one base register
/// id `0x9f0` -> `0x7f0`, team `0xa00` -> `0x800`, position `0xa90` -> `0x890`,
/// champion len/ptr `0x4e8/0x4e0` -> `0x2e8/0x2e0`. The resolver
/// (`patch_final_gate`'s container, rdx = athlete) gives the build `Vec` at
/// `[rdx+0x358]/[rdx+0x360]`, the items at `[rax+0x310]/[rax+0x318]` and the
/// gold again at `[rsi+0x868]`; both spawn callers copy `0x898` bytes, where
/// 0.6.2's copied `0xa98`.
///
/// # 0.7.0-beta (2026-10-07)
///
/// Nothing moved, measured rather than carried: `buy_item`, the resolver, the
/// roster walk and both spawn callers pair with 0.6.3's at identical size with
/// no differing displacement (`pairdiff --min-disp 0x4 --imm`), and the spawn
/// function (1818 -> 1754 bytes, changed only away from these reads) still
/// reads gold, id, team, position and champion len/ptr at the offsets below,
/// through the same registers. The `Game` reads there are unchanged too
/// (`[rcx+0x1e30]`, `[rcx+0x1ba0..0x1bb8]`, `[rsi+0x1db8]/[rsi+0x1dc0]`), and
/// the builders that take the catalog still do `lea rdx,[reg+0x1d98]`.
const O_ATHLETE_CHAMP_PTR: usize = 0x2e0; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x4e0, 0.6.0-beta was 0x4a0, 0.5.5..0.5.8 0x470, 0.5.4 0x410, 0.5.3 0x420)
const O_ATHLETE_CHAMP_LEN: usize = 0x2e8; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x4e8, 0.6.0-beta was 0x4a8, 0.5.5..0.5.8 0x478, 0.5.4 0x418, 0.5.3 0x428)
const O_ATHLETE_TEAM: usize = 0x800; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0xa00, 0.6.0-beta was 0x9c0, 0.5.5..0.5.8 0x930, 0.5.4 0x810, 0.5.3 0x820)
const O_ATHLETE_POS: usize = 0x890; // dword. 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0xa90, 0.6.0-beta was 0xa50, 0.5.5..0.5.8 0x9c0, 0.5.4 0x8b0)
/// The athlete's owned-item count and its build `Vec` (`{cap, ptr, len}`), the
/// size of the athlete the spawn callers copy, and the `Game` fields the spawn
/// detour reads. The detours spelled these out as literals through 0.6.2;
/// 0.6.3 moved every one, so they are named like the fields above. How each
/// was measured is in the 0.6.3 notes there and at `SPAWN_RVA`.
const O_ATHLETE_ITEMS_LEN: usize = 0x318; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x518)
const O_ATHLETE_BUILD_CAP: usize = 0x350; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x550)
const O_ATHLETE_BUILD_PTR: usize = 0x358; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x558)
const O_ATHLETE_BUILD_LEN: usize = 0x360; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0x560)
const ATHLETE_COPY_SIZE: usize = 0x898; // 0.6.3..0.7.0-beta (0.6.0-beta2..0.6.2 0xa98)
const O_GAME_PROVIDER: usize = 0x1ba0; // 0.6.3..0.7.0-beta (0.5.3..0.6.2 0x1dc0)
const O_GAME_CATALOG_PTR: usize = 0x1da0; // 0.6.3..0.7.0-beta (through 0.6.2 0x1fd0)
const O_GAME_CATALOG_LEN: usize = 0x1da8; // 0.6.3..0.7.0-beta (through 0.6.2 0x1fd8)
static MY_ATHLETES: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_PREV: AtomicPtr<std::collections::HashSet<u64>> =
    AtomicPtr::new(core::ptr::null_mut());
static MY_ATH_N: AtomicU64 = AtomicU64::new(0); // published starter count (0 = not obtained)
static ROSTER_TICK: AtomicU64 = AtomicU64::new(0);
// Publish my team's starting roster (after the swap the previous copy is released lazily - a sim thread may be reading it, so never free immediately).
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
/// The lane `athlete` plays in this match, off the athlete itself
/// (`O_ATHLETE_POS`, the game's own `Position`, Top = 0 — the order
/// `build_config::Role::LANES` follows): the answer
/// `build_config::role_for_champion` can otherwise only guess. `None` when it
/// does not read as one of the five lanes.
unsafe fn athlete_lane(athlete: usize) -> Option<crate::build_config::Role> {
    let pos = (safe_read_u64(athlete + O_ATHLETE_POS)? & 0xffff_ffff) as usize;
    (pos < 5).then(|| crate::build_config::Role::from_lane_code(pos))
}
// Is this athlete_id on my team (under contract, or in the last starting five)? If the roster is not obtained yet (before visiting the management screen), None = undecided (the caller decides).
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
// ** 0.5.4 (2026-08-04): found by its documented body rather than an exe2exe signature (no old exe - see
//   `tools/rederive.py`). `mov rdi,r9 / mov rsi,rcx / cmp r8,0x11` is **1 hit in .text**, at +0x11 inside fn
//   0x29a7640. The body is __rust_realloc outright: `cmp r8,0x11 / jae` splits the over-aligned path, the
//   align<=16 path tail-jmps to HeapReAlloc(heap, 0, ptr, size), and the over-aligned path allocs (0x29bb920),
//   memcpys, then frees. Argument contract (rcx=ptr, rdx=old, r8=align, r9=new) is unchanged.
const RVA_REALLOC: usize = 0x32c7bf0; // 0.7.0-beta (2026-10-07: exe2exe from 0.6.3 strict unique, FUNCTION START, size 174 both sides, pairdiff clean at --min-disp 0x4 --imm, the 12 prologue bytes unchanged). 0.6.3 was 0x2f855a0 (2026-10-06: exe2exe from 0.6.2 strict unique, FUNCTION START, size 174 both sides, the 12 prologue bytes unchanged). 0.6.2 was 0x2f9f0e0 (2026-09-29: exe2exe from 0.6.1 strict unique, FUNCTION START, size 174 both sides, pairdiff clean at --min-disp 0x4 --imm). 0.6.1 was 0x2f50ad0 (2026-09-21: exe2exe from 0.6.0 strict unique, FUNCTION START, size 174 both sides, pairdiff clean). 0.6.0 release was 0x2f23bf0 (2026-09-18: exe2exe from beta2 0x2f1b320 is unique, FUNCTION START, size 174 both sides; the 23-byte entry below is also unique in .text on its own). 0.6.0-beta2 was 0x2f1b320 (0.6.0-beta was 0x2dc0690, 0.5.7 0x2a9fb50, 0.5.6 0x2a9d1b0; exe2exe unique, size 174 both sides, pairdiff clean). History for 0.5.6 follows. (0.5.5 was 0x2a87a70; exe2exe unique, size 174 both sides, instruction-identical). History for 0.5.5 follows. (0.5.4 was 0x29a7640; exe2exe unique, size 174 both sides, body still the __rust_realloc shape). History for 0.5.4 follows. (0.5.3 was 0x28e3b10). History for 0.5.3 follows. (0.5.2 was 0x25c4dd0). The real __rust_realloc. (rcx=ptr, rdx=old, r8=align, r9=new) -> rax. A 112B masked signature from the old exe gave exactly 1 hit in the new exe + instruction-for-instruction identical body (mov rdi,r9 / mov rsi,rcx / cmp r8,0x11 / jae).
type ReallocFn = unsafe extern "win64" fn(usize, usize, usize, usize) -> usize;
/// First 12 bytes of `RVA_REALLOC` (6 push + `sub rsp,0x28`), checked before
/// every call. The call is a raw transmute, and on 2026-09-16 a stale beta2
/// address in this constant crashed matches (AV at exe+0x2f1b2c0): with this
/// check a stale address declines instead, and the build keeps the game's four.
const REALLOC_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x28,
];

/// Whether `RVA_REALLOC` still starts with [`REALLOC_PROLOGUE`]. Read once and
/// cached: the image does not change under a running game.
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

/// Catalog index -> item name (evt[0x50] shadow-call). The inverse of scan_recipe_safe_in.
///
/// Every read is VEH-guarded (`safe_read_*`) rather than `readable` + a raw
/// read: the buy detour names catalog entries dozens of times a decision, and
/// `readable` is a `VirtualQuery` syscall per check (7 per name, ~5.9 us
/// each, measured 2026-09-25). The one thing a guarded read cannot prove, that
/// the name getter is code, is proven once per getter ([`name_getter_ok`]).
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

/// [`catalog_name_at`] against a catalog array already read out of its
/// collection (`data`/`len`), which is what the index cache needs to re-check a
/// cached index without going back through a context.
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
// ** 0.5.5 re-derivation (2026-08-11). exe2exe `match` returns **0 hits** here even with `--loose`, which is
//   expected and not a warning sign: the frame immediate sits in the first 13 bytes, `match` deliberately keeps
//   non-address immediates concrete, and the frame moved 0x25168 -> 0x25438. `frames --near 0x25168` puts
//   0x14ac3e0 first at delta +0x2d0, with the next nearest 0x1770 away — the same isolated-outlier shape as
//   every previous migration. What actually settles it is the call structure, compared side by side: **60 direct
//   calls on both sides, in the same order, to callees of the same sizes**, including seedctor three times at
//   the identical instruction offsets (+0x1c8, +0x29b, +0x372) and the confirmed allocator repeatedly. The
//   `mov r12,r8` entry save and the `mov rdx,r12` before the seedctor call are both still there, so the
//   r8=seed contract holds.
// ** 0.5.7 re-derivation (2026-08-26). Again 0 exe2exe hits, again for the documented reason: the frame
//   immediate is in the first 13 bytes and moved 0x25438 -> **0x25418**, and this time the body also shrank
//   4398 -> 4369 bytes. `frames --near 0x25438` puts 0x106dd60 **first** at delta -0x20, next nearest 0x14c0
//   away. Three independent structural checks then agree, and all three are stronger than the size:
//     (1) 60 direct calls on both sides — seedctor three times at the *identical* instruction offsets
//         (+0x1c5, +0x298, +0x36f) and the confirmed allocator eight times, also identical
//         (+0x142/+0x1dd/+0x21d/+0x2c8/+0x308/+0x711/+0x751/+0x7fb). The call offsets only start to diverge
//         at call #29, past everything this mod cares about.
//     (2) 9 direct callers on both sides, with the same shape (one caller making two of them).
//     (3) the entry idiom is byte-for-byte the same apart from the frame imm, `mov r12,r8` still saves the
//         seed, and every rbp spill moved by exactly -0x20 = the frame delta. The r8=seed contract holds.
//   CL_LAUNCHER_PROLOGUE therefore needs its last 4 bytes changed with the frame; that is the only edit.
const CL_LAUNCHER_RVA: usize = 0x16cd4e0; // 0.7.0-beta (2026-10-07; 0.6.3 was 0x13f3950). exe2exe strict unique at 160 bytes, a function start, and the frame did not move this time (0x25ef8, so CL_LAUNCHER_PROLOGUE is unchanged); the body shrank 4367 -> 4319 past the head, whose first 39 instructions are 0.6.3's (`mov r12,r8` still saves the seed). Confirmed from the seedctor callers: 3 sites inside 0x16cd4e0 at +0x1c5/+0x298/+0x36f plus 1 in 0x16cf120 (+0x113); 10 caller sites now (4445/5757 identical; the match-sim megafunction 80906 -> 80806 is again the hook.rs target's caller). 0.6.3 notes follow: (2026-10-06; 0.6.2 was 0x17277c0). From the seedctor callers once more: 3 sites inside 0x13f3950 at +0x1c8/+0x29b/+0x372 (0.6.2: +0x1c5/+0x298/+0x36f) plus 1 in 0x13f55c0 (+0x116), 60 direct calls and 9 caller sites in 8 functions on both sides (4445/5757 identical; the match-sim megafunction 79953 -> 80906 is again the hook.rs target's caller), size 4490 -> 4367, frame 0x25478 -> 0x25ef8. `mov r12,r8` still saves the seed at +0x41 and the seedctor call still gets it in rdx. 0.6.2 notes follow: (2026-09-29; 0.6.1 was 0x1a88440). exe2exe finds nothing (size 4454 -> 4490); from the seedctor callers again: 3 sites inside 0x17277c0 at the SAME +0x1c5/+0x298/+0x36f plus 1 in 0x1729490 (+0x113 both builds), 9 direct callers both sides (3078/4312/4445/5757 identical; the match-sim megafunction 79985 -> 79953 is also the hook.rs target's caller), frame 0x25458 -> 0x25478. 0.6.1 notes follow: (2026-09-21; 0.6.0 release was 0x16d9180). exe2exe finds nothing again (size 4486 -> 4454), so it came from the seedctor callers once more: 3 sites inside 0x1a88440 at +0x1c5/+0x298/+0x36f (0.6.0: +0x1c8/+0x29b/+0x372) plus 1 in 0x1a8a0f0, 9 direct callers both sides with the same caller sizes (only the match-sim megafunction grew, 79937 -> 79985, and it is also the hook.rs target's caller), frame 0x25478 -> 0x25458. 0.6.0 release notes follow: (0.6.0-beta2 was 0x16f7270). exe2exe finds NOTHING here - the body changed - so it came from the seedctor callers, the same way 0.5.3 did: beta2's seedctor had 3 call sites inside the launcher plus 1 elsewhere, and the release's seedctor (0x16e98c0) has exactly 3 inside 0x16d9180 plus 1 inside 0x16dae50. Cross-checked three more ways: 9 direct callers (the same count beta2 had), two of them in one function (the render scene builder calling it twice, 0x81ba60 here vs 0x814a20 there), and the same prologue idiom with only the chkstk frame moving. 0.6.0-beta2 (0.6.0-beta was 0x1183bb0, 0.5.7 0x106dd60, 0.5.6 0x14dda60, 0.5.5 0x14ac3e0, 0.5.4 0x13b53d0, 0.5.3 0xeb8810). History for 0.5.3 follows. (0.5.2 was 0x1d96870). Evidence: (1) identical prologue idiom (8 push + mov eax,frame + call chkstk + lea rbp,[rsp+0x80] + xmm spills + [rbp+X]=-2) (2) **9 callers = the same count as the old exe** (3) the render scene builder (0x997740) calls it twice (4) internally it calls seedctor (0x12b9ab0) with rdx = the saved r8 (seed) = line-for-line correspondence with the old exe. The r8=seed entry contract still holds (mov r12,r8).
const CL_LAUNCHER_PROLOGUE: [u8; 17] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0xb8, 0xf8, 0x5e, 0x02,
    0x00,
]; // 0.6.3: 8 push + mov eax,0x25ef8 (0.6.2 was 0x25478, 0.6.1 0x25458, 0.6.0 release 0x25478, 0.6.0-beta2 0x25468) (0.6.0-beta was 0x25438, 0.5.7 0x25418, 0.5.6 0x25438, 0.5.5 0x25438, 0.5.4 0x25168, 0.5.3 0x25108, 0.5.2 0x165c8)
static CLAUNCH_INSTALLED: AtomicU64 = AtomicU64::new(0);
// * Is the current match a comp test? Comp test is a sandbox where the user composes both blue and red themselves, so
//   there is no notion of "my team" -> bypass the team gate and apply to both sides for designated champions.
static COMPTEST_MATCH: AtomicBool = AtomicBool::new(false);
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
    }
    0
}
static HK_L_TICK: AtomicU64 = AtomicU64::new(0);
fn install_launcher_hook() {
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
    if our != 0 && cur_tgt == our {
        CLAUNCH_INSTALLED.store(1, Ordering::Relaxed);
        return;
    } // entry point = our stub -> fine
    let is_foreign = b0 == 0x48 && cur_tgt >= 0x10000 && cur_tgt != our; // a foreign hook (serpen etc.) is present
    let waited = LAUNCH_WAIT.fetch_add(1, Ordering::Relaxed);
    if !is_foreign && b0 != 0x48 && waited < 240 {
        return;
    } // original prologue and still waiting -> wait for serpen to install
      // Install (or re-chain). install_detour_generic chains automatically when it detects a foreign hook.
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
// ** 0.5.5 (2026-08-11): two independent methods agree. exe2exe `match` gives **1 hit**, and `frames --near
//   0x11b98` puts the same function first at frame 0x11ba8 (delta +0x10, next nearest 0x4c0 away). Its size grew
//   4175 -> 4407, which is why the pair relationship matters more than the size: it is the function the launcher
//   calls three times with `rdx` = its saved seed. Entry shape unchanged (8 push + mov eax,frame + call chkstk),
//   so SEEDCTOR_PROLOGUE needs no edit.
// ** 0.5.7 (2026-08-26): the easy case for once — exe2exe `match` gives **1 hit**, size 4525 and 733
//   instructions on both sides, and it is confirmed by the launcher relationship anyway (the launcher calls it
//   at the same three instruction offsets, and its 4 caller sites correspond 3-in-launcher + 1-elsewhere on
//   both sides). `pairdiff` reports only two differing displacements, both in the TLS block reached through
//   `gs:[0x58]` (0x187e8 -> 0x18830), which is thread-local layout, not the provider struct. Entry shape
//   unchanged (8 push + mov eax,frame + call chkstk), so SEEDCTOR_PROLOGUE needs no edit.
const SEEDCTOR_RVA: usize = 0x16e61d0; // 0.7.0-beta (2026-10-07: exe2exe strict unique, fn start, size 4468 both sides, frame still 0x11f38; pairdiff at --min-disp 0x4 --imm shows two differing displacements, 0x176a0/0x176b0 -> 0x17768/0x17778, far above anything read here, and the `mov [rsi+0xec88],rax` seed store is byte-identical at the same +0xf06 and still the only one in the image. 12B prologue byte-identical). 0.6.3 was 0x13fddb0 (2026-10-06: exe2exe finds nothing, size 4586 -> 4468, frame 0x11bc8 -> 0x11f38. Found as the function holding the seed store, which moved: the masked `48 89 86 ?? ec 00 00` has 5 hits and one of them, `mov [rsi+0xec88],rax` at +0xf06, follows the same three `[rsi+0x70]/[rsi]/[rsi+0x10]` initialisers 0.6.2's store follows and stores the same value, the rdx seed spilled at +0x44. The launcher calls it three times and one other function once, as before. 12B prologue byte-identical). 0.6.2 was 0x1749a50 (2026-09-29: exe2exe finds nothing, size 4411 -> 4586, frame 0x11ba8 -> 0x11bc8; the `mov [rsi+0xec90],rax` O_PROVIDER_SEED store is unique in the image and sits in this function at +0xeed (0.6.1 +0xecd), and the launcher calls it at the same three offsets. 12B prologue byte-identical). 0.6.1 was 0x1a98e60 (2026-09-21: exe2exe strict unique, fn start, size 4411 both sides; the O_PROVIDER_SEED store is byte-identical at +0xecd). 0.6.0 release was 0x16e98c0 (0.6.0-beta2 was 0x1712c90; exe2exe unique at 154B, fn start, size 4407 -> 4411, 12B prologue byte-identical) (0.6.0-beta was 0x1697a30, 0.5.7 0x1635ae0, 0.5.6 0x10a3be0, 0.5.5 0x14c2380, 0.5.4 0x14e16d0, 0.5.3 0x12b9ab0). History for 0.5.3 follows. (0.5.2 was 0x22c1da0). The 12B prologue is completely identical (8 push); the chkstk frame went 0x11b58 -> 0x11b98; confirmed via the call inside launcher (0xeb8810) with rdx = the saved r8 (seed). WARNING: the seed store offset moved from provider+0xeab8 to **+0xeaf8** (measured at 0x12ba92d).
                                       // * 0.5.3: the seed store offset inside the provider struct moved (0.5.2 +0xeab8 -> 0.5.3 +0xeaf8).
                                       //   Measured = `mov [reg+0xeaf8], rdx` inside seedctor @0x12ba92d (the old exe has 0xeab8 in the same place).
                                       //   WARNING keep it in a single constant - updating only this on each patch carries the whole is_live gate along.
                                       // 0.5.4 = 0xeb28 (0.5.3 was 0xeaf8, 0.5.2 0xeab8). Measured, not guessed: seedctor spills its `rdx` (the seed)
                                       // to [rbp+0x11a48] at entry (0x14e1714), reloads it at 0x14e2596 and stores it to **[rsi+0xeb28]** at 0x14e259d.
                                       // The writes that follow (+0xeb30 = 0, +0xeb58 = 0, +0xeb59 = the bool arg) are the same field cluster.
                                       // 0.5.5 = 0xec90 (0.5.4 was 0xeb28). Measured the same way, and the whole
                                       // field cluster moved together by a uniform +0x168, which is what makes it
                                       // the same cluster rather than a lookalike: the seven big-displacement
                                       // stores in seedctor correspond one for one and in the same order —
                                       // 0xeb5a->0xecc2, **0xeb28->0xec90 (the `mov [rsi+...],rax` seed store,
                                       // at 0x14c324d)**, 0xeb30->0xec98, 0xeb58->0xecc0, 0xeb59->0xecc1,
                                       // 0xeb38->0xeca0, 0xeb48->0xecb0.
                                       // 0.5.6 and 0.5.7 = 0xec90, **unchanged and verified, not assumed**: the whole seven-store cluster inside
                                       // seedctor corresponds one for one, in the same order, at the same displacements — 0xecc2, **0xec90 (the
                                       // `mov [rsi+...],rax` seed store, at 0x1636a0d)**, 0xec98, 0xecc0, 0xecc1, 0xeca0, 0xecb0.
const O_PROVIDER_SEED: usize = 0xec88; // 0.6.3..0.7.0-beta: `mov [rsi+0xec88],rax` at seedctor +0xf06 on both (0.6.2 0xec90 at +0xeed; the qword after it is still zeroed by the next instruction)
const SEEDCTOR_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // ghidra-re confirmed: 8 push (12B) + mov eax,0x11b58 + call chkstk (same pattern as launcher)
const SEEDCTOR_ORIG_LEN: usize = 12; // relocate the 8 pushes only (excluding the chkstk call). The jmp lands on fn+12 = mov eax -> the frame is set up correctly
static SEEDCTOR_INSTALLED: AtomicU64 = AtomicU64::new(0);
static RENDER_PROVIDER: AtomicU64 = AtomicU64::new(0); // * rendered sim provider pointer (the primary is_live gate)
static LIVE_SEED: AtomicU64 = AtomicU64::new(0); // * my match's seed (captured from r8 in the launcher hook). The v13 value-comparison key.
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
    } // * skip only on 1 = success (0/2 = retry)
      // A failed attempt (2) backs off instead of re-running every frame; see
      // `install_retry_due`. State 0 is the untried first frame and is not delayed.
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
// * 0.5.8 re-derivation (2026-09-08) — the gate is ON again, so this address is live.
//   exe2exe `match` is useless here: the body changed at 0.5.3 -> 0.5.4 and gets 0 hits at every length,
//   strict and `--loose` alike. Derived structurally instead, and the filter was **validated against the
//   known 0.5.3 answer before being trusted**: "8-push prologue + `sub rsp,imm32`, size 600..3000, body
//   containing all four Game displacements 0x1dc0/0x1dc8/0x1dd0/0x1dd8" returns **exactly one** function in
//   0.5.3 — 0xebfe50, size 1202, frame 0xf8, 15 direct callers, matching every number recorded below — and
//   **exactly one** in 0.5.8: 0x1819300, size 1150, frame 0xf8.
//   The pair is then confirmed instruction-for-instruction over the whole head: every instruction sits at the
//   same offset with the same mnemonic through +0xcd, and only four operands differ, each of them a struct
//   field that independently moved:
//     [rdx+0x888] -> [rdx+0x998]   athlete **gold** — exactly the move the 0.5.5 layout table records
//     [rdx+0x598] -> [rdx+0x5e8]   athlete field, the same low-range shift
//     [r15+0x160] -> [r15+0x188]   provider vtable slot
//     plus relocated branch/call targets.
//   Two of those four *are* the argument contract: `rdx` is written at the athlete's own gold offset, so
//   **rdx = athlete** is proven rather than assumed, and `[rcx+0x2060]`/`[rcx+0x1dc0]`/`[rcx+0x1dc8]` keep
//   **rcx = Game**. The 0.5.3 warning (2) — r8/r9 becoming the descriptor's two-word pair — is moot for this
//   mod: `cap_spawn` reads only saved[0] (rcx) and saved[1] (rdx) and never touches r8/r9. Warning (1) is
//   satisfied too: the prologue is still 8 push + `sub rsp,0xf8`, byte-identical, so SPAWN_PROLOGUE and
//   SPAWN_ORIG_LEN=12 needed no edit and the generic detour still suffices.
// * 0.6.0-beta re-derivation (2026-09-08). The same validated filter, run 0.5.8 -> 0.6.0: exactly one
//   candidate on each side, 0x1819300 and **0x118c260**, both size 1150 and frame 0xf8 with 2 direct callers.
//   The heads pair 45 instructions identical / 8 differing, and every difference is a field that moved:
//     [rdx+0x998] -> [rdx+0xa28]   athlete **gold** — precisely the move the 0.6.0 layout note records
//     [rdx+0x5e8] -> [rdx+0x618]   +0x30, the documented 0.6.0 low-half shift
//     [r15+0x188] -> [r15+0x178]   provider vtable slot
//     [r14+0x5e8] -> [r14+0xb68]   a different struct's field, plus relocated branches/calls.
//   The gold offset is the clincher: it re-proves **rdx = athlete** against a layout derived independently
//   of this function. 0x118c260 is also named in that note's own list (0.5.7's 0x1076c40 -> 0x118c260) as a
//   function that reads the athlete id, which is a second, unrelated route to the same address.
//   Direct callers fell 15 -> 2, which is the documented trend, not a mismatch: 0.5.3 already noted the
//   callers moving to an indirect call through a global function pointer.
// * 0.6.1 re-derivation (2026-09-23). exe2exe `match` from beta's 0x118c260 gets 0 hits strict and
//   `--loose`: beta2 grew the body 1150 -> 1914 and the frame 0xf8 -> 0x108 (one inserted block
//   mid-body; the head and the call sequence around it are unchanged). The validated structural
//   filter above still returns **exactly one** function in every build, and reproduces both earlier
//   answers first: 0.5.8 0x1819300, beta 0x118c260, beta2 0x16ff930, 0.6.0 0x16dcdd0,
//   **0.6.1 0x1a8c100** (size 1914, frame 0x108, 2 direct callers like beta).
//   Contract re-proved on 0.6.1, not carried:
//     [rdx+0xa28] -> [rdx+0xa68]   athlete gold, beta +0x40 = the beta2 layout shift that
//     [rdx+0x618] -> [rdx+0x658]   O_ATHLETE_ID (0x9b0 -> 0x9f0) and ATH_STRIDE record
//     [rcx+0x2060], [rcx+0x1dc0..0x1dd8], [rsi+0x1fe8/0x1ff0]  Game fields byte-identical to beta,
//                                  so the catalog at Game+0x1fd0/+0x1fd8 has not moved either
//   The caller (0x1a8b750, site +0x24a) still memcpys the athlete into a stack slot of **0xa98**
//   bytes (beta 0xa58, +0x40) and passes it as rdx -> exactly `cap_spawn`'s `readable(athlete,
//   0xa98)` guard. Prologue: the same 12 push bytes, so SPAWN_PROLOGUE / ORIG_LEN are unchanged.
// * 0.6.2 re-derivation (2026-09-29). The caller matched strict (0x1a8b750 -> 0x172f540, 604B both,
//   pairdiff clean) and still calls spawn at +0x24a; the structural filter independently returns
//   exactly one function, the same one: **0x172ff50** (size 1914 -> 1924, frame still 0x108, the same
//   2 direct callers at the same offsets). Contract re-read, not carried: `mov [rdx+0xa68],rax` (gold)
//   at the same +0x41, and the caller's `mov r8d,0xa98` athlete copy at the same +0x21a.
// * 0.6.3 re-derivation (2026-10-06). exe2exe finds neither the function nor its caller: the athlete
//   moved (see `O_ATHLETE_CHAMP_PTR`), and so did the Game fields the structural filter above
//   searches for. Found by the gold store with its new displacement, `mov [rdx+0x868],rax`, which
//   has two hits in .text and one of them at a function's +0x41, where it has always been:
//   **0x13f7550** (size 1924 -> 1818, frame still 0x108, the same 2 direct callers: 0x13f6be0 at
//   +0x204 and 0x13f6f40 at +0x91). Contract re-read, not carried: rcx = Game, rdx = athlete, and
//   both callers memcpy **0x898** bytes (was 0xa98) into the stack slot they pass as rdx.
//   The Game fields moved by two different amounts, each read off this function:
//     [rcx+0x2060]                -> [rcx+0x1e30]                 (-0x230)
//     [rcx+0x1dc0..0x1dd8]        -> [rcx+0x1ba0..0x1bb8]         (-0x220; +0x1ba0 = the provider)
//     [rsi+0x1fe8], [rsi+0x1ff0]  -> [rsi+0x1db8], [rsi+0x1dc0]   (-0x230; the empty Vec beside the catalog)
//   which puts the catalog Vec at Game+0x1d98 {cap} / +0x1da0 {ptr} / +0x1da8 {len}. Confirmed
//   apart from that neighbour: the 67-byte drop glue `lea rcx,[rsi+0x1fc8] / call` exists with
//   `0x1d98` at the same +0x27 in 0.6.3, and so do the two large builders that take
//   `lea rdx,[reg+0x1fc8]`. For the next build, the filter's disp32s are 0x1ba0/0x1ba8/0x1bb0/0x1bb8.
// * 0.7.0-beta re-derivation (2026-10-07). exe2exe strict from 0.6.3 is unique at 160 bytes:
//   **0x16d1080** (size 1818 -> 1754, frame still 0x108, the same 2 direct callers: 0x16d0710 at
//   +0x204 and 0x16d0a70 at +0x91, both pairing clean and both still copying 0x898 bytes). The
//   first 165 instructions are 0.6.3's, so the gold store is at the same +0x41 and every Game
//   read is byte-identical; the id/team/position/champion reads are the same instructions 0x41
//   bytes earlier (+0x35f on). The gold store's second hit in .text is still mid-function.
const SPAWN_RVA: usize = 0x16d1080; // 0.7.0-beta (0.6.3 0x13f7550, 0.6.2 0x172ff50, 0.6.1 0x1a8c100, 0.6.0 0x16dcdd0, beta2 0x16ff930, beta 0x118c260, 0.5.8 0x1819300, 0.5.3 0xebfe50, 0.5.2 0x1d9e0e0, 0.5.1 0x2060280).
const SPAWN_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
]; // 0.5.8: unchanged since 0.5.3 - 8 push (12B) + sub rsp,0xf8, byte-identical at the new address (0.5.2 was 7 push + mov eax,0x4d20)
const SPAWN_ORIG_LEN: usize = 12; // 0.5.8: unchanged - relocate the 8 pushes only (12B = exactly an instruction boundary) => install_detour_r11 is unnecessary on re-enable (generic suffices).
                                  // ** ON again for game 0.6.1 (2026-09-23), with SPAWN_RVA re-derived above.
                                  // It had been OFF since 2026-09-16 and dead since beta2: the beta1 address
                                  // was carried forward unvalidated, `install_spawn_hook` refused it on the
                                  // prologue check, and slot 0 under `own_team_only` was always the engine's
                                  // pick. This is the only path that can set slot 0 in that mode.
const SPAWN_INJECT_ENABLED: bool = true; // was false 2026-09-16..09-23; was true (2026-09-08), confirmed in game: with this closed the first item was always the engine's pick, and with it open all four slots hold the configured build. ON after the 0.5.8 re-derivation above re-confirmed both sealing reasons: the prologue is unchanged (warning 1) and the r8/r9 contract change (warning 2) never applied to `cap_spawn`, which reads only rcx/rdx. This is the only path that can set build slot 0 under `own_team_only` — see `build_config::own_team_only_enabled`. History: OFF from 0.5.2 (logic change unconfirmed) through 0.5.7; 0.5.1 had true. ~~resumed (07-19)~~ the sealing reason "no catalog at spawn time" turned out to be an offset error.
                                         //   The old 0x1fe8/0x1ff0 = a neighbouring empty Vec (always len=0) -> the real catalog is Game+0x1fd0/+0x1fd8 (ghidra-re confirmed).
                                         //   The v15 team decision (athlete_id membership) is verified (aid valid 10/10, my team 5/10 correct) -> (4) injection expected to complete.
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
        // -- (1) Rendered (spectated) match gate: provider = *(Game+0x1dc0) - offset confirmed unchanged in 0.5.3 (07-29). WARNING the buy hook uses r9 (a separate path) --
        let provider = match safe_read_u64(game + O_GAME_PROVIDER) {
            Some(p) => p,
            None => return,
        };
        if provider < 0x10000 || provider >= 0x0000_8000_0000_0000 {
            return;
        }
        // Whether this athlete belongs to the match currently on screen.
        //
        // This used to `return` here, so nothing was injected unless the match
        // was being watched. That was right when the team decision below could
        // fall back on scene state — a scene answer is worthless off-screen —
        // but it is too strict for the athlete-id gate, which is scene-free by
        // construction and is the whole reason v15 exists. Under
        // `own_team_only` the only athletes this path ever writes to are the
        // player's own five, and whether the player happens to be watching does
        // not change whose they are. Keeping the old behaviour would have left
        // slot 0 correct in spectated matches and wrong in every simulated one,
        // which is a worse bug than the one being fixed because it looks
        // intermittent.
        //
        // So it is now a *fact* rather than a gate: definite roster membership
        // is honoured either way, and only the uncertain scene fallback still
        // requires it. See the team decision below.
        let lseed = LIVE_SEED.load(Ordering::Relaxed);
        let seed_ok =
            lseed != 0 && safe_read_u64(provider as usize + O_PROVIDER_SEED) == Some(lseed);
        let rp = RENDER_PROVIDER.load(Ordering::Relaxed);
        let rendered = seed_ok || (rp != 0 && provider == rp);
        // -- (2) Is this a designated champion? --
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
        // Was `is_champ_designated`, which OR-ed the pin set with the `SEL`
        // dropdown keys. `SEL` is gone, so the pin set is the whole answer.
        if !crate::build_config::has_pins(champ) {
            return;
        }
        // -- (3) Is it my team (v15): athlete_id (+0x810) in my starting roster = no scene tag9 needed -> holds at spawn time.
        //     WARNING because static analysis A2 established the sim has no team_id, this membership test is the only deterministic path.
        //     If the roster is not obtained yet (before visiting the management screen), stay undecided -> skip injection (avoiding enemy-team contamination > coverage). The buy path covers it.
        //     Also run the scene side as a fallback (for early frames where the roster exists but aid is not filled in).
        let mine = is_my_athlete(athlete);
        crate::own_team_log::line(|| {
            format!(
                "spawn: {} aid={:?} provider=0x{:x} seed=0x{:x} lane={:?} rendered={} mine={:?} roster_n={}",
                champ,
                safe_read_u64(athlete + O_ATHLETE_ID),
                provider,
                safe_read_u64(provider as usize + O_PROVIDER_SEED).unwrap_or(0),
                athlete_lane(athlete),
                rendered,
                mine,
                MY_ATH_N.load(Ordering::Relaxed)
            )
        });
        let ok = match mine {
            Some(true) => true,
            Some(false) => return, // definitely another team = do not inject
            None => {
                // Roster/aid unavailable -> scene fallback, which is only
                // meaningful for the match actually on screen. Off-screen there
                // is no scene side to compare against, so stay undecided and
                // leave it to the buy path (slots 1/2), exactly as before:
                // avoiding enemy-team contamination beats coverage.
                //
                // ** Observed cost of that choice (2026-09-08): slot 0 can be
                // wrong for the *first* match of a session and correct from the
                // second on. `MY_ATHLETES` is published from the Team record's
                // `last_starting` on the roster poll in `tactics_post_update`,
                // so until that first publish lands `is_my_athlete` answers
                // `None` and this arm declines — and the buy path, which does
                // cover slots 1/2, is structurally too late for slot 0. It is
                // the safe failure, not a bug to paper over: guessing here puts
                // the player's build on the enemy. If it needs tightening, the
                // fix belongs at the publish end (poll every frame until the
                // roster is first obtained, then back off to ROSTER_POLL),
                // not here.
                //
                // (2026-09-25, logged: the roster published about a second after
                // the save loaded and 25 s before the first spawn, and every
                // spawn of the player's athletes that session read `Some(true)`.
                // The miss above was not reproduced, so the poll is unchanged.)
                //
                // (2026-09-29, reproduced on a new save: `last_starting` stayed
                // empty until the first match began, so the starters were
                // published two seconds after they spawned. The fix was at the
                // publish end, as predicted, but in *what* is published rather
                // than how often: the roster now comes from every athlete's
                // contract, which exists from load. See `roster_scan_step`.)
                if !rendered {
                    return;
                }
                let side = if readable(athlete + O_ATHLETE_TEAM, 8) {
                    rd_u64(athlete + O_ATHLETE_TEAM)
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
            return;
        }
        // -- (4) Inject the build[] targets --
        // ** Catalog offset correction (07-19 ghidra-re confirmed): the old 0x1fe8/0x1ff0 were a **neighbouring empty Vec** 0x18 off
        //   (the Game ctor initializes it cap=0 / ptr=8 (dangling) / len=0, and there is no push site anywhere in the exe -> always len=0.
        //    That is exactly what the measured catlen=0 was - it was never a spawn timing problem).
        //   The real catalog = Game+0x1fc8{cap} / +0x1fd0{ptr} / +0x1fd8{len}, stride 0x10 {elem_ptr, vtable}.
        //   * Same index space: the ctx builder 0x1420571C8 puts ctx+0x30 = &(Game+0x1fc8) => it is the same heap buffer
        //    that buy indexes into = usable directly in build[] (no mapping table needed).
        //   * Ordering guaranteed: Game creation (catalog builder 0x21c0750) precedes spawn (in all 21 wrapper call sites).
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
            // The stable hook gave this athlete the build it gives both teams,
            // which does not know the pins. The player's athlete gets the
            // pin-aware one the hook recorded next to it, before the pins
            // below are written into it.
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
            // (The `Scope::Plain` argument these took is gone with `SEL`; a pin
            // has no scope, so there is nothing left to disambiguate.)
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
                    // The engine may already target the pinned item in another
                    // slot, or, for boots, another pair. Nothing is bought at
                    // spawn, so swap rather than duplicate: the engine's item
                    // for this slot moves to where the clash was. (The buy
                    // path does the same for the slots it can still reach; see
                    // there.) A slot the player pinned to what it holds is
                    // theirs and is left alone.
                    let pin_boots = spawn_is_boots(cat_base, cat_len, t);
                    let elsewhere = (0..blen as usize).filter(|&j| j != si as usize).find(|&j| {
                        let there = rd_u64(bptr + j * 8);
                        (there == t || (pin_boots && spawn_is_boots(cat_base, cat_len, there)))
                            && spawn_pin_at(champ, cat_base, cat_len, j) != Some(there)
                    });
                    if let Some(j) = elsewhere {
                        wr_u64(bptr + j * 8, here);
                    } else if !pin_boots && spawn_is_boots(cat_base, cat_len, here) {
                        // The pin covers the boots Smart Builds gave this
                        // build (rule 7). Unless the player pinned a pair of
                        // their own, move them where the rule would have put
                        // them (`displaced_boots_slot`), replacing the
                        // engine's pick there. Nothing is bought yet, so the
                        // first slot is open to them too.
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
        // Boots the player pinned past the slots above (the 5th and 6th, which
        // the buy path plants later): the engine's own pair is swapped for
        // them now, so the build holds one pair, the player's, bought when the
        // engine's would have been. `pin_placed_by_engine` then drops the
        // later pin.
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
    0 // the install_detour_generic stub does not use the return value (this is an observe/modify hook)
}
/// Where the boots Smart Builds put in build slot `si` go when a pin lands
/// there, following the rule's order (`smart_builds::BOOTS_SLOT`): the first
/// slot after `si` that no pin holds. A 5th or 6th slot the build has not
/// grown to yet (`len` is its length now) gets them when it grows
/// (`extra_slot_boots`), so there is nothing to move then: `None`. Failing
/// both, the first slot, when it is not bought yet (`first_open`) and no pin
/// holds it. `None` otherwise, and the build goes without.
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

/// Whether catalog entry `index` is a pair of boots, for `cap_spawn`, which
/// holds the catalog base and length rather than a buy context.
unsafe fn spawn_is_boots(cat_base: usize, cat_len: u64, index: u64) -> bool {
    catalog_name_in(cat_base, cat_len, index)
        .is_some_and(|name| crate::smart_builds::is_boots(&name))
}

/// The athlete's build as catalog names, for the `own_team_log` test log.
/// `None` for an entry the catalog does not name.
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

/// Swaps the pin-free build the stable hook handed this athlete for the
/// pin-aware one it recorded under `own_team_only`
/// (`crate::item_build_hook::remember_pinned_build`): the Smart Builds pass
/// that counted the player's pins. Only `cap_spawn`'s player gate reaches
/// this, so the enemy keeps the pin-free build. Nothing is written unless the
/// athlete still holds exactly the build the hook recorded and every item of
/// the pin-aware one resolves.
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

/// The player's pin for build slot `slot`, as a catalog index, for `cap_spawn`.
unsafe fn spawn_pin_at(champ: &str, cat_base: usize, cat_len: u64, slot: usize) -> Option<u64> {
    slot_n_catalog_index(champ, slot as u8, |key| {
        scan_catalog_index(cat_base, cat_len, key)
    })
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
            cap_spawn as *const () as usize,
            &SPAWN_PROLOGUE,
        )
    };
    SPAWN_INSTALLED.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    crate::own_team_log::line(|| format!("spawn hook install @ {SPAWN_RVA:#x}: {r:?}"));
}

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
// `scene: &mut Scene` became `in_game: bool` plus the `StableClient` itself,
// which answers what `Scene::InGame { data }.db()` used to. The `ui`, `_assets`
// and `_dt` parameters went with the code that used them.
fn tactics_post_update(client: &mut StableClient<'_>, in_game: bool) {
    {
        install_launcher_hook();
        install_seed_ctor_hook();
        install_spawn_hook();
        // * Capture the player team id, for team scoping.
        // (was `if let Scene::InGame { data } = scene`)
        //
        // `data.db()` returned `mod_api::ClientDatabase` — the *client* scene's
        // database, which a stable-ABI mod cannot be handed. What this block
        // read off it is read from the stable client instead;
        // `stable_last_starting` is the JSON-record equivalent of
        // `team.last_starting`.
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
            // (2026-09-24: or a lane/5v5 test by the route call's `mode` —
            //  `COMPTEST_MATCH` hangs on 0.5.3 launcher retaddrs that were never
            //  re-derived; see `build_config::training_match`.)
            let in_comptest =
                COMPTEST_MATCH.load(Ordering::Relaxed) || crate::build_config::training_match();
            let pu = pid as u64;
            // * Diagnostic: pid observation history. ** Confirmed by measurement (2026-07-30) - **from the user's point of view comp test is a
            //   background brief-sim, but under the SDK `Scene` enum it is `InGame`** (proved by LIVE_DB != 0, i.e. this block did run),
            //   and `player_team_id()` in that context returns **0**. That is the source of the pid=0 publications.
            // * Second extension (2026-07-30 measurement): `COMPTEST_MATCH` is only true **after the comp-test sim starts** (after the launcher
            //   fires), so 0 reports from the window **between entering the comp-test screen and the sim starting** leaked through and got published
            //   (measured: of 2416 observations of 0, only 1592 were blocked and the remaining 824 came from that window). => while the comp-test popup
            //   is open (`CT_OPEN`) treat it as the same context and ignore 0 as well.
            // ** `CT_OPEN` went with the comp-test screen handler that set it, so
            //   the "popup open but sim not started" window is no longer detected
            //   and those pid=0 observations are treated as clean again. Harmless
            //   here: a 0 is only *published* when no non-zero pid has ever been
            //   seen (see the rule below), and this half no longer reads personal
            //   tactics at all — the pid is used for the starter roster, which a
            //   comp test does not have.
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
            // ** v15: publish my team's athlete_ids - the material for the spawn hook's scene-free team decision.
            //   Everyone under contract (`roster_scan_step`, published the frame a pass finishes) plus the last starting five,
            //   refreshed on the ROSTER_POLL period (transfers and lineup changes are picked up automatically).
            {
                const ROSTER_POLL: u64 = 120; // frames
                let n = ROSTER_TICK.fetch_add(1, Ordering::Relaxed);
                let known = PLAYER_TEAM_ID.load(Ordering::Relaxed);
                let valid = known != u64::MAX && known < 10000;
                // Every frame: the contract scan reads a few records at a time.
                let scanned = valid && roster_scan_step(client, known as usize);
                if valid && (n % ROSTER_POLL == 0 || scanned) {
                    // (was `db.team(known).last_starting` / `.champion_personal_tactics`)
                    // The whole contracted roster, plus the last starting five
                    // as a floor: see `roster_scan_step` for why the starters
                    // alone missed the first match of a new save.
                    let starting = stable_last_starting(client, known as usize);
                    let roster = contracted_roster(known as usize);
                    let mut my = starting.clone();
                    my.extend(roster.iter().copied());
                    // * `pid=0` is **treated as undetermined and withheld by default** (withheld = is_my_athlete returns None
                    //   = the team gate closes on the safe side). But if 0 has been observed **long enough (600 ticks, ~10s) in an InGame unrelated to
                    //   comp test**, accept it as a genuine team-id-0 save and publish.
                    //   => it is never published in a comp-test-only session, and playing a normal match captures the real pid.
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
            // ** lean (07-18): spectate identification = launcher (LIVE_SEED) + seed-ctor (RENDER_PROVIDER) + buy r9 comparison (v13).
            //   The old db scan (v10), P6 probe and link scan are all gone. Only the scene side (my-team decision) and LIVE_DB/PID remain here.
            if !DIAG_BUY_OFF {
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
                // Restoring it needs the `ClientDatabase` address from somewhere,
                // such as another detour argument.
            }
        }
    }
}

// Server side. (Was `impl ModServerExtension for ItemTacticsServerExt`. Driven
// from the host mod's `StableServerExtension` — see `driver` and `src/lib.rs`.)
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
//  Stable-ABI replacement for the `ClientDatabase` read
// ===========================================================================
// `Scene::InGame { data }.db()` gave a `mod_api::ClientDatabase`, whose `team()`
// returned a struct the starting five were read straight off. The stable client
// exposes the same management records as JSON documents instead, so the shape
// of the answer is unchanged and only the route to it differs.
//
// It is called from a throttled path (the roster, every 120 frames), which is
// what makes a JSON round-trip per call acceptable where a field read was
// before.

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

// ── The contracted roster ─────────────────────────────────────────────────
//
// Athlete ids of everyone under contract with the player's team — starters,
// subs and academy alike — read off each athlete record's `contract`.
//
// `last_starting` alone is empty until the team has played a match, so on a
// new save the first match spawned before anything was published: the log of
// 2026-09-29 read `last_starting=[]` from load until the match began, and
// published the starters two seconds after the spawn, by which time every
// athlete had bought its first item from the engine's build. A contract is
// there from the moment the save loads, and it also covers a newly signed
// starter, who is not in `last_starting` until he has played.
//
// `contract` is an enum, `FreeAgent { requests }` or `InContract { team_id,
// start_date, end_date, weekly_salary, transfer_fee, incentives,
// transfer_requests, recruit_requests }` (the serde tables in the 0.6.2 exe),
// so the plain path `contract.team_id` matches nothing — the first build of
// this read 0 of 1,065. The fragment is read whole instead and its first
// `team_id` taken ([`contract_team_id`]), which holds however serde tags the
// enum: fields serialize in declaration order, so the contract's own team
// comes before any `team_id` inside its transfer requests.
//
// A read costs 0.1-0.2 ms (that same one-shot build: 1,065 in 114 ms, a
// visible hitch), so a pass is spread over frames, [`ROSTER_SCAN_BATCH`] at a
// time, and repeated only when the in-game day changes (signings land on day
// ticks) and at most every [`ROSTER_RESCAN_FRAMES`], so a fast-forward that
// ticks the date every second does not keep one running.

/// Athlete records [`roster_scan_step`] reads per frame. Measured on 0.6.2 at
/// 32 a frame: 1,065 contracts in 191 ms over 34 frames, ~5.6 ms a frame. At
/// 16 that is under 3 ms a frame and about a second for a whole pass.
const ROSTER_SCAN_BATCH: usize = 16;
/// Frames (~10 s) a finished pass waits before a day change starts another.
const ROSTER_RESCAN_FRAMES: u64 = 600;

type GameDay = Option<(i32, u32, u32)>;

#[derive(Default)]
struct RosterScan {
    team: usize,
    /// The athlete records this pass walks, and how far it has got.
    ids: Vec<usize>,
    next: usize,
    found: std::collections::HashSet<u64>,
    /// Time spent reading, summed over the frames of this pass.
    work: std::time::Duration,
    frames: u64,
    /// One contract fragment, logged once so the shape can be checked.
    sample: Option<String>,
    /// The last finished pass, and what has happened since.
    done: Option<std::collections::HashSet<u64>>,
    day: GameDay,
    idle_frames: u64,
}

static ROSTER_SCAN: Mutex<Option<RosterScan>> = Mutex::new(None);

/// Advances the contracted-roster scan for `team_id` by one frame. True on
/// the frame a pass finishes, so the caller can publish at once rather than
/// on its next poll.
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

/// The last finished pass of [`roster_scan_step`] for `team_id`; empty until
/// one has finished.
fn contracted_roster(team_id: usize) -> std::collections::HashSet<u64> {
    let guard = ROSTER_SCAN.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .filter(|scan| scan.team == team_id)
        .and_then(|scan| scan.done.clone())
        .unwrap_or_default()
}

/// The team a serialized `contract` is with: its first `team_id`, or `None`
/// for a free agent (whose `requests` can hold other teams' ids).
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
// ** 0.5.5 re-derivation (2026-08-11). Strict exe2exe gives **0 hits** and `--loose` gives exactly **1**, at a
//   function start of identical size 230 — which is itself the finding: the only bytes that changed are struct
//   displacements, i.e. the athlete moved. Disassembled side by side the two are 67 instructions each with zero
//   mnemonic mismatches and a single differing operand, `cmp qword [r8+0x490],0` -> `cmp qword [r8+0x4f0],0`.
//   That one instruction is the anchor the whole athlete layout below hangs off, because r8 is the athlete by
//   the argument contract. The prologue and the 19B relocation boundary are both unchanged.
// ** 0.5.7 (2026-08-26): exe2exe **strict** (no --loose), 1 hit, size 230 and 67 instructions on both sides.
//   That it matched strict is itself evidence the athlete struct did not move — `--loose` exists because a
//   moved struct defeats a strict match. `pairdiff --min-disp 0x4` then confirms it directly: zero differing
//   displacements in buy_item and in all three of its callees (0xdf3a70, 0xdf5580, 0xde0120), so the build
//   Vec, the items Vec and the id are all where they were. orig_len=19 unchanged.
// ** 0.6.3 re-derivation (2026-10-06): the function was rebuilt, not moved. exe2exe finds nothing,
//   strict or loose; the 25-byte thunk that forwarded to it is gone; the body is no longer 230 bytes.
//   What did not change is who calls it. `run_tick` still does `call [vtable+0x80]` on the same trait
//   object, and that vtable is found by its layout (drop, size, align 0x10, two methods, then the two
//   supertrait pointers at vtable-0x40 and vtable-0x20): 0x3afa198 on 0.6.2, **0x3b08bd8** on 0.6.3.
//   Slot +0x80 is now a bare `jmp` at 0xf61b80 to **0xf3f510** (317 bytes), and nothing else in the
//   image refers to that function. Its body is the old one behind a longer prologue:
//     mov r13,[rsp+0xc8]            8 pushes + sub rsp,0x58 -> entry+0x30 = Game (unchanged slot)
//     cmp qword [rsi+0x360],0       rsi = r8 = athlete, its build len (was [r8+0x560])
//     mov rax,[r13+0x30] / mov rbx,[rax+8] / mov r14,[rax+0x10]    the catalog
//     three callees of 709/917/1069 bytes (were 691/1007/1069), then the same
//     `call [rax+0x70]` / `sete al` tail with the index in rdx
//   r9 is still the provider: it is the receiver of the two `call [arg5+0x20]`/`[arg5+0x28]` at the
//   top, and `run_tick` loads it from the slot it loaded 0.6.2's from. So everything the detour
//   reads holds (r8 = athlete, r9 = provider, [rsp_entry+0x30] = Game, Game+0x30 = catalog, rax/rdx
//   out); only the prologue differs.
//   NOT this function: 0xea1670 (189 bytes) has the same `cmp qword [r8+0x360],0` head, but takes
//   the Game through r9 and returns a bare bool. It is the AI's "is there anything to buy" test,
//   new in this build, called from three AI functions and never from `run_tick`.
const RVA_BUY_ITEM: usize = 0xfd1b30; // 0.7.0-beta (2026-10-07: exe2exe strict unique, fn start, size 317 and 96 instructions both sides, prologue byte-identical; vtable 0x3e5dd48 +0x80 -> jmp 0xffa590 -> here, still the only reference. pairdiff at --min-disp 0x4 --imm: one displacement moved, `[r14+0x4448]` -> `[r14+0x44a8]`, a field of the AI object in rcx that nothing here reads; buy_item's resolver and the AI predicate pair clean with none). 0.6.3 was 0xf3f510 (2026-10-06: see the note above; vtable 0x3b08bd8 +0x80 -> jmp 0xf61b80 -> here, size 317). 0.6.2 was 0xe82e40 (2026-09-29: exe2exe strict unique, fn start, size 230 both sides; buy_item and all three callees pairdiff clean at --min-disp 0x4 --imm, no struct offset moved). 0.6.1 was 0xeae130 (2026-09-21: exe2exe strict unique, fn start, size 230 both sides, pairdiff: no struct offset moved). 0.6.0 release was 0xf3d570 (0.6.0-beta2 was 0xfe23b0; exe2exe unique, fn start, size 230 both sides, prologue byte-identical) (0.6.0-beta was 0xf33680, 0.5.7 0xdf5490, 0.5.6 0xebca20, 0.5.5 0xeb2c40, 0.5.4 0xe767e0, 0.5.3 0xd0c680). History for 0.5.3 follows.(0.5.2 was 0x211e070). **The first 24B of the entry are byte-identical** (a single unique hit in the whole exe) + the body is instruction-for-instruction isomorphic + the argument contract is unchanged (r8=athlete, [rsp_entry+0x30]=Game, Game+0x30=catalog). orig_len=19 is unchanged too (11B < 12B -> the next clean boundary is the 8B mov rax,[rsp+0xa8]). WARNING 0.5.3 change: the call path became a vtable (+0x78) thunk 0xd22340 instead of a direct call, but **since we hook the function entry, every call is still caught**. History for 0.5.2 follows. (0.5.1 was 0x1f01090; exe2exe skeleton UNIQUE, the 24B prologue completely identical = body unchanged, delta +0x21cfe0.) History for 0.5.1 follows: the function was heavily reworked (8 push/sub 0x38 -> 5 push/sub 0x50, with build/name comparison split out into the subfunction 0x1f00920) so mask-sig was NONE, but it was confirmed by the unchanged argument contract (r8=athlete, p6=Game@rsp_entry+0x30, Game+0x30=catalog). Cross-checked against the buy driver FUN_142234430 (successor to the old FUN_1420e76e0) + the vtable slot.
const BUY_PROLOGUE: [u8; 12] = [
    0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53,
]; // 0.6.3: push r15/r14/r13/r12/rsi/rdi/rbp/rbx = 12B, a clean boundary and all the trampoline relocates (`install_replace_4th`). Through 0.6.2 this was 41 57 41 56 56 57 53 48 83 EC 50 48, the first 12B of the 0.5.1 prologue: push r15/r14/rsi/rdi/rbx; sub rsp,0x50; (11B = a clean boundary) + the first byte of the following mov (0x48...). Trampoline relocation = 19B (next clean boundary = + mov rax,[rsp+0xa8])
static BUY_PROBE_INSTALLED: AtomicU64 = AtomicU64::new(0);

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
// ** 0.5.5 (2026-08-11): exe2exe unique, size 1609 on both sides, and `delta.py` reports no differing struct
//   displacement anywhere in the body — so the net layout the per-call re-validation checks (net+0x8 weight ptr,
//   +0x10 bound, +0x18) is untouched and that logic stays valid as-is.
// ** 0.5.7 (2026-08-26): exe2exe unique, size 1609 and 409 instructions both sides, `pairdiff` clean, so the
//   net layout (net+0x8 = weight ptr, +0x10 = 16384 bound, +0x18 = 1) is unchanged and the per-call
//   re-validation logic stays valid as-is.
// ** 0.6.3 (2026-10-07): 0x1932130, back in service for the 5th and 6th item (`network_pick`). It was not
//   migrated for 0.6.0 to 0.6.2, while nothing could reach it. Found from the item-build hook's target: that
//   calls one 6.3KB beam search (0x1932b40), and this is the beam search's scoring callee, 0xa10 below it as
//   in every build, still 1609 bytes, with the same five feature names. Its arguments are unchanged too:
//   rcx net, rdx lineup ([rdx+0x50] = lane, a panic above 4; [rdx+lane*8] = the champion; +0x28 on = the
//   five enemies, 9999 = nobody), r8/r9 the build slice, [rsp+0x20] a flag that adds noise when set. The
//   net's constructor (0x196a2c0) still writes 16384 / ptr / 16384 / 1, though only a dword of that 1.
//   xmm0 is the score; 0.6.3 also hands back p(1-p) in xmm1, which nothing here reads. A second function
//   (0x1931cd0) builds the same features to TRAIN the weights, so a score is only good for the moment it
//   was asked for: see `NETWORK_PICKS`.
const ITEMNET_FORWARD_RVA: usize = 0x1ad8080; // 0.7.0-beta (2026-10-07: exe2exe strict unique, fn start, size 1609 and 409 instructions both sides, pairdiff clean at --min-disp 0x4 --imm; still called only by the beam search, 0x1ad8a90, which only the hook.rs target calls, and the net's constructor, 0x1b296e0, pairs clean as well). 0.6.3 was 0x1932130 (0.6.0-beta2 was 0x1228050, 0.6.0-beta 0x12462b0, 0.5.7 0x17f09b0, 0.5.6 0xf53de0, 0.5.5 0x12624f0, 0.5.4 0x145a680, 0.5.3 0x10587e0). History for 0.5.3 follows. (0.5.2 was 0x1b9cce0). The first 24B of the entry are identical + all 5 feature-name strings match (self_item/champ_pos_build/lane_counter/synergy/global_counter) + the net layout is unchanged (net+0x8 = weight ptr, +0x10 = 16384 bound, +0x18 = 1) => the mod's per-call re-validation logic stays valid as-is. History for 0.5.2 follows. (0.5.1 was 0x1bc82e0; exe2exe UNIQUE, identical prologue.) History for 0.5.1 follows: (0.5.0_3 was 0x1b78420, mask-sig UNIQUE PROL-OK push8 554157415641554154565753). WARNING it was OFF via AUTO4_FORWARD_SCORE=false (an AV at +0x44a inside forward on 0.5.1; see the flag comment above). A matching prologue does not imply identical internals.
/// The eight pushes and `sub rsp, 0xd8`. The pushes alone open thousands of
/// functions; the frame size is what makes a stale address fail this.
const ITEMNET_FORWARD_PROLOGUE: [u8; 19] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0xd8,
    0x00, 0x00, 0x00,
];
type ItemNetFn = unsafe extern "C" fn(usize, usize, *const u64, u64, u8) -> f32;
/// The agent `hook::detour` was last handed, exactly as it came: the network
/// the 5th and 6th item are scored with ([`network_pick`]), which proves it
/// before every pick ([`network_ready`]).
static NETWORK_AGENT: AtomicU64 = AtomicU64::new(0);
/// Most weights [`network_ready`] accepts. The network has 16384.
const NETWORK_WEIGHTS_MAX: usize = 1 << 20;
/// Whether `net` can be handed to `itemnet_forward`: a weight array that is
/// there to be read, for as many weights as the network says it has. The
/// function checks every index against that count itself, so nothing else
/// about the network can make it read out of bounds.
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
const SHADOW_CALL_NAMES: bool = true; // name of a ctx+0x20 element = calling vtable[0x50] (AV risk, hence the gate)

// Item game id -> name key (0~29 = vanilla, 30+ = mod items). Used to scan names in the ctx+0x20 collection.
/// A start offset spread deterministically by champion name, so a rule that
/// walks a candidate list does not hand every champion the same answer.
///
/// FNV-1a over the name: the same champion always gets the same offset, which is
/// what keeps a replayed match identical to the one that was played.
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

/// First free final item that `matches`, starting from a champion-spread offset
/// so the whole league does not converge on one stand-in.
///
/// "Free" is both unclaimed by the earlier build slots (`taken`) and actually
/// present in this match's catalog with a recipe — which is what the scan
/// proves and an id alone does not.
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

fn item_id_to_key(id: u64) -> Option<String> {
    if (id as usize) < VANILLA_KEYS.len() {
        return Some(VANILLA_KEYS[id as usize].to_string());
    }
    let reg = MOD_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    reg.get((id as usize).checked_sub(30)?).cloned()
}
// * 0.5.0 build extension: RVA_REALLOC (the real function 0x25a56c0) confirmed -> ON. Real purchases via the buy build Vec 3->4 are back.
// ** OFF for game 0.6.0 (2026-09-16) -- this crashed users mid-match. `RVA_REALLOC` is still the beta2
//    address and was never re-derived. On the release image 0x2f1b320 is unrelated SIMD code, so the call
//    faults (ACCESS_VIOLATION at exe+0x2f1b2c0, return address riot_items_tfm2+0x389e5, args ptr/0x18/8/0x20).
//    f8f71ad made the path reachable: it replaced the `slot_count() != 4` early return with `picker_slots()`,
//    which is always 4, and dropped the separate `!BUILD_EXTEND_ENABLED` return. The game allocates
//    four slots itself now, so only the in-place write is needed. Re-derive RVA_REALLOC before turning this back on.
// ** ON again (2026-09-18) for the 5th and 6th slots. RVA_REALLOC was re-derived against the release
//    (0x2f23bf0, exe2exe unique at the same 174-byte size) and every call now goes through `realloc_ok`,
//    which checks the entry bytes first -- the check whose absence turned the stale address into a crash.
//    The Vec grows from whatever the engine built (normally 4) to `build_config::picker_slots()`.
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
// `item-builds.json` — written by the host mod's `#builds` editor — is the only
// source these consult. Each used to try the pin first and fall back to a `SEL`
// dropdown designation; `SEL` is gone, so the pin is the whole answer and the
// `scope` argument that disambiguated per-side comp-test selections went with
// it.
//
// Which slots reach here depends on the editor's scope toggle. The 5th and 6th
// always do: the engine's build `Vec` is four long until this half grows it, so
// the stable hook cannot deliver them. The game's own four reach here only
// under `own_team_only`, where the team scoping this side can do is the whole
// point; otherwise `crate::item_build_hook` sets them before the match.

/// The pinned item key for one build slot, normalized (radiant + alias) the way
/// the item catalog is keyed.
fn slot_n_item_key(champ: &str, si: u8) -> Option<String> {
    crate::build_config::pinned_key(champ, si as usize)
}

/// Catalog index of a slot's pinned item, looked up by key through `lookup`
/// (a name scan of the live catalog).
///
/// Never by id. This used to short-cut vanilla items as "id == catalog index",
/// but the catalog grows and reorders with the enabled mods and the save, so a
/// position in `VANILLA_KEYS` names whatever happens to sit there — often a
/// component with no recipe, which the game then never builds. Every item goes
/// through the scan, the way the stable hook goes through
/// `StableItemBuildContext::item_index`.
///
/// Tries keys in `build_config::resolve_key`'s order: the normalized key first
/// (`radiant_` + alias, so `"bloodthirster"` finds `warlords_final_judgement`),
/// then the key exactly as written, which is how a game-internal key like
/// `"warlords_final_judgement"` resolves.
fn slot_n_catalog_index(champ: &str, si: u8, lookup: impl Fn(&[u8]) -> Option<u64>) -> Option<u64> {
    let raw = crate::build_config::pinned_key_raw(champ, si as usize)?;
    slot_n_item_key(champ, si)
        .and_then(|key| lookup(key.as_bytes()))
        .or_else(|| lookup(raw.as_bytes()))
}

// * The 4th target = scan the catalog (ctx+0x20) by name to get the index + validate the recipe. (Mod items need a name scan because id != index.)
//   catalog = the same array the resolver indexes (RE confirmed). element{elem_ptr@0, vtable@8}, name = vtable[0x50],
//   has_recipe = calling vtable[0x68] (!=0 = has a recipe). Without a recipe the game panics in FUN_141d5ab40 -> always validate before use.
//   Returns = the catalog index of a valid final item that has a recipe (usable directly in build[3]). None otherwise (vanilla fallback).
// * For the spawn hook (v14): a scan that takes the catalog base/len directly (Game+0x1fd0/+0x1fd8). Cache key = len (see `SCAN_CACHE`).
//   Same index space as the buy path (the ctx+0x30 collection) - a build[] value *is* this index, so the resolver consumes it as-is.
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
// Shared scan core: find the index in the catalog array (element{elem_ptr@0, vtable@8}, stride 0x10) whose name matches and which has a recipe.
//
// ** Every read is VEH-guarded, through `catalog_entry_named` (2026-09-26, the
//   teamfight lag the buy memo only partly fixed). This loop used to prove each
//   entry with `readable`/`code_ptr_ok` first: up to four `VirtualQuery` syscalls
//   an entry at ~5.9 us each, so ~6 ms to walk a catalog of ~250, on every lookup
//   that missed `SCAN_CACHE`. A full buy pass makes dozens of lookups (every
//   `pick_candidate` step is one), and every sim copy of a match used to start
//   with a cold cache — see `SCAN_CACHE`.
unsafe fn scan_recipe_safe_in(data: usize, len: u64, want: &[u8]) -> Option<u64> {
    if want.is_empty() || data < 0x10000 || len == 0 || len > 100000 {
        return None;
    }
    let i = (0..len).find(|&i| catalog_entry_named(data, len, i, want))?;
    // * Recipe validation: calling vtable[0x68] must return !=0 for natural build-up to be safe (0 = a base item -> panic).
    //   Only the first entry with the name counts: a name match without a recipe falls back rather than looking further.
    let e = data + (i as usize) * 16;
    let edata = safe_read_u64(e)? as usize;
    let evt = safe_read_u64(e + 8)? as usize;
    let recfn = safe_read_u64(evt + 0x70)? as usize; // 0.5.1: the next_tier/recipe getter slot moved +0x68 -> +0x70 (ghidra-re)
    if !name_getter_ok(recfn) {
        return None;
    }
    let rf: unsafe extern "win64" fn(usize) -> usize = core::mem::transmute(recfn);
    (rf(edata) != 0).then_some(i)
}

// * Performance: scan cache (name -> index). Reduces the 96-element shadow-call scan to once per name. Value -1 = not found / no recipe.
//
// A catalog index is only meaningful against the catalog it was read from. The
// list grows and reorders with the enabled mods, and a save load or a new match
// can rebuild it at the address the old one had, so an address match is not
// proof the index still names the same item. This cache used to trust that, and
// a stale hit put the wrong item — or one with no recipe — into a build slot,
// which the game then never built. Every positive hit is re-read by name before
// it is returned, the way `StableItemBuildContext::item_index` looks items up by
// key on every call; a mismatch falls through to a fresh scan.
//
// ** Keyed by the catalog's length, not its address (2026-09-26). Every `Game`
//   builds its own catalog array, and a match plays in several sim copies (the
//   player's league match was seen buying in 4 providers), so the old
//   (base, len) key gave each copy a cold cache, and more than 16 live catalogs
//   (league fixtures x copies) cleared it outright. The copies of one session
//   are built from one item list, so their answers are the same; the checks
//   above are what make that safe to rely on. A hit is re-read by name in the
//   catalog asking, and a miss is trusted only while that catalog's last entry
//   has the name the cache started with. Two catalogs of the same length but a
//   different item set would only cost rescans: a failed check forgets the lot.
static SCAN_CACHE: Mutex<Option<HashMap<u64, CatalogCache>>> = Mutex::new(None);

/// Cached lookups against every catalog of one length.
struct CatalogCache {
    /// Name of the catalog's last entry when this cache was started — what a
    /// cached miss is checked against.
    last: Option<String>,
    /// Key -> catalog index, `-1` = not found / no recipe.
    found: HashMap<Vec<u8>, i64>,
}

/// A cached answer for `want` in this catalog, re-checked against it.
///
/// `Some(Some(i))` is a verified index and `Some(None)` a cached miss; `None`
/// means there is nothing usable and the caller has to scan.
///
/// A miss is re-checked more cheaply than a hit, because proving absence would
/// be the full scan again: it is trusted only while the last entry still has the
/// name it had when the cache was started. A rebuilt catalog with a different
/// item set essentially never keeps its last entry. Any failed check drops every
/// cached answer for the catalog, so one stale hit cannot leave stale misses
/// behind.
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

/// Whether catalog entry `idx` is named `want`.
///
/// The re-check behind every cache hit, so it runs on the buy hot path — the
/// fallback 4th-item search can make dozens of lookups per decision, for every
/// athlete. It therefore reads through the VEH-guarded `safe_read_*` rather
/// than [`catalog_name_in`]'s `readable`, which is a `VirtualQuery` syscall per
/// check. The one thing a protected read cannot prove is that the name getter is
/// code, so each getter is validated once with `code_ptr_ok` and remembered;
/// the catalog holds only a handful of item types.
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

/// Getter addresses [`catalog_entry_named`] and [`scan_recipe_safe_in`] have
/// already proven are code. Sixteen, not eight, since the recipe getters share
/// it: a name getter pushed out of the ring costs a syscall on every entry of
/// the next scan.
static NAME_GETTERS: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];
static NAME_GETTER_NEXT: AtomicUsize = AtomicUsize::new(0);

/// Whether `namefn` is a catalog getter (the name getter, or the recipe getter
/// `scan_recipe_safe_in` calls) proven to be code, proving and remembering it
/// on first sight: `code_ptr_ok` is a `VirtualQuery` syscall, and the catalog
/// holds only a handful of item types. The low-address test comes first
/// because an empty [`NAME_GETTERS`] slot is 0, and a null getter must not
/// match one.
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

/// VEH-guarded reads, not `readable`: the candidate searches call this once
/// per candidate, and two `VirtualQuery` syscalls each made a 5th/6th-slot
/// growth pass cost milliseconds (see [`catalog_name_at`]).
unsafe fn scan_idx_cached(ctx: usize, want: &[u8]) -> Option<u64> {
    if ctx < 0x10000 {
        return None;
    }
    let coll = safe_read_u64(ctx + 0x30)? as usize; // * 0.5.0: the catalog collection offset moved ctx+0x20 -> +0x30 (RE confirmed, the only change)
    if coll < 0x10000 {
        return None;
    }
    scan_catalog_index(
        safe_read_u64(coll + 8)? as usize,
        safe_read_u64(coll + 0x10)?,
        want,
    )
}

// ** fix B (2026-07-27): spectate == final. The is_live early exit was removed -> inject in background matches too, with the team scope = is_my_athlete (+0x810).
//   My players get designated items / everyone else gets the network, identically in background and spectated sims -> they converge. Being id-based, AI-vs-AI matches have my=0 = no designation = zero statistical contamination.
//   WARNING false = restores the old behaviour (is_live gate, no background injection). Kept for an immediate rollback on trouble.
const FIXB: bool = true;

/// What goes into build slot `si` when the Vec grows -- `si` 4 and 5 are the
/// 5th and 6th items, and 3 the 4th of the rare build the engine left three
/// long.
///
/// The team gate is `designate` in `buy_replace_ctx`: a designated athlete
/// gets its `item-builds.json` pin, and everyone else, or a slot left blank
/// in the editor, gets [`auto_extra_pick`]. There is no engine pick to fall
/// back on -- the engine never plans these slots -- so an excluded athlete
/// still gets them filled, just not with the player's pins.
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
            auto_extra_pick(ctx, buyer, champ, si, taken, &reserved)
        })
}

thread_local! {
    // Which of the ways above and below filled the slot `extra_slot_pick` was
    // last asked for on this thread. Only read back by the growth that asked,
    // for the Check Tactics panel's test log.
    static PICKED_BY: core::cell::Cell<&'static str> = const { core::cell::Cell::new("") };
}

fn picked_by(how: &'static str) {
    PICKED_BY.with(|picked| picked.set(how));
}

/// Smart Builds' boots for build slot `si` (the 5th or 6th), in a build that
/// has none yet: the rule (`smart_builds::enforce`) leaves them to the first
/// open slot here when a pin holds every slot from the second to the fourth.
/// A slot the player pinned is not open, and a pair the player pinned
/// anywhere is the build's only pair. The pair is the one the rule picked for
/// the champion, which saw the enemy lineup this path does not.
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

/// The pinned item for build slot `si`, as a catalog index. Planted exactly as
/// written: Smart Builds only ever rewrites the AI's picks, never the player's.
unsafe fn pinned_extra_slot(ctx: usize, champ: &str, si: usize, taken: &[u64]) -> Option<u64> {
    slot_n_catalog_index(champ, si as u8, |key| scan_idx_cached(ctx, key))
        .filter(|&t| !pin_placed_by_engine(ctx, champ, t, taken))
}

/// Whether pinned item `t` already sits in one of the earlier slots `taken`
/// because the ENGINE put it there, not the player. Under `own_team_only` slot 0
/// (and any blank slot) is the engine's pick, made without seeing the pins, so it
/// can be the very item pinned later. Planting the pin anyway builds it twice;
/// it is honoured already, just sooner, so the later slot is freed for an
/// automatic pick instead. A slot whose own pin is `t` is the player duplicating
/// on purpose, and that is still planted as written.
///
/// Boots count as placed when the engine holds any pair: one pair per build,
/// and by the time a 4th-6th slot is planted the engine's is usually bought.
unsafe fn pin_placed_by_engine(ctx: usize, champ: &str, t: u64, taken: &[u64]) -> bool {
    let pin_boots = buy_is_boots(ctx, t);
    taken.iter().enumerate().any(|(j, &v)| {
        (v == t || (pin_boots && buy_is_boots(ctx, v)))
            && slot_n_catalog_index(champ, j as u8, |key| scan_idx_cached(ctx, key)) != Some(v)
    })
}

/// Whether catalog index `index` is a pair of boots, on the buy path.
unsafe fn buy_is_boots(ctx: usize, index: u64) -> bool {
    catalog_name_at(ctx, index).is_some_and(|name| crate::smart_builds::is_boots(&name))
}

/// The player's pins for build slots `from` onward, as catalog indices — slots
/// not planted yet but already spoken for, which an automatic pick for an
/// earlier slot must not duplicate or crowd out.
unsafe fn later_pins(ctx: usize, champ: &str, from: usize) -> Vec<u64> {
    (from..crate::build_config::picker_slots())
        .filter_map(|si| slot_n_catalog_index(champ, si as u8, |key| scan_idx_cached(ctx, key)))
        .collect()
}

/// What the build slots before this one have spent of the Smart Builds budgets.
/// An index the catalog scan cannot name contributes nothing — the same way an
/// unclassifiable item is passed over on the other two paths.
///
/// `champ` decides what the champion may hold at all (support items, what it
/// scales with); its role is the one the last lineup gave it, since this path
/// is not told the lane.
unsafe fn spent_budget(ctx: usize, champ: &str, taken: &[u64]) -> crate::smart_builds::Budget {
    let keys: Vec<String> = taken
        .iter()
        .filter_map(|&index| catalog_name_at(ctx, index))
        .collect();
    let fit = crate::smart_builds::fit(champ, crate::build_config::role_for_champion(champ));
    crate::smart_builds::Budget::spent(keys.iter().map(String::as_str), fit)
}

/// The athlete a build slot is being filled for, and the seed of its match.
#[derive(Clone, Copy)]
struct Buyer {
    athlete: usize,
    seed: u64,
}

/// The automatic 5th and 6th item: what the game's own item network wants
/// most on top of the build so far ([`network_pick`]), the way the engine
/// arrives at the four it plans itself.
///
/// No slot decides it. Until 2026-10-07 the 5th copied the category of the
/// 1st item and the 6th that of the 2nd, which since Smart Builds' boots rule
/// is a pair of boots: Ionian Boots of Lucidity are a Magic item, so a Hunter
/// and a Dual Blader went looking for their 6th among the mage items. The
/// user's call was to drop the matching altogether rather than move the
/// anchor: the 5th and 6th are picked the way the AI picks.
///
/// Smart Builds still has the last word on what may be picked: always on
/// what suits the champion, and on the build as a whole while the toggle is
/// on. A support its rule 17 has looking to the heal, shield and buff items
/// first is asked about those alone before the rest.
/// Should the network be out of reach (its address not found after a game update, or no
/// build asked for yet this session) the slot goes to the first final the
/// rules accept, from a start spread by champion, and only then to any final
/// at all. Never a duplicate: `taken` is every slot before this one, and
/// `reserved` the player's pins for the slots after it, which count exactly
/// as if placed.
unsafe fn auto_extra_pick(
    ctx: usize,
    buyer: Buyer,
    champ: &str,
    si: usize,
    taken: &[u64],
    reserved: &[u64],
) -> Option<u64> {
    // `taken` stays the build in slot order, for the network; everything else
    // works from the whole build, pins still to come included.
    let spoken = [taken, reserved].concat();
    // What the champion may hold at all, toggle or not. The engine's own
    // search only offers a champion the finals whose tags match its own, and
    // these two slots are picked on its behalf; an item it could not keep
    // under the rules is the nearest thing here to one the engine would never
    // have offered. The same line `item_build_hook::score_item` draws.
    let fit = crate::smart_builds::fit(champ, crate::build_config::role_for_champion(champ));
    let suits = crate::smart_builds::Budget::empty(fit);
    // Smart Builds applies to a pick the mod made as much as to a pinned one: a
    // 5th item that cuts healing a second time, or pushes the build past the crit
    // cap, is the same wasted slot either way. `None` while the toggle is off,
    // which leaves the test above as the only one.
    let budget =
        crate::build_config::smart_builds_enabled().then(|| spent_budget(ctx, champ, &spoken));
    let allowed = |candidate: &str| {
        suits.rejects(candidate).is_none()
            && budget
                .as_ref()
                .is_none_or(|budget| budget.rejects(candidate).is_none())
    };
    // Smart Builds rule 17: a support that looks to the heal, shield and buff
    // items first takes these two slots from them while one is left, picked
    // the same way, by the network, among those alone. Only with the toggle
    // on: it is a preference among picks, not a line on what the champion
    // may hold.
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
        // Last resort, unconstrained but for the one rule that holds even
        // here while the toggle is on: a 5th item that breaks a rule still
        // beats an empty slot, a jungle item on a champion that is not
        // jungling does not. With the toggle off it is any final at all.
        .or_else(|| {
            pick_candidate(ctx, u64::MAX, &spoken, champ, |candidate| {
                budget.is_none() || !fit.off_role_jungle_item(candidate)
            })
            .inspect(|_| picked_by("last resort, no rule held"))
        })
}

/// What the item network's lineup table holds for a seat nobody is in.
const NET_NO_CHAMPION: u64 = 9999;

/// The network's picks so far: `(seed, team, lane, slot)` to item key.
///
/// The network learns. `0x1931cd0` builds the same features `itemnet_forward`
/// scores and moves the weights, so the same question can get a different
/// answer an hour later, and a match is played more than once: in the
/// background for its result and again on screen, each copy buying for itself
/// (see [`FIXB`]). What a seat was given the first time is what it gets every
/// time after, or the match the player watches stops being the one that was
/// recorded. By key, because a catalog index is only good for the catalog it
/// was read from.
static NETWORK_PICKS: Mutex<Option<HashMap<(u64, u64, u64, u64), String>>> = Mutex::new(None);

/// Seats [`NETWORK_PICKS`] holds before it starts over: about eight hundred
/// matches of ten athletes and two slots.
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

/// Drops every remembered pick: another save's matches reuse seeds.
fn network_picks_forget() {
    *NETWORK_PICKS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The final the game's item network scores highest as this athlete's next
/// item, among those `allowed` and not in `spoken`. `None` when the network
/// cannot be asked.
///
/// This is one step of the engine's own search. `get_item_builds_list` runs a
/// beam search four items deep, and at each depth it scores every candidate by
/// appending it to the build so far and calling `itemnet_forward`. The same
/// call is made here for a fifth and a sixth item, with three differences:
///
/// - **The candidates** are the finals `allowed`, which is Smart Builds'
///   word on the champion and the build, not the engine's own short list
///   (finals whose tags match the champion's).
/// - **The lineup** holds this champion in its lane and nobody else. The
///   detour is not told who the enemies are, so the two features that read
///   them (`lane_counter`, `global_counter`) sit out; the three that do not
///   (`self_item`, `champ_pos_build`, `synergy`) are scored in full.
/// - **Boots are left out of the build** the network is shown. It has never
///   seen a pair: they are not finals, so the engine never offers it one.
///
/// A seat keeps its first answer for as long as the session lasts; see
/// [`NETWORK_PICKS`].
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

/// Smart Builds' rules 6 and 9 over a build just grown to its 5th and 6th
/// slots, which the stable hook's pass never saw: the automatic picks not
/// bought yet, role items first, then early items, late items last
/// (`smart_builds::sort_by_timing`).
/// Fixed in place: every slot up to the one being built now (`owned`, whose
/// components may already be bought), the player's pins (`designate`), and
/// the boots, which rule 7 placed.
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
    // The fit too, so a support's or jungler's role item (rule 9) stays ahead
    // of the early items this sort pulls forward.
    let fit = crate::smart_builds::fit(champ, crate::build_config::role_for_champion(champ));
    crate::smart_builds::sort_by_timing(&mut picks, fit, |&index| catalog_name_at(ctx, index));
    for (&j, index) in movable.iter().zip(picks) {
        slots[j] = index;
    }
}

/// Grows the athlete's build `Vec` to `slots.len()` and writes every slot from
/// `old_len` on, then moves `len`. Returns the length the Vec has afterwards,
/// which is `old_len` if anything declined.
///
/// Reallocates only when `cap` is short. `__rust_realloc` returns null on
/// failure and leaves the old block alone, so that case changes nothing; on
/// success the old block may already be freed, so ptr/cap are updated before
/// anything else can go wrong.
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

/// Whether this athlete's build `Vec` still has to be grown to
/// `build_config::picker_slots()` (4 -> 6 on game 0.6.0).
///
/// Read through `safe_read_u64` (the VEH, no syscall) rather than `readable`
/// (`VirtualQuery`, a kernel call), because this runs on the buy hot path *ahead
/// of* the background early exit — the one place where a syscall per call was
/// measured at 75% of the mod's whole cost. Two protected reads of an address
/// that is about to be read anyway is the budget here.
///
/// Answers `false` for good once the extension has run (`len` reaches the
/// target), so no athlete keeps the exit open.
///
/// This deliberately mirrors the `grow` condition the extension itself tests
/// in `buy_replace_ctx`. If those two ever disagree the symptom is silent — the
/// gate opens for an athlete the extension then declines — so they are worth
/// changing together.
/// Whether the buy detour grows builds past the game's four slots, so a 5th
/// and 6th exist for Smart Builds' boots to wait for
/// (`build_config::later_slot_open`). The same conditions `grow` tests in
/// `buy_replace_ctx`, less the per-athlete ones, plus the detour being in.
pub(crate) fn builds_grow_past_four() -> bool {
    BUILD_EXTEND_ENABLED
        && BUY_PROBE_INSTALLED.load(Ordering::Relaxed) == 1
        && crate::build_config::picker_slots() > crate::build_config::game_slots()
        && realloc_ok()
}

unsafe fn needs_build_extension(athlete: usize) -> bool {
    // `realloc_ok` too: an athlete whose Vec can never grow must not keep
    // taking the slow path on every buy. It is one cached atomic load.
    if !BUILD_EXTEND_ENABLED || !realloc_ok() {
        return false;
    }
    let target = crate::build_config::picker_slots() as u64;
    // 0.6.0 build Vec: cap@+0x550, ptr@+0x558, len@+0x560.
    match (
        safe_read_u64(athlete + O_ATHLETE_BUILD_CAP),
        safe_read_u64(athlete + O_ATHLETE_BUILD_LEN),
    ) {
        (Some(cap), Some(len)) => len >= 3 && len < target && cap >= len,
        _ => false,
    }
}

/// Fixed words at the front of [`BuyInputs`]; the build targets follow.
const BUY_INPUT_FIXED: usize = 14;
/// Longest build [`BuyInputs`] holds. The detour grows builds to
/// `build_config::picker_slots()` (6); a longer one is never memoized.
const BUY_MEMO_BUILD_MAX: usize = 8;

/// Everything `buy_replace_ctx` decides an athlete's build from, read without
/// a syscall. Two calls with equal inputs make the same decision.
#[derive(Clone, Copy, PartialEq, Eq)]
struct BuyInputs([u64; BUY_INPUT_FIXED + BUY_MEMO_BUILD_MAX]);

/// Reads [`BuyInputs`] for `athlete`, or `None` when any of it is unreadable
/// or the build is too long to hold, and the call takes the full path.
///
/// It covers what the full path reads: the athlete's identity, champion,
/// side, lane, owned count and build Vec (header and every target), the
/// catalog context, the team-gate flags, both settings, the scene side, and
/// which pin snapshot is live. Not gold: nothing below decides anything from
/// it. The network's pick for a slot being grown is covered through its inputs
/// (the build so far, the champion and lane, and the match's seed).
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
        | (crate::build_config::smart_builds_enabled() as u64) << 6;
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
        SCENE_SIDE.load(Ordering::Relaxed),
        crate::build_config::pins_generation(),
    ];
    let mut words = [0u64; BUY_INPUT_FIXED + BUY_MEMO_BUILD_MAX];
    words[..BUY_INPUT_FIXED].copy_from_slice(&fixed);
    for i in 0..len as usize {
        words[BUY_INPUT_FIXED + i] = safe_read_u64(ptr as usize + i * 8)?;
    }
    Some(BuyInputs(words))
}

/// Athletes remembered per thread. A match has ten athletes, but a rayon
/// worker can step several background fixtures in turn, so this holds a few
/// matches' worth rather than letting them evict each other. A miss only costs
/// the full path, so the table does not need to be exact.
const BUY_MEMO_SLOTS: usize = 64;

/// Which athlete a [`BuyInputs`] belongs to: the match seed, the athlete id and
/// the address of its build `Vec`.
///
/// Not the `athlete` pointer the detour is handed, which was the key until
/// 2026-09-27. A logged session showed the game passing every athlete of a team
/// through the same buffer: five athletes (ids 0xbd..0xc1) took turns at one
/// address, each overwrote the last one's entry, and the memo missed on ~520k
/// calls in four minutes, every one a full pass that changed nothing. The seed
/// tells sim copies of different fixtures apart, the id tells athletes apart,
/// and the build `Vec` is the athlete's own heap buffer, which tells two copies
/// of one match on the same seed apart.
fn buy_memo_key(inputs: &BuyInputs) -> [u64; 3] {
    [inputs.0[0], inputs.0[1], inputs.0[8]]
}

/// Per athlete, the inputs of its last buy decision that changed nothing.
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

/// Whether this athlete's last decision on this thread changed nothing and was
/// made from exactly `inputs` -- so this one would change nothing either.
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

/// Remembers that a decision from `inputs` changed nothing for `athlete`,
/// replacing whatever this thread held for it.
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

unsafe extern "C" fn buy_replace_ctx(saved: *mut u64, rsp_entry: usize) -> u64 {
    // * Hot path (parallel rayon workers) - global atomic counters would make the measurement itself expensive through cache-line contention,
    //   so thread_local accumulation (rec_tl) is used. T_BUY_ALL = the whole detour (including catch_unwind),
    //   T_BUY_EARLY = the background-sim early exit portion (contained in ALL, so it is double counted - subtract when interpreting).
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> u64 {
        // Relabelled as the call gets further, so each exit is timed as the
        // kind of call it was: an early exit, a memo hit, or a full pass.
        let mut probe = crate::perf::Probe::sim(crate::perf::Section::BuyEarlyExit);
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
        //   the code below already had for it.
        //   A lane or 5v5 test's athletes get through too: both of its sides
        //   are the player's (see `is_training` below). The test is known by
        //   its match's seed (`build_config::note_test_spawn`), so a league
        //   fixture simulating alongside it still exits here, as cheaply as
        //   before.
        if FIXB
            && !is_live
            && !crate::build_config::is_test_match(seed_r9)
            && !matches!(is_my_athlete(athlete), Some(true))
            && !needs_build_extension(athlete)
        {
            return 0;
        }
        // ** Per-athlete memo (2026-09-25, the sim slowing down in teamfights).
        //   `run_tick` calls buy_item every tick for every athlete who is dead
        //   or standing in its fountain (the entity lookup at 0x1aff1b8 on
        //   0.6.1), so after a fight everything below ran 60 times a second
        //   per dead athlete: about 56 `readable`/`VirtualQuery` syscalls a
        //   call, ~0.33 ms at the ~5.9 us one costs, to rewrite build targets
        //   that had not changed. With the detour off (`DIAG_BUY_OFF`) the lag
        //   went away.
        //
        //   Everything below is a function of `BuyInputs`. A call whose inputs
        //   match the athlete's last call that changed nothing would change
        //   nothing too, so it passes through after ~20 VEH reads. A purchase
        //   (owned), an engine re-plan (the targets), a new pin snapshot or a
        //   flipped gate each change the inputs and run the full path again.
        probe.set(crate::perf::Section::BuyMemoHit);
        let memo_inputs = buy_inputs(athlete, rsp_entry, seed_r9, is_live);
        if memo_inputs.is_none() {
            crate::perf::count(crate::perf::Section::BuyUnmemoizable);
        }
        if memo_inputs.is_some_and(|inputs| buy_memo_hit(&inputs)) {
            return 0;
        }
        probe.set(crate::perf::Section::BuyFullPass);
        // -- From here on, only spectated-match buys (a small minority) and background buys by my 5 players get through --
        // * The athlete validity check, now VEH-guarded reads rather than a
        //   `readable` (VirtualQuery) syscall (2026-09-25; see `catalog_name_at`).
        //   The build len at +0x560 is read too, only as proof: it is the
        //   furthest of the fields read raw below (+0x550/+0x558/+0x560), which
        //   the old `readable(athlete, 0x538)` no longer reached.
        let (Some(owned), Some(cptr), Some(clen), Some(_)) = (
            safe_read_u64(athlete + O_ATHLETE_ITEMS_LEN), // 0.5.0 owned (was 0x3d0)
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
        // (`let champ_designated = is_champ_designated(champ)` used to sit here.
        //  Nothing read it — it was the safety net described at the `by_scene`
        //  comment below, and the team gate replaced it — so every buy that got
        //  this far paid two global mutex acquisitions, and sometimes a rebuild of
        //  the designated-champion `HashSet`, to compute a value it dropped.
        //  `is_champ_designated` itself is still used by the spawn path.)
        let side = safe_read_u64(athlete + O_ATHLETE_TEAM).unwrap_or(u64::MAX);
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
        // The `None` arm used to fall back to `player_side_for_match`, a majority
        // vote over champions the user had designated in the `SEL` dropdowns.
        // That went with `SEL`, so an undecided scene side is simply undecided —
        // which is what it already meant everywhere else, and what `FIXB` makes
        // moot anyway (`is_player` below takes the `is_my_athlete` branch).
        let by_scene: bool = is_live && scene_ps.is_some_and(|ps| side == ps);
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
        // ** 2026-09-24: lane and 5v5 tests, from the route call's `mode` rather
        //   than the launcher retaddrs above, which date from 0.5.3 and were
        //   never re-derived (and never listed the lane test). Both sides of a
        //   test are the player's, so `own_team_only` does not apply to them.
        // ** 2026-09-25: an athlete of the test's own match, not any athlete on
        //   one of its champions. The day's league fixtures simulate while a
        //   test runs, and the champion gate gave the player's pins to four
        //   league athletes during a lane test — the same leak the note above
        //   describes for comp tests. See `build_config::note_test_spawn`.
        let is_training = crate::build_config::is_test_match(seed_r9);
        let is_player = if is_comptest_live || is_training {
            true // comp test = both sides user-composed -> bypass the team gate
        } else if FIXB {
            matches!(is_my_athlete(athlete), Some(true))
        } else {
            is_live && by_scene
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
        // The editor's scope toggle, read once here rather than per slot: it is
        // an atomic load (see `build_config::own_team_only_enabled`), but this
        // is a per-buy-decision path and both the pins for the game's four
        // slots below and the 5th and 6th further down need the same answer.
        let own_team_only = crate::build_config::own_team_only_enabled();
        // This branch used to publish the player's lineup to `crate::my_team`,
        // which is how the host half guessed whether a set of item-build routes
        // belonged to the player. Nothing consumes that guess any more: the host
        // half either applies builds to both teams or leaves them entirely to
        // this one, and `is_player` — the real gate, not a guess — is right here.
        // (A `scope` was computed here — `Scope::CtBlue`/`CtRed` in comp test,
        // `Plain` otherwise — so that the same champion picked on both sides of a
        // comp test kept two separate `SEL` designations. Pins have no scope, so
        // it went with `SEL`.)
        // The game's four slots: the `item-builds.json` pin written straight
        // into the build Vec.
        //
        // This runs ONLY under `own_team_only`, and it is the reason that toggle
        // can exist. `crate::item_build_hook::decide_build` sets the same four
        // slots on the stable API, earlier and more cheaply — the engine is
        // handed the build before the match instead of having it overwritten per
        // buy decision — but it has no *usable* team gate, so what it sets
        // reaches BOTH sides. (Its context does carry a `team()`, measured
        // 2026-09-08 as a 0/1 side index within the match: it cannot say which
        // side is the player's, nor whether the player is in the match at all.
        // See `build_config::own_team_only_enabled`.) This path is the opposite
        // trade: it costs a per-buy write and it can only fire under
        // `is_player`, because a sim athlete carries no team id and the gate has
        // to be inferred from the athlete-id roster — which is exactly the
        // scoping the toggle asks for.
        //
        // The two must never both apply, or they fight over the same four
        // slots; `decide_build` returns the engine's own build untouched
        // whenever this is live, and the toggle is the single thing deciding
        // which of them runs.
        //
        // # What this path cannot do
        //
        // Slot 0. A buy decision is the earliest moment it sees the athlete, and
        // by then one item has completed, so the `owned > si` guard below skips
        // si=0 for the rest of the match — si=1 needs `owned >= 2` and si=2
        // `owned >= 3`, so only the first slot is ever locked out. That is why,
        // with the toggle on, every configured item applies except the first.
        // Setting it here is not possible. Moving it to `decide_build` was the
        // obvious answer and is ruled out (see above), so the remaining route is
        // `SPAWN_INJECT_ENABLED` — the spawn-time injector below, which runs
        // before any purchase, has no `owned` guard, and holds the athlete
        // pointer that makes `is_my_athlete` usable.
        if own_team_only && is_player {
            let ctx012 = rd_u64(rsp_entry + 0x30) as usize;
            let bptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize; // 0.5.0 build ptr
            let blen = rd_u64(athlete + O_ATHLETE_BUILD_LEN); // 0.5.0 build len
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
                      // By key, vanilla included: name scan + recipe validation.
                    let idx = slot_n_catalog_index(champ, si, |key| scan_idx_cached(ctx012, key));
                    if let Some(t) = idx {
                        // * Idempotence guard (07-19): skip the write if the target value is already there. Measured, the vast majority of 53,890 writes
                        //   were rewrites of the same value on the same athlete and slot -> a value comparison cut it to about 10 (removing the hot-path cost).
                        if rd_u64(bptr + (si as usize) * 8) == t {
                            continue;
                        }
                        // Never plant a second copy of an item the build already
                        // targets. The engine chose slot 0 (and any slot without a
                        // pin) knowing nothing of the pins, so it can pick the very
                        // item pinned here: K'Sante with Jak'Sho pinned 2nd bought
                        // the engine's Jak'Sho 1st and then the pin's 2nd. Same
                        // rule `merge_build` applies on the stable path, where AI
                        // fill skips pinned items.
                        //   - Earlier slot: it is bought or being built, so the pin
                        //     is already honoured, only sooner. Moving it would
                        //     throw away components; this slot keeps the engine's
                        //     item.
                        //   - Later slot: swap, so the pin lands where the player
                        //     put it and the engine's item moves back.
                        // Boots clash with any other pair, not just the same
                        // one. A slot the player pinned to what it holds is a
                        // deliberate duplicate, not a clash.
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
                            if j < si as usize {
                                continue;
                            }
                            if writable(bptr, (blen as usize) * 8) {
                                wr_u64(bptr + j * 8, rd_u64(bptr + (si as usize) * 8));
                                wr_u64(bptr + (si as usize) * 8, t);
                            }
                            continue;
                        }
                        // The pin covers the boots Smart Builds gave this build
                        // (rule 7): move them where the rule would have put
                        // them, as `cap_spawn` does (`displaced_boots_slot`),
                        // unless the player pinned a pair of their own. Every
                        // slot after this one is unbought; the first is only
                        // while nothing is.
                        let here = rd_u64(bptr + (si as usize) * 8);
                        if !pin_boots
                            && buy_is_boots(ctx012, here)
                            && !(0..crate::build_config::picker_slots())
                                .any(|j| pin_at(j).is_some_and(|pin| buy_is_boots(ctx012, pin)))
                        {
                            let free =
                                displaced_boots_slot(champ, si as usize, blen as usize, owned == 0);
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
        // The game's own four slots are not decided here. With `own_team_only`
        // off they are the stable hook's (`crate::item_build_hook`: pins and
        // Smart Builds over all four, before the match); with it on, the
        // player's pins for them are written above and in `cap_spawn`. Until
        // 2026-10-07 this block also chose the 4th, from when the game had
        // three slots, and with Smart Builds off that replaced the engine's
        // own pick. What is left is the slots the engine never plans:
        //   in_place -- the build has its four slots, and maybe the two an
        //               earlier buy grew: those take a pin that changed,
        //               rewritten where it stands;
        //   grow     -- the Vec is shorter than `target` (the game's 4 against
        //               the mod's 6), so it is extended and every new slot is
        //               filled before `len` moves. A 3-long build, which the
        //               engine still produces now and then, gets its 4th the
        //               same way: one more slot to fill.
        // `grow` is the only path that reaches `RVA_REALLOC`. It is called
        // through a raw transmute, so `realloc_ok` checks its entry bytes first:
        // a stale address in that constant crashed matches on 2026-09-16.
        // Keep this condition in step with `needs_build_extension`.
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
                // The team gate the pins below take, the one the game's four
                // slots take above: with `own_team_only` off a pin is keyed by
                // champion and applies to whoever plays it, both teams, as on
                // the stable API; with it on, only my athletes get theirs and
                // everyone else an automatic pick.
                let designate = !own_team_only || is_player;
                let smart = crate::build_config::smart_builds_enabled();
                // The whole build as it will stand, starting from the live
                // slots. Only `slots[build_len..]` is new memory.
                let mut slots: Vec<u64> = (0..live).map(|i| rd_u64(ptr + i * 8)).collect();
                // Slots 5 and 6 that an earlier buy already grew: only a pin,
                // only where it changed, and only while that slot is not
                // bought yet -- the same `owned > si` rule the game's four follow.
                // Their automatic picks were made once, at growth, and stand.
                if designate {
                    for si in 4..slots.len().min(target as usize) {
                        if owned > si as u64 {
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
                        if smart {
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
                        // `grow_build` writes only the new slots; rule 6 may
                        // have moved the old ones too. Only once the growth
                        // held, or an item moved into a slot that never came
                        // would be lost. The Vec may have moved.
                        let new_ptr = rd_u64(athlete + O_ATHLETE_BUILD_PTR) as usize;
                        if build_len as usize == slots.len()
                            && new_ptr >= 0x10000
                            && writable(new_ptr, old_len * 8)
                        {
                            for (j, &index) in slots.iter().enumerate().take(old_len) {
                                if rd_u64(new_ptr + j * 8) != index {
                                    wr_u64(new_ptr + j * 8, index);
                                }
                            }
                        }
                        // For the Check Tactics panel, which shows what each
                        // of the player's athletes will buy: the whole build
                        // as it stands now, the two new slots included and in
                        // the order rule 6 left it. Once per athlete, here
                        // where the growth held.
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
        // The memo's other half: inputs unchanged by the whole pass above mean
        // it wrote nothing, and a call with the same inputs would write nothing
        // too. A pass that did write is not remembered, so the next call runs in
        // full on what it wrote, as it always has (growth takes a second pass to
        // settle), and is remembered once it settles.
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
                                       // ** Confirmed (07-11, in game): the sim athlete side (`O_ATHLETE_TEAM`, then 0x820) is fixed at blue=0 / red=1. In a spectated match (my team blue), KT Aiming = meiling was
                                       //   dumped as sim side1 (red) -> confirming side0 = blue = my team. So the scene player <-> sim side mapping = blue is side0.
                                       //   WARNING this is a side-independent fixed mapping (not a constant inversion) - matching scene team_id <-> pid returns the correct sim side even when sides swap.
const SCENE_BLUE_IS_SIDE0: bool = true; // blue team = sim side0 (confirmed in game). update_scene_side matches pid with (s0,s1) = (blue,red).
static SCENE_SIDE: AtomicU64 = AtomicU64::new(u64::MAX); // 0/1 = the player's side in a live match, MAX = undetermined (not a match / not spectating)
static LIVE_DB: AtomicU64 = AtomicU64::new(0); // * v6: the absolute db address stored by the InGame post_update (for the spawn hook's early side decision)
static LIVE_PID: AtomicU64 = AtomicU64::new(u64::MAX); // * v6: the stored PLAYER_TEAM_ID

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
// * 2026-07-30: have we ever seen a valid **non-zero** pid? If 1, ignore later reports of 0 (prevents pid regression - measurement showed
//   the same save alternating between 105 and 0 depending on the moment, and trusting the 0 breaks the team gate).
static PID_NONZERO_SEEN: AtomicU64 = AtomicU64::new(0);
static PID_ZERO_CLEAN: AtomicU64 = AtomicU64::new(0); // * times pid=0 was observed in an InGame unrelated to comp test
                                                      //   (>=600 accepts it as "a save whose real team id is 0" = MY_ATHLETES publication allowed)
fn scene_player_side() -> Option<u64> {
    if !SCENE_GATE_ENABLED {
        return None;
    } // when OFF, use the roster fallback
    match SCENE_SIDE.load(Ordering::Relaxed) {
        v @ 0..=1 => Some(v),
        _ => None,
    }
}
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
    // orig_len=12 since 0.6.3: the prologue is 8 pushes, 12B, exactly the jmp patch. (Through 0.6.2 it was 19:
    // 5 push (7) + sub rsp,0x50 (4) = 11B could not cover the 12B patch, so it ran to the next clean boundary,
    // + mov rax,[rsp+0xa8] (8).)
    match unsafe { install_replace_buy(RVA_BUY_ITEM, 12, buy_replace_ctx as *const () as usize) } {
        Ok(_) => BUY_PROBE_INSTALLED.store(1, Ordering::Relaxed),
        Err(_) => BUY_PROBE_INSTALLED.store(3, Ordering::Relaxed),
    }
}

// ===========================================================================
//  5th and 6th item slots (game 0.6.0 release, derived 2026-09-18)
// ===========================================================================
//
// The game ships four slots. Four things stop at four, and each gets a byte
// patch:
//
//   * the resolver refuses to start a new final once four are complete
//     (`patch_final_gate`);
//   * the tick throws away a purchase buy_item approved once four finals are
//     owned (`patch_tick_finals_cap`) -- found 2026-09-19 by
//     `build_ext_diag.txt`: builds grew to 6, every other patch applied, and
//     max owned still stopped at exactly 4;
//   * the in-match item row is floored at four (`patch_row_floor`);
//   * the match-result row is floored at four (`patch_result_row_floor`).
//
// The other piece, a build `Vec` long enough to hold six targets, is not a
// patch: `buy_replace_ctx` grows it (see `grow_build`). No stat cap needs a
// patch: that tick path takes the resolver's answer with no count of its own.
//
// Every site below is pinned by a form that is unique in the release .text
// (checked with `tools/rederive.py sig`), and every write address is derived
// from the signature, never pinned separately. The 0.5.6 migration is why: a
// patch whose checked address was updated and whose write address was not
// passed its check and stamped bytes into unrelated live functions. A mismatch
// skips the patch and says so in `4items_patches.txt`; the build still grows,
// and the engine simply stops at four items the way it did before.
const EXTRA_SLOT_PATCHES: bool = true;

/// Checks `expect` at `sig`, then writes `writes`, both as (offset, byte).
/// Already-patched bytes count as a match, so a second init is a no-op.
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

// * The "four finals complete" gate inside the resolver `0xf3d660` -- the
//   second callee of buy_item (`RVA_BUY_ITEM`). It counts the athlete's
//   owned items that are final items, then
//
//       cmp rax,3 / jbe <price check>
//
//   so up to three finals go straight to the gold check, and at four the
//   target must also pass a vtable+0x70 test that a fresh final fails: the
//   engine never starts a 5th. `jbe` -> `jmp` sends four-and-up down the same
//   path as three-and-under. On a four-long build it changes nothing: with
//   four finals owned the resolver runs out of build before reaching this,
//   and returns "nothing to buy".
//
//   The beta2 site was a call's result spilled to the stack; the release
//   inlines that call as the counting loop just above, which is why the form
//   changed and exe2exe finds no match for the container (871 -> 1007 bytes).
//   The 11-byte form below, through the spill reload after the jbe, is unique
//   in .text. 0.6.3 recompiled the resolver (1007 -> 917 bytes): the count is in
//   rbx and the reload is `mov r14,[rsp+0x48]`, where 0.6.2 had `cmp rax,3` and
//   `mov r13,[rsp+0x68]`. Same gate, same `jbe`, different bytes.
unsafe fn patch_final_gate() -> String {
    let sig = exe_base_addr() + 0xe94a4a; // 0.7.0-beta, container 0xe94780 +0x2ca (a strict exe2exe match at identical size 917, pairdiff clean; bytes unchanged, form still unique) (0.6.3 0xea216a, container 0xea1ea0 +0x2ca) (buy_item's second callee again; the form changed with the address, see above) (0.6.2 0xe83251, container 0xe82f30 +0x321) (0.6.1 0xeae541; container a strict exe2exe match, bytes unchanged) (0.6.0 release 0xf3d981; container a strict exe2exe match, bytes unchanged)
    const EXPECT: [(usize, u8); 11] = [
        (0, 0x48),
        (1, 0x83),
        (2, 0xfb),
        (3, 0x03), //  cmp rbx, 3
        (4, 0x76),
        (5, 0x37), //                       jbe +0x37   <- opcode at +4
        (6, 0x4c),
        (7, 0x8b),
        (8, 0x74),
        (9, 0x24),
        (10, 0x48), // mov r14, [rsp+0x48]
    ];
    patch_bytes("final_gate", sig, &EXPECT, &[(4, 0xEB)])
}

// * The tick's own finals cap, the one `patch_final_gate` could not reach.
//
//   `run_tick` (0x1745420 on 0.7.0-beta, 0x1465e80 on 0.6.3, 0x17a7b60 on 0.6.2, 0x1afad10 on 0.6.1, 0x174d640 on 0.6.0) calls buy_item through its vtable (+0x80 on 0.6.2 through 0.7.0-beta), and on
//   an approved purchase counts the athlete's owned finals again -- the same
//   inlined loop the resolver has -- then
//
//       cmp rax,3 / ja <skip the purchase>
//
//   so a 5th item buy_item said yes to was dropped here, every time. Raising
//   the imm to `slots - 1` keeps a hard cap, now at `slots` finals, instead of
//   deleting the check. The 17-byte form below (through the
//   `mov r9,[rbp+0x4598]` after the ja; 0x42f0 on 0.6.3, 0x4cd0 on 0.6.2, 0x4e18 on 0.6.1, 0x4e20 on 0.6.0) is unique in .text; so is the
//   13-byte form with the rel32 masked, which is how to re-find it. That trailing frame slot has
//   moved on every update, so re-read it from the disassembly rather than carrying it forward.
unsafe fn patch_tick_finals_cap(slots: u8) -> String {
    let sig = exe_base_addr() + 0x174ab91; // 0.7.0-beta, run_tick 0x1745420 +0x5771 (run_tick 25507 -> 24170 bytes; the rel32-masked 13-byte form is still unique, the ja rel32 is still 0x24a, the trailing slot moved to [rbp+0x4598] and the gold read after it is still [rax+0x868]) (0.6.3 0x146baf3, run_tick 0x1465e80 +0x5c73) (run_tick 20694 -> 25507 bytes; the rel32-masked 13-byte form is still unique, the ja rel32 is 0x24a now and the gold read after it is [rax+0x868]) (0.6.2 0x17ac532, run_tick 0x17a7b60 +0x49d2) (0.6.1 0x1aff8f2; run_tick 21164 -> 20694 bytes, the rel32-masked 13-byte form is unique, the ja rel32 and the [rax+0xa68] gold read after it are unchanged) (0.6.0 release 0x1752342; run_tick changed size, found by the rel32-masked 13-byte form, unique)
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
        (13, 0x98),
        (14, 0x45),
        (15, 0x00),
        (16, 0x00), // mov r9, [rbp+0x4598] (0.6.3: 0x42f0, 0.6.2: 0x4cd0, 0.6.1: 0x4e18, 0.6.0: 0x4e20)
    ];
    patch_bytes("tick_finals_cap", sig, &EXPECT, &[(3, slots - 1)])
}

// * In-match row floor: `max(4, most items any player owns)` -> `max(slots, ..)`.
//
//       cmp rax,5 / mov ecx,4 / cmovae rcx,rax     (count = rax >= 5 ? rax : 4)
//
//   Both immediates move: the floor becomes `slots` and the compare
//   `slots + 1`, so the result stays `max(rax, slots)`. Raising only the
//   floor would turn 5 owned items into a 5-wide row under a 6 floor.
//   Safe because nothing else reads that count: the store feeds the four
//   `#items` populate calls and the four fill-loop bounds (blue/red x
//   compact/wide), and the fill loop checks `i < items.len()` before reading
//   `items[i]`, so a slot past what a player owns takes the empty branch
//   vanilla already uses for an unbought item. The icons shrink to fit the
//   vanilla `#items` width.
//
//   Pinned by a masked form -- the two byte loads from `[rbp+disp32]` ahead
//   of the compare, displacements masked -- because the bare cmp/mov/cmov is
//   a stock `max(x, n)` idiom with dozens of hits, and validating those bytes
//   alone at a stale address would pass on somebody else's arithmetic. The
//   masked form is unique in the image, which is also how to re-find it.
unsafe fn patch_row_floor(slots: u8) -> String {
    let sig = exe_base_addr() + 0x91b021; // 0.7.0-beta, inside the ingame mega-function 0x9148a0 at +0x6781 (135652 -> 135868 bytes; the masked form is still unique) (0.6.3 0xba33f1 in 0xb9c970) (0.6.2 0xb8b0f1 in 0xb84660) (0.6.1 0xb34d91 in 0xb2e260; the masked form is still unique) (0.6.0 release 0xa6a501 in 0xa63a70; the masked form is unique)
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

// * Match-result row floor, the same change on that screen's own count.
//
//       cmp rdx,5 / mov ecx,4 / cmovb rdx,rcx / test al,1 / cmove rdx,rcx
//
//   The post-match screen has its own item row, with its own layout, populate
//   routine and fit/shrink sizing, so `patch_row_floor` does nothing there.
//   Its count is one value for all ten rows (the most items any player
//   finished, floored here), so "more slots than this player owns" is a state
//   vanilla already draws: the surplus slots are simply empty. The 19-byte
//   form is unique in .text and stops before the `mov [rbp+disp],rdx` that
//   follows, so a frame-layout shift does not invalidate it.
unsafe fn patch_result_row_floor(slots: u8) -> String {
    let sig = exe_base_addr() + 0xaf96ea; // 0.7.0-beta, in the match-result screen builder 0xaf72a0 at the same +0x244a (a strict exe2exe match at identical size 33673; the full 19-byte form is still unique, bytes unchanged) (0.6.3 0x910f8a in 0x90eb40; a strict exe2exe match at identical size 33673; the full 19-byte form is still unique, bytes unchanged) (0.6.2 0xa6b9da in 0xa69590) (0.6.1 0x8bf9de in 0x8bd560; the full 19-byte form is still unique, bytes unchanged) (0.6.0 release 0xc5264e in 0xc501d0; bytes unchanged)
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
// `setting_set_json` on the item settings rebuilds every entry of `mod_items`
// from its JSON, which is eight data fields, and drops the entries it had.
// What is lost with them is the one thing an entry holds that is not data:
// the mod's item object, an `Option<Box<dyn ..>>` at `O_MOD_ITEM_OBJECT`,
// whose absence is what the game reads as "inactive" (0.6.3: `is_active` at
// 0x17da9f0 is `cmp qword [rcx+0x190],0`; the entry's drop, 0x2d5bb0, frees
// it). `item_stats::SYNC_SERVER_ITEMS` has what that did to players.
//
// Nothing in the stable API puts the object back, so the entries must not go
// through the write at all. The `Vec` they live in is three words in the
// Database. Emptied for the length of the write, the host serializes
// `mod_items: []`, builds new settings with an empty list, finds nothing of
// the old list to drop or free (capacity 0), and installs the new settings in
// the same place; the three words then go back, and the entries are the ones
// that were there, untouched. The base items' `next_tier` lists name mod
// items by key, as text, and pass through the write as they are.
//
// All four constants are 0.6.3's and are only used behind the version gate
// (`driver::lift_server_mod_items`). They are also checked against the
// server's own account of the list before anything is written, so a build
// they are wrong for gets no write rather than a wrong one.

/// Where the state the host hands every server call keeps its `Database`
/// (0.6.3: the vtable's own `setting_set_json`, 0x2dccbe0, loads it with
/// `mov rcx,[rdi+0x10]` before calling the handler).
const O_SERVER_STATE_DATABASE: usize = 0x10;

/// The Database's `mod_items`: a `Vec<ModItemEntry>` as capacity, pointer,
/// length (0.6.3: the item settings are at +0x136c0 and the list 0x3028 into
/// them; handler 0x2dc7670 drops the old one through
/// `[r12+0x166e8] / [r12+0x166f0] / [r12+0x166f8]`).
const O_DATABASE_MOD_ITEMS: usize = 0x166e8;

/// Size of one `ModItemEntry` (0.6.3: `lea rsi,[rcx+0x1a8]` steps that drop
/// loop). Its key, a `String`, is the first field: capacity, pointer, length.
const MOD_ITEM_ENTRY_SIZE: usize = 0x1a8;

/// Where an entry holds the mod's item object: data pointer, then vtable.
const O_MOD_ITEM_OBJECT: usize = 0x190;

/// An empty `Vec<ModItemEntry>`: no capacity, the dangling pointer of an
/// 8-aligned type, no length. What `Vec::new()` is, and what the host's own
/// code leaves where it deserializes an empty list.
const EMPTY_VEC: [u64; 3] = [0, 8, 0];

/// The server's mod items, out of its item settings until this is dropped.
pub(crate) struct ModItemsLift {
    /// Address of the `Vec`'s three words in the Database.
    header: usize,
    /// What they held.
    taken: [u64; 3],
}

impl Drop for ModItemsLift {
    /// Puts the list back, whatever the writes in between came to: accepted,
    /// the host has installed new settings with an empty list at this same
    /// address; refused, the empty list written by [`lift_server_mod_items`]
    /// is still there. Either way these three words own nothing.
    fn drop(&mut self) {
        unsafe {
            for (word, &value) in self.taken.iter().enumerate() {
                wr_u64(self.header + word * 8, value);
            }
        }
    }
}

/// Empties the server's `mod_items` and returns what puts it back.
///
/// `state` is the host state of the server call in progress and `keys` the
/// keys of the list as the server reports it, in order. Nothing is touched
/// unless the memory read through the constants above IS that list: the same
/// number of entries, and every entry's key the one reported at its place.
/// `None` otherwise, and the caller must not write to the item settings.
///
/// # Safety
///
/// Only from inside a server hook, on the thread it runs on: that is where
/// the host itself replaces these settings, so nothing else is reading them.
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
//  ** Game version gate - 0.6.0_beta1 only. On any other version **every feature disables itself automatically**.
// ═══════════════════════════════════════════════════════════════════════════
//  Why: this mod depends on 12 hardcoded RVAs + 2 byte patches + many struct offsets.
//  Once the game is patched past 0.6.0_beta1 all those addresses are wrong and we would **hook/patch the wrong code**
//  (hooks with prologue validation simply fail to install, but the weakly validated places risk crashes and data corruption).
//  => check the version at init and, on a mismatch, install **not a single** hook or patch.
//
//  Two-part decision (both must pass to enable):
//   (1) exe file size - 0.6.0_beta1 = 81,422,336B (0.5.7 was 77,111,808B, 0.5.6 77,101,056B, 0.5.5 76,957,696B, 0.5.4 75,936,256B, 0.5.3 74,970,624B). It reliably differs per version and costs nothing to read.
//   (2) measured entry prologues of 3 key hooks - catches a repackage that happens to have the same size but different code.
//  WARNING a loose check (size only) could misbehave on a hotfix, so we look at the prologues too.
const GAME_EXE_SIZE_070_BETA: u64 = 91_006_464; // 0.7.0-beta, the exe that reports 0.7.0_beta1 (0.6.3 was 86_804_992, 0.6.2 86_674_944, 0.6.1 86_330_880, 0.6.0 release 86_082_048, 0.6.0_beta2 86_023_680) (0.6.0_beta1 was 81_422_336)
static VERSION_MSG: Mutex<String> = Mutex::new(String::new());
/// Decides whether this is the one game build this half is pinned to. Called once from init.
fn check_game_version() -> bool {
    let mut why = String::new();
    // (1) exe size
    let size_ok = match exe_path().and_then(|p| fs::metadata(p).ok()) {
        Some(m) => {
            let sz = m.len();
            if sz == GAME_EXE_SIZE_070_BETA {
                true
            } else {
                why = format!(
                    "exe size mismatch: {}B (0.7.0-beta = {}B)",
                    sz, GAME_EXE_SIZE_070_BETA
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
        "0.7.0-beta confirmed - active".to_string()
    } else {
        format!("version mismatch -> this half is fully disabled ({})", why)
    };
    ok
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
    // Bisect switch: behave exactly as a closed version gate (see TACTICS_ENABLED).
    if !TACTICS_ENABLED {
        return false;
    }
    // Register the VEH before anything else. `safe_copy` returns `false` on
    // entry while `SEH_INSTALLED` is false, so until this runs EVERY protected
    // read in this module fails — `safe_read_u64`, `safe_read_bytes`, all of it.
    //
    // It used to be registered only as a side effect of a per-frame UI
    // handler, and gating that handler off left it unregistered.
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
    // Byte-patch results always leave a trace, regardless of `LOG_ENABLED`.
    // Every patch validates its target byte-for-byte and skips silently on a
    // mismatch, so through `append_log`, which `LOG_ENABLED = false` turns off
    // in production, "the signature moved" and "the feature works" would
    // produce identical evidence: no file either way.
    let mut patch_report = String::new();
    // * 5th and 6th item slots (2026-09-18).
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
    true
}
