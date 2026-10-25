//! Native detour on the game's item-build route function
//! (`LogisticSGDAgent::get_item_builds_list`). It is fail-closed: if the target
//! cannot be identified unambiguously, nothing is patched.
//!
//! League builds are decided in `item_build_hook` on the stable API; this stays
//! because its arguments are the only route to the item network, the item
//! catalog and the full champion roster, and because the engine never asks the
//! stable hook about training-screen builds.
//!
//! The target comes from `hook-target.json` beside the DLL (`rva` or
//! `signature`), else [`FALLBACK_SIGNATURE`], and must be a `.pdata` function
//! start with the [`PROLOGUE_PUSHES`] prologue. Exactly those 12 bytes are
//! stolen: they are position-independent and the size of an absolute jump.

use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

extern crate game_core;
use game_core::{ChampionInfoSheet, ItemInfo, LogisticSGDAgent, Position};

use crate::build_config;

// `push rbp; push r15; push r14; push r13; push r12; push rsi; push rdi; push rbx`
const PROLOGUE_PUSHES: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];

const STOLEN_LEN: usize = PROLOGUE_PUSHES.len();

const ABSOLUTE_JUMP_LEN: usize = 12;

// Signature for game 0.6.5 (the same 48 bytes as 0.6.4), where the target is
// `0x23c1610` (2709 bytes); 47 bytes already match 2 functions, so do not
// shorten it.
//
// When it is stale, league builds still work but training-screen builds and the
// tactics half stop. Re-run `tools/find_item_build_hook.py` and paste the
// result here, or ship a `hook-target.json`, which takes precedence and needs
// no rebuild.
const FALLBACK_SIGNATURE: [u8; 48] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xEC, 0x38,
    0x02, 0x00, 0x00, 0x48, 0x8D, 0xAC, 0x24, 0x80, 0x00, 0x00, 0x00, 0x0F, 0x29, 0xB5, 0xA0, 0x01,
    0x00, 0x00, 0x48, 0xC7, 0x85, 0x98, 0x01, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0x4D, 0x89, 0xCE,
];

// Size range for the diagnostic candidate list only; never used to pick the
// target.
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

// Contents of `hook-target.json`. Both fields are optional; `rva` wins.
#[derive(serde::Deserialize, Default)]
struct TargetConfig {
    // Function start as a module-relative virtual address, e.g. `"0x10591f0"`.
    rva: Option<String>,
    // Hex byte signature, e.g. `"554157..."`. Must match exactly once.
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

// Address and virtual size of a named section in the loaded main module.
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

// Every function's `(start_rva, end_rva)` from the PE exception table, sorted
// by start.
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

// The `.pdata` entry containing `rva`, if any.
fn enclosing(functions: &[(u32, u32)], rva: u32) -> Option<(u32, u32)> {
    let index = functions.partition_point(|(start, _)| *start <= rva);
    let (start, end) = *functions.get(index.checked_sub(1)?)?;
    (start <= rva && rva < end).then_some((start, end))
}

// Confirms `target` is a function start whose prologue is the expected pushes.
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
            "{error}; the built-in signature is for game 0.6.5 and this build differs — \
             re-run tools/find_item_build_hook.py and ship the hook-target.json it writes"
        )
    })?;
    verify(base, functions, target)?;
    Ok(target)
}

// Every function sharing the target's prologue and size range, for re-deriving
// the target after a game update. It never picks one: dozens match, and
// detouring the wrong one corrupts the simulation.
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

// Only `ItemInfo::key` and `next_tier` are safe to call here. This crate links
// the 0.5.2 `game_core`, whose `ItemInfo` lacks methods the current game added,
// so later vtable slots (`category`, `tier`, ...) dispatch to the wrong
// function and crash the game.
//
// Derive any other item fact from its key instead (`item_catalog`,
// `build_config`, `strategy_ui`).
unsafe fn detour(
    agent: &LogisticSGDAgent,
    items: &Vec<Box<dyn ItemInfo>>,
    champions: &ChampionInfoSheet,
    champion_ids: &Vec<String>,
    team1: &Vec<(Position, String)>,
    team2: &Vec<(Position, String)>,
    mode: bool,
) -> Vec<Vec<usize>> {
    let _probe = crate::perf::Probe::start(crate::perf::Section::NativeBuildHook);
    // `agent` is the tactics half's only route to the game's `Database`;
    // `record_item_net` validates it and keeps the first one that sticks.
    crate::tactics::driver::record_item_net(agent as *const LogisticSGDAgent as usize);

    // Which items exist and which are final, for the tactics half. The guard
    // avoids allocating two strings per item on every call, since the argument
    // is built per call.
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

    // The full 60-champion roster (mod champions included) for the build
    // editor; the stable hook only sees one match's ten.
    build_config::record_champion_roster(champion_ids);

    // Each champion's role, for the buy detour's pin lookup.
    build_config::record_lineup_roles(&lineup_roles(team1));

    // League routes come back untouched; training (`mode`) routes are rewritten
    // below. Timed separately because the game's build decision includes this
    // mod's stable hooks.
    let mut routes = crate::perf::time(crate::perf::Section::NativeBuildGame, || {
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
    });

    // The buy detour's team gate needs to know a test is running, since both
    // sides then get the 5th and 6th slots.
    build_config::record_route_call(
        mode,
        team1.iter().chain(team2.iter()).map(|(_, champion)| champion.as_str()),
    );
    crate::own_team_log::line(|| {
        format!(
            "route call: training={} team1={:?} team2={:?}",
            mode,
            team1.iter().map(|(_, champion)| champion.as_str()).collect::<Vec<_>>(),
            team2.iter().map(|(_, champion)| champion.as_str()).collect::<Vec<_>>()
        )
    });
    if mode {
        apply_training_builds(&mut routes, items, team1, team2);
    }

    routes
}

// Applies editor builds and the Smart Builds pass to training-screen matches
// (lane and composition tests), where the engine never calls the stable
// item-build hook.
//
// Routes are the game's allocation, so they are overwritten in place at their
// own length. Route `i` belongs to `team1[i]` (position order); `team2` is the
// enemy lineup for boots.
fn apply_training_builds(
    routes: &mut [Vec<usize>],
    items: &[Box<dyn ItemInfo>],
    team1: &[(Position, String)],
    team2: &[(Position, String)],
) {
    // No player's pins go in here in multiplayer: these routes do not say whose
    // team is whose, and the native detours plant each player's on their own
    // athletes.
    let config = if crate::multiplayer::active() {
        Default::default()
    } else {
        build_config::load_cached()
    };
    // An empty config or Smart Builds being off is no reason to return: the
    // pass also covers the engine's own picks, and the tactics panel is told
    // every build.
    let smart = build_config::smart_builds_enabled();

    // An owned key per comparison is fine: this runs only a few hundred times
    // per training match.
    let index_of = |key: &str| items.iter().position(|item| item.key().to_string() == key);
    let enemies: Vec<&str> = team2.iter().map(|(_, champion)| champion.as_str()).collect();
    let allies: Vec<&str> = team1.iter().map(|(_, champion)| champion.as_str()).collect();
    let roles = lineup_roles(team1);

    for (route, (champion, role)) in routes.iter_mut().zip(&roles) {
        let role = *role;
        let mut pinned = Vec::new();
        let mut reserved = Vec::new();
        if let Some(build) = (!config.is_empty())
            .then(|| build_config::build_for_champion(&config, champion, role, &index_of, route))
            .flatten()
        {
            for (slot, item) in build.items.iter().enumerate().take(route.len()) {
                route[slot] = *item;
            }
            pinned = build.pinned;
            reserved = build.reserved;
        }
        if smart {
            let fit = crate::smart_builds::fit(champion, role);
            let boots_key = crate::smart_builds::boots_for(champion, role, &enemies);
            build_config::remember_rule_boots(champion, boots_key);
            let boots = index_of(boots_key);
            let later_open = build_config::later_slot_open(&build_config::pin_row(champion, role));
            enforce_smart_build(items, route, &pinned, &reserved, later_open, fit, boots);
        }

        // Noted for the tactics panel, as the stable hook does for league
        // matches, with the lane the champion plays.
        let keys = route
            .iter()
            .map(|&index| items.get(index).map(|item| item.key().to_string()))
            .collect::<Option<Vec<String>>>();
        if let Some(keys) = keys {
            let lane = build_config::Role::LANES.iter().position(|lane| *lane == role);
            crate::match_builds::note_decision(champion, lane, &allies, &enemies, keys, None);
        }
    }
}

// The role each champion of `team` plays, in its order. From its `Position`,
// by name, so the variants' order is not relied on; failing that, from its
// index in a five-a-side lineup, which comes position-ordered (Top = 0). A lane
// test's lineup is only that lane's champions, so there the index says nothing:
// a bottom lane test's support is second, and was taken for the jungler.
fn lineup_roles(team: &[(Position, String)]) -> Vec<(String, build_config::Role)> {
    use crate::build_config::Role;
    team.iter()
        .enumerate()
        .map(|(index, (position, champion))| {
            let named = match format!("{position:?}").as_str() {
                "Top" => Role::Top,
                "Jungle" => Role::Jungle,
                "Mid" | "Middle" => Role::Mid,
                "Bottom" | "Bot" => Role::Bottom,
                "Support" => Role::Support,
                _ => Role::Any,
            };
            let role = match named {
                Role::Any if team.len() == Role::LANES.len() => Role::from_lane_code(index),
                role => role,
            };
            (champion.clone(), role)
        })
        .collect()
}

// Training-screen twin of `item_build_hook::enforce_smart_build`, over the
// game's `Vec<Box<dyn ItemInfo>>`.
//
// Only `key` and `next_tier` are safe to call (see `detour`), so the category
// comes from `item_catalog::category_of` and an item is final when its
// `next_tier()` is empty.
fn enforce_smart_build(
    items: &[Box<dyn ItemInfo>],
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    later_open: bool,
    fit: crate::smart_builds::Fit,
    boots: Option<usize>,
) {
    crate::smart_builds::enforce(
        items.len(),
        build,
        pinned,
        reserved,
        later_open,
        fit,
        boots,
        |index| items.get(index).map(|item| item.key().to_string()),
        |index| {
            // Bound to a local because `base_slug` borrows the key.
            let key = items.get(index)?.key().to_string();
            crate::item_catalog::category_of(build_config::base_slug(&key))
        },
        |index| {
            items
                .get(index)
                .is_some_and(|item| item.next_tier().is_empty())
        },
    );
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
