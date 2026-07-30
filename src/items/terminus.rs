use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_stacks, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Terminus {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    armor_pen_buff_buff: &'static str,
    magic_resistance_pen_buff_buff: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_armor_pen_per_stack: usize,
    effect_magic_pen_per_stack: usize,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    flip_flop: bool,
}

impl Terminus {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("terminus", &["twin_stormblade"], &["radiant_terminus"]),
            armor_pen_buff_buff: "terminus_armor_pen_buff",
            magic_resistance_pen_buff_buff: "terminus_magic_resistance_pen_buff",
            price: 1400,
            attack: 30,
            attack_speed_mult: 35,
            crit_chance: 20,
            effect_armor_pen_per_stack: 4,
            effect_magic_pen_per_stack: 4,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
            flip_flop: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_terminus", &["terminus"]),
            armor_pen_buff_buff: "radiant_terminus_armor_pen_buff",
            magic_resistance_pen_buff_buff: "radiant_terminus_magic_resistance_pen_buff",
            price: 2000,
            attack: 50,
            attack_speed_mult: 60,
            crit_chance: 25,
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
                crit_chance,
                effect_armor_pen_per_stack,
                effect_magic_pen_per_stack,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for Terminus {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Terminus {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.flip_flop = true;
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
    ) {
        let Some(caster_entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let defenses_stack_count = buff_stacks(&caster_entity_ref, self.armor_pen_buff_buff);
        let resistance_stack_count =
            buff_stacks(&caster_entity_ref, self.magic_resistance_pen_buff_buff);
        if self.flip_flop {
            if defenses_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    &BuffV1 {
                        defence_penetration: self.effect_armor_pen_per_stack,
                        ..BuffV1::timed(self.armor_pen_buff_buff, ticks(self.effect_duration_seconds))
                    },
                );
            }
            self.flip_flop = false;
        } else {
            if resistance_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    &BuffV1 {
                        magic_resistance_penetration: self.effect_magic_pen_per_stack,
                        ..BuffV1::timed(self.magic_resistance_pen_buff_buff, ticks(self.effect_duration_seconds))
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::DefensePenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
