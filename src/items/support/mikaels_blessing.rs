use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, sized_range, ticks, Elapsed, ItemMeta};

/// Ticks between two looks for a champion to purify while the cooldown is up: a
/// tenth of a second. Nobody sees the wait, and the look costs a few host
/// calls for every champion in the match.
const PURIFY_POLL_TICKS: usize = 6;
/// The cleanse, played on every champion it reaches, the one purified and,
/// when that is an ally, the carrier too (`effects/mikaels_purify`): a round
/// of pale light that opens about the body and lets go as glints rise off it.
/// Bound in `view/effects.view_effects`.
const PURIFY_EFFECT: &str = "riot_mikaels_purify";

/// Mikael's Blessing — Purify, a cleanse and a heal for a champion of the
/// carrier's team caught at low health, the carrier included.
///
/// # Why it is looked for
///
/// An item's hooks report its carrier's own events, so an ally being crowd
/// controlled is something to go and find: while the cooldown is up, the
/// team's champions are gone over for one that is below the health threshold,
/// under a crowd control and within range, and the one worst off is purified.
/// The range is measured last and once (`sized_range` reads the carrier's
/// buffs), only when an ally has passed the other two tests.
///
/// # The carrier itself
///
/// One of the champions gone over, on the same two tests and with no range
/// to pass: Purify can be cast on its own carrier (the user, 2026-10-09). It
/// has no say over an ally worse off, as the worst off is still the one
/// purified. An ally purified takes the carrier's crowd control with it, as
/// the tooltip says; the carrier purified is the only one cleansed.
///
/// # What counts as crowd control
///
/// Everything the host lists as one on the champion, bar an animation lock,
/// which is the champion's own doing. A slow is not on that list: the host
/// keeps it as a stat buff, so Purify neither answers to one nor removes it.
/// The cleanse itself is the host's (`entity_clear_cc`), which takes all of
/// it off.
///
/// # Whose level
///
/// The purified champion's, as the tooltip says ("based on the target's
/// level") and as League has it: the user's call (2026-10-09). Every other
/// "(based on level)" amount in the mod goes by the carrier's.
///
/// # The cooldown
///
/// An item-side tick counter that a respawn does not clear, unlike most in
/// the mod: two minutes is two minutes, death or no death (the user,
/// 2026-10-09). It also keeps running while the carrier is dead, the way
/// `fiendhunter_bolts` keeps its own.
///
/// The passive is the same on base and Radiant; Radiant buys the stat line.
#[derive(Clone, Debug)]
pub struct MikaelsBlessing {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_hp_percent_threshold: f64,
    effect_min_heal: usize,
    effect_max_heal: usize,
    effect_max_distance: usize,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    purify_cooldown: usize,
    /// Steps the cooldown by the time gone by, so it runs through a death.
    clock: Elapsed,
}

impl MikaelsBlessing {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "mikaels_blessing",
                &["forbidden_idol"],
                &["radiant_mikaels_blessing"],
            ),
            price: 550,
            hp: 200,
            hp_regen: 2,
            magic_power: 10,
            skill_cooldown_mult: 10,
            effect_hp_percent_threshold: 50.0,
            effect_min_heal: 100,
            effect_max_heal: 265,
            effect_max_distance: 65,
            effect_cooldown_seconds: 120.0,
            // Non-vital stats (internals)
            purify_cooldown: 0,
            clock: Elapsed::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_mikaels_blessing", &["mikaels_blessing"]),
            price: 750,
            hp: 300,
            hp_regen: 3,
            magic_power: 20,
            skill_cooldown_mult: 15,
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
                effect_hp_percent_threshold,
                effect_min_heal,
                effect_max_heal,
                effect_max_distance,
                effect_cooldown_seconds
            ]
        );
        self
    }

    /// Level 1 heals `effect_min_heal` and level 12 `effect_max_heal`, the
    /// eleven-step ramp `locket_of_the_iron_solari` uses for its shield.
    fn heal_amount(&self, level: usize) -> usize {
        let span = self.effect_max_heal.saturating_sub(self.effect_min_heal);
        let per_level = (span as f64 / 11.0).round() as usize;
        self.effect_min_heal + level.saturating_sub(1) * per_level
    }

    /// Whether the entity is under a crowd control Purify answers to.
    fn crowd_controlled(entity: &StableEntity<'_, '_>) -> bool {
        (0..entity.cc_count())
            .filter_map(|index| entity.cc_at(index))
            .any(|cc| cc.kind != CcKindV1::Animation.code())
    }

    fn purify(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.purify_cooldown > 0 || ctx.tick() % PURIFY_POLL_TICKS != 0 {
            return;
        }
        let Some((caster, caster_team)) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|caster_ref| (caster_ref.id(), caster_ref.team()))
        else {
            return;
        };

        let mut reach_sq: Option<u64> = None;
        // The champion worst off, by the share of its health it has left:
        // (entity, health, maximum health, level).
        let mut worst: Option<(usize, usize, usize, usize)> = None;
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(ally_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !ally_ref.is_alive() || ally_ref.team() != caster_team {
                continue;
            }
            let (hp, max_hp) = ally_ref.hp();
            if hp >= percent_of(max_hp, self.effect_hp_percent_threshold)
                || !Self::crowd_controlled(&ally_ref)
            {
                continue;
            }
            if id != caster {
                let within_sq = *reach_sq.get_or_insert_with(|| {
                    let range = sized_range(ctx, caster, self.effect_max_distance);
                    range * range
                });
                if ctx.distance_sq(caster, id) > within_sq {
                    continue;
                }
            }
            if worst.is_none_or(|(_, low_hp, low_max, _)| hp * low_max < low_hp * max_hp) {
                worst = Some((id, hp, max_hp, ally_ref.level()));
            }
        }
        let Some((purified, _, _, level)) = worst else {
            return;
        };

        // The carrier is cleansed along with an ally, and both show it; the
        // heal is the purified champion's alone.
        let also = (purified != caster).then_some(caster);
        for cleansed in [Some(purified), also].into_iter().flatten() {
            ctx.entity_clear_cc(cleansed);
            ctx.play_view_effect(
                PURIFY_EFFECT,
                cleansed,
                &InputTargetV1::target(cleansed),
                0,
                0,
                0,
            );
        }
        ctx.heal(caster, purified, self.heal_amount(level));
        self.purify_cooldown = ticks(self.effect_cooldown_seconds);
    }
}

impl Default for MikaelsBlessing {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for MikaelsBlessing {
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

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // By the time gone by, not by one: no `update` runs for a dead carrier,
        // and the cooldown is to keep running through the death.
        let gone = self.clock.since_last(ctx);
        self.purify_cooldown = self.purify_cooldown.saturating_sub(gone);
        self.purify(ctx, player);
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
