use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE};

#[derive(Clone, Debug)]
pub struct BladeOfTheRuinedKing {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    vamp: i32,
    effect_melee_hp_percent_damage: f64,
    effect_ranged_hp_percent_damage: f64,
    effect_melee_distance: usize,
    effect_minion_damage_cap: usize,
    procs: ProcQueue,
}

impl BladeOfTheRuinedKing {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "blade_of_the_ruined_king",
                &["hearthbound_axe", "ruinous_blade"],
                &["radiant_blade_of_the_ruined_king"],
            ),
            price: 1450,
            attack: 50,
            attack_speed_mult: 25,
            vamp: 5,
            effect_melee_hp_percent_damage: 8.0,
            effect_ranged_hp_percent_damage: 5.0,
            effect_melee_distance: 35,
            effect_minion_damage_cap: 50,
            // Non-vital state (internal)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_blade_of_the_ruined_king",
                &["blade_of_the_ruined_king"],
            ),
            price: 2100,
            attack: 60,
            attack_speed_mult: 50,
            vamp: 10,
            effect_melee_hp_percent_damage: 8.0,
            effect_ranged_hp_percent_damage: 5.0,
            effect_melee_distance: 35,
            effect_minion_damage_cap: 50,
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
                attack_speed_mult,
                vamp,
                effect_melee_hp_percent_damage,
                effect_ranged_hp_percent_damage,
                effect_melee_distance,
                effect_minion_damage_cap,
            ]
        );
        self
    }
}

impl Default for BladeOfTheRuinedKing {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for BladeOfTheRuinedKing {
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
            attack_speed_mult: self.attack_speed_mult,
            vamp: self.vamp,
            ..Default::default()
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        let target_hp = target_ref.hp().0;
        let is_champion = target_ref.is_champion();

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let hp_percent = if ctx.distance_sq(caster, target) > reach * reach {
            self.effect_ranged_hp_percent_damage
        } else {
            self.effect_melee_hp_percent_damage
        };
        let mut bonus_damage = percent_of(target_hp, hp_percent);
        if !is_champion {
            bonus_damage = bonus_damage.clamp(0, self.effect_minion_damage_cap);
        }

        // Fixed here rather than re-read when it lands: the health the tooltip
        // promises a share of is the health the target had when it was hit.
        self.procs.push_physical(ctx, target, bonus_damage);
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    /// Lands the hits whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::AttackSpeed,
            ItemTagV1::Vamp,
            ItemTagV1::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
