use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_lethality, count_takedowns, mark_enemy_champion};

#[derive(Clone, Debug)]
pub struct Hubris {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
    // Enemy champions recently damaged: (entity id, remaining window ticks).
    takedown_marks: Vec<(usize, usize)>,
}

impl Default for Hubris {
    fn default() -> Self {
        Self {
            price: 1250,
            attack: 75,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            eminence_stacks: 0,
            takedown_marks: Vec::new(),
        }
    }
}

impl Hubris {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_attack: cfg
                .effect_bonus_flat_attack
                .unwrap_or(d.effect_bonus_flat_attack),
            effect_stack_attack: cfg.effect_stack_attack.unwrap_or(d.effect_stack_attack),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            eminence_stacks: 0,
            takedown_marks: Vec::new(),
        }
    }

    fn eminence_bonus_ad(&self) -> i32 {
        self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32
    }

    // Grants `count` Eminence stacks to the wielder (a growing, decaying AD buff per
    // stack), matching the "12 (+3 per stack)" payout using the pre-increment count.
    fn grant_eminence(&mut self, ctx: &mut GameCtx, player: usize, count: usize) {
        if count == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        for _ in 0..count {
            let ad = self.eminence_bonus_ad();
            ctx.add_buff(
                champion_id,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0).round() as usize,
                    },
                    attack: ad,
                    ..Default::default()
                },
            );
            self.eminence_stacks += 1;
        }
    }
}

impl ModItemInfo for Hubris {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "hubris"
    }

    fn icon(&self) -> &str {
        "hubris"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serrated_dirk".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_hubris".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.eminence_stacks = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let takedowns = count_takedowns(&mut self.takedown_marks, ctx);
        self.grant_eminence(ctx, player, takedowns);
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantHubris {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
    takedown_marks: Vec<(usize, usize)>,
}

impl Default for RadiantHubris {
    fn default() -> Self {
        Self {
            price: 1850,
            attack: 115,
            skill_cooldown_mult: 15,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            eminence_stacks: 0,
            takedown_marks: Vec::new(),
        }
    }
}

impl RadiantHubris {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_attack: cfg
                .effect_bonus_flat_attack
                .unwrap_or(d.effect_bonus_flat_attack),
            effect_stack_attack: cfg.effect_stack_attack.unwrap_or(d.effect_stack_attack),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            eminence_stacks: 0,
            takedown_marks: Vec::new(),
        }
    }

    fn eminence_bonus_ad(&self) -> i32 {
        self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32
    }

    fn grant_eminence(&mut self, ctx: &mut GameCtx, player: usize, count: usize) {
        if count == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        for _ in 0..count {
            let ad = self.eminence_bonus_ad();
            ctx.add_buff(
                champion_id,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0).round() as usize,
                    },
                    attack: ad,
                    ..Default::default()
                },
            );
            self.eminence_stacks += 1;
        }
    }
}

impl ModItemInfo for RadiantHubris {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_hubris"
    }

    fn icon(&self) -> &str {
        "radiant_hubris"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hubris".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.eminence_stacks = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let takedowns = count_takedowns(&mut self.takedown_marks, ctx);
        self.grant_eminence(ctx, player, takedowns);
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
