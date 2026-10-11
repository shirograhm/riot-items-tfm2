use mod_api_stable::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::config::ItemConfig;
use crate::{add_stack, apply_config, ticks, ItemMeta};

// Wrath and Ruin: Using an Ability grants 5% critical strike chance for 5 seconds (max 5 stacks).
// Salvage the Wreckage: Using an Ability has a <crit_icon> chance to grant you a shield for 3 seconds that absorbs 95 - 260 (based on level) damage.
//
// Both answer to the cast, whatever it hits. No hook reports one, so it is
// read the way Eternity reads it: an ability's remaining cooldown going up
// between two ticks.
#[derive(Clone, Debug)]
pub struct RiteOfRuin {
    meta: ItemMeta,
    stack_crit_buff: &'static str,
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    crit_chance: i32,
    effect_stack_crit_chance: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_shield_seconds: f64,
    effect_min_shield: usize,
    effect_max_shield: usize,
    // Non-vital stats (internals)
    // The carrier's remaining ability cooldowns (skill, skill2, ult) last
    // tick. `None` until the first reading after a spawn, which is only a
    // baseline.
    last_cooldowns: Option<(usize, usize, usize)>,
}

impl RiteOfRuin {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "rite_of_ruin",
                &["staff_of_rapture"],
                &["radiant_rite_of_ruin"],
            ),
            stack_crit_buff: "rite_of_ruin_crit_buff",
            price: 700,
            magic_power: 55,
            skill_cooldown_mult: 10,
            crit_chance: 20,
            effect_stack_crit_chance: 5,
            effect_max_stacks: 5,
            effect_duration_seconds: 5.0,
            effect_shield_seconds: 3.0,
            effect_min_shield: 35,
            effect_max_shield: 90,
            // Non-vital stats (internals)
            last_cooldowns: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_rite_of_ruin", &["rite_of_ruin"]),
            stack_crit_buff: "rite_of_ruin_crit_buff",
            price: 1000,
            magic_power: 95,
            skill_cooldown_mult: 15,
            crit_chance: 25,
            effect_stack_crit_chance: 5,
            effect_max_stacks: 5,
            effect_duration_seconds: 5.0,
            effect_shield_seconds: 3.0,
            effect_min_shield: 65,
            effect_max_shield: 175,
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
                magic_power,
                skill_cooldown_mult,
                crit_chance,
                effect_stack_crit_chance,
                effect_max_stacks,
                effect_duration_seconds,
                effect_shield_seconds,
                effect_min_shield,
                effect_max_shield,
            ]
        );
        self
    }

    fn shield_amount(&self, level: usize) -> usize {
        let per_level =
            ((self.effect_max_shield - self.effect_min_shield) as f64 / 11.0).round() as usize;
        self.effect_min_shield + level.saturating_sub(1) * per_level
    }

    // Crit chance Wrath and Ruin grants at full stacks, for Smart Builds.
    pub fn max_passive_crit(&self) -> i32 {
        self.effect_stack_crit_chance * self.effect_max_stacks as i32
    }
}

impl Default for RiteOfRuin {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RiteOfRuin {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.last_cooldowns = None;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, rng_seed: u64, player: usize) {
        let cooldowns = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, skill, skill2, ult)| (skill, skill2, ult));
        let cast = matches!(
            (cooldowns, self.last_cooldowns),
            (Some(now), Some(before))
                if now.0 > before.0 || now.1 > before.1 || now.2 > before.2
        );
        self.last_cooldowns = cooldowns;
        if !cast {
            return;
        }

        let Some((caster, level, crit_chance)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.level(), c.stat().crit_chance))
        else {
            return;
        };

        let mut rng = StdRng::seed_from_u64(rng_seed);
        let roll: f64 = rng.random::<f64>();

        if roll < (crit_chance as f64 / 100.0) {
            ctx.entity_add_shield(
                caster,
                self.shield_amount(level),
                ticks(self.effect_shield_seconds),
            );
        }

        let duration = ticks(self.effect_duration_seconds);
        add_stack(
            ctx,
            caster,
            &BuffV1 {
                crit_chance: self.effect_stack_crit_chance,
                ..BuffV1::timed(self.stack_crit_buff, duration)
            },
            self.effect_max_stacks,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
