use mod_api_stable::*;

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
    effect_bonus_gold: usize,
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
            effect_bonus_gold: 25,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_collector", &["collector"]),
            price: 2100,
            attack: 105,
            crit_chance: 25,
            effect_lethality: 10,
            effect_hp_percent_threshold: 6.0,
            effect_bonus_gold: 25,
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
                effect_hp_percent_threshold,
                effect_bonus_gold
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

impl StableItem for Collector {
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
            crit_chance: self.crit_chance,
            ..Default::default()
        }
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
        let is_target_champion = target_ref.is_champion();
        let (target_curr_hp, target_max_hp) = target_ref.hp();

        // Apply lethality to all damage except towers
        if !is_target_tower {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }

        // Apply execute threshold to champions
        if is_target_champion {
            let hp_threshold = percent_of(target_max_hp, self.effect_hp_percent_threshold);
            if target_curr_hp - *damage <= hp_threshold {
                *damage = target_curr_hp;
            }
        }
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        player: usize,
        _entity: usize,
        _victim: usize,
    ) {
        sim.player_add_gold(player, self.effect_bonus_gold as i64);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
