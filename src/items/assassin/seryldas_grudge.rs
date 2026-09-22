use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, ticks, ItemMeta};

const SLOW_BUFF: &str = "seryldas_grudge_slow";

#[derive(Clone, Debug)]
pub struct SeryldasGrudge {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    defence_penetration: usize,
    effect_hp_percent_threshold: f64,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
}

impl SeryldasGrudge {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "seryldas_grudge",
                &["last_whisper", "caulfields_warhammer"],
                &["radiant_seryldas_grudge"],
            ),
            price: 700,
            attack: 25,
            skill_cooldown_mult: 10,
            defence_penetration: 25,
            effect_hp_percent_threshold: 50.0,
            effect_slow_amount: 30,
            effect_duration_seconds: 1.5,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_seryldas_grudge", &["seryldas_grudge"]),
            price: 1050,
            attack: 45,
            skill_cooldown_mult: 15,
            defence_penetration: 35,
            effect_hp_percent_threshold: 50.0,
            effect_slow_amount: 30,
            effect_duration_seconds: 1.5,
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
                defence_penetration,
                effect_hp_percent_threshold,
                effect_slow_amount,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for SeryldasGrudge {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SeryldasGrudge {
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
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    // Bitter Cold. `on_attack` fires before the hit lands (`damage` is still
    // adjustable), so the threshold is checked against the health the target is
    // left with — the hit that takes an enemy to 50% slows it too.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::Skill {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let (target_curr_hp, target_max_hp) = target_ref.hp();
        let remaining = target_curr_hp.saturating_sub(*damage);
        if remaining > percent_of(target_max_hp, self.effect_hp_percent_threshold) {
            return;
        }

        refresh_buff(
            ctx,
            target,
            SLOW_BUFF,
            &BuffV1 {
                move_speed_mult: -self.effect_slow_amount,
                ..BuffV1::timed(SLOW_BUFF, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::DefensePenetration,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
