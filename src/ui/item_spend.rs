//! What an item has cost the athlete holding it, for the two tooltips a match
//! shows an item in.
//!
//! An item's price is the price of one step: what it costs to build it out of
//! the item below it. That is what the game's tooltip shows, so a Radiant an
//! athlete has put five purchases into reads as the last of them. Nor is the
//! whole of it a number the item has: most items can be built out of more
//! than one thing (`previous_tier` lists alternatives, not ingredients), each
//! at its own price, so what was paid depends on the way the athlete took.
//!
//! So the way is watched. The match hook reads what every athlete of the
//! match on screen holds, on every tick ([`on_match_tick`]), and keeps for
//! each item slot every item it has held ([`Ledger`]): an upgrade replaces
//! the item in its slot and is charged at the new item's price, so the sum of
//! those prices is what the slot has cost.
//!
//! * The game's own tooltip, over an item an athlete holds ([`sync`]): its
//!   price label is written with what that athlete has paid for the item.
//!   Whose slot is under the cursor is found the way the Check Tactics board
//!   finds its own ([`hovered`]).
//! * The Check Tactics board's tooltip, over an item an athlete is to buy
//!   ([`planned`]): what the item comes to in all once it is built, which is
//!   what its slot has cost so far and the steps still to go.
//!
//! Where nothing was watched (a spectated match is not simulated through the
//! hook) the way is taken to be the first one each item names
//! ([`route_to`]).
//!
//! Not seen in game yet. The names game code gives the player rows are not
//! known: `spend.rows` in the test log says what they are, and an item whose
//! slot could not be placed is still answered for by whoever holds it
//! ([`spent`]).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use mod_api_stable::{SimOriginKindV1, StableClient, StableSim};
use serde_json::Value;

use super::match_builds::{self, LANES};

// -- prices and build paths ---------------------------------------------------

// What the sums are made of, for one item.
struct Entry {
    // The price of the step up to it.
    price: usize,
    // What it can be built out of, any one of them, and what it is built
    // into.
    below: Vec<String>,
    above: Vec<String>,
}

// This mod's items as they register, in that order.
static MOD_ITEMS: Mutex<Vec<(String, Entry)>> = Mutex::new(Vec::new());

// Notes one of this mod's items as it is registered
// (`match_builds::note_mod_item`).
pub(super) fn note_mod_item(key: &str, price: usize, below: Vec<String>, above: Vec<String>) {
    if let Ok(mut items) = MOD_ITEMS.lock() {
        items.push((
            key.to_string(),
            Entry {
                price,
                below,
                above,
            },
        ));
    }
}

// Every item by key: the game's own thirty from the mod's settings file,
// which has the prices matches are played at, and this mod's as they
// registered. Made on first use, which is in a match and long after the last
// item registered.
//
// One of the game's items only says what it is built into, so every link is
// filled in at both of its ends. In the order the items came in, the game's
// first: the first way into an item is then the same one on every run.
fn catalog() -> &'static HashMap<String, Entry> {
    static CATALOG: OnceLock<HashMap<String, Entry>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut listed: Vec<(String, Entry)> = Vec::new();
        crate::item_stats::each_item(
            crate::item_stats::game_item_file(),
            0,
            &mut |key: &str, object: &serde_json::Map<String, Value>| {
                let price = object
                    .get("price")
                    .and_then(|price| {
                        price
                            .as_u64()
                            .or_else(|| price.as_f64().map(|price| price as u64))
                    })
                    .unwrap_or(0) as usize;
                let above = object
                    .get("next_tier")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|next| next.as_str().map(str::to_string))
                    .collect();
                listed.push((
                    key.to_string(),
                    Entry {
                        price,
                        below: Vec::new(),
                        above,
                    },
                ));
            },
        );
        if let Ok(mut items) = MOD_ITEMS.lock() {
            listed.append(&mut items);
        }

        // (below, above)
        let mut links: Vec<(String, String)> = Vec::new();
        for (key, entry) in &listed {
            links.extend(entry.above.iter().map(|above| (key.clone(), above.clone())));
            links.extend(entry.below.iter().map(|below| (below.clone(), key.clone())));
        }
        let mut items: HashMap<String, Entry> = listed.into_iter().collect();
        for (below, above) in links {
            if let Some(entry) = items.get_mut(&above) {
                if !entry.below.contains(&below) {
                    entry.below.push(below.clone());
                }
            }
            if let Some(entry) = items.get_mut(&below) {
                if !entry.above.contains(&above) {
                    entry.above.push(above);
                }
            }
        }
        // A link to an item nobody registered leads nowhere a price is known.
        let known: HashSet<String> = items.keys().cloned().collect();
        for entry in items.values_mut() {
            entry.below.retain(|key| known.contains(key));
            entry.above.retain(|key| known.contains(key));
        }
        items
    })
}

// Steps a way up is followed for at most: more than there are tiers. What
// ends a walk through links that run in a circle.
const MAX_STEPS: usize = 8;

// The way up to `key` from the item its line starts with, `key` last, by the
// first item each one names as built out of: the way an athlete is taken to
// have gone where nobody watched it go.
fn route_to(items: &HashMap<String, Entry>, key: &str) -> Vec<String> {
    let mut route = vec![key.to_string()];
    let mut at = key;
    while let Some(below) = items.get(at).and_then(|entry| entry.below.first()) {
        if route.len() >= MAX_STEPS || route.contains(below) {
            break;
        }
        route.push(below.clone());
        at = below;
    }
    route.reverse();
    route
}

// The purchases that turn `from` into `to`, `to` last and `from` left out:
// the fewest there can be, and of two ways as short the one through the item
// named first. Nothing where `from` is not built into `to` at all.
fn steps<'a>(items: &'a HashMap<String, Entry>, from: &str, to: &str) -> Option<Vec<String>> {
    if from == to {
        return Some(Vec::new());
    }
    // Each item reached and the one it was reached from, which is nothing
    // for one built straight out of `from`.
    let mut came: HashMap<&'a str, Option<&'a str>> = HashMap::new();
    let mut queue: VecDeque<Option<&'a str>> = VecDeque::from([None]);
    while let Some(at) = queue.pop_front() {
        let Some(entry) = items.get(at.unwrap_or(from)) else {
            continue;
        };
        for next in &entry.above {
            let next = next.as_str();
            if next == from || came.contains_key(next) {
                continue;
            }
            came.insert(next, at);
            if next == to {
                let mut way = vec![next.to_string()];
                let mut at = at;
                while let Some(below) = at {
                    way.push(below.to_string());
                    at = came.get(below).copied().flatten();
                }
                way.reverse();
                return Some(way);
            }
            queue.push_back(Some(next));
        }
    }
    None
}

// The purchases that turn `from` into `to` as an athlete is taken to make
// them: along the way nobody watched where `from` is on it, so that a slot
// comes to the same before its first purchase and after it, and by the
// fewest steps where it is not.
fn way_from(items: &HashMap<String, Entry>, from: &str, to: &str) -> Option<Vec<String>> {
    let route = route_to(items, to);
    match route.iter().position(|key| key == from) {
        Some(at) => Some(route[at + 1..].to_vec()),
        None => steps(items, from, to),
    }
}

// What buying each of `keys` comes to. Nothing where one of them is no item
// a price is known for: another mod's.
fn cost(items: &HashMap<String, Entry>, keys: &[String]) -> Option<usize> {
    keys.iter()
        .map(|key| items.get(key).map(|entry| entry.price))
        .sum()
}

// -- the match on screen ------------------------------------------------------

// What every athlete of the match on screen has bought.
struct Ledger {
    seed: u64,
    // By side (0 blue, 1 red) and by lane, a list an item slot: every item
    // the slot has held in the order it was bought, the one it holds now
    // last.
    seats: [[Vec<Vec<String>>; LANES]; 2],
}

static LEDGER: Mutex<Option<Ledger>> = Mutex::new(None);

// Whether `sim` is the simulation the client is showing, as the host has it.
pub(super) fn on_screen(sim: &StableSim<'_>) -> bool {
    sim.sim_origin().is_some_and(|origin| {
        matches!(
            SimOriginKindV1::from_code(origin.kind),
            Some(
                SimOriginKindV1::ClientMatchView
                    | SimOriginKindV1::ClientSpectate
                    | SimOriginKindV1::ClientReplay
            )
        )
    })
}

// Follows what the athletes of the match on screen hold. Called from the
// mod's match hook on every tick of every simulation: one read of where the
// simulation runs and a return for any but the client's own.
//
// On every tick and not now and then, because an athlete with the gold for
// it buys a step a tick, and the way it went is only known while each step
// is seen.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>) {
    if !on_screen(sim) {
        return;
    }

    let seed = sim.seed();
    // (side, lane, what the athlete holds)
    let mut held: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let side = player.team();
        let Some(lane) = player
            .lane()
            .map(|lane| lane.code() as usize)
            .filter(|lane| *lane < LANES)
        else {
            continue;
        };
        if side >= 2 {
            continue;
        }
        held.push((side, lane, player.item_keys()));
    }

    let items = catalog();
    let Ok(mut ledger) = LEDGER.lock() else {
        return;
    };
    if ledger.as_ref().is_none_or(|ledger| ledger.seed != seed) {
        *ledger = Some(Ledger {
            seed,
            seats: Default::default(),
        });
    }
    let Some(ledger) = ledger.as_mut() else {
        return;
    };
    for (side, lane, keys) in held {
        let slots = &mut ledger.seats[side][lane];
        // A slot a held item, new ones empty. Fewer items than before is the
        // match wound back.
        slots.resize(keys.len(), Vec::new());
        for (bought, key) in slots.iter_mut().zip(keys) {
            if bought.last() == Some(&key) {
                continue;
            }
            // An item the slot held before: wound back to it.
            if let Some(at) = bought.iter().position(|known| *known == key) {
                bought.truncate(at + 1);
                continue;
            }
            // The step up from what it held, or the steps where some went
            // unseen. And for a slot first seen with more than a starter
            // item in it, or holding something its last item is not built
            // into, the way up nobody watched.
            match bought.last().and_then(|last| way_from(items, last, &key)) {
                Some(way) => bought.extend(way),
                None => *bought = route_to(items, &key),
            }
        }
    }
}

// Forgets the match, as its layout goes. A simulation still running puts
// itself back on its next tick.
pub(super) fn clear() {
    if let Ok(mut ledger) = LEDGER.lock() {
        *ledger = None;
    }
}

// What a held `key` has cost its athlete. `seat` is the slot the cursor is
// over, where that could be found: side, lane where the row said which, and
// slot.
//
// The slot's own sum where it holds `key`. Where the slot could not be
// placed, or holds something else, what everyone holding `key` has paid for
// it, if they have all paid the same: first those on the cursor's side with
// it in the cursor's slot, then anyone. Two athletes only differ where they
// took two ways up to the same item at two prices. And where the match was
// not watched, the way nobody watched.
fn spent(seat: Option<(usize, Option<usize>, usize)>, key: &str) -> Option<usize> {
    let items = catalog();
    if let Ok(ledger) = LEDGER.lock() {
        if let Some(ledger) = ledger.as_ref() {
            let paid = |bought: &Vec<String>| {
                (bought.last().map(String::as_str) == Some(key))
                    .then(|| cost(items, bought))
                    .flatten()
            };
            if let Some((side, Some(lane), slot)) = seat {
                let here = ledger
                    .seats
                    .get(side)
                    .and_then(|lanes| lanes.get(lane))
                    .and_then(|slots| slots.get(slot))
                    .and_then(paid);
                if here.is_some() {
                    return here;
                }
            }
            let mut same_slot = Vec::new();
            let mut anyone = Vec::new();
            for (side, lanes) in ledger.seats.iter().enumerate() {
                for slots in lanes {
                    for (slot, bought) in slots.iter().enumerate() {
                        let Some(sum) = paid(bought) else {
                            continue;
                        };
                        let cursors =
                            seat.is_some_and(|(at, _, slot_at)| at == side && slot_at == slot);
                        if cursors {
                            same_slot.push(sum);
                        }
                        anyone.push(sum);
                    }
                }
            }
            for sums in [same_slot, anyone] {
                if let Some(first) = sums.first() {
                    if sums.iter().all(|sum| sum == first) {
                        return Some(*first);
                    }
                }
            }
        }
    }
    cost(items, &route_to(items, key))
}

// What the item an athlete is to buy in a slot comes to in all: what the
// slot has cost so far, and the steps from what it holds up to `key`. `seed`
// is the match the board is drawn for and `spot` the slot's side, lane and
// place in the build, which is its place among what the athlete holds: a
// build is bought in order.
//
// A slot nothing has been bought for yet, or holding something `key` is not
// built out of, comes to the way nobody watched.
pub(super) fn planned(
    seed: Option<u64>,
    (side, lane, slot): (usize, usize, usize),
    key: &str,
) -> Option<usize> {
    let items = catalog();
    let watched = LEDGER.lock().ok().and_then(|ledger| {
        let ledger = ledger.as_ref().filter(|ledger| seed == Some(ledger.seed))?;
        let bought = ledger.seats.get(side)?.get(lane)?.get(slot)?;
        let left = way_from(items, bought.last()?, key)?;
        Some(cost(items, bought)? + cost(items, &left)?)
    });
    watched.or_else(|| cost(items, &route_to(items, key)))
}

// -- the game's tooltip -------------------------------------------------------

// The hover tooltip of an item slot in a match, and the two labels in it
// this reads and writes.
const TOOLTIP: &str = "ingame.item_tooltip";
const NAME: &str = "ingame.item_tooltip.data.name";
const PRICE: &str = "ingame.item_tooltip.data.price";

// Where the two views put their player rows: the node that is shown while
// the view is up, and the one game code spawns the rows into, one a lane,
// each with a half for either side and its item slots in `items`.
pub(super) const ROW_HOSTS: [(&str, &str); 2] = [
    ("ingame.player_info", "ingame.player_info"),
    ("ingame.wide_data", "ingame.wide_data.player_info"),
];
pub(super) const HALVES: [&str; 2] = ["blue_player", "red_player"];

// The lane of the player row at `place` among `rows`, a host's children: the
// one the row is named for, as the camera buttons' nodes are, or the row's
// place where there are five of them; nothing where neither says.
pub(super) fn row_lane(rows: &[String], place: usize) -> Option<usize> {
    match_builds::lane_of(rows.get(place)?).or((rows.len() == LANES).then_some(place))
}

// The item slots game code has spawned into `items`, a half of a player row,
// by path and in the order of the items they show: by the number their
// names end in where they all have one, and else as they come. What game
// code calls them is not taken for granted: `slot0` was, and in game
// (2026-10-14) no `items.slot0` was there to be found.
pub(super) fn slot_nodes(ctx: &StableClient<'_>, items: &str) -> Vec<String> {
    let mut names = ctx.ui_child_names(items);
    let number = |name: &str| {
        name.trim_start_matches(|c: char| !c.is_ascii_digit())
            .parse::<usize>()
            .ok()
    };
    if names.iter().all(|name| number(name).is_some()) {
        names.sort_by_key(|name| number(name));
    }
    names.into_iter().map(|name| format!("{items}.{name}")).collect()
}

// The item slot of a player row the cursor is over: side, lane, slot.
//
// The lane is the row's ([`row_lane`]). [`spent`] checks the answer against
// what that athlete holds, so a row placed wrongly costs nothing but the
// check.
fn hovered(ctx: &StableClient<'_>) -> Option<(usize, Option<usize>, usize)> {
    let (x, y) = match_builds::cursor(ctx)?;
    let under = |path: &str| {
        ctx.ui_node_rect(path)
            .is_some_and(|(left, top, width, height)| {
                x >= left && x < left + width && y >= top && y < top + height
            })
    };
    for (view, host) in ROW_HOSTS {
        if ctx.ui_visible(view) != Some(true) || !under(host) {
            continue;
        }
        let rows = ctx.ui_child_names(host);
        match_builds::log("spend.rows", || format!("{host} holds {rows:?}"));
        for (place, row) in rows.iter().enumerate() {
            let row_path = format!("{host}.{row}");
            if !under(&row_path) {
                continue;
            }
            let lane = row_lane(&rows, place);
            for (side, half) in HALVES.iter().enumerate() {
                let items = format!("{row_path}.{half}.items");
                if !under(&items) {
                    continue;
                }
                return slot_nodes(ctx, &items)
                    .iter()
                    .position(|slot| under(slot))
                    .map(|slot| (side, lane, slot));
            }
            return None;
        }
    }
    None
}

// Writes what the item under the cursor has cost its athlete into the game's
// tooltip, in place of the item's own price. Every client frame: outside a
// match one failed lookup, and in one a single read while no tooltip is up.
//
// The label is written whenever it does not say the sum, which covers both
// ways game code may fill it: once when the hover starts, or every frame.
// An item this cannot put a sum to (another mod's) keeps its price.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    if ctx.ui_visible(TOOLTIP) != Some(true) {
        return;
    }
    let Some(key) = ctx
        .ui_text(NAME)
        .and_then(|name| crate::patches::text::texts().key_of_name(&name))
    else {
        return;
    };
    let seat = hovered(ctx);
    let Some(paid) = spent(seat, key) else {
        return;
    };
    let wanted = paid.to_string();
    let shown = ctx.ui_text(PRICE);
    if shown.as_deref() == Some(wanted.as_str()) {
        return;
    }
    match_builds::log(&format!("spend.tip.{key}"), || {
        format!("{shown:?} -> {wanted}, the cursor over {seat:?}")
    });
    ctx.ui_set_text(PRICE, &wanted);
}
