use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct GuinsoosRageblade {
    price: usize,
    attack: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_magic_damage: i32,
    effect_stack_attack_speed_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: usize,
}

impl Default for GuinsoosRageblade {
    fn default() -> Self {
        Self {
            price: 1350,
            attack: 30,
            magic_power: 30,
            attack_speed_mult: 30,
            effect_bonus_magic_damage: 30,
            effect_stack_attack_speed_mult: 8,
            effect_max_stacks: 4,
            effect_duration_seconds: 4,
        }
    }
}

impl GuinsoosRageblade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_bonus_magic_damage: cfg
                .effect_bonus_magic_damage
                .unwrap_or(d.effect_bonus_magic_damage),
            effect_stack_attack_speed_mult: cfg
                .effect_stack_attack_speed_mult
                .unwrap_or(d.effect_stack_attack_speed_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for GuinsoosRageblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "guinsoos_rageblade"
    }

    fn icon(&self) -> &str {
        "t9_5"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "soldiers_longsword".to_string(),
            "arcane_crystal".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_guinsoos_rageblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
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
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "guinsoos_rageblade_buff")
            .count();

        if !target_ref.is_tower() {
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_magic_damage as usize,
                AttackType::BaseAttack,
            );
        }

        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    attack_speed_mult: self.effect_stack_attack_speed_mult,
                    name: ArrayString::try_from("guinsoos_rageblade_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantGuinsoosRageblade {
    price: usize,
    attack: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_magic_damage: i32,
    effect_stack_attack_speed_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: usize,
}

impl Default for RadiantGuinsoosRageblade {
    fn default() -> Self {
        Self {
            price: 1900,
            attack: 50,
            magic_power: 50,
            attack_speed_mult: 50,
            effect_bonus_magic_damage: 30,
            effect_stack_attack_speed_mult: 8,
            effect_max_stacks: 4,
            effect_duration_seconds: 4,
        }
    }
}

impl RadiantGuinsoosRageblade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_bonus_magic_damage: cfg
                .effect_bonus_magic_damage
                .unwrap_or(d.effect_bonus_magic_damage),
            effect_stack_attack_speed_mult: cfg
                .effect_stack_attack_speed_mult
                .unwrap_or(d.effect_stack_attack_speed_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantGuinsoosRageblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_guinsoos_rageblade"
    }

    fn icon(&self) -> &str {
        "t9_6"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["guinsoos_rageblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
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
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| {
                entity_ref.buff_at(i).name.as_str() == "radiant_guinsoos_rageblade_buff"
            })
            .count();

        if !target_ref.is_tower() {
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_magic_damage as usize,
                AttackType::BaseAttack,
            );
        }

        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    attack_speed_mult: self.effect_stack_attack_speed_mult,
                    name: ArrayString::try_from("radiant_guinsoos_rageblade_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
