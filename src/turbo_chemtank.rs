use arrayvec::ArrayString;
use mod_api::*;

use crate::apply_adaptive_force;
use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct TurboChemtank {
    price: usize,
    hp: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for TurboChemtank {
    fn default() -> Self {
        Self {
            price: 1250,
            hp: 350,
            adaptive_force: 80,
            effect_move_speed_mult: 25,
            effect_duration_seconds: 6,
            effect_cooldown_seconds: 40,
        }
    }
}

impl TurboChemtank {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for TurboChemtank {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "turbo_chemtank"
    }

    fn icon(&self) -> &str {
        "t12_4"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_turbo_chemtank".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "turbo_chemtank_adaptive_force",
        );
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "turbo_chemtank_adaptive_force",
        );
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _rng_seed: usize,
        player: usize,
        attacker: usize,
        _damage: usize,
    ) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        if !attacker_ref.is_champion() {
            return;
        }

        let is_on_cooldown = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "turbo_chemtank_cooldown");
        if !is_on_cooldown {
            ctx.add_buff(
                player,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    cc_immune: true,
                    move_speed_mult: self.effect_move_speed_mult,
                    ..Default::default()
                },
            );
            ctx.add_buff(
                player,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_cooldown_seconds * 60,
                    },
                    name: ArrayString::try_from("turbo_chemtank_cooldown").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::AP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantTurboChemtank {
    price: usize,
    hp: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for RadiantTurboChemtank {
    fn default() -> Self {
        Self {
            price: 1850,
            hp: 450,
            adaptive_force: 150,
            effect_move_speed_mult: 25,
            effect_duration_seconds: 6,
            effect_cooldown_seconds: 40,
        }
    }
}

impl RadiantTurboChemtank {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
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
            .any(|i| entity_ref.buff_at(i).name.as_str() == "turbo_chemtank_adaptive_force");
        let force_to_apply = if is_prior_buff_applied {
            self.adaptive_force - RadiantTurboChemtank::default().adaptive_force
        } else {
            self.adaptive_force
        };

        apply_adaptive_force(
            ctx,
            player,
            force_to_apply,
            "radiant_turbo_chemtank_adaptive_force",
        );
    }
}

impl ModItemInfo for RadiantTurboChemtank {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_turbo_chemtank"
    }

    fn icon(&self) -> &str {
        "t12_5"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["turbo_chemtank".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _rng_seed: usize,
        player: usize,
        attacker: usize,
        _damage: usize,
    ) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        if !attacker_ref.is_champion() {
            return;
        }

        let is_on_cooldown = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_turbo_chemtank_cooldown");
        if !is_on_cooldown {
            ctx.add_buff(
                player,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    cc_immune: true,
                    move_speed_mult: self.effect_move_speed_mult,
                    ..Default::default()
                },
            );
            ctx.add_buff(
                player,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_cooldown_seconds * 60,
                    },
                    name: ArrayString::try_from("radiant_turbo_chemtank_cooldown").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::AP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
