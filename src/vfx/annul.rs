//! Annul: Grants a Spell Shield that blocks the next enemy Ability. Shared by
//! Verdant Barrier, Banshee's Veil and Edge of Night; each item owns its
//! cooldown and hands it to the item it upgrades into.
//!
//! Item hooks run after a hit has resolved, so the block has to be in place
//! before the ability lands. While Annul is up the carrier holds `ANNUL_BUFF`:
//! `skill_damaged_reduce` takes ability damage to nothing and `cc_immune` stops
//! its crowd control. Basic attacks are reduced by a separate stat
//! (`base_attack_damaged_reduce`), so they land in full and never pop it.
//!
//! The first enemy ability to reach the carrier pops the shield, and the buff
//! comes off on the next `update` rather than in the hook: an ability that
//! deals its damage before its stun would otherwise lose the immunity between
//! the two. Everything else landing in that same tick is blocked with it.
//!
//! The host of game 0.6.2 never calls the upgrade hooks, so the hand-over runs
//! through `crate::upgrade_carry`: every pop notes the tick the shield is ready
//! again under the item's upgrade line, and an item built from another on that
//! line starts from the latest one.

use mod_api_stable::*;

use crate::{refresh_buff, ticks, upgrade_carry};

/// One name for every Annul item, so a champion holding two has one shield:
/// the hit that pops it reaches both items and starts both cooldowns. Also the
/// `view_buffs` name in `view/effects.view_effects` that draws the shield
/// bubble (`effects/annul_spell_shield`) for as long as it is up.
const ANNUL_BUFF: &str = "riot_annul";

#[derive(Clone, Debug, Default)]
pub(crate) struct Annul {
    cooldown: usize,
    /// The Spell Shield is on the carrier and has not been spent.
    up: bool,
    /// Popped this tick; `update` takes the buff off the carrier.
    popped: bool,
    /// The upgrade line the cooldown is noted under: the key of the first item
    /// of those that hand it on to one another.
    line: &'static str,
    /// Whether this item is built from another on its line, and so takes over
    /// that one's cooldown.
    built_up: bool,
    /// Whether this instance has taken over the cooldown of the item it replaced.
    inherited: bool,
}

/// Flags packed above the cooldown in [`Annul::carry`].
const CARRY_UP: u64 = 1 << 62;
const CARRY_POPPED: u64 = 1 << 63;

/// The carrier's ability damage reduction from every other buff, and the
/// shield's own if it is on, so the shield only fills the room left under 100%.
fn skill_damaged_reduce(entity: &StableEntity<'_, '_>) -> (usize, Option<usize>) {
    let mut others = 0;
    let mut own = None;
    for buff in (0..entity.buff_count()).filter_map(|i| entity.buff_at(i)) {
        if buff.name() == ANNUL_BUFF {
            own = Some(buff.skill_damaged_reduce);
        } else {
            others += buff.skill_damaged_reduce;
        }
    }
    (others, own)
}

impl Annul {
    /// The Annul of an item on `line`; `built_up` is whether the item is built
    /// from another on it.
    pub(crate) fn on_line(line: &'static str, built_up: bool) -> Self {
        Self {
            line,
            built_up,
            ..Self::default()
        }
    }

    /// The carrier respawned: the shield starts over, ready.
    pub(crate) fn reset(&mut self, ctx: &StableSim<'_>, player: usize) {
        self.cooldown = 0;
        self.up = false;
        self.popped = false;
        // Nothing left to take over, and nothing for a later upgrade to either.
        self.inherited = true;
        upgrade_carry::note(self.line, ctx, player, 0);
    }

    /// What an upgrade hands its successor, so building the next tier neither
    /// resets the cooldown nor strands a popped shield on the carrier: the old
    /// item is gone before its `update` could take the buff off.
    pub(crate) fn carry(&self) -> u64 {
        let mut carry = self.cooldown as u64;
        if self.up {
            carry |= CARRY_UP;
        }
        if self.popped {
            carry |= CARRY_POPPED;
        }
        carry
    }

    /// Picks up where the item this one was built from left off.
    pub(crate) fn resume(&mut self, carry: u64) {
        self.cooldown = (carry & !(CARRY_UP | CARRY_POPPED)) as usize;
        self.up = carry & CARRY_UP != 0;
        self.popped = carry & CARRY_POPPED != 0;
    }

    /// Takes a popped shield off, then keeps a ready one on the carrier. The
    /// buff is re-checked every tick rather than added once: death can strip
    /// it, and its size follows the carrier's other ability damage reduction
    /// (Cloak of Starry Night's grows with magic resistance).
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // An item built from another arrives as a fresh instance: it takes over
        // what is left of that one's cooldown, once.
        if !std::mem::replace(&mut self.inherited, true) && self.built_up {
            if let Some((_, ready_at)) = upgrade_carry::latest(self.line, ctx, player) {
                let left = (ready_at as usize).saturating_sub(ctx.tick());
                self.cooldown = self.cooldown.max(left);
            }
        }
        let champion = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.is_alive()));
        if self.popped {
            if let Some((entity, _)) = champion {
                ctx.entity_remove_buff(entity, ANNUL_BUFF);
                self.popped = false;
            }
            return;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        if self.cooldown > 0 {
            return;
        }
        let Some((entity, true)) = champion else {
            return;
        };
        let Some((others, own)) = ctx.get_entity(entity).map(|c| skill_damaged_reduce(&c)) else {
            return;
        };

        let reduce = 100usize.saturating_sub(others);
        self.up = true;
        if own == Some(reduce) {
            return;
        }
        refresh_buff(
            ctx,
            entity,
            ANNUL_BUFF,
            &BuffV1 {
                skill_damaged_reduce: reduce,
                cc_immune: true,
                ..BuffV1::named(ANNUL_BUFF)
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        attack_type: AttackTypeV1,
        cooldown_seconds: f64,
    ) {
        // A tick of damage over time is ability damage like a skill's hit. An
        // `Item` hit only counts when the shield took it to nothing.
        let blocked = match attack_type {
            AttackTypeV1::Skill | AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield => true,
            AttackTypeV1::Item => damage == 0,
            _ => false,
        };
        if blocked {
            self.pop(ctx, player, entity, attacker, cooldown_seconds);
        }
    }

    /// A pure crowd-control ability deals no damage, so this is what spends
    /// the shield on one. Whether the host still reports CC that `cc_immune`
    /// turned away is not known yet.
    pub(crate) fn on_cc(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        caster: usize,
        cooldown_seconds: f64,
    ) {
        let Some(entity) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        self.pop(ctx, player, entity, caster, cooldown_seconds);
    }

    /// Spends the shield on a hit from `source`, if it is up and `source` is
    /// not on the carrier's team. A source the host no longer knows (a caster
    /// who died with the spell in flight) still counts: the hit was blocked.
    fn pop(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        source: usize,
        cooldown_seconds: f64,
    ) {
        if !self.up {
            return;
        }
        let Some(team) = ctx.get_entity(entity).map(|e| e.team()) else {
            return;
        };
        if ctx.get_entity(source).is_some_and(|s| s.team() == team) {
            return;
        }
        self.up = false;
        self.popped = true;
        self.cooldown = ticks(cooldown_seconds);
        let ready_at = ctx.tick() + self.cooldown;
        upgrade_carry::note(self.line, ctx, player, ready_at as u64);
    }
}
