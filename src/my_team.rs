//! Which champions the player's own team is fielding, so configured builds can
//! be confined to the player's players.
//!
//! `hook::detour` computes one team per call and nothing in its arguments says
//! which team that is — `mode` was `false` on every call observed. A build keyed
//! by champion therefore reaches whoever plays that champion, the enemy top
//! laner included.
//!
//! # Why this is read out of the simulation
//!
//! Three rounds of probing established that the champion an athlete is fielding
//! is not reachable any other way:
//!
//! * The `Team` record has `last_starting` (athlete ids), `strategy` and
//!   `champion_tiers`, and nothing that pairs the two.
//! * The `Athlete` record has `recent_champions` (a history — the last five
//!   played) and `champion_proficiency` (every champion in the game). Neither is
//!   the current pick.
//! * `MatchNormal` records carry `{"Normal": team_id}`, a date and a score. The
//!   newest is a schedule stub with `WinnerOf` placeholders. No lineups.
//! * The strategy screen's Matchup card names the *players* per side
//!   (`…matchup.top.blue.name` = "BrokenBlade") but renders champions as bare
//!   `icon [image]` nodes with no readable state.
//!
//! The draft result reaches a mod in exactly one place: the running simulation.
//! The tactics half is already there every time an athlete buys an item, with
//! the athlete pointer in hand — it reads the champion from `athlete+0x420` and
//! decides team membership with `is_my_athlete` (athlete id at `+0x810`, matched
//! against the starters published from the record API). Both were built for the
//! fourth-item feature and are proven in game. [`note_my_champion`] is that
//! answer, escaping to this side of the mod.
//!
//! The cost is the cost of everything in `tactics`: hardcoded struct offsets
//! pinned to game 0.5.3, behind that half's version gate. Where it cannot
//! answer, this fails open.
//!
//! # Fail open
//!
//! An unknown lineup leaves builds applying to both teams, which is the
//! behaviour that shipped. The alternative — applying nothing until the lineup
//! is known — turns "the enemy occasionally gets my build" into "my builds
//! silently do nothing", and the second is the worse failure. Every path here
//! that cannot answer leaves [`owns_lineup`] returning `true`.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// A lineup is five champions, so this is how many make an answer complete.
/// Once reached, [`note_my_champion`] stops taking the lock at all — it runs in
/// the buy detour, which fires hundreds of thousands of times a match.
const LINEUP_SIZE: usize = 5;

/// Champions the player's athletes have been seen playing this match. Empty
/// means "not known", which every caller must read as "do not filter".
static MY_CHAMPIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The last match's answer, kept because of *when* the hook asks.
///
/// Builds are computed as a match starts, before anybody has bought anything —
/// so at the moment the gate is consulted, this match's lineup is empty. Were
/// that the whole story the gate would fail open every time and never do
/// anything. The previous lineup is a good enough stand-in because the question
/// is comparative (see [`owns_lineup`]): the player fields the same five
/// athletes match to match, so their picks overlap their own last lineup far
/// more often than the opponent's do.
static PREVIOUS_CHAMPIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// `MY_CHAMPIONS.len()`, readable without the lock.
static KNOWN: AtomicUsize = AtomicUsize::new(0);

/// Identity of the match the current lineup belongs to, so a stale one is not
/// carried into the next. See [`note_lineups`].
static MATCH_ID: AtomicU64 = AtomicU64::new(0);

/// Records that one of the player's athletes is playing `champion`.
///
/// Called from the tactics half's buy detour, on the branch that has already
/// established the athlete is the player's. That branch is the only place in the
/// mod where "this champion belongs to the player" is known for certain.
///
/// Hot: the detour runs per buy decision, so the common case — a lineup already
/// complete — is one relaxed atomic load.
pub fn note_my_champion(champion: &str) {
    if KNOWN.load(Ordering::Relaxed) >= LINEUP_SIZE || champion.is_empty() {
        return;
    }
    let Ok(mut mine) = MY_CHAMPIONS.lock() else {
        return;
    };
    if mine.iter().any(|known| known.as_str() == champion) || mine.len() >= LINEUP_SIZE {
        return;
    }
    mine.push(champion.to_string());
    KNOWN.store(mine.len(), Ordering::Relaxed);
}

/// Tells this module which match the hook is computing builds for, so a lineup
/// learned in the previous one cannot gate this one.
///
/// The two lineups are the identity: `detour` is called once per team with the
/// pair swapped, so the *unordered* pair is stable within a match and changes
/// with it. Order-independent by construction — a sum of per-champion hashes —
/// because which side arrives as `team1` differs between the two calls.
///
/// A new match retires the lineup into [`PREVIOUS_CHAMPIONS`] rather than
/// dropping it, so the gate has something to work with before this match's
/// first buy.
pub fn note_lineups(team1: &[String], team2: &[String]) {
    let identity = team1
        .iter()
        .chain(team2.iter())
        .map(|champion| {
            // FNV-1a, summed rather than chained so the result does not depend
            // on the order the two lineups arrive in.
            champion.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
        .fold(0u64, u64::wrapping_add);

    if MATCH_ID.swap(identity, Ordering::Relaxed) == identity {
        return;
    }
    if let Ok(mut mine) = MY_CHAMPIONS.lock() {
        if !mine.is_empty() {
            if let Ok(mut previous) = PREVIOUS_CHAMPIONS.lock() {
                *previous = std::mem::take(&mut *mine);
            }
        }
        mine.clear();
    }
    KNOWN.store(0, Ordering::Relaxed);
}

/// Whether the team being built for is the player's own, given both lineups.
///
/// Comparative rather than absolute: whichever of the two shares more champions
/// with the player's known lineup is the player's. That is what lets a lineup
/// from the *previous* match answer usefully — this match's picks need only
/// resemble the player's own last five more than the opponent's do, which is a
/// far weaker claim than being identical to them.
///
/// When this match's lineup is known it is exact anyway: a draft gives each
/// champion to at most one side, so any overlap at all settles it.
///
/// A tie — including nothing known, and both lineups overlapping equally —
/// returns `true`. See the fail-open note in the module docs.
pub fn owns_lineup(lineup: &[String], opponents: &[String]) -> bool {
    let known = if KNOWN.load(Ordering::Relaxed) > 0 {
        MY_CHAMPIONS.lock().ok().map(|mine| mine.clone())
    } else {
        PREVIOUS_CHAMPIONS.lock().ok().map(|previous| previous.clone())
    };
    let Some(known) = known.filter(|known| !known.is_empty()) else {
        return true;
    };

    let overlap = |lineup: &[String]| {
        lineup
            .iter()
            .filter(|champion| known.contains(*champion))
            .count()
    };
    overlap(lineup) >= overlap(opponents)
}
