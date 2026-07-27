use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_adaptive_force, apply_config, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct MirageBlade {
    meta: ItemMeta,
    adaptive_force_buff: &'static str,
    // The previous tier's buff and the Adaptive Force it grants. When the holder
    // already carries that item, this one only tops up the difference instead of
    // granting its full amount. `None` on the base variant, which has no earlier
    // Adaptive Force tier to stack with.
    upgrades_from: Option<(&'static str, i32)>,
    price: usize,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl MirageBlade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "mirage_blade",
                &["scouts_slingshot"],
                &["radiant_mirage_blade"],
            ),
            adaptive_force_buff: "mirage_blade_adaptive_force",
            upgrades_from: None,
            price: 1500,
            attack_speed_mult: 40,
            move_speed_mult: 10,
            adaptive_force: 60,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_mirage_blade", &["mirage_blade"]),
            adaptive_force_buff: "radiant_mirage_blade_adaptive_force",
            upgrades_from: Some(("mirage_blade_adaptive_force", 60)),
            price: 2100,
            attack_speed_mult: 65,
            move_speed_mult: 15,
            adaptive_force: 100,
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
                attack_speed_mult,
                move_speed_mult,
                adaptive_force,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        self
    }

    fn apply_buff(&self, ctx: &mut GameCtx, player: usize) {
        let mut force = self.adaptive_force;
        if let Some((prior_buff, prior_force)) = self.upgrades_from {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            if has_buff(&champion_ref, prior_buff) {
                force = self.adaptive_force - prior_force;
            }
        }
        apply_adaptive_force(ctx, player, force, self.adaptive_force_buff);
    }
}

impl Default for MirageBlade {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for MirageBlade {
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
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = has_buff(&champion_ref, "mirage_blade_move_speed");

        if !is_buff_applied {
            ctx.add_buff(
                champion_ref.id(),
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("mirage_blade_move_speed").unwrap(),
                    ..Default::default()
                },
            )
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
