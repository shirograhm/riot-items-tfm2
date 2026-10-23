//! The base stats `config.json` can give any of this mod's items, whether or
//! not the item has them: `"deaths_dance": { "magic_resistance": 20 }` gives
//! Death's Dance 20 magic resistance on top of its own stats. A stat the item
//! has of its own it reads itself, as it always has; [`WithStats`] adds the
//! others to its stat block, which is what the game, the tooltips, Smart
//! Builds and the balance patches all read.
//!
//! Which stats an item reads itself is found as it registers ([`learn`]), so a
//! new item needs nothing here.

use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::RwLock;

use mod_api_stable::*;

use crate::config::ItemConfig;

macro_rules! stats {
    ($($field:ident),* $(,)?) => {
        // The stats, by their name in `config.json`, which is also their name
        // in a stat block.
        pub(crate) const FIELDS: &[&str] = &[$(stringify!($field)),*];

        // What `config` gives of each of [`FIELDS`], in that order.
        fn given(config: &ItemConfig) -> [Option<i64>; COUNT] {
            [$(config.$field.map(|number| number as i64)),*]
        }

        // `stat` with `extra`, a number for each of [`FIELDS`], added.
        fn add(mut stat: BuffV1, extra: &[i64; COUNT]) -> BuffV1 {
            let mut extra = extra.iter().copied();
            $(
                let number = extra.next().unwrap_or(0);
                stat.$field = (stat.$field as i64 + number) as _;
            )*
            stat
        }
    };
}

stats! {
    attack,
    magic_power,
    attack_speed_mult,
    crit_chance,
    hp,
    defence,
    magic_resistance,
    defence_penetration,
    magic_resistance_penetration,
    move_speed_mult,
    toughness,
    skill_damaged_reduce,
}

const COUNT: usize = FIELDS.len();

// Every one of [`FIELDS`]: what an item that was never learned is taken to
// read itself, so nothing is added to it twice.
const ALL: u32 = (1 << COUNT) - 1;

// Two numbers an item is made with to see whether it reads a stat, far enough
// apart that no rounding of the item's own makes them one.
const PROBES: [i64; 2] = [100, 200];

// Item key -> which of [`FIELDS`] it reads itself, a bit each.
static OWN: RwLock<Option<HashMap<&'static str, u32>>> = RwLock::new(None);

// Notes which of [`FIELDS`] the item `key` reads itself: made with a stat at
// each of [`PROBES`], an item that reads it comes out two different items.
// Once an item as it registers, ahead of its first [`WithStats::new`].
pub(crate) fn learn<T: Debug>(key: &'static str, build: fn(&ItemConfig) -> T) {
    let made = |field: &str, number: i64| {
        serde_json::from_value::<ItemConfig>(serde_json::json!({ field: number }))
            .ok()
            .map(|config| format!("{:?}", build(&config)))
    };
    let own = FIELDS
        .iter()
        .enumerate()
        .filter(|&(_, &field)| made(field, PROBES[0]) != made(field, PROBES[1]))
        .fold(0u32, |own, (index, _)| own | (1 << index));
    if let Ok(mut table) = OWN.write() {
        table.get_or_insert_with(HashMap::new).insert(key, own);
    }
}

// Which of [`FIELDS`] the item `key` reads itself, as [`learn`] found.
fn own_of(key: &str) -> u32 {
    OWN.read()
        .ok()
        .and_then(|table| table.as_ref()?.get(key).copied())
        .unwrap_or(ALL)
}

// An item with the stats its config gives that it does not read itself;
// derefs to the item.
#[derive(Clone)]
pub struct WithStats<T> {
    inner: T,
    extra: [i64; COUNT],
}

impl<T> Deref for WithStats<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> WithStats<T> {
    // `item`, registered as `key`, with what `config` gives of the stats it
    // does not read itself. No config, no stats added.
    pub fn new(key: &str, item: T, config: Option<&ItemConfig>) -> Self {
        let mut extra = [0; COUNT];
        if let Some(config) = config {
            let own = own_of(key);
            for (index, number) in given(config).into_iter().enumerate() {
                if own & (1 << index) == 0 {
                    extra[index] = number.unwrap_or(0);
                }
            }
        }
        Self { inner: item, extra }
    }
}

impl<T: StableItem + Clone> StableItem for WithStats<T> {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.inner.key()
    }

    fn icon(&self) -> String {
        self.inner.icon()
    }

    fn price(&self) -> usize {
        self.inner.price()
    }

    fn tier(&self) -> usize {
        self.inner.tier()
    }

    fn stat(&self) -> BuffV1 {
        add(self.inner.stat(), &self.extra)
    }

    fn next_tier(&self) -> Vec<String> {
        self.inner.next_tier()
    }

    fn previous_tier(&self) -> Vec<String> {
        self.inner.previous_tier()
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        self.inner.tags()
    }

    fn category(&self) -> ItemCategoryV1 {
        self.inner.category()
    }

    fn on_attack(
        &mut self,
        sim: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        self.inner
            .on_attack(sim, caster, target, damage, damage_type, attack_type, is_crit)
    }

    fn on_base_attack(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        player: usize,
        entity: usize,
    ) {
        self.inner.on_base_attack(sim, rng_seed, player, entity)
    }

    fn update(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize) {
        self.inner.update(sim, rng_seed, player)
    }

    fn on_spawn(&mut self, sim: &mut StableSim<'_>, player: usize) {
        self.inner.on_spawn(sim, player)
    }

    fn on_healed(
        &mut self,
        sim: &mut StableSim<'_>,
        caster: Option<usize>,
        entity: usize,
        heal: usize,
    ) {
        self.inner.on_healed(sim, caster, entity, heal)
    }

    fn on_damaged(
        &mut self,
        sim: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        self.inner.on_damaged(
            sim,
            player,
            entity,
            attacker,
            damage,
            damage_type,
            attack_type,
            is_crit,
        )
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        player: usize,
        entity: usize,
        victim: usize,
    ) {
        self.inner.on_kill(sim, rng_seed, player, entity, victim)
    }

    fn on_assist(&mut self, sim: &mut StableSim<'_>, player: usize, entity: usize) {
        self.inner.on_assist(sim, player, entity)
    }

    fn on_dead(&mut self, sim: &mut StableSim<'_>, player: usize) {
        self.inner.on_dead(sim, player)
    }

    fn on_cc(&mut self, sim: &mut StableSim<'_>, rng_seed: u64, player: usize, caster: usize) {
        self.inner.on_cc(sim, rng_seed, player, caster)
    }

    fn on_skill_hit(
        &mut self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        self.inner.on_skill_hit(sim, rng_seed, caster, target, is_ally)
    }

    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        self.inner.on_upgrade(next_key)
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        self.inner.on_upgraded_from(prev_key, carry)
    }
}
