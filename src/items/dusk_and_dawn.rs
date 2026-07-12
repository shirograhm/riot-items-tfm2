use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// Spellblade: after landing an Ability on an enemy champion, the next basic
// attack deals bonus magic damage (flat + % of Ability Power) and heals the
// wielder (% of Ability Power + % of maximum health), gated by a self
// cooldown-buff. `spellblade_ready` is charged in `on_skill_hit` and consumed
// in `on_attack` (self state, since buffs cannot be removed to spend).

#[derive(Clone, Debug)]
pub struct DuskAndDawn {
    price: usize,
    hp: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_caster_ap_percent_heal: f64,
    effect_caster_hp_percent_heal: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl Default for DuskAndDawn {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 200,
            magic_power: 60,
            attack_speed_mult: 15,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 3.5,
            spellblade_ready: false,
        }
    }
}

impl DuskAndDawn {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            effect_caster_ap_percent_heal: cfg
                .effect_caster_ap_percent_heal
                .unwrap_or(d.effect_caster_ap_percent_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            spellblade_ready: false,
        }
    }
}

impl ModItemInfo for DuskAndDawn {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "dusk_and_dawn"
    }

    fn icon(&self) -> &str {
        "dusk_and_dawn"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["sheen".to_string(), "haunting_guise".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_dusk_and_dawn".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
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
        let on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "spellblade_cooldown");
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
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);
        let heal_amount = percent_of(
            caster_ref.stat().magic_power,
            self.effect_caster_ap_percent_heal,
        ) + percent_of(caster_ref.hp().max, self.effect_caster_hp_percent_heal);
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.heal(caster, caster, heal_amount);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AP,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantDuskAndDawn {
    price: usize,
    hp: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_caster_ap_percent_heal: f64,
    effect_caster_hp_percent_heal: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl Default for RadiantDuskAndDawn {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 300,
            magic_power: 100,
            attack_speed_mult: 25,
            skill_cooldown_mult: 20,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 3.5,
            spellblade_ready: false,
        }
    }
}

impl RadiantDuskAndDawn {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            effect_caster_ap_percent_heal: cfg
                .effect_caster_ap_percent_heal
                .unwrap_or(d.effect_caster_ap_percent_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            spellblade_ready: false,
        }
    }
}

impl ModItemInfo for RadiantDuskAndDawn {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_dusk_and_dawn"
    }

    fn icon(&self) -> &str {
        "radiant_dusk_and_dawn"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["dusk_and_dawn".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
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
        let on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "spellblade_cooldown");
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
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);
        let heal_amount = percent_of(
            caster_ref.stat().magic_power,
            self.effect_caster_ap_percent_heal,
        ) + percent_of(caster_ref.hp().max, self.effect_caster_hp_percent_heal);
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.heal(caster, caster, heal_amount);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AP,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
