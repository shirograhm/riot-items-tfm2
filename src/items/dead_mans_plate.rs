use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_enemy_champion, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS, TICKS_PER_SECOND,
};

#[derive(Clone, Debug)]
pub struct DeadMansPlate {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
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
    /// Momentum currently held. Owned by the item rather than counted from
    /// stacked buffs: at 100 stacks that would be 100 buffs on the champion.
    momentum: usize,
    /// Sub-stack carry, in ticks-worth of generation, so `effect_stacks_per_second`
    /// need not divide evenly into the tick rate.
    stack_progress: usize,
    refresh_cooldown: usize,
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
            momentum: 0,
            stack_progress: 0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dead_mans_plate", &["dead_mans_plate"]),
            momentum_buff: "radiant_dead_mans_plate_momentum",
            price: 2100,
            hp: 650,
            defence: 70,
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

    /// Shipwrecker's damage for `consumed` Momentum: a straight line from
    /// `effect_min_bonus_damage` at no Momentum to `effect_max_bonus_damage` at a
    /// full bar. The per-stack step is derived rather than configured so the two
    /// ends of the range stay exactly where they are set, whatever
    /// `effect_max_stacks` is.
    fn proc_damage(&self, consumed: usize) -> usize {
        let full_bar = self.effect_max_stacks.max(1);
        let span = self
            .effect_max_bonus_damage
            .saturating_sub(self.effect_min_bonus_damage);
        self.effect_min_bonus_damage + span * consumed.min(full_bar) / full_bar
    }

    /// Accrues Momentum at `effect_stacks_per_second`, independent of whether the
    /// champion is actually moving — TFM2 gives no per-entity velocity to gate on.
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

    /// Re-grants the movement speed Momentum is currently worth. The bonus has to
    /// track a value that moves both ways (it drops to nothing on a proc), so it
    /// is a short `Time` buff refreshed on a slightly shorter cycle.
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
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.build_momentum();
        self.apply_move_speed(ctx, player);
    }

    /// Shipwrecker. Basic attacks are the only physical damage instance a mod can
    /// observe, so the proc rides `on_attack` and ignores ability hits.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageTypeV1,
    ) {
        // There has to be Momentum to consume: with no stacks there is nothing to
        // spend, so the proc waits rather than paying out its floor for free.
        if damage_type != DamageTypeV1::Ad || self.momentum == 0 {
            return;
        }
        if !is_enemy_champion(ctx, caster, target) {
            return;
        }

        let consumed = self.momentum;
        self.momentum = 0;
        self.stack_progress = 0;

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
