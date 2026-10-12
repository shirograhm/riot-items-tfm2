//! Tells when the carrier has immobilized an enemy champion. Shared by
//! Imperial Mandate's Command and Solstice Sleigh's Going Sledding; each item
//! owns what happens when it does.
//!
//! A taunt is the easy one: it names its taunter, so `update` reports every
//! new taunt onto the carrier, with no skill hit needed (Knight's taunt deals
//! no damage, and it never set Command off while it waited on one).
//!
//! For the rest the host does not say who applied a crowd control, so "you
//! immobilized them" is read as two things happening to the same enemy
//! champion within [`WINDOW_TICKS`] of each other, in either order: one of
//! your skills hitting it, and it becoming newly immobilized. "Newly" means
//! more immobilizing effects than a tick before, so chaining a second stun
//! onto a stunned target counts again.
//!
//! Either order, and a window of a sixth of a second, because how a skill
//! lays its crowd control down beside its damage is the skill's own business:
//! a stun that lands before its hit is remembered for it, and a skill's hit
//! is taken from `on_attack` as well as `on_skill_hit`. Waiting for the stun
//! on the hit alone, for two ticks, misses a knock-up like Berserker's slam.
//!
//! An ally's stun landing on a champion inside that window of your skill
//! hitting it reads the same and is credited to you, which is why a carrier
//! with no such skill of its own claims nothing (`claims_hits`): its slows
//! would otherwise collect every stun its allies land.

use mod_api_stable::*;

// Crowd control that takes movement out of the target's hands: stun, root,
// knock-up, knockback/pull, fear and charm, and taunt, which is counted on its
// own below. League's "immobilize" set, plus the three that walk the target
// somewhere; disarm, silence and ground leave them free to walk, and a slow
// is not crowd control here at all. `champion_traits::Immobilize` is the same
// set read off a champion's kit: keep the two in step.
const IMMOBILIZING: [CcKindV1; 6] = [
    CcKindV1::Airborne,
    CcKindV1::Stun,
    CcKindV1::Bind,
    CcKindV1::ForceMove,
    CcKindV1::Fear,
    CcKindV1::Charm,
];

// What the entity is under right now, read in one pass: (immobilizing effects,
// taunts aside; taunts onto `taunter`). A taunt names who it forces its target
// to attack, which is who applied it: the one immobilize whose source the host
// does give.
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

// How many immobilizing effects the entity is under right now, taunts aside.
fn immobilize_count(entity: &StableEntity<'_, '_>) -> usize {
    (0..entity.cc_count())
        .filter(|&i| {
            entity
                .cc_at(i)
                .is_some_and(|cc| IMMOBILIZING.iter().any(|kind| kind.code() == cc.kind))
        })
        .count()
}

// Whether a skill hit by `champion` may claim an immobilize (`claims_hits`):
// yes unless its kit is known to have none but a taunt.
fn claims_hits(champion: &StableEntity<'_, '_>) -> bool {
    champion
        .name()
        .and_then(|key| crate::champion_traits::traits(&key))
        .and_then(|traits| traits.immobilize)
        .map_or(true, |immobilize| immobilize.other)
}

// Ticks a skill hit and a new immobilize on the same champion may be apart
// and still be one thing: a sixth of a second, either way round.
const WINDOW_TICKS: usize = 10;

#[derive(Clone, Debug, Default)]
pub(crate) struct ImmobilizeWatch {
    // Each enemy champion as of the last tick: (entity, immobilize count,
    // taunts onto the carrier). What a new immobilize, or a new taunt, is
    // new against.
    baseline: Vec<(usize, usize, usize)>,
    // The tick before's `baseline`, kept to swap with rather than reallocate.
    previous: Vec<(usize, usize, usize)>,
    // The carrier's skill hits still waiting for their target to be
    // immobilized: (target, tick of the hit).
    hits: Vec<(usize, usize)>,
    // Enemy champions newly immobilized and still waiting for a skill of the
    // carrier's to hit them: (target, tick it was seen).
    stuns: Vec<(usize, usize)>,
    // Whether a skill hit may claim an immobilize that lands with it: not for
    // a carrier whose kit is known to immobilize with nothing but a taunt, or
    // nothing at all, or every stun an ally lands on a target of its slows
    // would be credited to it. `None` until the carrier's champion is read.
    claims_hits: Option<bool>,
}

impl ImmobilizeWatch {
    // Forgets what it was following, for `on_spawn`. What it knows of the
    // carrier's kit stays.
    pub(crate) fn reset(&mut self) {
        self.baseline.clear();
        self.previous.clear();
        self.hits.clear();
        self.stuns.clear();
    }

    // From `on_skill_hit`, and from `on_attack` for a hit of a skill's:
    // whether this hit of the carrier's goes with an immobilize on `target`,
    // one already seen or one that is there now. A hit with neither is
    // remembered for [`WINDOW_TICKS`], and `update` reports it if its
    // immobilize turns up.
    //
    // Both hooks may tell of the same hit, and a skill may hit the same
    // champion several times: an immobilize is only ever answered to once.
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
        let tick = ctx.tick();

        // An immobilize that turned up on this champion a moment ago, with no
        // hit to its name yet.
        let waiting = self
            .stuns
            .iter()
            .position(|&(id, at)| id == target && tick.saturating_sub(at) <= WINDOW_TICKS);
        if let Some(index) = waiting {
            self.stuns.swap_remove(index);
            return true;
        }
        // One that has landed since the last tick, in step with this hit. The
        // count is taken up at once, so `update` does not find it new again.
        if now > self.baseline_of(target) {
            self.set_baseline(target, now);
            return true;
        }
        // Neither yet: the immobilize may still be on its way.
        match self.hits.iter_mut().find(|(id, _)| *id == target) {
            Some(hit) => hit.1 = tick,
            None => self.hits.push((target, tick)),
        }
        false
    }

    // From `update`, every tick: the enemy champions the carrier has
    // immobilized since the last one. Those whose immobilize has caught up
    // with a skill hit, then new taunts.
    pub(crate) fn update(&mut self, ctx: &StableSim<'_>, player: usize) -> Vec<usize> {
        let tick = ctx.tick();
        let mut immobilized = Vec::new();
        let fresh = |&(_, at): &(usize, usize)| tick.saturating_sub(at) <= WINDOW_TICKS;
        self.hits.retain(fresh);
        self.stuns.retain(fresh);

        let Some(carrier) = ctx.get_player(player).and_then(|p| p.champion()) else {
            return immobilized;
        };
        let (carrier_id, team) = (carrier.id(), carrier.team());
        let claims = *self
            .claims_hits
            .get_or_insert_with(|| claims_hits(&carrier));

        // This tick's counts are what the next is new against. A champion
        // under more immobilizing effects than a tick ago has just been
        // immobilized by someone; one under more of the carrier's taunts, by
        // the carrier.
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
            let (count_before, taunts_before) = self
                .previous
                .iter()
                .find(|&&(known, _, _)| known == id)
                .map_or((0, 0), |&(_, count, taunts)| (count, taunts));
            if taunts > taunts_before {
                immobilized.push(id);
            }
            if claims && count > count_before {
                // The carrier's if one of its skills has just hit this
                // champion; otherwise it waits to see whether one is about to.
                match self.hits.iter().position(|&(target, _)| target == id) {
                    Some(hit) => {
                        self.hits.swap_remove(hit);
                        if !immobilized.contains(&id) {
                            immobilized.push(id);
                        }
                    }
                    None => self.stuns.push((id, tick)),
                }
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

    // Takes `count` as what `target` was already under, so the next tick
    // does not see it as new.
    fn set_baseline(&mut self, target: usize, count: usize) {
        match self.baseline.iter_mut().find(|(id, _, _)| *id == target) {
            Some(known) => known.1 = count,
            None => self.baseline.push((target, count, 0)),
        }
    }
}
