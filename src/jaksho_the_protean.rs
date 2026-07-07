use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct JakshoTheProtean {
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_stack_defence_mult: i32,
    effect_stack_magic_resistance_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for JakshoTheProtean {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 300,
            defence: 40,
            magic_resistance: 65,
            effect_stack_defence_mult: 6,
            effect_stack_magic_resistance_mult: 6,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }
}

impl JakshoTheProtean {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            effect_stack_defence_mult: cfg
                .effect_stack_defence_mult
                .unwrap_or(d.effect_stack_defence_mult),
            effect_stack_magic_resistance_mult: cfg
                .effect_stack_magic_resistance_mult
                .unwrap_or(d.effect_stack_magic_resistance_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for JakshoTheProtean {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "jaksho_the_protean"
    }

    fn icon(&self) -> &str {
        "t8_5"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["gatekeepers_armor".to_string(), "night_hood".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_jaksho_the_protean".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        if !attacker_ref.is_champion() {
            return;
        }
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "jaksho_the_protean_stack")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    defence_mult: self.effect_stack_defence_mult,
                    magic_resistance_mult: self.effect_stack_magic_resistance_mult,
                    name: ArrayString::try_from("jaksho_the_protean_stack").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantJakshoTheProtean {
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_stack_defence_mult: i32,
    effect_stack_magic_resistance_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for RadiantJakshoTheProtean {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 550,
            defence: 65,
            magic_resistance: 65,
            effect_stack_defence_mult: 10,
            effect_stack_magic_resistance_mult: 10,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }
}

impl RadiantJakshoTheProtean {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            effect_stack_defence_mult: cfg
                .effect_stack_defence_mult
                .unwrap_or(d.effect_stack_defence_mult),
            effect_stack_magic_resistance_mult: cfg
                .effect_stack_magic_resistance_mult
                .unwrap_or(d.effect_stack_magic_resistance_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantJakshoTheProtean {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_jaksho_the_protean"
    }

    fn icon(&self) -> &str {
        "t8_6"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["jaksho_the_protean".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        if !attacker_ref.is_champion() {
            return;
        }
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "radiant_jaksho_the_protean_stack")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    defence_mult: self.effect_stack_defence_mult,
                    magic_resistance_mult: self.effect_stack_magic_resistance_mult,
                    name: ArrayString::try_from("radiant_jaksho_the_protean_stack").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
