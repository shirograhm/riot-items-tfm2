use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct BlackCleaver {
    price: usize,
    attack: i32,
    hp: i32,
    skill_cooldown_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_percent_armor_shred: i32,
}

impl Default for BlackCleaver {
    fn default() -> Self {
        Self {
            price: 1500,
            attack: 45,
            hp: 300,
            skill_cooldown_mult: 5,
            effect_max_stacks: 5,
            effect_duration_seconds: 6.0,
            effect_percent_armor_shred: 6,
        }
    }
}

impl BlackCleaver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_percent_armor_shred: cfg
                .effect_percent_armor_shred
                .unwrap_or(d.effect_percent_armor_shred),
        }
    }
}

impl ModItemInfo for BlackCleaver {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "black_cleaver"
    }

    fn icon(&self) -> &str {
        "black_cleaver"
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
        vec!["radiant_black_cleaver".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
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

        if damage_type != DamageType::AD {
            return;
        }

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "black_cleaver_armor_shred")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    defence_mult: -self.effect_percent_armor_shred,
                    name: ArrayString::try_from("black_cleaver_armor_shred").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBlackCleaver {
    price: usize,
    attack: i32,
    hp: i32,
    skill_cooldown_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_percent_armor_shred: i32,
}

impl Default for RadiantBlackCleaver {
    fn default() -> Self {
        Self {
            price: 2200,
            attack: 70,
            hp: 500,
            skill_cooldown_mult: 10,
            effect_max_stacks: 5,
            effect_duration_seconds: 6.0,
            effect_percent_armor_shred: 6,
        }
    }
}

impl RadiantBlackCleaver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_percent_armor_shred: cfg
                .effect_percent_armor_shred
                .unwrap_or(d.effect_percent_armor_shred),
        }
    }
}

impl ModItemInfo for RadiantBlackCleaver {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_black_cleaver"
    }

    fn icon(&self) -> &str {
        "radiant_black_cleaver"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["black_cleaver".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
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

        if damage_type != DamageType::AD {
            return;
        }

        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "black_cleaver_armor_shred")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    defence_mult: -self.effect_percent_armor_shred,
                    name: ArrayString::try_from("black_cleaver_armor_shred").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
