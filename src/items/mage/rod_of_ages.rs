use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ticks, upgrade_carry, Elapsed, Eternity, ItemMeta};

// Timeless: the item grows while it is held. Every
// `effect_growth_interval_seconds` it gains a stack of health, Ability Power
// and Ability Haste, up to `effect_max_growth_stacks`. The time counts whether
// the carrier is alive or not, and the Radiant item carries on from where the
// base item was.
//
// Eternity is Catalyst of Aeons' passive, kept ([`crate::Eternity`]): a heal
// for every Ability cast.

// Timeless's stats on the carrier. One name for both tiers, so what the base
// item granted stays on through the Radiant upgrade and is counted once.
const TIMELESS_BUFF: &str = "riot_timeless";
// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "rod_of_ages";

#[derive(Clone, Debug)]
pub struct RodOfAges {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_growth_hp: i32,
    effect_growth_magic_power: i32,
    effect_growth_skill_cooldown_mult: i32,
    effect_growth_interval_seconds: f64,
    effect_max_growth_stacks: usize,
    effect_min_heal: usize,
    effect_max_heal: usize,
    // Non-vital stats (internals)
    // How long the item has been held, in ticks, up to the time the last
    // Timeless stack takes.
    held_ticks: usize,
    // Whether this instance has taken over the stacks of the item it replaced.
    inherited: bool,
    // Counts the time gone by, so Timeless keeps growing through a death.
    clock: Elapsed,
    eternity: Eternity,
}

impl RodOfAges {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(BASE_KEY, &["catalyst_of_aeons"], &["radiant_rod_of_ages"]),
            price: 700,
            hp: 150,
            magic_power: 40,
            skill_cooldown_mult: 5,
            effect_growth_hp: 10,
            effect_growth_magic_power: 2,
            effect_growth_skill_cooldown_mult: 1,
            effect_growth_interval_seconds: 30.0,
            effect_max_growth_stacks: 10,
            effect_min_heal: 15,
            effect_max_heal: 40,
            // Non-vital stats (internals)
            held_ticks: 0,
            inherited: false,
            clock: Elapsed::default(),
            eternity: Eternity::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_rod_of_ages", &[BASE_KEY]),
            price: 1000,
            hp: 250,
            magic_power: 65,
            skill_cooldown_mult: 10,
            effect_growth_hp: 10,
            effect_growth_magic_power: 2,
            effect_growth_skill_cooldown_mult: 1,
            effect_growth_interval_seconds: 30.0,
            effect_max_growth_stacks: 10,
            effect_min_heal: 15,
            effect_max_heal: 40,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                hp,
                magic_power,
                skill_cooldown_mult,
                effect_growth_hp,
                effect_growth_magic_power,
                effect_growth_skill_cooldown_mult,
                effect_growth_interval_seconds,
                effect_max_growth_stacks,
                effect_min_heal,
                effect_max_heal
            ]
        );
        self
    }

    // Ticks between two Timeless stacks.
    fn growth_every(&self) -> usize {
        ticks(self.effect_growth_interval_seconds).max(1)
    }

    // Timeless stacks gained so far.
    fn timeless_stacks(&self) -> usize {
        (self.held_ticks / self.growth_every()).min(self.effect_max_growth_stacks)
    }

    // Sets the time held to what `stacks` Timeless stacks take, clamped to
    // this item's own ceiling.
    fn set_timeless_stacks(&mut self, stacks: usize) {
        self.held_ticks = stacks.min(self.effect_max_growth_stacks) * self.growth_every();
    }

    // What `stacks` Timeless stacks are worth, as one buff.
    fn timeless_buff(&self, stacks: usize) -> BuffV1 {
        let stacks = stacks as i32;
        BuffV1 {
            hp: self.effect_growth_hp * stacks,
            magic_power: self.effect_growth_magic_power * stacks,
            skill_cooldown_mult: self.effect_growth_skill_cooldown_mult * stacks,
            ..BuffV1::named(TIMELESS_BUFF)
        }
    }

    // The Radiant item arrives as a fresh instance: it takes over the base
    // item's stacks once, before it first counts its own. Only the count
    // moves. What those stacks grant is already on the champion as
    // `TIMELESS_BUFF`, and `on_spawn` puts it back from the count on the next
    // respawn.
    fn inherit_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        if std::mem::replace(&mut self.inherited, true) || !self.meta.upgrades_from(BASE_KEY) {
            return;
        }
        if let Some((_, stacks)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
            if stacks as usize > self.timeless_stacks() {
                self.set_timeless_stacks(stacks as usize);
            }
        }
    }
}

impl Default for RodOfAges {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RodOfAges {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.inherit_stacks(ctx, player);
        self.eternity.reset();

        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        // Same-name buffs stack and this carries every stack gained so far, so
        // the copies worn last life go before the new one lands.
        ctx.entity_remove_buff(champion, TIMELESS_BUFF);
        let stacks = self.timeless_stacks();
        if stacks > 0 {
            ctx.add_buff(champion, &self.timeless_buff(stacks));
        }
    }

    // Grows Timeless by the time gone by and heals for an Ability cast since
    // the last tick.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.inherit_stacks(ctx, player);

        // One tick, or the whole of a death on the first update after it: no
        // `update` runs for a dead carrier, and the item is held all the same.
        let gone = self.clock.since_last(ctx);
        let before = self.timeless_stacks();
        let full = self.effect_max_growth_stacks * self.growth_every();
        self.held_ticks = (self.held_ticks + gone).min(full);
        let stacks = self.timeless_stacks();
        if stacks > before {
            upgrade_carry::note(BASE_KEY, ctx, player, stacks as u64);
            // Added on top of the stacks already worn rather than replacing
            // them: taking health off and putting it back mid-fight is not
            // the same as leaving it on.
            let champion = ctx
                .get_player(player)
                .and_then(|p| p.champion())
                .filter(|c| c.is_alive())
                .map(|c| c.id());
            if let Some(champion) = champion {
                ctx.add_buff(champion, &self.timeless_buff(stacks - before));
            }
        }

        self.eternity
            .update(ctx, player, self.effect_min_heal, self.effect_max_heal);
    }

    // Timeless stacks survive the Radiant upgrade, clamped to the successor's
    // own ceiling in case the config gives the two variants different caps.
    //
    // The host of game 0.6.2 never calls these two (`crate::upgrade_carry`);
    // `inherit_stacks` does the carrying, and they stay for a host that does.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.timeless_stacks() as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.set_timeless_stacks(carry as usize);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
