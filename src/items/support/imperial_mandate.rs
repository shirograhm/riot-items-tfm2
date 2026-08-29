use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta, DISTANCE_UNITS_PER_RANGE};

/// One marked enemy this carrier is watching.
///
/// The stable API hands an item only its *owner's* events — there is no "an
/// ally dealt damage" hook — so the detonation is inferred: `hp` is what the
/// target had at the previous poll, and a drop since then is the damage that
/// spends the mark. `cooldown` keeps the entry alive after a detonation so the
/// same target cannot be marked again until it runs out.
#[derive(Clone, Copy, Debug)]
struct Marked {
    target: usize,
    hp: usize,
    cooldown: usize,
}

#[derive(Clone, Debug)]
pub struct ImperialMandate {
    meta: ItemMeta,
    /// Shared by both variants, the way `bloodsong` shares its own: the mark is
    /// a state on the target rather than a per-carrier stack, and the two
    /// variants detonate for the same amount.
    mark_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_hp_percent_damage: f64,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    effect_max_distance: usize,
    marked: Vec<Marked>,
}

impl ImperialMandate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "imperial_mandate",
                &["bandleglass_mirror"],
                &["radiant_imperial_mandate"],
            ),
            mark_buff: "imperial_mandate_mark",
            price: 1100,
            hp: 250,
            hp_regen: 2,
            magic_power: 50,
            skill_cooldown_mult: 5,
            effect_hp_percent_damage: 10.0,
            effect_duration_seconds: 5.0,
            effect_cooldown_seconds: 9.0,
            effect_max_distance: 100,
            // Non-vital stats (internals)
            marked: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_imperial_mandate", &["imperial_mandate"]),
            price: 1650,
            hp: 350,
            hp_regen: 3,
            magic_power: 100,
            skill_cooldown_mult: 10,
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
                effect_hp_percent_damage,
                effect_duration_seconds,
                effect_cooldown_seconds,
                effect_max_distance
            ]
        );
        self
    }

    /// Whether an allied champion other than the carrier is close enough to the
    /// target to be what damaged it. Proximity is all there is to go on: the
    /// host reports who dealt damage only for the carrier's own hits, so a
    /// marked enemy chipped by a minion beside a teammate reads the same as a
    /// teammate's ability and will spend the mark, while a teammate hitting
    /// from further out than `effect_max_distance` will not.
    fn ally_in_range(
        &self,
        ctx: &StableSim<'_>,
        caster_id: usize,
        team: usize,
        target: usize,
        range_sq: u64,
    ) -> bool {
        (0..ctx.champion_count()).any(|index| {
            let id = ctx.champion_id_at(index);
            if id == caster_id || id == target {
                return false;
            }
            let Some(entity_ref) = ctx.get_entity(id) else {
                return false;
            };
            entity_ref.is_alive()
                && entity_ref.team() == team
                && ctx.distance_sq(id, target) <= range_sq
        })
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
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        // Entity ids from the previous life mean nothing now, and a mark cannot
        // outlive the carrier's death anyway.
        self.marked.clear();
    }

    // Coordinated Fire, first half: the mark. Re-marking is a remove followed by
    // an add rather than a `has_buff` gate: refreshing means replacing the
    // instance, and one `entity_remove_buff` clears every copy, so a multi-hit
    // cast cannot leave two marks on the same target. It also sidesteps the ~3
    // tick delay before a fresh buff becomes visible to `has_buff`, which would
    // otherwise let a fast second hit stack a duplicate.
    //
    // The per-target cooldown is tracked here rather than as a second buff on
    // the target for the same reason: `update` decrements it every tick, so a
    // skill landing immediately after a detonation sees it.
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
        // Read before mutating: `StableEntity` holds an immutable borrow of the
        // sim that the calls below need back.
        let Some((is_champion, hp)) = ctx
            .get_entity(target)
            .map(|target_ref| (target_ref.is_champion(), target_ref.hp().0))
        else {
            return;
        };
        if !is_champion {
            return;
        }

        match self.marked.iter_mut().find(|entry| entry.target == target) {
            // Still cooling down from its last detonation: marking it again
            // would promise damage the cooldown is about to refuse.
            Some(entry) if entry.cooldown > 0 => return,
            Some(entry) => entry.hp = hp,
            None => self.marked.push(Marked {
                target,
                hp,
                cooldown: 0,
            }),
        }

        ctx.entity_remove_buff(target, self.mark_buff);
        ctx.add_buff(
            target,
            &BuffV1::timed(self.mark_buff, ticks(self.effect_duration_seconds)),
        );
    }

    // Coordinated Fire, second half: the detonation. Polling is what stands in
    // for an ally-damage event — see `Marked`.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.marked.is_empty() {
            return;
        }

        let Some((caster_id, caster_team)) = ctx.get_player(player).and_then(|player_ref| {
            let champion_ref = player_ref.champion()?;
            Some((champion_ref.id(), champion_ref.team()))
        }) else {
            return;
        };

        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut kept = Vec::with_capacity(self.marked.len());
        for mut entry in std::mem::take(&mut self.marked) {
            entry.cooldown = entry.cooldown.saturating_sub(1);

            let Some((is_alive, is_marked, hp)) = ctx.get_entity(entry.target).map(|target_ref| {
                (
                    target_ref.is_alive(),
                    has_buff(&target_ref, self.mark_buff),
                    target_ref.hp().0,
                )
            }) else {
                continue;
            };
            if !is_alive {
                continue;
            }

            // The mark expired, or a second carrier's Mandate already spent it.
            // The entry stays only for as long as its cooldown still refuses a
            // re-mark.
            if !is_marked {
                if entry.cooldown > 0 {
                    entry.hp = hp;
                    kept.push(entry);
                }
                continue;
            }

            let took_damage = hp < entry.hp;
            entry.hp = hp;
            if !took_damage
                || !self.ally_in_range(ctx, caster_id, caster_team, entry.target, range_sq)
            {
                kept.push(entry);
                continue;
            }

            let damage = percent_of(hp, self.effect_hp_percent_damage);
            ctx.entity_remove_buff(entry.target, self.mark_buff);
            ctx.deal_damage(caster_id, entry.target, 0, damage, AttackTypeV1::Item);
            entry.cooldown = ticks(self.effect_cooldown_seconds);
            kept.push(entry);
        }
        self.marked = kept;
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
