//! Heal and Shield Power inside the engine ([`crate::heal_shield_power`]): its
//! one heal function and its one add-shield function, detoured so what a
//! carrier gives another unit is raised by the carrier's total before it lands.
//!
//! Pinned to the game build like the rest of this half. Both were found through
//! the host's `SimVtableV1` (RVA 0x3d2ddf8 in 0.6.5): its `heal` slot (39) calls
//! the heal function, which also serves every skill heal; the add-shield
//! function is the one the engine's Shield effect calls (the host's own
//! `entity_add_shield` pushes the layer itself, past it).

use std::sync::atomic::{AtomicU64, Ordering};

use super::{install_detour_generic, install_retry_due, safe_read_u64};
use crate::heal_shield_power;

// Off switch for both detours.
const ENABLED: bool = true;

// The heal: (game, world, counted, healer, target, amount, ..). It applies the
// target's healing reduction to `amount`, caps it at missing health and adds it.
const HEAL_RVA: usize = 0x175d4f0; // 0.6.5
const HEAL_ORIG_LEN: usize = 12;
// push rbp, r15, r14, r13, r12, rsi, rdi, rbx
const HEAL_PROLOGUE: [u8; 12] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
];

// The add-shield: (unit, &shield). It drops a zero shield or one on a unit
// with `shield_gain_blocked`, else pushes the record onto the unit's layers.
const SHIELD_RVA: usize = 0x175c2b0; // 0.6.5
const SHIELD_ORIG_LEN: usize = 13;
// push rbp, rsi, rdi, rbx; sub rsp,0x48; lea rbp,[rsp+0x40]
const SHIELD_PROLOGUE: [u8; 13] = [
    0x55, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x48, 0x48, 0x8d, 0x6c, 0x24, 0x40,
];

// Where the capture stub saved the entry state ([`install_detour_generic`]):
// rcx, rdx, r8, r9 first, the return address at 10, then the caller's shadow
// space and its stack arguments from 15 on, written back when the stub returns.
const SAVED_RCX: usize = 0;
const SAVED_RDX: usize = 1;
const SAVED_R9: usize = 3;
const SAVED_ARG5: usize = 15;
const SAVED_ARG6: usize = 16;

// The game object (0.6.5).
// Units, `ENTITY_SIZE` bytes each, and their count.
const O_GAME_ENTITIES: usize = 0x730;
const O_GAME_ENTITY_COUNT: usize = 0x738;
// Unit id -> index into the units, a u32 each, `u32::MAX` for none.
const O_GAME_ENTITY_SLOTS: usize = 0x748;
const O_GAME_ENTITY_SLOT_COUNT: usize = 0x750;
const ENTITY_SIZE: usize = 0x6b8;
// The players, a pointer each, and beside them what a shield names its caster
// by, one per player (`Option` at the record's start, from the world's 0x160
// slot, which finds the player whose champion the caster is).
const O_GAME_PLAYERS: usize = 0x850;
const O_GAME_PLAYER_COUNT: usize = 0x858;
const O_GAME_SHIELD_OWNERS: usize = 0x880;
const O_GAME_SHIELD_OWNER_COUNT: usize = 0x888;
// A player's champion: `Option<unit id>`.
const O_PLAYER_HAS_CHAMPION: usize = 0x8b8;
const O_PLAYER_CHAMPION: usize = 0x8c0;
// A shield record: `Option<owner>` at 0 / 8, then the amount.
const O_SHIELD_HAS_OWNER: usize = 0x0;
const O_SHIELD_OWNER: usize = 0x8;
const O_SHIELD_AMOUNT: usize = 0x28;

// A heal from a carrier to another unit, raised. `game` is the heal's own
// first argument, so the healer's total is the right match's.
unsafe extern "C" fn cap_heal(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let game = *saved.add(SAVED_RCX) as usize;
    let healer = *saved.add(SAVED_R9) as usize;
    let target = *saved.add(SAVED_ARG5) as usize;
    let amount = saved.add(SAVED_ARG6);
    if healer == target || *amount == 0 {
        return 0;
    }
    let percent = heal_shield_power::power(game, healer);
    if percent > 0.0 {
        *amount = heal_shield_power::raised(*amount as usize, percent) as u64;
    }
    0
}

// A shield from a carrier on another unit, raised.
unsafe extern "C" fn cap_shield(saved: *mut u64, _e: usize) -> u64 {
    if saved.is_null() {
        return 0;
    }
    let unit = *saved.add(SAVED_RCX) as usize;
    let record = *saved.add(SAVED_RDX) as usize;
    if let Some(game) = heal_shield_power::current_game() {
        raise_shield(game, unit, record);
    }
    0
}

unsafe fn raise_shield(game: usize, unit: usize, record: usize) -> Option<()> {
    // The unit must be one of this match's: the add-shield is handed no game,
    // so the match is the one this thread last ticked.
    let units = read(game + O_GAME_ENTITIES)?;
    let count = read(game + O_GAME_ENTITY_COUNT)?;
    if unit < units || unit >= units + count * ENTITY_SIZE {
        return None;
    }
    if read(record + O_SHIELD_HAS_OWNER)? != 1 {
        return None;
    }
    let caster = caster_champion(game, read(record + O_SHIELD_OWNER)?)?;
    // A shield of the caster's own is not raised.
    if unit_at(game, caster)? == unit {
        return None;
    }
    let percent = heal_shield_power::power(game, caster);
    if percent <= 0.0 {
        return None;
    }
    let amount = read(record + O_SHIELD_AMOUNT)?;
    *((record + O_SHIELD_AMOUNT) as *mut usize) = heal_shield_power::raised(amount, percent);
    Some(())
}

// The champion of the player a shield names as its owner.
unsafe fn caster_champion(game: usize, owner: usize) -> Option<usize> {
    let players = read(game + O_GAME_PLAYERS)?;
    let owners = read(game + O_GAME_SHIELD_OWNERS)?;
    let count = read(game + O_GAME_PLAYER_COUNT)?.min(read(game + O_GAME_SHIELD_OWNER_COUNT)?);
    if count > 64 {
        return None;
    }
    for index in 0..count {
        if read(owners + index * 8)? != owner {
            continue;
        }
        let player = read(players + index * 8)?;
        if read(player + O_PLAYER_HAS_CHAMPION)? == 0 {
            return None;
        }
        return read(player + O_PLAYER_CHAMPION);
    }
    None
}

// The address of the unit with id `id`.
unsafe fn unit_at(game: usize, id: usize) -> Option<usize> {
    if id >= read(game + O_GAME_ENTITY_SLOT_COUNT)? {
        return None;
    }
    let slots = read(game + O_GAME_ENTITY_SLOTS)?;
    let index = read(slots + id * 4)? as u32 as usize;
    if index == u32::MAX as usize || index >= read(game + O_GAME_ENTITY_COUNT)? {
        return None;
    }
    Some(read(game + O_GAME_ENTITIES)? + index * ENTITY_SIZE)
}

unsafe fn read(addr: usize) -> Option<usize> {
    safe_read_u64(addr).map(|value| value as usize)
}

static HEAL_INSTALLED: AtomicU64 = AtomicU64::new(0);
static SHIELD_INSTALLED: AtomicU64 = AtomicU64::new(0);

// Installs both detours, from wherever the other hooks of this half install;
// each retries on its own after a failure ([`install_retry_due`]).
pub(super) fn install() {
    if !ENABLED {
        return;
    }
    static HEAL_RETRY: AtomicU64 = AtomicU64::new(0);
    static SHIELD_RETRY: AtomicU64 = AtomicU64::new(0);
    install_one(
        "heal",
        &HEAL_INSTALLED,
        &HEAL_RETRY,
        HEAL_RVA,
        HEAL_ORIG_LEN,
        &HEAL_PROLOGUE,
        cap_heal as *const () as usize,
    );
    install_one(
        "add-shield",
        &SHIELD_INSTALLED,
        &SHIELD_RETRY,
        SHIELD_RVA,
        SHIELD_ORIG_LEN,
        &SHIELD_PROLOGUE,
        cap_shield as *const () as usize,
    );
}

fn install_one(
    name: &str,
    installed: &AtomicU64,
    retry: &AtomicU64,
    rva: usize,
    orig_len: usize,
    prologue: &[u8],
    cap: usize,
) {
    let state = installed.load(Ordering::Relaxed);
    if state == 1 || (state == 2 && !install_retry_due(retry)) {
        return;
    }
    let r = unsafe { install_detour_generic(rva, orig_len, cap, prologue) };
    installed.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    if state == 0 || r.is_ok() {
        crate::own_team_log::line(|| format!("heal/shield power: {name} hook @ {rva:#x}: {r:?}"));
    }
}
