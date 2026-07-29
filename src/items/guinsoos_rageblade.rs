use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, buff_stacks, ticks, try_proc_on_hit, ItemMeta};

#[derive(Clone, Debug)]
pub struct GuinsoosRageblade {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    stack_buff: &'static str,
    price: usize,
    attack: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_magic_damage: usize,
    effect_stack_attack_speed_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    on_hit_cooldown_seconds: f64,
}

impl GuinsoosRageblade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "guinsoos_rageblade",
                &["scouts_slingshot"],
                &["radiant_guinsoos_rageblade"],
            ),
            stack_buff: "guinsoos_rageblade_buff",
            price: 1350,
            attack: 30,
            magic_power: 30,
            attack_speed_mult: 30,
            effect_bonus_magic_damage: 30,
            effect_stack_attack_speed_mult: 8,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
            on_hit_cooldown_seconds: 0.75,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_guinsoos_rageblade", &["guinsoos_rageblade"]),
            stack_buff: "radiant_guinsoos_rageblade_buff",
            price: 1900,
            attack: 50,
            magic_power: 50,
            attack_speed_mult: 50,
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
                magic_power,
                attack_speed_mult,
                effect_bonus_magic_damage,
                effect_stack_attack_speed_mult,
                effect_max_stacks,
                effect_duration_seconds,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for GuinsoosRageblade {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for GuinsoosRageblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let stack_count = buff_stacks(&caster_ref, self.stack_buff);

        if try_proc_on_hit(
            ctx,
            target,
            "guinsoos_rageblade_on_hit_cooldown",
            self.on_hit_cooldown_seconds,
        ) {
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_magic_damage,
                AttackType::BaseAttack,
            );
        }

        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    attack_speed_mult: self.effect_stack_attack_speed_mult,
                    name: buff_name(self.stack_buff),
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
