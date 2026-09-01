use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

const BURN_PROCS_PER_SECOND: f64 = 5.0;

#[derive(Clone, Debug)]
pub struct DeathsDance {
    meta: ItemMeta,
    burn_buff: &'static str,
    price: usize,
    attack: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_delayed_damage_percent: f64,
    effect_burn_hp_percent_cap: f64,
    effect_bonus_flat_heal: i32,
    effect_kill_heal_missing_percent: f64,
    accumulated_damage: i32,
    self_inflicted_credit: i32,
    last_damaged_by: usize,
}

impl DeathsDance {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "deaths_dance",
                &["steel_sigil", "caulfields_warhammer"],
                &["radiant_deaths_dance"],
            ),
            burn_buff: "deaths_dance_burn",
            price: 1450,
            attack: 45,
            defence: 45,
            skill_cooldown_mult: 10,
            effect_delayed_damage_percent: 25.0,
            effect_burn_hp_percent_cap: 2.5,
            effect_bonus_flat_heal: 45,
            effect_kill_heal_missing_percent: 15.0,
            // Non-vital stats (internals)
            accumulated_damage: 0,
            self_inflicted_credit: 0,
            last_damaged_by: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_deaths_dance", &["deaths_dance"]),
            burn_buff: "deaths_dance_burn",
            price: 2100,
            attack: 75,
            defence: 75,
            skill_cooldown_mult: 10,
            effect_delayed_damage_percent: 25.0,
            effect_burn_hp_percent_cap: 2.5,
            effect_bonus_flat_heal: 75,
            effect_kill_heal_missing_percent: 25.0,
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
                attack,
                defence,
                skill_cooldown_mult,
                effect_delayed_damage_percent,
                effect_burn_hp_percent_cap,
                effect_bonus_flat_heal,
                effect_kill_heal_missing_percent
            ]
        );
        self
    }

    fn mitigation_scale(&self) -> f64 {
        // Reducing 100% of damage would divide by zero, and there is no sensible
        // bleed for a champion that takes nothing, so leave a sliver through.
        100.0 / (100.0 - self.effect_delayed_damage_percent).max(1.0)
    }

    fn defy(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };

        self.accumulated_damage = 0;

        let (hp_current, hp_max) = entity_ref.hp();
        let missing_hp = hp_max - hp_current;

        let heal = self.effect_bonus_flat_heal as usize
            + percent_of(missing_hp, self.effect_kill_heal_missing_percent);
        if heal == 0 {
            return;
        }

        ctx.heal(entity, entity, heal);
    }
}

impl Default for DeathsDance {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DeathsDance {
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
            attack: self.attack,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            damaged_reduce: self.effect_delayed_damage_percent as usize,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.accumulated_damage = 0;
        self.self_inflicted_credit = 0;
        self.last_damaged_by = 0;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // Ignore Pain: bleed the stored damage back over time.
        if self.accumulated_damage <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let is_burn_applied = has_buff(&champion_ref, self.burn_buff);
        if is_burn_applied {
            return;
        }

        let entity = champion_ref.id();
        let per_proc_cap = percent_of(
            champion_ref.hp().1,
            self.effect_burn_hp_percent_cap / BURN_PROCS_PER_SECOND,
        ) as i32;

        let tick_damage = self.accumulated_damage.min(per_proc_cap);
        if tick_damage <= 0 {
            return;
        }
        ctx.add_buff(
            entity,
            &BuffV1::timed(self.burn_buff, ticks(1.0 / BURN_PROCS_PER_SECOND)),
        );
        let dealt = (tick_damage as f64 * self.mitigation_scale()).round() as usize;
        self.self_inflicted_credit += tick_damage;
        ctx.deal_damage(self.last_damaged_by, entity, dealt, 0, AttackTypeV1::Item);
        self.accumulated_damage -= tick_damage;
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attacker == entity {
            return;
        }

        let mut damage = damage as i32;
        if self.self_inflicted_credit > 0 {
            let discounted = damage.min(self.self_inflicted_credit);
            self.self_inflicted_credit -= discounted;
            damage -= discounted;
        }
        if damage <= 0 {
            return;
        }

        self.accumulated_damage += percent_of(
            (damage as f64 * self.mitigation_scale()).round() as usize,
            self.effect_delayed_damage_percent,
        ) as i32;

        self.last_damaged_by = attacker
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        _victim: usize,
    ) {
        self.defy(sim, entity);
    }

    fn on_assist(&mut self, sim: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.defy(sim, entity);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Defense, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
