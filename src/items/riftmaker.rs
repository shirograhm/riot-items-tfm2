use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, buff_name, buff_stacks, percent_of, ticks, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

#[derive(Clone, Debug)]
pub struct Riftmaker {
    meta: ItemMeta,
    // Buff names are namespaced per variant so a champion holding both the base
    // and the radiant item accumulates two independent stacks of each.
    infusion_buff: &'static str,
    corruption_buff: &'static str,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_caster_hp_percent_power: f64,
    effect_vamp: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    refresh_cooldown: usize,
}

impl Riftmaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("riftmaker", &["haunting_guise"], &["radiant_riftmaker"]),
            infusion_buff: "riftmaker_infusion",
            corruption_buff: "riftmaker_corruption",
            price: 1300,
            hp: 400,
            magic_power: 75,
            effect_caster_hp_percent_power: 2.0,
            effect_vamp: 2,
            effect_max_stacks: 3,
            effect_duration_seconds: 3.0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_riftmaker", &["riftmaker"]),
            infusion_buff: "radiant_riftmaker_infusion",
            corruption_buff: "radiant_riftmaker_corruption",
            price: 1900,
            hp: 600,
            magic_power: 150,
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
                hp,
                magic_power,
                effect_caster_hp_percent_power,
                effect_vamp,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }

    fn apply_infusion(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let bonus_power =
            percent_of(champion_ref.hp().max, self.effect_caster_hp_percent_power) as i32;
        if bonus_power <= 0 {
            return;
        }

        let entity_id = champion_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: buff_name(self.infusion_buff),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                magic_power: bonus_power,
                ..Default::default()
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for Riftmaker {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Riftmaker {
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
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_infusion(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_infusion(ctx, player);
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = buff_stacks(&caster_ref, self.corruption_buff);
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    vamp: self.effect_vamp,
                    name: buff_name(self.corruption_buff),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
