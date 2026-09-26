use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta};

/// Crowd control that takes movement out of the target's hands: stun, root,
/// knock-up, knockback/pull and taunt. League's "immobilize" set, plus taunt;
/// fear and charm still don't count, and disarm, silence and ground leave the
/// target free to walk.
const IMMOBILIZING: [CcKindV1; 7] = [
    CcKindV1::Airborne,
    CcKindV1::Stun,
    CcKindV1::Bind,
    CcKindV1::ForceMove,
    CcKindV1::Taunt,
    CcKindV1::Fear,
    CcKindV1::Charm,
];

/// How many immobilizing effects the entity is under right now.
fn immobilize_count(entity: &StableEntity<'_, '_>) -> usize {
    (0..entity.cc_count())
        .filter(|&i| {
            entity
                .cc_at(i)
                .is_some_and(|cc| IMMOBILIZING.iter().any(|kind| kind.code() == cc.kind))
        })
        .count()
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

#[derive(Clone, Debug)]
pub struct ImperialMandate {
    meta: ItemMeta,
    /// Shared by both variants: Vulnerable is a state on the target, and the
    /// two variants grant the same amount, so a second carrier refreshes it
    /// rather than doubling it. The name is also the `view_buffs` binding in
    /// `view/effects.view_effects` that draws the mini flag over the target;
    /// rename both together or the flag stops showing.
    vulnerable_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    // Non-vital stats (internals)
    /// Each enemy champion's immobilize count as of the last tick: the "before"
    /// a skill hit is compared against.
    baseline: Vec<(usize, usize)>,
    pending: Vec<PendingHit>,
}

impl ImperialMandate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "imperial_mandate",
                &["bandleglass_mirror"],
                &["radiant_imperial_mandate"],
            ),
            vulnerable_buff: "imperial_mandate_vulnerable",
            price: 550,
            hp: 100,
            hp_regen: 1,
            magic_power: 25,
            skill_cooldown_mult: 15,
            effect_damaged_amplify: 9,
            effect_duration_seconds: 3.0,
            // Non-vital stats (internals)
            baseline: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_imperial_mandate", &["imperial_mandate"]),
            price: 750,
            hp: 150,
            hp_regen: 2,
            magic_power: 40,
            skill_cooldown_mult: 20,
            effect_damaged_amplify: 9,
            effect_duration_seconds: 3.0,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_damaged_amplify,
                effect_duration_seconds
            ]
        );
        self
    }

    fn mark(&self, ctx: &mut StableSim<'_>, target: usize) {
        refresh_buff(
            ctx,
            target,
            self.vulnerable_buff,
            &BuffV1 {
                damaged_amplify: self.effect_damaged_amplify,
                ..BuffV1::timed(self.vulnerable_buff, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn baseline_of(&self, target: usize) -> usize {
        self.baseline
            .iter()
            .find(|&&(id, _)| id == target)
            .map_or(0, |&(_, count)| count)
    }
}

impl Default for ImperialMandate {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ImperialMandate {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.baseline.clear();
        self.pending.clear();
    }

    // Command: immobilizing an enemy champion. The host does not say who
    // applied a crowd control, so "you immobilized them" is read as one of your
    // skills hitting a champion who becomes newly immobilized at that hit or
    // within `IMMOBILIZE_WINDOW_TICKS` after it (a skill's stun may land either
    // side of its hit report). "Newly" means more immobilizing effects than the
    // last tick's baseline, so chaining a second stun onto a stunned target
    // refreshes the mark too. An ally's stun landing in that same window reads
    // the same and will be credited to you.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if is_ally {
            return;
        }
        let Some(now) = ctx
            .get_entity(target)
            .filter(|t| t.is_champion() && t.is_alive())
            .map(|t| immobilize_count(&t))
        else {
            return;
        };
        let before = self.baseline_of(target);
        if now > before {
            self.mark(ctx, target);
            return;
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
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let tick = ctx.tick();

        // Hits still waiting on their stun, judged against the count from
        // before they landed.
        if !self.pending.is_empty() {
            let mut marked = Vec::new();
            self.pending.retain(|p| {
                let now = ctx
                    .get_entity(p.target)
                    .filter(|t| t.is_alive())
                    .map_or(0, |t| immobilize_count(&t));
                if now > p.before {
                    marked.push(p.target);
                    return false;
                }
                tick < p.expires_at
            });
            for target in marked {
                self.mark(ctx, target);
            }
        }

        // This tick's counts become the next hit's "before".
        let Some(team) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.team())
        else {
            return;
        };
        self.baseline.clear();
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(count) = ctx
                .get_entity(id)
                .filter(|e| e.team() != team && e.is_alive())
                .map(|e| immobilize_count(&e))
            else {
                continue;
            };
            self.baseline.push((id, count));
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
