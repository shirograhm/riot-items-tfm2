//! Tells when the carrier has immobilized an enemy champion. Shared by
//! Imperial Mandate's Command and Solstice Sleigh's Going Sledding; each item
//! owns what happens when it does.
//!
//! A taunt is the easy one: it names its taunter, so `update` reports every
//! new taunt onto the carrier, with no skill hit needed (Knight's taunt deals
//! no damage, and it never set Command off while it waited on one).
//!
//! For the rest the host does not say who applied a crowd control, so "you
//! immobilized them" is read as one of your skills hitting a champion who
//! becomes newly immobilized at that hit or within `IMMOBILIZE_WINDOW_TICKS`
//! after it (a skill's stun may land either side of its hit report). "Newly"
//! means more immobilizing effects than the last tick's baseline, so chaining
//! a second stun onto a stunned target counts again. An ally's stun landing in
//! that same window reads the same and will be credited to you, which is why a
//! carrier with no such skill of its own claims nothing (`claims_hits`): its
//! slows would otherwise collect every stun its allies land.

use mod_api_stable::*;

/// Crowd control that takes movement out of the target's hands: stun, root,
/// knock-up, knockback/pull, fear and charm, and taunt, which is counted on its
/// own below. League's "immobilize" set, plus the three that walk the target
/// somewhere; disarm, silence and ground leave them free to walk, and a slow
/// is not crowd control here at all. `champion_traits::Immobilize` is the same
/// set read off a champion's kit: keep the two in step.
const IMMOBILIZING: [CcKindV1; 6] = [
    CcKindV1::Airborne,
    CcKindV1::Stun,
    CcKindV1::Bind,
    CcKindV1::ForceMove,
    CcKindV1::Fear,
    CcKindV1::Charm,
];

/// What the entity is under right now, read in one pass: (immobilizing effects,
/// taunts aside; taunts onto `taunter`). A taunt names who it forces its target
/// to attack, which is who applied it: the one immobilize whose source the host
/// does give.
fn held(entity: &StableEntity<'_, '_>, taunter: usize) -> (usize, usize) {
    let (mut immobilized, mut taunted) = (0, 0);
    for cc in (0..entity.cc_count()).filter_map(|i| entity.cc_at(i)) {
        if cc.kind == CcKindV1::Taunt.code() {
            taunted += usize::from(cc.target == taunter);
        } else if IMMOBILIZING.iter().any(|kind| kind.code() == cc.kind) {
            immobilized += 1;
        }
    }
    (immobilized, taunted)
}

/// How many immobilizing effects the entity is under right now, taunts aside.
fn immobilize_count(entity: &StableEntity<'_, '_>) -> usize {
    (0..entity.cc_count())
        .filter(|&i| {
            entity
                .cc_at(i)
                .is_some_and(|cc| IMMOBILIZING.iter().any(|kind| kind.code() == cc.kind))
        })
        .count()
}

/// Whether a skill hit by `champion` may claim an immobilize (`claims_hits`):
/// yes unless its kit is known to have none but a taunt.
fn claims_hits(champion: &StableEntity<'_, '_>) -> bool {
    champion
        .name()
        .and_then(|key| crate::champion_traits::traits(&key))
        .and_then(|traits| traits.immobilize)
        .map_or(true, |immobilize| immobilize.other)
}

/// Ticks after a skill hit in which a new immobilize still counts as that
/// hit's: a skill's stun may land after its hit is reported, not before.
const IMMOBILIZE_WINDOW_TICKS: usize = 2;

/// A skill hit waiting to see whether it immobilized its target.
#[derive(Clone, Copy, Debug)]
struct PendingHit {
    target: usize,
    /// Immobilizing effects the target was under before the hit.
    before: usize,
    expires_at: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImmobilizeWatch {
    /// Each enemy champion as of the last tick: (entity, immobilize count,
    /// taunts onto the carrier). The first count is the "before" a skill hit is
    /// compared against, the second the one a new taunt is.
    baseline: Vec<(usize, usize, usize)>,
    /// The tick before's `baseline`, kept to swap with rather than reallocate.
    previous: Vec<(usize, usize, usize)>,
    pending: Vec<PendingHit>,
    /// Whether a skill hit may claim an immobilize that lands with it: not for
    /// a carrier whose kit is known to immobilize with nothing but a taunt, or
    /// nothing at all, or every stun an ally lands on a target of its slows
    /// would be credited to it. `None` until the carrier's champion is read.
    claims_hits: Option<bool>,
}

impl ImmobilizeWatch {
    /// Forgets what it was following, for `on_spawn`. What it knows of the
    /// carrier's kit stays.
    pub(crate) fn reset(&mut self) {
        self.baseline.clear();
        self.previous.clear();
        self.pending.clear();
    }

    /// From `on_skill_hit`: whether this hit of the carrier's immobilized
    /// `target` there and then. A hit that has not yet is kept an eye on for
    /// `IMMOBILIZE_WINDOW_TICKS`, and `update` reports it if it does.
    pub(crate) fn skill_hit(&mut self, ctx: &StableSim<'_>, target: usize, is_ally: bool) -> bool {
        if is_ally || self.claims_hits == Some(false) {
            return false;
        }
        let Some(now) = ctx
            .get_entity(target)
            .filter(|t| t.is_champion() && t.is_alive())
            .map(|t| immobilize_count(&t))
        else {
            return false;
        };
        let before = self.baseline_of(target);
        if now > before {
            return true;
        }
        let expires_at = ctx.tick() + IMMOBILIZE_WINDOW_TICKS;
        match self.pending.iter_mut().find(|p| p.target == target) {
            Some(pending) => pending.expires_at = expires_at,
            None => self.pending.push(PendingHit {
                target,
                before,
                expires_at,
            }),
        }
        false
    }

    /// From `update`, every tick: the enemy champions the carrier has
    /// immobilized since the last one. Skill hits that were waiting on their
    /// stun first, then new taunts.
    pub(crate) fn update(&mut self, ctx: &StableSim<'_>, player: usize) -> Vec<usize> {
        let tick = ctx.tick();
        let mut immobilized = Vec::new();

        // Hits still waiting on their stun, judged against the count from
        // before they landed.
        if !self.pending.is_empty() {
            self.pending.retain(|p| {
                let now = ctx
                    .get_entity(p.target)
                    .filter(|t| t.is_alive())
                    .map_or(0, |t| immobilize_count(&t));
                if now > p.before {
                    immobilized.push(p.target);
                    return false;
                }
                tick < p.expires_at
            });
        }

        let Some(carrier) = ctx.get_player(player).and_then(|p| p.champion()) else {
            return immobilized;
        };
        let (carrier_id, team) = (carrier.id(), carrier.team());
        if self.claims_hits.is_none() {
            self.claims_hits = Some(claims_hits(&carrier));
        }

        // This tick's counts become the next hit's "before", and a champion
        // under more of the carrier's taunts than a tick ago has just been
        // taunted by them.
        std::mem::swap(&mut self.baseline, &mut self.previous);
        self.baseline.clear();
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some((count, taunts)) = ctx
                .get_entity(id)
                .filter(|e| e.team() != team && e.is_alive())
                .map(|e| held(&e, carrier_id))
            else {
                continue;
            };
            let before = self
                .previous
                .iter()
                .find(|&&(known, _, _)| known == id)
                .map_or(0, |&(_, _, taunts)| taunts);
            if taunts > before {
                immobilized.push(id);
            }
            self.baseline.push((id, count, taunts));
        }
        immobilized
    }

    fn baseline_of(&self, target: usize) -> usize {
        self.baseline
            .iter()
            .find(|&&(id, _, _)| id == target)
            .map_or(0, |&(_, count, _)| count)
    }
}
