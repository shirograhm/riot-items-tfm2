use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta, DISTANCE_UNITS_PER_RANGE};

/// Zaz'Zak's Realmspike — what World Atlas grows into for a support that
/// fights with its abilities: Void Explosion, and the gold the Atlas line pays
/// ([`crate::SharedRiches`]).
///
/// # Void Explosion
///
/// Ability damage to an enemy champion marks the spot that champion stands on.
/// After a short delay the spot goes off, on whoever is standing there by
/// then: the champion may have walked out of it, and another may have walked
/// in. Each enemy caught takes a flat amount, a share of the carrier's ability
/// power and a share of its own maximum health, as magic damage.
///
/// The spot is a place and a timer, like Iceborn Gauntlet's frost zone: no
/// unit stands for it. Its damage goes out as an `Item` hit, which the trigger
/// does not answer to, so one explosion never sets off the next.
#[derive(Clone, Debug)]
pub struct ZazzaksRealmspike {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_delay_seconds: f64,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_enemy_max_hp_damage: usize,
    effect_cooldown_seconds: f64,
    effect_max_distance: usize,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    /// Ticks until Void Explosion can be set off again.
    cooldown: usize,
    /// Every spot this carrier has marked that has yet to go off.
    blasts: Vec<Blast>,
}

/// The spot gathering, played once as it is marked, and the blast, played as
/// it goes off: two tags of one sheet (`effects/void_explosion`), bound in
/// `view/effects.view_effects`. The gathering is drawn half a second long, so
/// a longer delay in the config leaves the spot bare for the rest of it.
const GATHER_EFFECT: &str = "riot_void_explosion_gather";
const BLAST_EFFECT: &str = "riot_void_explosion_blast";

/// A marked spot waiting to go off.
#[derive(Clone, Copy, Debug)]
struct Blast {
    x: u64,
    y: u64,
    /// Ticks until it does.
    remaining: usize,
}

impl ZazzaksRealmspike {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "zazzaks_realmspike",
                &["runic_compass"],
                &["radiant_zazzaks_realmspike"],
            ),
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_delay_seconds: 0.5,
            effect_bonus_flat_damage: 10,
            effect_ap_percent_damage: 15.0,
            effect_enemy_max_hp_damage: 3,
            effect_cooldown_seconds: 10.0,
            effect_max_distance: 40,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            cooldown: 0,
            blasts: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zazzaks_realmspike", &["zazzaks_realmspike"]),
            price: 750,
            hp: 300,
            hp_regen: 5,
            // Void Explosion itself is unchanged — Radiant buys the stat line only.
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
                effect_delay_seconds,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_enemy_max_hp_damage,
                effect_cooldown_seconds,
                effect_max_distance,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    /// Enemy units (not turrets) within `range` of a spot, each with its
    /// maximum health.
    fn enemies_near(
        ctx: &StableSim<'_>,
        team: usize,
        (x, y): (u64, u64),
        range: usize,
    ) -> Vec<(usize, usize)> {
        let range = (range * DISTANCE_UNITS_PER_RANGE) as i128;
        let range_sq = range * range;
        (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_alive() && !e.is_tower() && e.team() != team)
            .filter(|e| {
                let (ex, ey) = e.pos();
                let dx = ex as i128 - x as i128;
                let dy = ey as i128 - y as i128;
                dx * dx + dy * dy <= range_sq
            })
            .map(|e| (e.id(), e.hp().1))
            .collect()
    }

    /// Counts every marked spot down and sets off the ones whose delay has
    /// run out.
    fn run_blasts(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.blasts.is_empty() {
            return;
        }
        let mut due = Vec::new();
        self.blasts.retain_mut(|blast| {
            blast.remaining = blast.remaining.saturating_sub(1);
            if blast.remaining > 0 {
                return true;
            }
            due.push((blast.x, blast.y));
            false
        });
        if due.is_empty() {
            return;
        }

        // A marked spot outlives its carrier: it goes off all the same, with
        // the ability power the carrier has by then.
        let Some((carrier, team, ability_power)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.team(), c.stat().magic_power))
        else {
            return;
        };
        let damage = self.effect_bonus_flat_damage
            + percent_of(ability_power, self.effect_ap_percent_damage);

        for at in due {
            ctx.play_view_effect(
                BLAST_EFFECT,
                carrier,
                &InputTargetV1::pos(at.0, at.1),
                0,
                0,
                0,
            );
            for (target, max_hp) in Self::enemies_near(ctx, team, at, self.effect_max_distance) {
                let amount = damage + percent_of(max_hp, self.effect_enemy_max_hp_damage as f64);
                if amount > 0 {
                    ctx.deal_damage(carrier, target, 0, amount, AttackTypeV1::Item);
                }
            }
        }
    }

    /// What Shared Riches pays a holder: this much gold, this often.
    /// [`crate::SharedRiches`] does the paying, from the match hook.
    pub(crate) fn shared_riches(&self) -> (usize, f64) {
        (self.effect_bonus_gold, self.effect_gold_interval_seconds)
    }
}

impl Default for ZazzaksRealmspike {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ZazzaksRealmspike {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
    }

    /// Void Explosion's trigger: ability damage to an enemy champion, off
    /// cooldown. The cooldown runs from here, not from the blast.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::Skill || self.cooldown > 0 {
            return;
        }
        let Some((x, y)) = ctx
            .get_entity(target)
            .filter(|target| target.is_champion())
            .map(|target| target.pos())
        else {
            return;
        };
        // What the host answers for a unit it cannot place.
        if (x, y) == (0, 0) {
            return;
        }
        self.cooldown = ticks(self.effect_cooldown_seconds);
        self.blasts.push(Blast {
            x,
            y,
            remaining: ticks(self.effect_delay_seconds),
        });
        ctx.play_view_effect(GATHER_EFFECT, caster, &InputTargetV1::pos(x, y), 0, 0, 0);
    }

    /// Runs the cooldown and the marked spots.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
        self.run_blasts(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
