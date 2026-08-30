use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct YunTalWildarrows {
    meta: ItemMeta,
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
            // Non-vital stats (internals)
            accumulated_stacks: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_yun_tal_wildarrows", &["yun_tal_wildarrows"]),
            yun_tal_practice_buff: "yun_tal_practice",
            yun_tal_flurry_cooldown_buff: "yun_tal_flurry_cooldown",
            yun_tal_flurry_buff: "yun_tal_flurry",
            price: 2200,
            attack: 80,
            attack_speed_mult: 50,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
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

impl StableItem for YunTalWildarrows {
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
            ..Default::default()
        }
    }

    // Practice: permanent crit chance earned so far is re-applied each spawn.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
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
            &BuffV1 {
                crit_chance: self.accumulated_stacks as i32 * self.effect_stack_crit_chance,
                ..BuffV1::named(self.yun_tal_practice_buff)
            },
        );
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
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let is_flurry_on_cooldown = has_buff(&caster_ref, self.yun_tal_flurry_cooldown_buff);

        // Practice: base attacks grants permanent crit chance, capped.
        if self.accumulated_stacks < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                &BuffV1 {
                    crit_chance: self.effect_stack_crit_chance,
                    ..BuffV1::named(self.yun_tal_practice_buff)
                },
            );
            self.accumulated_stacks += 1;
        }

        // Flurry: on attack, gain a burst of attack speed on an internal cooldown.
        if !is_flurry_on_cooldown {
            ctx.add_buff(
                caster,
                &BuffV1 {
                    attack_speed_mult: self.effect_flurry_attack_speed_mult,
                    ..BuffV1::timed(
                        self.yun_tal_flurry_buff,
                        ticks(self.effect_duration_seconds),
                    )
                },
            );
            ctx.add_buff(
                caster,
                &BuffV1::timed(
                    self.yun_tal_flurry_cooldown_buff,
                    ticks(self.effect_cooldown_seconds),
                ),
            );
        }
    }

    /// Practice stacks are permanent, so they cross the Radiant upgrade. Only
    /// the counter moves: the crit chance already granted this life is sitting
    /// on the champion as `yun_tal_practice` buffs, and `on_spawn` re-applies it
    /// from the carried count on the next respawn.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
