use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, percent_of, ticks, ItemMeta, PROC_DELAY_SECONDS};

#[derive(Clone, Debug)]
pub struct Collector {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_lethality: usize,
    effect_hp_percent_threshold: f64,
    effect_bonus_gold: usize,
    pending_gold: Vec<(usize, i64)>,
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
            // Non-vital state (internal)
            pending_gold: Vec::new(),
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

        if is_target_champion && target_curr_hp > *damage {
            let hp_threshold = percent_of(target_max_hp, self.effect_hp_percent_threshold);
            let remaining = target_curr_hp - *damage;
            if remaining <= hp_threshold {
                ctx.deal_damage(caster, target, remaining, 0, AttackTypeV1::Item);
            }
        }
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        _entity: usize,
        victim: usize,
    ) {
        let Some(victim_ref) = sim.get_entity(victim) else {
            return;
        };

        if victim_ref.is_champion() {
            self.pending_gold
                .push((ticks(PROC_DELAY_SECONDS), self.effect_bonus_gold as i64));
        }
    }

    fn update(&mut self, sim: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.pending_gold.is_empty() {
            return;
        }
        self.pending_gold.retain_mut(|(remaining, gold)| {
            *remaining = remaining.saturating_sub(1);
            if *remaining > 0 {
                return true;
            }
            sim.player_add_gold(player, *gold);
            false
        });
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
