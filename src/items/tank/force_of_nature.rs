use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{add_stack, apply_config, item_counts, refresh_buff, ticks, ItemMeta};

// Force of Nature: Steadfast. Magic damage from enemy champions (at most
// `effect_stacks_per_second` stacks a second) and their hard crowd control
// (`effect_immobilize_stacks` at once) build stacks on the carrier; a full set
// grants bonus magic resistance and movement speed.
//
// The stacks are statless copies of one buff (`add_stack`), each new one
// rewriting every copy's duration, so they all run out together. The bonus is
// put up again, with the same duration, by every stack that leaves the count
// at the cap, so it lapses with them. Both tiers share the buff names: one
// pool of stacks and one bonus whichever is held.
#[derive(Clone, Debug)]
pub struct ForceOfNature {
    meta: ItemMeta,
    stack_buff: &'static str,
    steadfast_buff: &'static str,
    price: usize,
    hp: i32,
    magic_resistance: i32,
    move_speed_mult: i32,
    effect_max_stacks: usize,
    effect_stacks_per_second: usize,
    effect_immobilize_stacks: usize,
    effect_duration_seconds: f64,
    effect_bonus_magic_resistance: i32,
    effect_move_speed_mult: i32,
    // Non-vital stats (internals)
    // Ticks until magic damage may give another stack.
    damage_stack_cooldown: usize,
}

impl ForceOfNature {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "force_of_nature",
                &["winged_moonplate"],
                &["radiant_force_of_nature"],
            ),
            stack_buff: "force_of_nature_stack",
            steadfast_buff: "force_of_nature_steadfast",
            price: 700,
            hp: 300,
            magic_resistance: 50,
            move_speed_mult: 4,
            effect_max_stacks: 8,
            effect_stacks_per_second: 1,
            effect_immobilize_stacks: 2,
            effect_duration_seconds: 7.0,
            effect_bonus_magic_resistance: 70,
            effect_move_speed_mult: 6,
            // Non-vital stats (internals)
            damage_stack_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_force_of_nature", &["force_of_nature"]),
            price: 950,
            hp: 500,
            magic_resistance: 75,
            move_speed_mult: 4,
            // Steadfast itself is unchanged — Radiant buys the stat line only.
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
                magic_resistance,
                move_speed_mult,
                effect_max_stacks,
                effect_stacks_per_second,
                effect_immobilize_stacks,
                effect_duration_seconds,
                effect_bonus_magic_resistance,
                effect_move_speed_mult
            ]
        );
        self
    }

    // Whether `source` is a champion on the other team from `entity`.
    fn is_enemy_champion(ctx: &StableSim<'_>, entity: usize, source: usize) -> bool {
        let Some(team) = ctx.get_entity(entity).map(|e| e.team()) else {
            return false;
        };
        ctx.get_entity(source)
            .is_some_and(|s| s.is_champion() && s.team() != team)
    }

    // Adds `count` Steadfast stacks to `entity`, and puts the bonus up again
    // when they leave it at the cap.
    fn gain_stacks(&self, ctx: &mut StableSim<'_>, entity: usize, count: usize) {
        let duration = ticks(self.effect_duration_seconds);
        let stack = BuffV1::timed(self.stack_buff, duration);
        let mut stacks = 0;
        for _ in 0..count {
            stacks = add_stack(ctx, entity, &stack, self.effect_max_stacks);
        }
        if stacks == 0 || stacks < self.effect_max_stacks {
            return;
        }
        refresh_buff(
            ctx,
            entity,
            self.steadfast_buff,
            &BuffV1 {
                magic_resistance: self.effect_bonus_magic_resistance,
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(self.steadfast_buff, duration)
            },
        );
    }
}

impl Default for ForceOfNature {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ForceOfNature {
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
            magic_resistance: self.magic_resistance,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.damage_stack_cooldown = 0;
    }

    // The stacks on the carrier, for the number on the item's icon; they run
    // out together, so it drops to nothing at once.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.damage_stack_cooldown = self.damage_stack_cooldown.saturating_sub(1);

        let stacks = self.stack_buff;
        item_counts::note_in_combat(ctx, player, self.meta.key, || {
            item_counts::buff_copies(ctx, player, stacks)
        });
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if damage == 0
            || damage_type != DamageTypeV1::Ap
            || self.effect_stacks_per_second == 0
            || self.damage_stack_cooldown > 0
            || !Self::is_enemy_champion(ctx, entity, attacker)
        {
            return;
        }
        // One stack, then none for the rest of its share of the second.
        self.damage_stack_cooldown = ticks(1.0 / self.effect_stacks_per_second as f64).max(1);
        self.gain_stacks(ctx, entity, 1);
    }

    // The host reports hard crowd control only, which is what "immobilized"
    // means here; it does not say which kind.
    fn on_cc(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize, caster: usize) {
        let Some(entity) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        if !Self::is_enemy_champion(ctx, entity, caster) {
            return;
        }
        self.gain_stacks(ctx, entity, self.effect_immobilize_stacks);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::MagicResistance,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::MagicResistance
    }
}
