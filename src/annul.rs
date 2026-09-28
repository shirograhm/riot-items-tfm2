//! Annul: Grants a Spell Shield that blocks the next enemy Ability. Shared by
//! Banshee's Veil and Edge of Night; each item owns its cooldown.
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

use mod_api_stable::*;

use crate::{refresh_buff, ticks};

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
    /// Popped this tick; `update` takes the buff off this entity.
    popped: Option<usize>,
}

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
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Takes a popped shield off, then keeps a ready one on the carrier. The
    /// buff is re-checked every tick rather than added once: death can strip
    /// it, and its size follows the carrier's other ability damage reduction
    /// (Cloak of Starry Night's grows with magic resistance).
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if let Some(entity) = self.popped.take() {
            ctx.entity_remove_buff(entity, ANNUL_BUFF);
            return;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        if self.cooldown > 0 {
            return;
        }
        let Some((entity, others, own)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| {
                let (others, own) = skill_damaged_reduce(&c);
                (c.id(), others, own)
            })
        else {
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

    /// Abilities pop the shield. A burn or an item proc only does once it
    /// reaches the carrier as 0 damage: that is the shield having eaten it, and
    /// it must not go on eating them for free. Anything that still hurts went
    /// past the shield and leaves it up.
    pub(crate) fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        entity: usize,
        attacker: usize,
        damage: usize,
        attack_type: AttackTypeV1,
        cooldown_seconds: f64,
    ) {
        let blocked = match attack_type {
            AttackTypeV1::Skill => true,
            AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield | AttackTypeV1::Item => damage == 0,
            _ => false,
        };
        if blocked {
            self.pop(ctx, entity, attacker, cooldown_seconds);
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
        self.pop(ctx, entity, caster, cooldown_seconds);
    }

    /// Spends the shield on a hit from `source`, if it is up and `source` is
    /// not on the carrier's team. A source the host no longer knows (a caster
    /// who died with the spell in flight) still counts: the hit was blocked.
    fn pop(
        &mut self,
        ctx: &mut StableSim<'_>,
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
        self.popped = Some(entity);
        self.cooldown = ticks(cooldown_seconds);
    }
}
