use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks, ProcQueue};

#[derive(Clone, Debug)]
pub struct Sheen {
    price: usize,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
    procs: ProcQueue,
}

impl Default for Sheen {
    fn default() -> Self {
        Self {
            price: 1300,
            attack_speed_mult: 25,
            skill_cooldown_mult: 10,
            effect_min_bonus_damage: 30,
            effect_max_bonus_damage: 85,
            effect_cooldown_seconds: 1.5,
            // Non-vital stats (internals)
            spellblade_ready: false,
            procs: ProcQueue::new(),
        }
    }
}

impl Sheen {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                attack_speed_mult,
                skill_cooldown_mult,
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_cooldown_seconds
            ]
        );
        item
    }

    // Bonus damage scales linearly from min (level 1) to max (level 12).
    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }
}

impl StableItem for Sheen {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "sheen".to_string()
    }

    fn icon(&self) -> String {
        "sheen".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["dagger".to_string(), "glowing_mote".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "trinity_force".to_string(),
            "dusk_and_dawn".to_string(),
            "bloodsong".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade_ready = false;
        self.procs.clear();
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || is_ally {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = has_buff(&caster_ref, "spellblade_cooldown");
        if !on_cooldown {
            self.spellblade_ready = true;
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !self.spellblade_ready || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.spellblade_damage(caster_ref.level());

        self.procs.push_physical(ctx, target, bonus_damage);
        ctx.add_buff(
            caster,
            &BuffV1::timed("spellblade_cooldown", ticks(self.effect_cooldown_seconds)),
        );
        self.spellblade_ready = false;
    }

    /// Lands the Spellblade damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::AttackSpeed, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
