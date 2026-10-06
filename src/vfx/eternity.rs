//! Eternity: taking damage from an enemy champion grants a stack of Ability
//! Haste for a while, and every Ability the carrier casts heals them. Shared
//! by Catalyst of Aeons and Rod of Ages; each item owns its numbers.
//!
//! No hook reports a cast, so one is read the way Spellblade reads it: an
//! ability's remaining cooldown going *up* between two ticks, since it only
//! ever counts down otherwise. That covers every ability, the ones that hit
//! nothing included.
//!
//! # One Eternity for the champion
//!
//! The haste is `HASTE_BUFF` under one name for every Eternity item, so the
//! stacks are one pool with one limit however many of them the champion holds,
//! and they stay through an upgrade from one item to the next.
//!
//! The heal has no buff to share. Every Eternity item sees the same cast in
//! the same tick, so the first to heal for it leaves a note (`HEALED`) and the
//! others find it and stand down.

use std::cell::Cell;

use mod_api_stable::*;

use crate::{add_stack, is_enemy_champion};

/// The haste stacks on the carrier.
const HASTE_BUFF: &str = "riot_eternity";
/// Levels run from 1 to 12, so the heal grows over eleven steps.
const LEVEL_STEPS: f64 = 11.0;

thread_local! {
    /// The cast last healed for, as (match seed, tick, champion). A match is
    /// simulated on one thread, so the items of one champion all read it.
    static HEALED: Cell<Option<(u64, usize, usize)>> = const { Cell::new(None) };
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Eternity {
    /// The carrier's remaining ability cooldowns (skill, skill2, ult) last
    /// tick. `None` until the first reading after a spawn, which is only a
    /// baseline.
    last_cooldowns: Option<(usize, usize, usize)>,
}

impl Eternity {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Heals the carrier when they cast an Ability: `min_heal` at level 1 and
    /// `max_heal` at level 12, the same per-level step the other level-scaled
    /// effects use. Call once per `update`.
    pub(crate) fn update(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        min_heal: usize,
        max_heal: usize,
    ) {
        let cooldowns = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, skill, skill2, ult)| (skill, skill2, ult));
        let cast = matches!(
            (cooldowns, self.last_cooldowns),
            (Some(now), Some(before))
                if now.0 > before.0 || now.1 > before.1 || now.2 > before.2
        );
        self.last_cooldowns = cooldowns;
        if !cast {
            return;
        }

        let Some((champion, level)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.level()))
        else {
            return;
        };
        let this_cast = (ctx.seed(), ctx.tick(), champion);
        if HEALED.replace(Some(this_cast)) == Some(this_cast) {
            return;
        }

        let per_level = (max_heal.saturating_sub(min_heal) as f64 / LEVEL_STEPS).round() as usize;
        let heal = min_heal + level.saturating_sub(1) * per_level;
        ctx.heal(champion, champion, heal);
    }

    /// A hit on the carrier: one from an enemy champion adds a stack of
    /// `haste` and restarts the duration of the stacks already there. For an
    /// Eternity item's `on_damaged`.
    pub(crate) fn damaged(
        ctx: &mut StableSim<'_>,
        entity: usize,
        attacker: usize,
        damage: usize,
        haste: i32,
        duration_ticks: usize,
        max_stacks: usize,
    ) {
        if damage == 0 || !is_enemy_champion(ctx, entity, attacker) {
            return;
        }
        add_stack(
            ctx,
            entity,
            &BuffV1 {
                skill_cooldown_mult: haste,
                ..BuffV1::timed(HASTE_BUFF, duration_ticks)
            },
            max_stacks,
        );
    }
}
