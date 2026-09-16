use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, DISTANCE_UNITS_PER_RANGE, TICKS_PER_SECOND};

#[derive(Clone, Debug)]
pub struct BamisCinder {
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    /// Ticks until the next second of Immolate lands on everyone in range.
    until_next_burn: usize,
}

impl Default for BamisCinder {
    fn default() -> Self {
        Self {
            price: 800,
            hp: 300,
            effect_bonus_flat_damage: 5,
            effect_caster_hp_percent_damage: 0.5,
            effect_max_distance: 30,
            // Non-vital stats (internals)
            until_next_burn: TICKS_PER_SECOND as usize,
        }
    }
}

impl BamisCinder {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_max_distance
            ]
        );
        item
    }

    fn nearby_enemies(&self, ctx: &StableSim<'_>, caster: usize, caster_team: usize) -> Vec<usize> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            // Towers are enemy entities too, and Immolate is not meant for them.
            .filter(|entity| entity.is_alive() && !entity.is_tower() && entity.team() != caster_team)
            .map(|entity| entity.id())
            .filter(|&id| ctx.distance_sq(caster, id) <= range_sq)
            .collect()
    }
}

impl StableItem for BamisCinder {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "bamis_cinder".to_string()
    }

    fn icon(&self) -> String {
        "bamis_cinder".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        // Sunfire Cape, under the key the game keeps it by.
        vec!["hourglass_of_eternity".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.until_next_burn = TICKS_PER_SECOND as usize;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.until_next_burn = self.until_next_burn.saturating_sub(1);
        if self.until_next_burn > 0 {
            return;
        }
        self.until_next_burn = TICKS_PER_SECOND as usize;

        let Some((caster, caster_team, max_hp)) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| (champion.id(), champion.team(), champion.hp().1))
        else {
            return;
        };

        let damage =
            self.effect_bonus_flat_damage + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        for target in self.nearby_enemies(ctx, caster, caster_team) {
            ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::DotDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
