use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, mark_immolate, percent_of, ItemMeta, DISTANCE_UNITS_PER_RANGE, TICKS_PER_SECOND,
};

// Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies
// within 30 range.
//
// Desolate: Killing a unit causes an eruption around their death location,
// dealing 30 + 2% of your maximum health as magic damage to all enemies nearby.
// This effect is 50% as effective against minions and monsters.

/// The eruption, drawn to Desolate's size at the death location. Bound in
/// `view/effects.view_effects`.
const DESOLATE_EFFECT: &str = "riot_hollow_radiance_desolate";

#[derive(Clone, Debug)]
pub struct HollowRadiance {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_resistance: i32,
    skill_damaged_reduce: usize,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
    effect_explosion_flat_damage: usize,
    effect_explosion_caster_hp_percent: f64,
    effect_explosion_distance: usize,
    effect_minion_percent: f64,
    // Non-vital stats (internals)
    until_next_burn: usize,
    /// Death locations still to erupt. Kills are only noted in `on_kill`; the
    /// eruption lands on the next `update`, so damage dealt from inside the
    /// kill hook never re-enters it, and an eruption that kills sets off the
    /// next one a tick later.
    eruptions: Vec<(u64, u64)>,
}

impl HollowRadiance {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "hollow_radiance",
                &["bamis_cinder", "dusk_raven"],
                &["radiant_hollow_radiance"],
            ),
            price: 750,
            hp: 200,
            magic_resistance: 50,
            skill_damaged_reduce: 4,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_explosion_flat_damage: 30,
            effect_explosion_caster_hp_percent: 2.0,
            effect_explosion_distance: 35,
            effect_minion_percent: 50.0,
            // Non-vital stats (internals)
            until_next_burn: TICKS_PER_SECOND as usize,
            eruptions: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hollow_radiance", &["hollow_radiance"]),
            price: 1050,
            hp: 350,
            magic_resistance: 70,
            skill_damaged_reduce: 6,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_explosion_flat_damage: 60,
            effect_explosion_caster_hp_percent: 4.0,
            effect_explosion_distance: 35,
            effect_minion_percent: 50.0,
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
                magic_resistance,
                skill_damaged_reduce,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_max_distance,
                effect_explosion_flat_damage,
                effect_explosion_caster_hp_percent,
                effect_explosion_distance,
                effect_minion_percent
            ]
        );
        self
    }

    /// Enemy units (not turrets) within `range` of a point, with whether each
    /// is a champion.
    fn enemies_near(
        ctx: &StableSim<'_>,
        team: usize,
        (x, y): (u64, u64),
        range: usize,
    ) -> Vec<(usize, bool)> {
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
            .map(|e| (e.id(), e.is_champion()))
            .collect()
    }
}

impl Default for HollowRadiance {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for HollowRadiance {
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
            magic_resistance: self.magic_resistance,
            skill_damaged_reduce: self.skill_damaged_reduce,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.until_next_burn = TICKS_PER_SECOND as usize;
        self.eruptions.clear();
        // Flames up from the first frame rather than the first burn.
        if let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        {
            mark_immolate(ctx, champion);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let Some((caster, team, max_hp, alive)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.team(), c.hp().1, c.is_alive()))
        else {
            return;
        };

        // Desolate
        for at in std::mem::take(&mut self.eruptions) {
            let damage = self.effect_explosion_flat_damage
                + percent_of(max_hp, self.effect_explosion_caster_hp_percent);
            let targets = Self::enemies_near(ctx, team, at, self.effect_explosion_distance);
            ctx.play_view_effect(
                DESOLATE_EFFECT,
                caster,
                &InputTargetV1::pos(at.0, at.1),
                0,
                0,
                0,
            );
            for (target, is_champion) in targets {
                let amount = if is_champion {
                    damage
                } else {
                    percent_of(damage, self.effect_minion_percent)
                };
                if amount > 0 {
                    ctx.deal_damage(caster, target, 0, amount, AttackTypeV1::Item);
                }
            }
        }

        // Immolate, once a second while alive, the same burn as Bami's Cinder.
        self.until_next_burn = self.until_next_burn.saturating_sub(1);
        if self.until_next_burn > 0 {
            return;
        }
        self.until_next_burn = TICKS_PER_SECOND as usize;
        if !alive {
            return;
        }
        mark_immolate(ctx, caster);
        let damage = self.effect_bonus_flat_damage
            + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;
        let targets: Vec<usize> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_alive() && !e.is_tower() && e.team() != team)
            .map(|e| e.id())
            .filter(|&id| ctx.distance_sq(caster, id) <= range_sq)
            .collect();
        for target in targets {
            ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        }
    }

    // Desolate: any unit but a turret. The victim is already dead here, so its
    // position is taken now and the eruption itself waits for `update`.
    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        _entity: usize,
        victim: usize,
    ) {
        let Some((is_tower, at)) = ctx.get_entity(victim).map(|v| (v.is_tower(), v.pos())) else {
            return;
        };
        if is_tower || at == (0, 0) {
            return;
        }
        self.eruptions.push(at);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::MagicResistance,
            ItemTagV1::DotDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
