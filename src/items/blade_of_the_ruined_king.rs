use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, try_proc_on_hit, ItemMeta};

#[derive(Clone, Debug)]
pub struct BladeOfTheRuinedKing {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    vamp: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    on_hit_cooldown_seconds: f64,
}

impl BladeOfTheRuinedKing {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "blade_of_the_ruined_king",
                &["wind_dagger", "ruinous_blade"],
                &["radiant_blade_of_the_ruined_king"],
            ),
            price: 1450,
            attack: 50,
            attack_speed_mult: 25,
            vamp: 5,
            effect_hp_percent_damage: 5.0,
            effect_minion_damage_cap: 50,
            on_hit_cooldown_seconds: 0.5,
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
                effect_hp_percent_damage,
                effect_minion_damage_cap,
                on_hit_cooldown_seconds
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

impl ModItemInfo for BladeOfTheRuinedKing {
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
            attack_speed_mult: self.attack_speed_mult,
            vamp: self.vamp,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let mut bonus_damage = percent_of(
            target_ref.hp().current,
            self.effect_hp_percent_damage as f64,
        );
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, self.effect_minion_damage_cap);
        }

        if !try_proc_on_hit(
            ctx,
            target,
            "blade_of_the_ruined_king_on_hit_cooldown",
            self.on_hit_cooldown_seconds,
        ) {
            return;
        }
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::Vamp,
            ItemTag::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
