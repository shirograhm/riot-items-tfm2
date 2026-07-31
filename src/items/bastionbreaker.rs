use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, apply_lethality, count_takedowns, mark_enemy_champion, percent_of, ticks,
    ItemMeta,
};

fn sabotage_bonus(ctx: &mut StableSim<'_>, caster: usize, flat: usize, ad_percent: f64) -> usize {
    let caster_ad = ctx.get_entity(caster).map(|c| c.stat().attack).unwrap_or(0);
    flat + percent_of(caster_ad, ad_percent)
}

#[derive(Clone, Debug)]
pub struct Bastionbreaker {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_duration_seconds: f64,
    sabotage_charge: usize,
    takedown_marks: Vec<(usize, usize)>,
}

impl Bastionbreaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "bastionbreaker",
                &["serrated_dirk"],
                &["radiant_bastionbreaker"],
            ),
            price: 1300,
            attack: 65,
            skill_cooldown_mult: 15,
            effect_lethality: 22,
            effect_bonus_flat_damage: 150,
            effect_ad_percent_damage: 15.0,
            effect_duration_seconds: 90.0,
            sabotage_charge: 0,
            takedown_marks: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bastionbreaker", &["bastionbreaker"]),
            price: 1950,
            attack: 110,
            skill_cooldown_mult: 20,
            effect_bonus_flat_damage: 200,
            effect_ad_percent_damage: 20.0,
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

    fn charge_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds)
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.sabotage_charge = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        // Expire the held charge, then (re)grant a full-duration one on a takedown.
        self.sabotage_charge = self.sabotage_charge.saturating_sub(1);
        if count_takedowns(&mut self.takedown_marks, ctx) > 0 {
            self.sabotage_charge = self.charge_ticks();
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);

        if self.sabotage_charge == 0 {
            return;
        }
        let is_tower = ctx
            .get_entity(target)
            .map(|t| t.is_tower())
            .unwrap_or(false);
        if !is_tower {
            return;
        }
        let bonus = sabotage_bonus(
            ctx,
            caster,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        ctx.deal_damage(caster, target, bonus, 0, AttackTypeV1::Item);
        self.sabotage_charge = 0; // spend the charge
    }

    fn on_skill_hit(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
