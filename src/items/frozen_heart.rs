use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, ItemMeta, AURA_DURATION_TICKS, AURA_REFRESH_TICKS, DISTANCE_UNITS_PER_RANGE,
};

#[derive(Clone, Debug)]
pub struct FrozenHeart {
    meta: ItemMeta,
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
                &["glacial_buckler"],
                &["radiant_frozen_heart"],
            ),
            aura_buff: "frozen_heart_aura",
            price: 1300,
            defence: 65,
            skill_cooldown_mult: 10,
            skill_damaged_reduce: 10,
            effect_attack_speed_reduce: 30,
            effect_max_distance: 100,
            // Non-vital stats (internals)
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_frozen_heart", &["frozen_heart"]),
            aura_buff: "frozen_heart_aura",
            price: 1900,
            defence: 110,
            skill_cooldown_mult: 15,
            skill_damaged_reduce: 15,
            effect_attack_speed_reduce: 30,
            effect_max_distance: 100,
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
            targets.push(id);
        }

        for id in targets {
            // Both halves land in the same tick, so the target is never
            // observed without the buff.
            ctx.entity_remove_buff(id, self.aura_buff);
            ctx.add_buff(
                id,
                &BuffV1 {
                    attack_speed_mult: -self.effect_attack_speed_reduce,
                    ..BuffV1::timed(self.aura_buff, AURA_DURATION_TICKS)
                },
            );
        }

        self.refresh_cooldown = AURA_REFRESH_TICKS;
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
