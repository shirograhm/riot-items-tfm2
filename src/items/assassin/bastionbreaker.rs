use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, has_buff, percent_of, ticks, ItemMeta};

fn sabotage_bonus(ctx: &mut StableSim<'_>, caster: usize, flat: usize, ad_percent: f64) -> usize {
    let caster_ad = ctx.get_entity(caster).map(|c| c.stat().attack).unwrap_or(0);
    flat + percent_of(caster_ad, ad_percent)
}

// Gain 22 Lethality.
// Sabotage: Scoring a takedown on an enemy champion grants Sabotage for 90 seconds, empowering your next basic attack
// against a turret to deal 150 + 15% AD as bonus physical damage.
#[derive(Clone, Debug)]
pub struct Bastionbreaker {
    meta: ItemMeta,
    sabotage_buff: &'static str,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_duration_seconds: f64,
}

impl Bastionbreaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "bastionbreaker",
                &["serrated_dirk"],
                &["radiant_bastionbreaker"],
            ),
            sabotage_buff: "sabotage_charge",
            price: 1300,
            attack: 65,
            skill_cooldown_mult: 15,
            effect_lethality: 22,
            effect_bonus_flat_damage: 150,
            effect_ad_percent_damage: 15.0,
            effect_duration_seconds: 90.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bastionbreaker", &["bastionbreaker"]),
            price: 1950,
            attack: 110,
            skill_cooldown_mult: 20,
            effect_lethality: 22,
            effect_bonus_flat_damage: 200,
            effect_ad_percent_damage: 20.0,
            effect_duration_seconds: 90.0,
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
                skill_cooldown_mult,
                effect_lethality,
                effect_bonus_flat_damage,
                effect_ad_percent_damage,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for Bastionbreaker {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Bastionbreaker {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_entity) = player_ref.champion() else {
            return;
        };

        ctx.entity_remove_buff(player_entity.id(), self.sabotage_buff);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_target_tower = target_ref.is_tower();
        let has_sabotage_charged = has_buff(&caster_ref, self.sabotage_buff);

        // Apply lethality for all damage except towers
        if !is_target_tower {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }

        // Only process on-hit for basic attacks on tower
        if attack_type == AttackTypeV1::BaseAttack && is_target_tower && has_sabotage_charged {
            let bonus = sabotage_bonus(
                ctx,
                caster,
                self.effect_bonus_flat_damage,
                self.effect_ad_percent_damage,
            );
            ctx.deal_damage(caster, target, bonus, 0, AttackTypeV1::Item);
            ctx.entity_remove_buff(caster, self.sabotage_buff);
        }
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        _victim: usize,
    ) {
        sim.add_buff(
            entity,
            &BuffV1::timed(self.sabotage_buff, ticks(self.effect_duration_seconds)),
        );
    }

    fn on_assist(&mut self, sim: &mut StableSim<'_>, _player: usize, entity: usize) {
        sim.add_buff(
            entity,
            &BuffV1::timed(self.sabotage_buff, ticks(self.effect_duration_seconds)),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
