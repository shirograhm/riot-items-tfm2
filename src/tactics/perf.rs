// ===========================================================================
//  perf.rs - per-hook cost measurement (how much load the mod puts on the game)
// ===========================================================================
// Design principles:
//  1. **The probe must not be bigger than what it measures.** The buy detour's early-exit path is about 2 memory reads (~100 cycles),
//     so two fetch_adds on a shared AtomicU64 would cause cache-line ping-pong across contending rayon workers
//     and the measurement itself becomes the load -> **the buy family uses thread_local accumulation + periodic flush**.
//  2. Everything else (the main-thread post_update family and low-frequency hooks) is called rarely enough that plain atomics are fine.
//     But the sites are **separated onto their own cache lines** (#[repr(align(64))]).
//  3. Time base = rdtsc (raw cycles). Conversion to ns is **calibrated at runtime** against the wall clock when the report is produced
//     (never assume a fixed frequency - turbo/power saving change the real one).
//  4. **Measure the probe's own cost too** and print it in the report (measuring an empty region = PROBE_SELF).
//     Every site's number includes one probe's worth, so it can be subtracted when interpreting.
//  WARNING cap_launcher is a minimal detour running on a 91KB chkstk frame (no locks, allocation or catch_unwind),
//     so even here it only calls rec(), which uses **atomics only** (never rec_tl - that avoids the TLS lazy-init path).

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global gate. When false, every rec/rec_tl becomes an empty function and even the call sites are DCE'd.
/// * OFF in production (measurements finished 2026-07-22). If measurement is needed again, flipping this to true revives every site
///   (the measurement code and site wiring are deliberately left in place for the next patch/optimization round).
pub const PERF_ON: bool = false;
/// Report period (in post_update frames). About 10 seconds at 60fps.
pub const REPORT_EVERY: u64 = 600;

// -- Site IDs (global atomic counters) ---------------------------------------
pub const S_POST_TOTAL: usize = 0;
pub const S_POST_PT: usize = 1;
pub const S_POST_TACTICS: usize = 2;
pub const S_POST_COMPTEST: usize = 3;
pub const S_POST_HIDE_DD: usize = 4;
pub const S_POST_HIDE_CT: usize = 5;
pub const S_POST_SCENESIDE: usize = 6;
pub const S_POST_ROSTER: usize = 7;
pub const S_POST_UINJ: usize = 8;
pub const S_POST_SPACING: usize = 9;
pub const S_HOOK_RETRY: usize = 10;
pub const S_LAUNCHER: usize = 11;
pub const S_SEEDCTOR: usize = 12;
pub const S_SPAWN: usize = 13;
pub const S_FILLSLOTS: usize = 14;
pub const S_ITEMNET: usize = 15;
pub const S_PROBE_SELF: usize = 16;
pub const N_SITES: usize = 17;

pub const NAMES: [&str; N_SITES] = [
    "post_update(전체)",
    "  ├ PT스냅샷+override",
    "  ├ 개인전술 화면",
    "  ├ 조합테스트 화면",
    "  ├ 네이티브DD 숨김",
    "  ├ 조합테스트DD 숨김",
    "  ├ scene side 직독",
    "  ├ 로스터 폴링",
    "  ├ uinj::install(멱등)",
    "  └ 블루슬롯 간격강제",
    "훅 재시도(멱등,매프레임)",
    "cap_launcher(경기시작)",
    "cap_seed_ctor",
    "cap_spawn",
    "fill_slots(슬롯배열)",
    "itemnet forward",
    "※프로브 자체비용",
];

// -- thread-local sites (exclusively for the sim worker hot path) ------------
pub const T_BUY_ALL: usize = 0;
pub const T_BUY_EARLY: usize = 1;
pub const N_TL: usize = 2;
pub const TL_NAMES: [&str; N_TL] = [
    "buy 디투어(전체, sim워커)",
    "  └ 그중 조기탈출(배경sim)",
];
/// How often (in calls) thread_local accumulations are handed over to the globals.
const TL_FLUSH_EVERY: u64 = 4096;

#[repr(align(64))] // one cache line per site - prevents false sharing between adjacent sites
pub struct Slot {
    pub calls: AtomicU64,
    pub cycles: AtomicU64,
    // * min: rdtsc measures **elapsed time, not CPU time**, so if the thread is preempted mid-region the whole off-CPU
    //   period is counted (which happens constantly on the main thread when a background sim takes every core). The minimum = the value from
    //   a frame with no preemption => **the gap between average and minimum is preemption noise**, and the minimum is approximately the real work cost.
    pub min: AtomicU64,
}
impl Slot {
    const fn new() -> Self { Slot { calls: AtomicU64::new(0), cycles: AtomicU64::new(0), min: AtomicU64::new(u64::MAX) } }
}

static SLOTS: [Slot; N_SITES] = [const { Slot::new() }; N_SITES];
static TL_SLOTS: [Slot; N_TL] = [const { Slot::new() }; N_TL];

// Calibration reference point (the time of the first rec). The effective frequency is derived from the tsc <-> wall correspondence.
static CAL_TSC0: AtomicU64 = AtomicU64::new(0);
static CAL_MS0: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
pub fn tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::x86_64::_rdtsc() }
    #[cfg(not(target_arch = "x86_64"))]
    0
}

/// Record a global site. start = tsc() taken when entering the region.
#[inline(always)]
pub fn rec(site: usize, start: u64) {
    if !PERF_ON { return; }
    let d = tsc().wrapping_sub(start);
    if let Some(s) = SLOTS.get(site) {
        s.calls.fetch_add(1, Ordering::Relaxed);
        s.cycles.fetch_add(d, Ordering::Relaxed);
        s.min.fetch_min(d, Ordering::Relaxed);
    }
}

struct Tl {
    calls: [Cell<u64>; N_TL],
    cycles: [Cell<u64>; N_TL],
    n: Cell<u64>,
}
thread_local! {
    // const initialization = no lazy-init flag check (minimum-cost TLS access). No Drop -> no TLS destructor is registered.
    static TL: Tl = const {
        Tl { calls: [const { Cell::new(0) }; N_TL], cycles: [const { Cell::new(0) }; N_TL], n: Cell::new(0) }
    };
}

/// Record a worker hot path (thread_local accumulation -> pushed to the globals every TL_FLUSH_EVERY calls).
#[inline(always)]
pub fn rec_tl(site: usize, start: u64) {
    if !PERF_ON { return; }
    let d = tsc().wrapping_sub(start);
    let _ = TL.try_with(|t| {
        if site < N_TL {
            t.calls[site].set(t.calls[site].get() + 1);
            t.cycles[site].set(t.cycles[site].get() + d);
            TL_SLOTS[site].min.fetch_min(d, Ordering::Relaxed); // only min is applied immediately (negligible contention)
        }
        let n = t.n.get() + 1;
        t.n.set(n);
        if n >= TL_FLUSH_EVERY {
            t.n.set(0);
            for i in 0..N_TL {
                let c = t.calls[i].replace(0);
                let cy = t.cycles[i].replace(0);
                if c != 0 {
                    TL_SLOTS[i].calls.fetch_add(c, Ordering::Relaxed);
                    TL_SLOTS[i].cycles.fetch_add(cy, Ordering::Relaxed);
                }
            }
        }
    });
}

/// Sample the probe's own cost (measuring an empty region) - reported as one probe's worth per site.
#[inline(always)]
pub fn sample_self() {
    if !PERF_ON { return; }
    let t = tsc();
    rec(S_PROBE_SELF, t);
}

fn calib(now_ms: u64) -> (u64, u64) {
    let t = tsc();
    let t0 = CAL_TSC0.load(Ordering::Relaxed);
    if t0 == 0 {
        CAL_TSC0.store(t, Ordering::Relaxed);
        CAL_MS0.store(now_ms, Ordering::Relaxed);
        return (0, 0);
    }
    (t.wrapping_sub(t0), now_ms.saturating_sub(CAL_MS0.load(Ordering::Relaxed)))
}

/// Build the report. frames = the number of post_update calls (the frame basis for all sites).
pub fn report(now_ms: u64, frames: u64) -> String {
    let (dtsc, dms) = calib(now_ms);
    // Effective frequency: if the calibration window is too short, skip the conversion (report cycles only).
    let cyc_per_ms = if dms >= 1000 { dtsc / dms.max(1) } else { 0 };
    let to_ns = |cycles: u64| -> Option<u64> {
        if cyc_per_ms == 0 { None } else { Some(cycles.saturating_mul(1_000_000) / cyc_per_ms.max(1)) }
    };
    let mut s = String::new();
    s.push_str(&format!(
        "[perf] 경과 {:.1}s / post_update {} 프레임 / 실효 TSC {}\n",
        dms as f64 / 1000.0, frames,
        if cyc_per_ms == 0 { "측정중(≥1s 필요)".to_string() }
        else { format!("{:.2} GHz", cyc_per_ms as f64 / 1_000_000.0) }
    ));
    s.push_str("  ⚠ 각 수치에는 프로브 1회분(맨 아래)이 포함됨. 사이클→ns 는 wall-clock 대비 실측 환산.\n\n");
    s.push_str("  ★최소 = 선점(preempt) 없이 끝난 프레임의 값 ≈ 실제 작업 비용. 평균≫최소면 그 차이는\n     구간 도중 스레드가 밀려난 시간이지 모드가 쓴 CPU가 아니다.\n\n");
    s.push_str(&format!("{:<28} {:>12} {:>12} {:>12} {:>12} {:>10}\n", "사이트", "호출", "총 ms", "평균 ns", "★최소 ns", "wall 점유"));
    s.push_str(&"─".repeat(92));
    s.push('\n');

    let mut row = |name: &str, calls: u64, cycles: u64, mn: u64, out: &mut String| {
        let tot_ns = to_ns(cycles);
        let avg_ns = if calls == 0 { None } else { to_ns(cycles / calls) };
        let occupancy = match (tot_ns, dms) {
            (Some(t), d) if d > 0 => format!("{:.3}%", (t as f64 / 1e6) / d as f64 * 100.0),
            _ => "-".to_string(),
        };
        let min_ns = if mn == u64::MAX { None } else { to_ns(mn) };
        out.push_str(&format!(
            "{:<28} {:>12} {:>12} {:>12} {:>12} {:>10}\n",
            name,
            calls,
            tot_ns.map(|n| format!("{:.2}", n as f64 / 1e6)).unwrap_or_else(|| "-".into()),
            avg_ns.map(|n| n.to_string()).unwrap_or_else(|| format!("{}cyc", if calls == 0 { 0 } else { cycles / calls })),
            min_ns.map(|n| n.to_string()).unwrap_or_else(|| if mn == u64::MAX { "-".into() } else { format!("{}cyc", mn) }),
            occupancy
        ));
    };

    // Main-thread family
    for i in 0..N_SITES {
        if i == S_PROBE_SELF { continue; }
        let c = SLOTS[i].calls.load(Ordering::Relaxed);
        let cy = SLOTS[i].cycles.load(Ordering::Relaxed);
        if c == 0 { continue; }
        row(NAMES[i], c, cy, SLOTS[i].min.load(Ordering::Relaxed), &mut s);
    }
    // sim worker family (thread_local totals - up to 4096 calls per thread may be missing if not yet flushed)
    s.push('\n');
    s.push_str("── sim 워커(rayon, 여러 스레드 합산) ──\n");
    for i in 0..N_TL {
        let c = TL_SLOTS[i].calls.load(Ordering::Relaxed);
        let cy = TL_SLOTS[i].cycles.load(Ordering::Relaxed);
        if c == 0 { continue; }
        row(TL_NAMES[i], c, cy, TL_SLOTS[i].min.load(Ordering::Relaxed), &mut s);
    }
    s.push_str("  ※ wall 점유가 100%를 넘을 수 있음 = 여러 워커 스레드 시간의 합(정상).\n");
    s.push_str("  ※ 스레드별 미flush 잔량(최대 4096콜)은 누락 — 총량이 클수록 오차 무시 가능.\n");

    // Probe's own cost
    let pc = SLOTS[S_PROBE_SELF].calls.load(Ordering::Relaxed);
    let pcy = SLOTS[S_PROBE_SELF].cycles.load(Ordering::Relaxed);
    s.push('\n');
    if pc > 0 {
        let avg = pcy / pc;
        s.push_str(&format!(
            "{:<28} {:>12} {:>12} {:>12}\n", NAMES[S_PROBE_SELF], pc, "-",
            to_ns(avg).map(|n| n.to_string()).unwrap_or_else(|| format!("{}cyc", avg))
        ));
        s.push_str("  → 위 모든 사이트의 '평균'에서 이 값을 빼면 순수 작업 비용에 가깝다.\n");
    }
    s
}
