use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// Spellblade: after landing an Ability on an enemy champion, the next basic
// attack deals bonus physical damage (flat + % of Attack Damage), gated by a
// self cooldown-buff. `spellblade_ready` is charged in `on_skill_hit` and
// consumed in `on_attack` (self state, since buffs cannot be removed to spend).

#[derive(Clone, Debug)]
pub struct TrinityForce {
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl Default for TrinityForce {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 333,
            attack: 33,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 33,
            effect_ad_percent_damage: 33.0,
            effect_cooldown_seconds: 3.5,
            spellblade_ready: false,
        }
    }
}

impl TrinityForce {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            spellblade_ready: false,
        }
    }
}

impl ModItemInfo for TrinityForce {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "trinity_force"
    }

    fn icon(&self) -> &str {
        "trinity_force"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["sheen".to_string(), "phage".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_trinity_force".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.spellblade_ready = false;
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "trinity_force_spellblade_cooldown");
        if !on_cooldown {
            self.spellblade_ready = true;
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
        if !self.spellblade_ready {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("trinity_force_spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantTrinityForce {
    price: usize,
    hp: i32,
    attack: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl Default for RadiantTrinityForce {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 333,
            attack: 33,
            attack_speed_mult: 33,
            skill_cooldown_mult: 20,
            effect_bonus_flat_damage: 33,
            effect_ad_percent_damage: 33.0,
            effect_cooldown_seconds: 3.5,
            spellblade_ready: false,
        }
    }
}

impl RadiantTrinityForce {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            spellblade_ready: false,
        }
    }
}

impl ModItemInfo for RadiantTrinityForce {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_trinity_force"
    }

    fn icon(&self) -> &str {
        "radiant_trinity_force"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["trinity_force".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.spellblade_ready = false;
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = (0..caster_ref.buff_count()).any(|i| {
            caster_ref.buff_at(i).name.as_str() == "radiant_trinity_force_spellblade_cooldown"
        });
        if !on_cooldown {
            self.spellblade_ready = true;
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
        if !self.spellblade_ready {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("radiant_trinity_force_spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
