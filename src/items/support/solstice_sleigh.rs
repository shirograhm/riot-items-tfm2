use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, sized_range, ticks, ImmobilizeWatch, ItemMeta};

/// Solstice Sleigh — what World Atlas grows into for a support that locks
/// enemies down: Going Sledding, and the gold the Atlas line pays
/// ([`crate::SharedRiches`]).
///
/// # Going Sledding
///
/// When the carrier immobilizes an enemy champion ([`ImmobilizeWatch`], the
/// trigger Imperial Mandate's Command uses), the carrier and the most wounded
/// allied champion in range speed up and gain bonus health for a moment.
///
/// The bonus health is maximum health on the buff, and the same amount is
/// restored as it lands: a buff's `hp` raises the ceiling and leaves the
/// health under it where it was, so without that the bonus would be health
/// nobody has. When the buff runs out the ceiling comes back down.
///
/// # Both variants share the buff name
///
/// Re-applying is a remove followed by an add, and one `entity_remove_buff`
/// clears every copy, so two Sleighs sledding the same ally leave one buff on
/// them, not two.
#[derive(Clone, Debug)]
pub struct SolsticeSleigh {
    meta: ItemMeta,
    sledding_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_move_speed_mult: i32,
    effect_min_bonus_hp: usize,
    effect_max_bonus_hp: usize,
    effect_duration_seconds: f64,
    effect_max_distance: usize,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    watch: ImmobilizeWatch,
}

impl SolsticeSleigh {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "solstice_sleigh",
                &["runic_compass"],
                &["radiant_solstice_sleigh"],
            ),
            sledding_buff: "solstice_sleigh_sledding",
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_move_speed_mult: 20,
            effect_min_bonus_hp: 50,
            effect_max_bonus_hp: 215,
            effect_duration_seconds: 2.0,
            effect_max_distance: 100,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            watch: ImmobilizeWatch::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_solstice_sleigh", &["solstice_sleigh"]),
            price: 750,
            hp: 300,
            hp_regen: 5,
            // Going Sledding itself is unchanged — Radiant buys the stat line only.
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
                effect_move_speed_mult,
                effect_min_bonus_hp,
                effect_max_bonus_hp,
                effect_duration_seconds,
                effect_max_distance,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    /// Level 1 gets `effect_min_bonus_hp` and level 12 `effect_max_bonus_hp`,
    /// the same eleven-step ramp `bloodsong` uses for Spellblade.
    fn bonus_hp(&self, level: usize) -> usize {
        let per_level = (self
            .effect_max_bonus_hp
            .saturating_sub(self.effect_min_bonus_hp) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_hp + level.saturating_sub(1) * per_level
    }

    /// The living allied champion in range of the carrier with the least of
    /// its health left, the nearer of two as wounded. Not the carrier.
    fn most_wounded_ally(&self, ctx: &StableSim<'_>, caster: usize, team: usize) -> Option<usize> {
        let range = sized_range(ctx, caster, self.effect_max_distance);
        let range_sq = range * range;

        let mut best: Option<(usize, f64, u64)> = None;
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            if id == caster {
                continue;
            }
            let Some(ally_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !ally_ref.is_alive() || !ally_ref.is_champion() || ally_ref.team() != team {
                continue;
            }
            let distance = ctx.distance_sq(caster, id);
            if distance > range_sq {
                continue;
            }
            let (current, max) = ally_ref.hp();
            if max == 0 {
                continue;
            }
            let wounded = current as f64 / max as f64;
            let better = best.is_none_or(|(_, best_wounded, best_distance)| {
                wounded < best_wounded || (wounded == best_wounded && distance < best_distance)
            });
            if better {
                best = Some((id, wounded, distance));
            }
        }
        best.map(|(id, _, _)| id)
    }

    /// Going Sledding, on the carrier and its most wounded ally in range.
    fn sled(&self, ctx: &mut StableSim<'_>, caster: usize) {
        let Some((level, team)) = ctx
            .get_entity(caster)
            .filter(|caster_ref| caster_ref.is_alive())
            .map(|caster_ref| (caster_ref.level(), caster_ref.team()))
        else {
            return;
        };
        let bonus_hp = self.bonus_hp(level);
        let ally = self.most_wounded_ally(ctx, caster, team);

        for rider in std::iter::once(caster).chain(ally) {
            refresh_buff(
                ctx,
                rider,
                self.sledding_buff,
                &BuffV1 {
                    move_speed_mult: self.effect_move_speed_mult,
                    hp: bonus_hp as i32,
                    ..BuffV1::timed(self.sledding_buff, ticks(self.effect_duration_seconds))
                },
            );
            ctx.heal(caster, rider, bonus_hp);
        }
    }

    /// What Shared Riches pays a holder: this much gold, this often.
    /// [`crate::SharedRiches`] does the paying, from the match hook.
    pub(crate) fn shared_riches(&self) -> (usize, f64) {
        (self.effect_bonus_gold, self.effect_gold_interval_seconds)
    }
}

impl Default for SolsticeSleigh {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SolsticeSleigh {
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
        self.watch.reset();
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if self.watch.skill_hit(ctx, target, is_ally) {
            self.sled(ctx, caster);
        }
    }

    /// A skill's hit again, as `on_attack` tells of it: it is not known that
    /// `on_skill_hit` hears of every one (see [`ImmobilizeWatch`]).
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
        if matches!(attack_type, AttackTypeV1::Skill) && self.watch.skill_hit(ctx, target, false) {
            self.sled(ctx, caster);
        }
    }

    /// Sleds once however many enemies were immobilized this tick.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if !self.watch.update(ctx, player).is_empty() {
            let carrier = ctx
                .get_player(player)
                .and_then(|player_ref| player_ref.champion())
                .map(|champion_ref| champion_ref.id());
            if let Some(carrier) = carrier {
                self.sled(ctx, carrier);
            }
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
