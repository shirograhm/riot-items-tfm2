use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, percent_of, total_lethality, ItemMeta};

// Axiom Arc: Lethality, and Flux, a takedown on an enemy champion refunding a
// share of the ultimate's full cooldown (`entity_reduce_cooldown`, ABI level
// 10; an older host refunds nothing).
#[derive(Clone, Debug)]
pub struct AxiomArc {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    // Flux's refund, in percent of the ultimate's full cooldown: this much,
    // plus `effect_ult_cooldown_per_lethality` for each point of the
    // carrier's Lethality. The names are from when Flux was Ultimate Haste;
    // they are kept so configs and item patches carry over.
    effect_ult_cooldown_mult: i32,
    effect_ult_cooldown_per_lethality: f64,
}

impl AxiomArc {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("axiom_arc", &["serrated_dirk"], &["radiant_axiom_arc"]),
            price: 650,
            attack: 35,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_ult_cooldown_mult: 10,
            effect_ult_cooldown_per_lethality: 0.2,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_axiom_arc", &["axiom_arc"]),
            price: 950,
            attack: 55,
            skill_cooldown_mult: 15,
            effect_lethality: 18,
            effect_ult_cooldown_mult: 10,
            effect_ult_cooldown_per_lethality: 0.2,
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
                effect_ult_cooldown_mult,
                effect_ult_cooldown_per_lethality
            ]
        );
        self
    }

    // Flux: takes the refund off what is left of the ultimate's cooldown. The
    // Lethality is read at the takedown, so it counts every item held then.
    fn flux(&self, ctx: &mut StableSim<'_>, player: usize, entity: usize) {
        let Some(cooltime) = ctx
            .get_entity(entity)
            .and_then(|e| e.cooldown(SkillSlotV1::Ult))
            .filter(|cooldown| cooldown.unlocked)
            .map(|cooldown| cooldown.cooltime)
        else {
            return;
        };
        let lethality = total_lethality(ctx, player);
        let percent = self.effect_ult_cooldown_mult as f64
            + self.effect_ult_cooldown_per_lethality * lethality as f64;
        let refund = percent_of(cooltime, percent);
        if refund > 0 {
            ctx.entity_reduce_cooldown(entity, SkillSlotV1::Ult, refund);
        }
    }
}

impl Default for AxiomArc {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for AxiomArc {
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

    // A takedown on a champion. Minions, monsters and turrets refund nothing.
    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        player: usize,
        entity: usize,
        victim: usize,
    ) {
        if ctx.get_entity(victim).is_some_and(|victim| victim.is_champion()) {
            self.flux(ctx, player, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, player: usize, entity: usize) {
        self.flux(ctx, player, entity);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_target_tower = target_ref.is_tower();

        if !is_target_tower {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
