use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, item_counts, ticks, upgrade_carry, ItemMeta};

// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "hubris";

#[derive(Clone, Debug)]
pub struct Hubris {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
    // Whether this instance has taken over the stacks of the item it replaced.
    inherited: bool,
}

impl Hubris {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(BASE_KEY, &["serrated_dirk"], &["radiant_hubris"]),
            price: 650,
            attack: 35,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            // Non-vital stats (internals)
            eminence_stacks: 0,
            inherited: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hubris", &[BASE_KEY]),
            price: 1000,
            attack: 60,
            skill_cooldown_mult: 15,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
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
                skill_cooldown_mult,
                effect_lethality,
                effect_bonus_flat_attack,
                effect_stack_attack,
                effect_duration_seconds
            ]
        );
        self
    }

    // The Radiant item arrives as a fresh instance: it takes over the base
    // item's stacks, once, before it first counts anything. Only stacks of
    // the carrier's life now: a death ends them.
    fn inherit_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        if self.inherited || !self.meta.upgrades_from(BASE_KEY) {
            return;
        }
        let Some(entity) = champion_of(ctx, player) else {
            return;
        };
        self.inherited = true;
        if let Some(stacks) = noted_stacks(ctx, player, entity) {
            self.eminence_stacks = self.eminence_stacks.max(stacks);
        }
    }

    // A takedown: a bonus sized by the stacks so far, and one more stack.
    fn takedown(&mut self, ctx: &mut StableSim<'_>, player: usize, entity: usize) {
        self.inherit_stacks(ctx, player);
        let bonus_ad =
            self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32;
        self.eminence_stacks += 1;
        upgrade_carry::note(BASE_KEY, ctx, player, pack(entity, self.eminence_stacks));

        ctx.add_buff(
            entity,
            &BuffV1 {
                attack: bonus_ad,
                ..BuffV1::timed("hubris_bonus", ticks(self.effect_duration_seconds))
            },
        );
    }
}

// What a note under the upgrade line carries: the carrier's champion entity
// and the stacks, in one number. The game makes the champion a new entity on
// every respawn, so the entity tells this life's note from an earlier one's.
fn pack(entity: usize, stacks: usize) -> u64 {
    ((entity as u64) << 32) | (stacks as u64 & 0xffff_ffff)
}

fn champion_of(ctx: &StableSim<'_>, player: usize) -> Option<usize> {
    ctx.get_player(player)?.champion().map(|champion| champion.id())
}

// The stacks noted in the life of the champion `entity`, where any were.
fn noted_stacks(ctx: &StableSim<'_>, player: usize, entity: usize) -> Option<usize> {
    let (_, value) = upgrade_carry::latest(BASE_KEY, ctx, player)?;
    ((value >> 32) as usize == entity).then_some((value & 0xffff_ffff) as usize)
}

impl Default for Hubris {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Hubris {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // A respawn ends Eminence: its bonuses and its stacks. But the host tells
    // an item bought in the middle of a life `on_spawn` too, and an upgrade
    // to the Radiant item is one: that lost every stack and bonus (user,
    // 2026-10-14). A note from the champion as it is now is this life's, so
    // there the stacks are taken over and the bonuses left running.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some(entity) = champion_of(ctx, player) else {
            return;
        };
        self.inherited = true;
        if let Some(stacks) = noted_stacks(ctx, player, entity) {
            self.eminence_stacks = self.eminence_stacks.max(stacks);
            return;
        }
        ctx.entity_remove_buff(entity, "hubris_bonus");
        self.eminence_stacks = 0;
        upgrade_carry::note(BASE_KEY, ctx, player, pack(entity, 0));
    }

    // The stacks are taken over first, so a Radiant item has the base item's
    // from its first tick and not only from its first takedown, which is
    // when it would take them over anyway.
    //
    // The number on the item's icon is the Attack Damage Eminence gives now:
    // every takedown's `hubris_bonus` still running, added up. It goes up
    // with each takedown and down as each runs out, and a death clears them.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.inherit_stacks(ctx, player);
        item_counts::note_in_combat(ctx, player, self.meta.key, || {
            item_counts::buff_sum(ctx, player, "hubris_bonus", |buff| buff.attack)
        });
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_target_tower = target_ref.is_tower();

        if !is_target_tower {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }
    }

    fn on_kill(
        &mut self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        player: usize,
        entity: usize,
        victim: usize,
    ) {
        let Some(victim_ref) = sim.get_entity(victim) else {
            return;
        };
        if !victim_ref.is_champion() {
            return;
        }
        self.takedown(sim, player, entity);
    }

    fn on_assist(&mut self, sim: &mut StableSim<'_>, player: usize, entity: usize) {
        self.takedown(sim, player, entity);
    }

    // Eminence is bought, not earned twice: the kills banked on the base item
    // keep scaling the bonus after the Radiant upgrade replaces it.
    //
    // The host of game 0.6.2 never calls these two (`crate::upgrade_carry`);
    // `inherit_stacks` does the carrying, and they stay for a host that does.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.eminence_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.eminence_stacks = carry as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
