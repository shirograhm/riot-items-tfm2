use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct YunTalWildarrows {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    yun_tal_practice_buff: &'static str,
    yun_tal_flurry_cooldown_buff: &'static str,
    yun_tal_flurry_buff: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_stack_crit_chance: i32,
    effect_max_stacks: usize,
    effect_flurry_attack_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    accumulated_stacks: usize,
}

impl YunTalWildarrows {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "yun_tal_wildarrows",
                &["noonquiver"],
                &["radiant_yun_tal_wildarrows"],
            ),
            yun_tal_practice_buff: "yun_tal_practice",
            yun_tal_flurry_cooldown_buff: "yun_tal_flurry_cooldown",
            yun_tal_flurry_buff: "yun_tal_flurry",
            price: 1500,
            attack: 65,
            attack_speed_mult: 20,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
            accumulated_stacks: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_yun_tal_wildarrows", &["yun_tal_wildarrows"]),
            yun_tal_practice_buff: "radiant_yun_tal_practice",
            yun_tal_flurry_cooldown_buff: "radiant_yun_tal_flurry_cooldown",
            yun_tal_flurry_buff: "radiant_yun_tal_flurry",
            price: 2200,
            attack: 80,
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
                attack_speed_mult,
                effect_stack_crit_chance,
                effect_max_stacks,
                effect_flurry_attack_speed_mult,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for YunTalWildarrows {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for YunTalWildarrows {
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
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    // Practice: permanent crit chance earned so far is re-applied each spawn.

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_stacks == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                crit_chance: self.accumulated_stacks as i32 * self.effect_stack_crit_chance,
                name: buff_name(self.yun_tal_practice_buff),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let is_flurry_on_cooldown = has_buff(&caster_ref, self.yun_tal_flurry_cooldown_buff);

        // Practice: dealing physical damage grants permanent crit chance, capped.
        if damage_type == DamageType::AD && self.accumulated_stacks < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Permanent,
                    crit_chance: self.effect_stack_crit_chance,
                    name: buff_name(self.yun_tal_practice_buff),
                    ..Default::default()
                },
            );
            self.accumulated_stacks += 1;
        }

        // Flurry: on attack, gain a burst of attack speed on an internal cooldown.
        if !is_flurry_on_cooldown {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    attack_speed_mult: self.effect_flurry_attack_speed_mult,
                    name: buff_name(self.yun_tal_flurry_buff),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_cooldown_seconds),
                    },
                    name: buff_name(self.yun_tal_flurry_cooldown_buff),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
