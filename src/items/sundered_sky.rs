use mod_api::*;

use crate::{config::ItemConfig, percent_of_i32, try_proc_on_hit};

#[derive(Clone, Debug)]
pub struct SunderedSky {
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
    on_hit_cooldown_seconds: f64,
}

impl Default for SunderedSky {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 400,
            attack: 30,
            skill_cooldown_mult: 10,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_flat_heal: 60,
            effect_caster_hp_percent_heal: 6.0,
            on_hit_cooldown_seconds: 20.0,
        }
    }
}

impl SunderedSky {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_percent_bonus_damage: cfg
                .effect_percent_bonus_damage
                .unwrap_or(d.effect_percent_bonus_damage),
            effect_bonus_flat_heal: cfg
                .effect_bonus_flat_heal
                .unwrap_or(d.effect_bonus_flat_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

impl ModItemInfo for SunderedSky {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "sundered_sky"
    }

    fn icon(&self) -> &str {
        "sundered_sky"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["phage".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_sundered_sky".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        if damage_type != DamageType::AD {
            return;
        }

        let missing_health = (caster_ref.hp().max - caster_ref.hp().current) as i32;
        let heal_amount = self.effect_bonus_flat_heal
            + percent_of_i32(missing_health, self.effect_caster_hp_percent_heal);

        if !try_proc_on_hit(
            ctx,
            target,
            "sundered_sky_cooldown",
            self.on_hit_cooldown_seconds,
        ) {
            return;
        }

        let ratio = 1.0 + (self.effect_percent_bonus_damage / 100.0);
        *damage = (*damage as f64 * ratio) as usize;
        ctx.heal(caster, caster, heal_amount as usize);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantSunderedSky {
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
    on_hit_cooldown_seconds: f64,
}

impl Default for RadiantSunderedSky {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 550,
            attack: 65,
            skill_cooldown_mult: 20,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_flat_heal: 60,
            effect_caster_hp_percent_heal: 10.0,
            on_hit_cooldown_seconds: 20.0,
        }
    }
}

impl RadiantSunderedSky {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_percent_bonus_damage: cfg
                .effect_percent_bonus_damage
                .unwrap_or(d.effect_percent_bonus_damage),
            effect_bonus_flat_heal: cfg
                .effect_bonus_flat_heal
                .unwrap_or(d.effect_bonus_flat_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantSunderedSky {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_sundered_sky"
    }

    fn icon(&self) -> &str {
        "radiant_sundered_sky"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["sundered_sky".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        if damage_type != DamageType::AD {
            return;
        }

        let missing_health = (caster_ref.hp().max - caster_ref.hp().current) as i32;
        let heal_amount = self.effect_bonus_flat_heal
            + percent_of_i32(missing_health, self.effect_caster_hp_percent_heal);

        if !try_proc_on_hit(
            ctx,
            target,
            "sundered_sky_cooldown",
            self.on_hit_cooldown_seconds,
        ) {
            return;
        }

        let ratio = 1.0 + (self.effect_percent_bonus_damage / 100.0);
        *damage = (*damage as f64 * ratio) as usize;
        ctx.heal(caster, caster, heal_amount as usize);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
