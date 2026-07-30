//! Call-counting probe for identifying the item-build function empirically.
//!
//! Static search has run out of road. `tools/find_item_build_hook.py` narrows
//! the executable to 56 functions that share the target's prologue, size and
//! call shape, and ranking those by the recorded call profile picked two wrong
//! ones in a row (`0x1eee3d0`, which reserves ~80KB via `__chkstk`; then
//! `0x10898c0`, which is simply never called). The tool's own docstring says the
//! last step needs a debugger. This is that step, automated.
//!
//! Every candidate listed in `hook-probe.json` is patched with a stub that does
//! nothing but count:
//!
//! ```text
//!   pushfq                      ; the counter must not disturb flags
//!   lock inc qword [rip+0x1f]   ; touches no general-purpose register
//!   popfq
//!   <the 12 stolen prologue bytes>
//!   mov rax, target+12 ; jmp rax
//! ```
//!
//! Unlike the real detour this never reads an argument, so a wrong guess cannot
//! misinterpret a `Vec` — the worst case is a few wasted instructions per call.
//! `rax` is clobbered, which is safe at a function entry under Win64 (volatile,
//! and never an argument register).
//!
//! Play one match, then read the counts out of `riot-items.log`: the target is
//! called a handful of times around match start, not thousands of times per
//! frame. Cross-referenced with the structural ranking, that names it.
//!
//! Entirely opt-in — with no `hook-probe.json` nothing is patched. The Windows
//! declarations are duplicated from `hook.rs` rather than shared, so this whole
//! module can be deleted once the target is known.
//!
//! # Delete `hook-probe.json` as soon as it has answered
//!
//! The stub is safe once installed, but *installing* it is not free: writing 12
//! bytes over a function entry is not atomic, and any thread executing that
//! entry mid-write sees a torn instruction stream. With 55 candidates patched at
//! once — several of them extremely hot, one observed at 4.7M calls — that is a
//! real crash risk, and random crashes were reported while it was enabled. This
//! is a diagnostic to run for one session, not something to leave on or ship.

use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;

/// `push rbp; push r15; push r14; push r13; push r12; push rsi; push rdi; push rbx`
const PROLOGUE_PUSHES: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];
const STOLEN_LEN: usize = PROLOGUE_PUSHES.len();

/// Bytes reserved per candidate: code, then the counter at a fixed offset.
const SLOT_STRIDE: usize = 64;
/// Offset of the 8-byte counter inside a slot. The `lock inc` displacement is
/// computed from this, so the two must stay in step.
const COUNTER_OFFSET: usize = 40;

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

/// Contents of `hook-probe.json`.
#[derive(serde::Deserialize, Default)]
struct ProbeConfig {
    /// Function starts to count, as module-relative hex strings.
    rvas: Vec<String>,
}

struct Probe {
    rva: usize,
    counter: *const u64,
}

// The counters live in a VirtualAlloc'd page owned for the process lifetime and
// are only ever read here, so sharing them across threads is sound.
unsafe impl Send for Probe {}

static PROBES: Mutex<Vec<Probe>> = Mutex::new(Vec::new());
/// Last reported counts, so `report_changes` only logs when something moved.
static LAST: Mutex<Vec<u64>> = Mutex::new(Vec::new());

fn parse_rva(text: &str) -> Option<usize> {
    let trimmed = text.trim();
    match trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        Some(digits) => usize::from_str_radix(digits, 16).ok(),
        None => trimmed.parse().ok(),
    }
}

/// Installs a counting stub on every candidate in `hook-probe.json`.
///
/// `skip` is the address the real hook already patched, if any — double
/// patching would leave one of the two detours reading the other's jump.
pub(crate) fn install(skip: Option<usize>) {
    let path = crate::config::mod_dir().join("hook-probe.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return; // Not opted in.
    };
    let Ok(config) = serde_json::from_str::<ProbeConfig>(&text) else {
        crate::diag::write("probe: hook-probe.json is malformed");
        return;
    };

    let rvas: Vec<usize> = config.rvas.iter().filter_map(|text| parse_rva(text)).collect();
    if rvas.is_empty() {
        crate::diag::write("probe: no usable rvas in hook-probe.json");
        return;
    }

    unsafe {
        let base = GetModuleHandleW(ptr::null()).cast::<u8>();
        if base.is_null() {
            crate::diag::write("probe: GetModuleHandleW returned null");
            return;
        }

        let slab = VirtualAlloc(
            ptr::null_mut(),
            rvas.len() * SLOT_STRIDE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
        .cast::<u8>();
        if slab.is_null() {
            crate::diag::write("probe: VirtualAlloc failed");
            return;
        }

        let mut installed = Vec::new();
        let mut refused = 0usize;
        for (index, rva) in rvas.iter().copied().enumerate() {
            let target = base.add(rva);
            if Some(target as usize) == skip {
                continue;
            }
            // The stub steals exactly these 12 bytes, so they must be present.
            if std::slice::from_raw_parts(target, STOLEN_LEN) != PROLOGUE_PUSHES {
                refused += 1;
                continue;
            }

            let slot = slab.add(index * SLOT_STRIDE);
            write_stub(slot, target);
            if patch_to(target, slot).is_err() {
                refused += 1;
                continue;
            }
            installed.push(Probe {
                rva,
                counter: slot.add(COUNTER_OFFSET).cast::<u64>(),
            });
        }

        if FlushInstructionCache(GetCurrentProcess(), slab.cast(), rvas.len() * SLOT_STRIDE) == 0 {
            crate::diag::write("probe: FlushInstructionCache failed for the stub slab");
        }
        crate::diag::write(&format!(
            "probe: counting {} candidates ({refused} refused)",
            installed.len()
        ));
        if let Ok(mut probes) = PROBES.lock() {
            *probes = installed;
        }
    }
}

/// Builds one counting stub in `slot` that returns to `target + STOLEN_LEN`.
unsafe fn write_stub(slot: *mut u8, target: *mut u8) {
    ptr::write_bytes(slot, 0x90, SLOT_STRIDE);

    *slot = 0x9C; // pushfq

    // lock inc qword [rip+disp32] — 8 bytes at offset 1, so RIP is slot+9.
    let disp = (COUNTER_OFFSET - 9) as u32;
    slot.add(1).write(0xF0);
    slot.add(2).write(0x48);
    slot.add(3).write(0xFF);
    slot.add(4).write(0x05);
    ptr::copy_nonoverlapping(disp.to_le_bytes().as_ptr(), slot.add(5), 4);

    slot.add(9).write(0x9D); // popfq

    ptr::copy_nonoverlapping(target, slot.add(10), STOLEN_LEN);

    // mov rax, target+STOLEN_LEN ; jmp rax
    let resume = target.add(STOLEN_LEN) as usize as u64;
    slot.add(22).write(0x48);
    slot.add(23).write(0xB8);
    ptr::copy_nonoverlapping(resume.to_le_bytes().as_ptr(), slot.add(24), 8);
    slot.add(32).write(0xFF);
    slot.add(33).write(0xE0);

    ptr::write_bytes(slot.add(COUNTER_OFFSET), 0, 8);
}

/// Redirects `target`'s first 12 bytes to `slot`.
unsafe fn patch_to(target: *mut u8, slot: *mut u8) -> Result<(), ()> {
    let mut old_protect = 0u32;
    if VirtualProtect(
        target.cast(),
        STOLEN_LEN,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return Err(());
    }

    let mut jump = [0x90u8; STOLEN_LEN];
    jump[0] = 0x48;
    jump[1] = 0xB8;
    jump[2..10].copy_from_slice(&(slot as usize as u64).to_le_bytes());
    jump[10] = 0xFF;
    jump[11] = 0xE0;
    ptr::copy_nonoverlapping(jump.as_ptr(), target, jump.len());

    FlushInstructionCache(GetCurrentProcess(), target.cast(), STOLEN_LEN);
    let mut ignored = 0u32;
    VirtualProtect(target.cast(), STOLEN_LEN, old_protect, &mut ignored);
    Ok(())
}

/// Counts above this are per-frame engine plumbing rather than per-match work,
/// and are dropped from the log — the first run had one candidate reach 4.7
/// million calls and bury everything else.
///
/// Deliberately generous: the target is called once per *simulated* match, and
/// fast-forwarding simulates a lot of them, so a few thousand calls is an
/// expected result and must not be filtered out. The known per-frame offenders
/// are excluded from `hook-probe.json` instead.
const HOT_THRESHOLD: u64 = 20000;

/// Logs every candidate's count under `label`, ignoring both the change filter
/// and [`HOT_THRESHOLD`].
///
/// Called at the two moments that bracket a match — the strategy screen
/// appearing and disappearing — so the delta between the two lines is exactly
/// "calls made while playing one match". That is the measurement that
/// identifies the target, and taking it automatically beats asking for a
/// fast-forward of a counted number of matches: the last two attempts at that
/// produced logs with no match in them at all.
pub(crate) fn snapshot(label: &str) {
    let Ok(probes) = PROBES.lock() else {
        return;
    };
    if probes.is_empty() {
        return;
    }
    let counts: Vec<String> = probes
        .iter()
        // SAFETY: the counter lives in the stub slab, alive for the process.
        .map(|probe| unsafe {
            format!("0x{:x}={}", probe.rva, ptr::read_volatile(probe.counter))
        })
        .collect();
    crate::diag::write(&format!("probe {label}: {}", counts.join(" ")));
}

/// Logs any candidate whose count changed since the last call. Cheap enough to
/// run every client tick: it is a handful of reads and an integer compare.
pub(crate) fn report_changes() {
    let Ok(probes) = PROBES.lock() else {
        return;
    };
    if probes.is_empty() {
        return;
    }
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    if last.len() != probes.len() {
        last.resize(probes.len(), 0);
    }

    let mut moved = Vec::new();
    for (index, probe) in probes.iter().enumerate() {
        // SAFETY: the counter lives in the stub slab, alive for the process.
        let count = unsafe { ptr::read_volatile(probe.counter) };
        if count != last[index] {
            last[index] = count;
            if count <= HOT_THRESHOLD {
                moved.push(format!("0x{:x}={count}", probe.rva));
            }
        }
    }
    if !moved.is_empty() {
        crate::diag::write(&format!("probe counts: {}", moved.join(" ")));
    }
}
