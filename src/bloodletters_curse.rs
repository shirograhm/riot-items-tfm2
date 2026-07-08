use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct BloodlettersCurse {
    price: usize,
    magic_power: i32,
    hp: i32,
    skill_cooldown_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_percent_mr_shred: i32,
}

impl Default for BloodlettersCurse {
    fn default() -> Self {
        Self {
            price: 1500,
            magic_power: 110,
            hp: 300,
            skill_cooldown_mult: 5,
            effect_max_stacks: 5,
            effect_duration_seconds: 6.0,
            effect_percent_mr_shred: 6,
        }
    }
}

impl BloodlettersCurse {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_percent_mr_shred: cfg
                .effect_percent_mr_shred
                .unwrap_or(d.effect_percent_mr_shred),
        }
    }
}

impl ModItemInfo for BloodlettersCurse {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bloodletters_curse"
    }

    fn icon(&self) -> &str {
        "bloodletters_curse"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["haunting_guise".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_bloodletters_curse".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        if !entity_ref.is_champion() {
            return;
        }

        if damage_type != DamageType::AP {
            return;
        }

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "bloodletters_curse_mr_shred")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_resistance_mult: -self.effect_percent_mr_shred,
                    name: ArrayString::try_from("bloodletters_curse_mr_shred").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBloodlettersCurse {
    price: usize,
    magic_power: i32,
    hp: i32,
    skill_cooldown_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_percent_mr_shred: i32,
}

impl Default for RadiantBloodlettersCurse {
    fn default() -> Self {
        Self {
            price: 2200,
            magic_power: 180,
            hp: 500,
            skill_cooldown_mult: 10,
            effect_max_stacks: 5,
            effect_duration_seconds: 6.0,
            effect_percent_mr_shred: 6,
        }
    }
}

impl RadiantBloodlettersCurse {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_percent_mr_shred: cfg
                .effect_percent_mr_shred
                .unwrap_or(d.effect_percent_mr_shred),
        }
    }
}

impl ModItemInfo for RadiantBloodlettersCurse {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_bloodletters_curse"
    }

    fn icon(&self) -> &str {
        "radiant_bloodletters_curse"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bloodletters_curse".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        if !entity_ref.is_champion() {
            return;
        }

        if damage_type != DamageType::AP {
            return;
        }

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "bloodletters_curse_mr_shred")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_resistance_mult: -self.effect_percent_mr_shred,
                    name: ArrayString::try_from("bloodletters_curse_mr_shred").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
