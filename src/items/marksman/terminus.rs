use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{add_stack, apply_config, percent_of, ticks, ItemMeta, ProcQueue};

#[derive(Clone, Debug)]
pub struct Terminus {
    meta: ItemMeta,
    armor_pen_buff_buff: &'static str,
    magic_resistance_pen_buff_buff: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_ap_percent_damage: f64,
    effect_armor_pen_per_stack: usize,
    effect_magic_pen_per_stack: usize,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    flip_flop: bool,
    procs: ProcQueue,
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
            price: 700,
            attack: 15,
            attack_speed_mult: 35,
            crit_chance: 20,
            effect_bonus_flat_damage: 30,
            effect_ad_percent_damage: 5.0,
            effect_ap_percent_damage: 10.0,
            effect_armor_pen_per_stack: 4,
            effect_magic_pen_per_stack: 4,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
            // Non-vital stats (internals)
            flip_flop: false,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_terminus", &["terminus"]),
            armor_pen_buff_buff: "terminus_armor_pen_buff",
            magic_resistance_pen_buff_buff: "terminus_magic_resistance_pen_buff",
            price: 1000,
            attack: 25,
            attack_speed_mult: 60,
            crit_chance: 25,
            effect_bonus_flat_damage: 30,
            effect_ad_percent_damage: 5.0,
            effect_ap_percent_damage: 10.0,
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
                effect_bonus_flat_damage,
                effect_ad_percent_damage,
                effect_ap_percent_damage,
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
        self.procs.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage)
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);
        let is_tower = ctx
            .get_entity(target)
            .is_some_and(|target_ref| target_ref.is_tower());
        if !is_tower {
            self.procs
                .on_hit_magic(ctx, target, damage, damage_type, is_crit, bonus_damage);
        }

        let duration = ticks(self.effect_duration_seconds);
        if self.flip_flop {
            add_stack(
                ctx,
                caster,
                &BuffV1 {
                    defence_penetration: self.effect_armor_pen_per_stack,
                    ..BuffV1::timed(self.armor_pen_buff_buff, duration)
                },
                self.effect_max_stacks,
            );
            self.flip_flop = false;
        } else {
            add_stack(
                ctx,
                caster,
                &BuffV1 {
                    magic_resistance_penetration: self.effect_magic_pen_per_stack,
                    ..BuffV1::timed(self.magic_resistance_pen_buff_buff, duration)
                },
                self.effect_max_stacks,
            );
            self.flip_flop = true;
        }
    }

    /// Lands the on-hit damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
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
