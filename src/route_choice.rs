use mod_api_stable::StableItem;

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::build_config::Role;
use crate::champion_traits::{self, Class, Scaling};

const STATS: [(&str, f64); 16] = [
    ("attack", 25.0),
    ("magic_power", 16.67),
    ("attack_speed_mult", 31.25),
    ("crit_chance", 11.88),
    ("hp", 2.5),
    ("defence", 25.0),
    ("magic_resistance", 8.33),
    ("skill_cooldown_mult", 25.0),
    ("ult_cooldown_mult", 20.83),
    ("move_speed_mult", 68.75),
    ("hp_regen", 66.67),
    ("vamp", 55.0),
    ("defence_penetration", 37.5),
    ("magic_resistance_penetration", 23.33),
    ("toughness", 8.89),
    ("skill_damaged_reduce", 27.08),
];

// Positions in [`STATS`].
const ATTACK: usize = 0;
const MAGIC_POWER: usize = 1;
const ATTACK_SPEED: usize = 2;
const CRIT: usize = 3;
const HEALTH: usize = 4;
const ARMOR: usize = 5;
const MAGIC_RESIST: usize = 6;
const HASTE: usize = 7;
const ULT_HASTE: usize = 8;
const MOVE_SPEED: usize = 9;
const HEALTH_REGEN: usize = 10;
const OMNIVAMP: usize = 11;
const ARMOR_PEN: usize = 12;
const MAGIC_PEN: usize = 13;
const TENACITY: usize = 14;
const SKILL_REDUCTION: usize = 15;

// An item's stats in gold, per [`STATS`] entry.
type Worth = [f64; STATS.len()];

fn worth_of(amount: impl Fn(&str) -> f64) -> Worth {
    let mut worth = [0.0; STATS.len()];
    for (slot, &(name, gold)) in worth.iter_mut().zip(STATS.iter()) {
        *slot = amount(name).max(0.0) * gold;
    }
    worth
}

#[derive(Clone, Default)]
struct Table {
    // This mod's items as `init` registers them; authoritative for its keys.
    mod_items: HashMap<String, Worth>,
    // The game's items from the mod's settings file, other mods' from the
    // client's settings document (`item_stats`).
    engine_items: HashMap<String, Worth>,
}

static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

fn edit_table(edit: impl FnOnce(&mut Table)) {
    if let Ok(mut table) = TABLE.lock() {
        let table = table.get_or_insert_with(|| Arc::new(Table::default()));
        edit(Arc::make_mut(table));
    }
}

fn table() -> Arc<Table> {
    TABLE
        .lock()
        .ok()
        .and_then(|table| table.clone())
        .unwrap_or_default()
}

// Records one of this mod's items from the registration macros in `lib.rs`.
pub(crate) fn note_mod_item<T: StableItem + ?Sized>(key: &str, item: &T) {
    let stat = item.stat();
    let worth = worth_of(|name| {
        (match name {
            "attack" => stat.attack,
            "magic_power" => stat.magic_power,
            "attack_speed_mult" => stat.attack_speed_mult,
            "crit_chance" => stat.crit_chance,
            "hp" => stat.hp,
            "defence" => stat.defence,
            "magic_resistance" => stat.magic_resistance,
            "skill_cooldown_mult" => stat.skill_cooldown_mult,
            "ult_cooldown_mult" => stat.ult_cooldown_mult,
            "move_speed_mult" => stat.move_speed_mult,
            "hp_regen" => stat.hp_regen,
            "vamp" => stat.vamp,
            "defence_penetration" => stat.defence_penetration as i32,
            "magic_resistance_penetration" => stat.magic_resistance_penetration as i32,
            "toughness" => stat.toughness as i32,
            "skill_damaged_reduce" => stat.skill_damaged_reduce as i32,
            _ => 0,
        }) as f64
    });
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), worth);
    });
}

// Records one of the game's items (or another mod's) from a settings object;
// `stat` reads one of its stats by field name.
pub(crate) fn note_engine_item(key: &str, stat: impl Fn(&str) -> i64) {
    let worth = worth_of(|name| stat(name) as f64);
    edit_table(|table| {
        // A later object under the same key with no stats must not undo the
        // first.
        let known = table.engine_items.entry(key.to_string()).or_insert(worth);
        if worth.iter().any(|&gold| gold > 0.0) {
            *known = worth;
        }
    });
}

// -- what the champion wants ------------------------------------------------------

// How much each [`STATS`] entry counts toward a route, for one champion in one
// match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Weights([f64; STATS.len()]);

impl Weights {
    // `enemies` is the enemy team's (physical, magic) count, `None` when the
    // match's lineup is unknown, which weighs armor and magic resist alike.
    pub(crate) fn new(champion: &str, role: Option<Role>, enemies: Option<(u32, u32)>) -> Self {
        let traits = champion_traits::traits(champion).unwrap_or_default();
        let support = role == Some(Role::Support);
        let marksman = traits.class == Some(Class::Range);
        let (physical, magic) = match traits.scaling {
            Some(Scaling::Ad) => (1.0, 0.0),
            Some(Scaling::Ap) => (0.0, 1.0),
            Some(Scaling::Hybrid) => (0.6, 0.6),
            None => (0.5, 0.5),
        };
        // How much the champion is there to take hits.
        let tanking = if traits.tank {
            1.0
        } else if support || traits.class == Some(Class::Melee) {
            0.7
        } else {
            0.4
        };
        // The enemies' damage, as shares that average 1 over both resists.
        let (armor_share, resist_share) = match enemies {
            Some((p, m)) if p + m > 0 => {
                let total = (p + m) as f64;
                (2.0 * p as f64 / total, 2.0 * m as f64 / total)
            }
            _ => (1.0, 1.0),
        };
        let caster = support || matches!(traits.class, Some(Class::Magician | Class::Util));
        let mut weights = [0.0; STATS.len()];
        weights[ATTACK] = physical;
        weights[MAGIC_POWER] = magic;
        weights[ATTACK_SPEED] = if marksman {
            1.0
        } else {
            0.7 * physical + 0.2 * magic
        };
        weights[CRIT] = if marksman { 1.0 } else { 0.5 * physical };
        weights[ARMOR_PEN] = 0.8 * physical;
        weights[MAGIC_PEN] = 0.8 * magic;
        weights[OMNIVAMP] = 0.4;
        weights[HASTE] = if caster { 0.8 } else { 0.5 };
        weights[ULT_HASTE] = weights[HASTE];
        weights[MOVE_SPEED] = 0.3;
        weights[HEALTH_REGEN] = 0.2;
        weights[HEALTH] = tanking;
        weights[ARMOR] = tanking * armor_share;
        weights[MAGIC_RESIST] = tanking * resist_share;
        weights[TENACITY] = 0.5 * tanking;
        weights[SKILL_REDUCTION] = tanking;
        Self(weights)
    }
}

// What `key`'s stats are worth to a champion with these weights; 0 for an item
// nothing describes.
pub(crate) fn item_score(table: &RouteTable, key: &str, weights: &Weights) -> f64 {
    let Some(worth) = table
        .0
        .mod_items
        .get(key)
        .or_else(|| table.0.engine_items.get(key))
    else {
        return 0.0;
    };
    worth
        .iter()
        .zip(weights.0.iter())
        .map(|(gold, weight)| gold * weight)
        .sum()
}

// The item stats as they are now, taken once per decision.
pub(crate) struct RouteTable(Arc<Table>);

pub(crate) fn route_table() -> RouteTable {
    RouteTable(table())
}

// -- the enemies --------------------------------------------------------------------

// Every champion of a match as it spawned: (side, champion), one entry each.
struct Lineup {
    seed: u64,
    seats: Vec<(u64, String)>,
}

// Matches kept; a session simulates a few at once.
const LINEUPS_KEPT: usize = 64;

static LINEUPS: Mutex<VecDeque<Lineup>> = Mutex::new(VecDeque::new());

// Notes a champion spawning into the match of `seed` on `side`, from the spawn
// detour, which sees all of a match's athletes before it plays.
pub(crate) fn note_spawn(seed: u64, side: u64, champion: &str) {
    if seed == 0 || champion.is_empty() {
        return;
    }
    let Ok(mut lineups) = LINEUPS.lock() else {
        return;
    };
    if let Some(lineup) = lineups.iter_mut().find(|lineup| lineup.seed == seed) {
        if !lineup
            .seats
            .iter()
            .any(|(s, c)| *s == side && c == champion)
        {
            lineup.seats.push((side, champion.to_string()));
        }
        return;
    }
    lineups.push_back(Lineup {
        seed,
        seats: vec![(side, champion.to_string())],
    });
    while lineups.len() > LINEUPS_KEPT {
        lineups.pop_front();
    }
}

// The enemy team's (physical, magic) count for `champion` on `side` in the
// match of `seed`; `None` unless that match's lineup has the champion on that
// side and someone on the other.
pub(crate) fn enemy_damage(seed: u64, side: u64, champion: &str) -> Option<(u32, u32)> {
    if seed == 0 {
        return None;
    }
    let lineups = LINEUPS.lock().ok()?;
    let lineup = lineups.iter().find(|lineup| lineup.seed == seed)?;
    if !lineup
        .seats
        .iter()
        .any(|(s, c)| *s == side && c == champion)
    {
        return None;
    }
    let enemies: Vec<&str> = lineup
        .seats
        .iter()
        .filter(|(s, _)| *s != side)
        .map(|(_, c)| c.as_str())
        .collect();
    (!enemies.is_empty()).then(|| crate::smart_builds::damage_split(&enemies))
}

// -- which match an athlete is in ---------------------------------------------------

// The route picker is not handed the match, so the buy detour, which is, notes
// each athlete's seed on its thread (every copy of a match plays on one thread
// from start to finish, and buys before it upgrades or checks a price).
const SEATS: usize = 16;

struct Seats {
    slots: [Cell<(usize, u64)>; SEATS],
    next: Cell<usize>,
}

thread_local! {
    static THREAD_SEATS: Seats = const {
        Seats {
            slots: [const { Cell::new((0, 0)) }; SEATS],
            next: Cell::new(0),
        }
    };
}

// Notes that `athlete` plays in the match of `seed`, from the buy detour.
pub(crate) fn note_seat(athlete: usize, seed: u64) {
    if athlete == 0 || seed == 0 {
        return;
    }
    let _ = THREAD_SEATS.try_with(|seats| {
        for slot in &seats.slots {
            let (held, was) = slot.get();
            if held == athlete {
                if was != seed {
                    slot.set((athlete, seed));
                }
                return;
            }
        }
        let next = seats.next.get();
        seats.slots[next % SEATS].set((athlete, seed));
        seats.next.set(next.wrapping_add(1));
    });
}

// The seed [`note_seat`] last noted for `athlete` on this thread, 0 if none.
pub(crate) fn seat_seed(athlete: usize) -> u64 {
    THREAD_SEATS
        .try_with(|seats| {
            seats
                .slots
                .iter()
                .map(Cell::get)
                .find(|&(held, _)| held == athlete)
                .map_or(0, |(_, seed)| seed)
        })
        .unwrap_or(0)
}
