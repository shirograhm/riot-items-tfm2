use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_enemy_champion, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS, TICKS_PER_SECOND,
};

const PROC_LOCKOUT_TICKS: usize = BUFF_REFRESH_DURATION_TICKS;

#[derive(Clone, Debug)]
pub struct DeadMansPlate {
    meta: ItemMeta,
    momentum_buff: &'static str,
    price: usize,
    hp: i32,
    defence: i32,
    move_speed_mult: i32,
    effect_stacks_per_second: usize,
    effect_max_stacks: usize,
    effect_move_speed_per_stack: f64,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    momentum: usize,
    stack_progress: usize,
    refresh_cooldown: usize,
    proc_cooldown: usize,
}

impl DeadMansPlate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "dead_mans_plate",
                &["winged_moonplate"],
                &["radiant_dead_mans_plate"],
            ),
            momentum_buff: "dead_mans_plate_momentum",
            price: 1450,
            hp: 300,
            defence: 55,
            move_speed_mult: 4,
            effect_stacks_per_second: 7,
            effect_max_stacks: 100,
            effect_move_speed_per_stack: 0.25,
            effect_min_bonus_damage: 0,
            effect_max_bonus_damage: 200,
            // Non-vital stats (internals)
            momentum: 0,
            stack_progress: 0,
            refresh_cooldown: 0,
            proc_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dead_mans_plate", &["dead_mans_plate"]),
            momentum_buff: "radiant_dead_mans_plate_momentum",
            price: 2100,
            hp: 650,
            defence: 70,
            move_speed_mult: 4,
            effect_stacks_per_second: 7,
            effect_max_stacks: 100,
            effect_move_speed_per_stack: 0.25,
            effect_min_bonus_damage: 0,
            effect_max_bonus_damage: 200,
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
                defence,
                move_speed_mult,
                effect_stacks_per_second,
                effect_max_stacks,
                effect_move_speed_per_stack,
                effect_min_bonus_damage,
                effect_max_bonus_damage
            ]
        );
        self
    }

    fn proc_damage(&self, consumed: usize) -> usize {
        let full_bar = self.effect_max_stacks.max(1);
        let span = self
            .effect_max_bonus_damage
            .saturating_sub(self.effect_min_bonus_damage);
        self.effect_min_bonus_damage + span * consumed.min(full_bar) / full_bar
    }

    fn build_momentum(&mut self) {
        if self.momentum >= self.effect_max_stacks {
            self.momentum = self.effect_max_stacks;
            self.stack_progress = 0;
            return;
        }
        let per_second = TICKS_PER_SECOND as usize;
        self.stack_progress += self.effect_stacks_per_second;
        while self.stack_progress >= per_second {
            self.stack_progress -= per_second;
            self.momentum = (self.momentum + 1).min(self.effect_max_stacks);
        }
    }

    fn apply_move_speed(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let bonus = (self.momentum as f64 * self.effect_move_speed_per_stack).round() as i32;
        if bonus <= 0 {
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let entity_id = champion_ref.id();

        ctx.add_buff(
            entity_id,
            &BuffV1 {
                move_speed_mult: bonus,
                ..BuffV1::timed(self.momentum_buff, BUFF_REFRESH_DURATION_TICKS)
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for DeadMansPlate {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DeadMansPlate {
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
            hp: self.hp,
            defence: self.defence,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.momentum = 0;
        self.stack_progress = 0;
        self.refresh_cooldown = 0;
        self.proc_cooldown = 0;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.proc_cooldown > 0 {
            self.proc_cooldown -= 1;
            return;
        }
        self.build_momentum();
        self.apply_move_speed(ctx, player);
    }

    fn on_base_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
    ) {
        if self.momentum == 0 {
            return;
        }
        if !is_enemy_champion(ctx, caster, target) {
            return;
        }

        let consumed = self.momentum;
        self.momentum = 0;
        self.stack_progress = 0;
        self.proc_cooldown = PROC_LOCKOUT_TICKS;
        self.refresh_cooldown = 0;

        ctx.deal_damage(
            caster,
            target,
            self.proc_damage(consumed),
            0,
            AttackTypeV1::Item,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Defense, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}
