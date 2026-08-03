use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, has_buff, ItemMeta, BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS,
    DISTANCE_UNITS_PER_RANGE,
};

#[derive(Clone, Debug)]
pub struct FrozenHeart {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    aura_buff: &'static str,
    price: usize,
    defence: i32,
    skill_cooldown_mult: i32,
    skill_damaged_reduce: usize,
    effect_attack_speed_reduce: i32,
    effect_max_distance: usize,
    refresh_cooldown: usize,
}

impl FrozenHeart {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "frozen_heart",
                &["gatekeepers_armor"],
                &["radiant_frozen_heart"],
            ),
            aura_buff: "frozen_heart_aura",
            price: 1300,
            defence: 65,
            skill_cooldown_mult: 10,
            skill_damaged_reduce: 10,
            effect_attack_speed_reduce: 30,
            effect_max_distance: 100,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_frozen_heart", &["frozen_heart"]),
            aura_buff: "radiant_frozen_heart_aura",
            price: 1900,
            defence: 110,
            skill_cooldown_mult: 15,
            skill_damaged_reduce: 15,
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
                defence,
                skill_cooldown_mult,
                skill_damaged_reduce,
                effect_attack_speed_reduce,
                effect_max_distance
            ]
        );
        self
    }

    /// Winter's Caress. Same shape as the friendly auras (`zekes_herald`,
    /// `locket_of_the_iron_solari`) but pointed at the other team: a short
    /// `Time` buff re-applied on a slightly shorter cycle, so an enemy that
    /// walks out of range recovers its attack speed within a second.
    fn apply_aura(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(caster) = player_ref.champion() else {
            return;
        };
        let caster_id = caster.id();
        let caster_team = caster.team();

        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut targets: Vec<usize> = Vec::new();
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() == caster_team {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            if !has_buff(&entity_ref, self.aura_buff) {
                targets.push(id);
            }
        }

        for id in targets {
            ctx.add_buff(
                id,
                &BuffV1 {
                    attack_speed_mult: -self.effect_attack_speed_reduce,
                    ..BuffV1::timed(self.aura_buff, BUFF_REFRESH_DURATION_TICKS)
                },
            );
        }

        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for FrozenHeart {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for FrozenHeart {
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
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            skill_damaged_reduce: self.skill_damaged_reduce,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_aura(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_aura(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Defense,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::AsDebuff,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}
