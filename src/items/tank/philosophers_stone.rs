use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, immolate_burn, is_monster, mark_immolate, percent_of, upgrade_carry, ItemMeta,
    TICKS_PER_SECOND,
};

// Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies
// within 30 range. This effect deals 150% more damage to minions and monsters.
//
// Cinderhulk: Gain 1% maximum health for each champion takedown and monster
// killed, up to 15% (Radiant: up to 25%).

// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "philosophers_stone";

// The maximum health Cinderhulk has granted, worn by the carrier.
const CINDERHULK_BUFF: &str = "philosophers_stone_cinderhulk";

#[derive(Clone, Debug)]
pub struct PhilosophersStone {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
    effect_minion_bonus_percent: f64,
    effect_stack_hp_mult: i32,
    effect_max_stacks: usize,
    // Non-vital stats (internals)
    until_next_burn: usize,
    cinderhulk_stacks: usize,
    // The stack count last noted for an upgrade to take over.
    noted_stacks: usize,
    // Whether this instance has taken over the stacks of the item it replaced.
    inherited: bool,
}

impl PhilosophersStone {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(BASE_KEY, &["bamis_cinder"], &["radiant_philosophers_stone"]),
            price: 750,
            hp: 250,
            defence: 15,
            magic_resistance: 25,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_minion_bonus_percent: 150.0,
            effect_stack_hp_mult: 1,
            effect_max_stacks: 15,
            // Non-vital stats (internals)
            until_next_burn: TICKS_PER_SECOND as usize,
            cinderhulk_stacks: 0,
            noted_stacks: 0,
            inherited: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_philosophers_stone", &[BASE_KEY]),
            price: 1050,
            hp: 350,
            defence: 30,
            magic_resistance: 45,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_minion_bonus_percent: 150.0,
            effect_stack_hp_mult: 1,
            effect_max_stacks: 25,
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
                magic_resistance,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_max_distance,
                effect_minion_bonus_percent,
                effect_stack_hp_mult,
                effect_max_stacks
            ]
        );
        self
    }

    // Cinderhulk: one more step of maximum health, capped. The buff carries
    // the step rather than the running total because same-name buffs stack:
    // putting a new total on would mean taking the old one off first, and
    // maximum health that dips mid-fight can cost the carrier current health.
    fn grow(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        if self.cinderhulk_stacks >= self.effect_max_stacks {
            return;
        }
        self.cinderhulk_stacks += 1;
        ctx.add_buff(
            entity,
            &BuffV1 {
                hp_mult: self.effect_stack_hp_mult,
                ..BuffV1::named(CINDERHULK_BUFF)
            },
        );
    }

    // Keeps the stacks where an upgrade can find them. The Radiant item
    // arrives as a fresh instance: it takes over the base item's stacks once,
    // clamped to its own ceiling, and every instance notes its count when it
    // grows. Only the counter moves. The health gained this life is already on
    // the champion as `cinderhulk` buffs, and `on_spawn` re-applies it from
    // the count on the next respawn.
    fn carry_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        if !std::mem::replace(&mut self.inherited, true) && self.meta.upgrades_from(BASE_KEY) {
            if let Some((_, stacks)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
                let stacks = (stacks as usize).min(self.effect_max_stacks);
                self.cinderhulk_stacks = self.cinderhulk_stacks.max(stacks);
                self.noted_stacks = self.cinderhulk_stacks;
            }
        }
        if self.cinderhulk_stacks != self.noted_stacks {
            self.noted_stacks = self.cinderhulk_stacks;
            upgrade_carry::note(BASE_KEY, ctx, player, self.cinderhulk_stacks as u64);
        }
    }
}

impl Default for PhilosophersStone {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for PhilosophersStone {
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
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.until_next_burn = TICKS_PER_SECOND as usize;
        self.carry_stacks(ctx, player);

        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        // Flames up from the first frame rather than the first burn.
        mark_immolate(ctx, champion);

        // Cinderhulk is permanent, so the health earned so far is re-applied
        // each spawn from the banked count. Same-name buffs stack and this one
        // carries the whole total, so the previous life's copies go first.
        if self.cinderhulk_stacks == 0 {
            return;
        }
        ctx.entity_remove_buff(champion, CINDERHULK_BUFF);
        ctx.add_buff(
            champion,
            &BuffV1 {
                hp_mult: self.cinderhulk_stacks as i32 * self.effect_stack_hp_mult,
                ..BuffV1::named(CINDERHULK_BUFF)
            },
        );
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_stacks(ctx, player);

        // Immolate, once a second while alive, the same burn as Bami's Cinder.
        self.until_next_burn = self.until_next_burn.saturating_sub(1);
        if self.until_next_burn > 0 {
            return;
        }
        self.until_next_burn = TICKS_PER_SECOND as usize;

        let Some((caster, team, max_hp)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.team(), c.hp().1))
        else {
            return;
        };

        mark_immolate(ctx, caster);
        let damage = self.effect_bonus_flat_damage
            + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let minion_damage = damage + percent_of(damage, self.effect_minion_bonus_percent);
        immolate_burn(
            ctx,
            caster,
            team,
            self.effect_max_distance,
            damage,
            minion_damage,
        );
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        let Some(victim_ref) = ctx.get_entity(victim) else {
            return;
        };
        // Champion and monster kills grow; minions do not, which is what
        // `is_monster` rules out on top of champions and towers.
        if victim_ref.is_champion() || is_monster(&victim_ref) {
            self.grow(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.grow(ctx, entity);
    }

    // Cinderhulk stacks survive the Radiant upgrade, clamped to the
    // successor's own ceiling in case the config gives the two variants
    // different caps.
    //
    // The host of game 0.6.2 never calls these two (`crate::upgrade_carry`);
    // `carry_stacks` does the carrying, and they stay for a host that does.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.cinderhulk_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.cinderhulk_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::DotDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
