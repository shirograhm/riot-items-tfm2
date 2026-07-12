use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct Bloodsong {
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    spellblade_ready: bool,
}

impl Default for Bloodsong {
    fn default() -> Self {
        Self {
            price: 1050,
            attack_speed_mult: 15,
            hp: 250,
            hp_regen: 2,
            magic_power: 20,
            skill_cooldown_mult: 10,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 125,
            effect_cooldown_seconds: 3.5,
            effect_damaged_amplify: 8,
            effect_duration_seconds: 4.0,
            spellblade_ready: false,
        }
    }
}

impl Bloodsong {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_min_bonus_damage: cfg
                .effect_min_bonus_damage
                .unwrap_or(d.effect_min_bonus_damage),
            effect_max_bonus_damage: cfg
                .effect_max_bonus_damage
                .unwrap_or(d.effect_max_bonus_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            effect_damaged_amplify: cfg
                .effect_damaged_amplify
                .unwrap_or(d.effect_damaged_amplify),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            spellblade_ready: false,
        }
    }

    // Bonus damage scales linearly from min (level 1) to max (level 12).
    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }
}

impl ModItemInfo for Bloodsong {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bloodsong"
    }

    fn icon(&self) -> &str {
        "bloodsong"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["sheen".to_string(), "bandleglass_mirror".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_bloodsong".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
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
        let bonus_damage = self.spellblade_damage(caster_ref.level());
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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

        // Increase the target's damage taken, but only while it is an enemy
        // champion and does not already carry the debuff (so it never stacks
        // past the configured amplification).
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let already_vulnerable = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "bloodsong_vulnerable");
        if !already_vulnerable {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0).round() as usize,
                    },
                    damaged_amplify: self.effect_damaged_amplify,
                    name: ArrayString::try_from("bloodsong_vulnerable").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::AP,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBloodsong {
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    spellblade_ready: bool,
}

impl Default for RadiantBloodsong {
    fn default() -> Self {
        Self {
            price: 1500,
            attack_speed_mult: 15,
            hp: 450,
            hp_regen: 4,
            magic_power: 40,
            skill_cooldown_mult: 20,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 125,
            effect_cooldown_seconds: 3.5,
            effect_damaged_amplify: 8,
            effect_duration_seconds: 4.0,
            spellblade_ready: false,
        }
    }
}

impl RadiantBloodsong {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_min_bonus_damage: cfg
                .effect_min_bonus_damage
                .unwrap_or(d.effect_min_bonus_damage),
            effect_max_bonus_damage: cfg
                .effect_max_bonus_damage
                .unwrap_or(d.effect_max_bonus_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            effect_damaged_amplify: cfg
                .effect_damaged_amplify
                .unwrap_or(d.effect_damaged_amplify),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            spellblade_ready: false,
        }
    }

    // Bonus damage scales linearly from min (level 1) to max (level 12).
    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }
}

impl ModItemInfo for RadiantBloodsong {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_bloodsong"
    }

    fn icon(&self) -> &str {
        "radiant_bloodsong"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bloodsong".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
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
        let bonus_damage = self.spellblade_damage(caster_ref.level());
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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

        // Increase the target's damage taken, but only while it is an enemy
        // champion and does not already carry the debuff (so it never stacks
        // past the configured amplification).
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let already_vulnerable = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "radiant_bloodsong_vulnerable");
        if !already_vulnerable {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0).round() as usize,
                    },
                    damaged_amplify: self.effect_damaged_amplify,
                    name: ArrayString::try_from("radiant_bloodsong_vulnerable").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::AP,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
