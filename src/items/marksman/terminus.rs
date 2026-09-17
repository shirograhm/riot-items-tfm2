use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, Stacks};

#[derive(Clone, Debug)]
pub struct Terminus {
    meta: ItemMeta,
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
    armor_pen_stacks: Stacks,
    magic_pen_stacks: Stacks,
}

impl Terminus {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "terminus",
                &["twin_stormblade", "scouts_slingshot"],
                &["radiant_terminus"],
            ),
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
            // Non-vital stats (internals)
            flip_flop: false,
            armor_pen_stacks: Stacks::new(),
            magic_pen_stacks: Stacks::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_terminus", &["terminus"]),
            armor_pen_buff_buff: "terminus_armor_pen_buff",
            magic_resistance_pen_buff_buff: "terminus_magic_resistance_pen_buff",
            price: 2000,
            attack: 50,
            attack_speed_mult: 60,
            crit_chance: 25,
            effect_armor_pen_per_stack: 4,
            effect_magic_pen_per_stack: 4,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
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
        self.armor_pen_stacks.clear();
        self.magic_pen_stacks.clear();
        self.flip_flop = true;
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.armor_pen_stacks.tick();
        self.magic_pen_stacks.tick();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        if ctx.get_entity(caster).is_none() {
            return;
        }
        let duration = ticks(self.effect_duration_seconds);
        if self.flip_flop {
            let stacks = self.armor_pen_stacks.add(caster, self.effect_max_stacks, duration);
            if stacks > 0 {
                refresh_buff(
                    ctx,
                    caster,
                    self.armor_pen_buff_buff,
                    &BuffV1 {
                        defence_penetration: self.effect_armor_pen_per_stack * stacks,
                        ..BuffV1::timed(self.armor_pen_buff_buff, duration)
                    },
                );
            }
            self.flip_flop = false;
        } else {
            let stacks = self.magic_pen_stacks.add(caster, self.effect_max_stacks, duration);
            if stacks > 0 {
                refresh_buff(
                    ctx,
                    caster,
                    self.magic_resistance_pen_buff_buff,
                    &BuffV1 {
                        magic_resistance_penetration: self.effect_magic_pen_per_stack * stacks,
                        ..BuffV1::timed(self.magic_resistance_pen_buff_buff, duration)
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::AttackSpeed,
            ItemTagV1::DefensePenetration,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
