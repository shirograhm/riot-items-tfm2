use arrayvec::ArrayString;
use mod_api::*;

use crate::apply_adaptive_force;
use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct MirageBlade {
    price: usize,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl Default for MirageBlade {
    fn default() -> Self {
        Self {
            price: 1500,
            attack_speed_mult: 40,
            move_speed_mult: 10,
            adaptive_force: 60,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 2.0,
        }
    }
}

impl MirageBlade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for MirageBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "mirage_blade"
    }

    fn icon(&self) -> &str {
        "t9_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_mirage_blade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "mirage_blade_adaptive_force",
        );
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "mirage_blade_adaptive_force",
        );
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_move_speed");

        if !is_buff_applied {
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
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

#[derive(Clone, Debug)]
pub struct RadiantMirageBlade {
    price: usize,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl Default for RadiantMirageBlade {
    fn default() -> Self {
        Self {
            price: 2100,
            attack_speed_mult: 65,
            move_speed_mult: 15,
            adaptive_force: 100,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 2.0,
        }
    }
}

impl RadiantMirageBlade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }

    pub fn apply_buff(&self, ctx: &mut GameCtx, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_prior_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_adaptive_force");
        let force_to_apply = if is_prior_buff_applied {
            self.adaptive_force - MirageBlade::default().adaptive_force
        } else {
            self.adaptive_force
        };

        apply_adaptive_force(
            ctx,
            player,
            force_to_apply,
            "radiant_mirage_blade_adaptive_force",
        );
    }
}

impl ModItemInfo for RadiantMirageBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_mirage_blade"
    }

    fn icon(&self) -> &str {
        "t9_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["mirage_blade".to_string()]
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
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_move_speed");

        if !is_buff_applied {
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
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
