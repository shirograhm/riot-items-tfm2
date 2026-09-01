use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta, ProcQueue};

#[derive(Clone, Debug)]
pub struct NightHarvester {
    meta: ItemMeta,
    cooldown_buff: &'static str,
    soulrend_buff: &'static str,
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl NightHarvester {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "night_harvester",
                &["haunting_guise"],
                &["radiant_night_harvester"],
            ),
            cooldown_buff: "night_harvester_cooldown",
            soulrend_buff: "night_harvester_soulrend",
            price: 1400,
            hp: 300,
            magic_power: 100,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 100,
            effect_ap_percent_damage: 20.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2.0,
            effect_cooldown_seconds: 45.0,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_night_harvester", &["night_harvester"]),
            cooldown_buff: "radiant_night_harvester_cooldown",
            soulrend_buff: "radiant_night_harvester_soulrend",
            price: 2000,
            hp: 500,
            magic_power: 160,
            effect_bonus_flat_damage: 150,
            effect_ap_percent_damage: 30.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2.0,
            effect_cooldown_seconds: 45.0,
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
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_move_speed_mult,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for NightHarvester {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for NightHarvester {
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
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || is_ally {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        let is_cooldown_ticking = has_buff(&target_ref, self.cooldown_buff);
        if is_cooldown_ticking {
            return;
        }

        ctx.add_buff(
            target,
            &BuffV1::timed(self.cooldown_buff, ticks(self.effect_cooldown_seconds)),
        );
        // Soulrend's haste is the carrier's reward for the cast and lands with
        // it; only the Soulrend damage waits out the delay.
        self.procs.push_magic(ctx, target, bonus_damage);
        ctx.add_buff(
            caster,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(self.soulrend_buff, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    /// Lands the Soulrend damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
