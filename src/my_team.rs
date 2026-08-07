use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

const LINEUP_SIZE: usize = 5;

static MY_CHAMPIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PREVIOUS_CHAMPIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

static KNOWN: AtomicUsize = AtomicUsize::new(0);

static MATCH_ID: AtomicU64 = AtomicU64::new(0);

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

pub fn note_lineups(team1: &[&str], team2: &[&str]) {
    let identity = team1
        .iter()
        .chain(team2.iter())
        .map(|champion| {
            champion
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
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

pub fn owns_lineup(lineup: &[&str], opponents: &[&str]) -> bool {
    let known = if KNOWN.load(Ordering::Relaxed) > 0 {
        MY_CHAMPIONS.lock().ok().map(|mine| mine.clone())
    } else {
        PREVIOUS_CHAMPIONS
            .lock()
            .ok()
            .map(|previous| previous.clone())
    };
    let Some(known) = known.filter(|known| !known.is_empty()) else {
        return true;
    };

    let overlap = |lineup: &[&str]| {
        lineup
            .iter()
            .filter(|champion| known.iter().any(|mine| mine == *champion))
            .count()
    };
    overlap(lineup) >= overlap(opponents)
}
