use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct Collector {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_lethality: usize,
    effect_hp_percent_threshold: f64,
}

impl Collector {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "collector",
                &["serrated_dirk", "noonquiver"],
                &["radiant_collector"],
            ),
            price: 1450,
            attack: 60,
            crit_chance: 20,
            effect_lethality: 10,
            effect_hp_percent_threshold: 6.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_collector", &["collector"]),
            price: 2100,
            attack: 105,
            crit_chance: 25,
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
                crit_chance,
                effect_lethality,
                effect_hp_percent_threshold
            ]
        );
        self
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Collector {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);

        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let hp_threshold = percent_of(target_ref.hp().max, self.effect_hp_percent_threshold);
        if target_ref.hp().current - *damage <= hp_threshold {
            let lethal_damage = target_ref.hp().current;
            ctx.deal_damage(caster, target, lethal_damage, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
