use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, percent_of_i32, ItemMeta, AURA_DURATION_TICKS, AURA_REFRESH_TICKS,
    DISTANCE_UNITS_PER_RANGE,
};

#[derive(Clone, Debug)]
pub struct LocketOfTheIronSolari {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    aura_buff: &'static str,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    skill_cooldown_mult: i32,
    effect_bonus_defence: i32,
    effect_bonus_magic_resistance: i32,
    effect_bonus_hp_regen: i32,
    effect_minion_percent: f64,
    effect_max_distance: usize,
    refresh_cooldown: usize,
}

impl LocketOfTheIronSolari {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "locket_of_the_iron_solari",
                &["aegis_of_the_legion"],
                &["radiant_locket_of_the_iron_solari"],
            ),
            aura_buff: "locket_of_the_iron_solari_aura",
            price: 1100,
            hp: 200,
            defence: 40,
            magic_resistance: 60,
            skill_cooldown_mult: 10,
            effect_bonus_defence: 6,
            effect_bonus_magic_resistance: 12,
            effect_bonus_hp_regen: 3,
            effect_minion_percent: 150.0,
            effect_max_distance: 100,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_locket_of_the_iron_solari",
                &["locket_of_the_iron_solari"],
            ),
            aura_buff: "radiant_locket_of_the_iron_solari_aura",
            price: 1650,
            hp: 300,
            defence: 75,
            magic_resistance: 100,
            skill_cooldown_mult: 15,
            effect_bonus_defence: 10,
            effect_bonus_magic_resistance: 20,
            effect_bonus_hp_regen: 4,
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
                defence,
                magic_resistance,
                skill_cooldown_mult,
                effect_bonus_defence,
                effect_bonus_magic_resistance,
                effect_bonus_hp_regen,
                effect_minion_percent,
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

        let mut targets: Vec<(usize, bool)> = Vec::new();
        for index in 0..ctx.entity_count() {
            let Some(entity_ref) = ctx.entity_at(index) else {
                continue;
            };
            let id = entity_ref.id();
            if !entity_ref.is_alive() || entity_ref.team() != caster_team || id == caster_id {
                continue;
            }
            let is_minion = entity_ref.is_minion();
            // Towers are allied entities too, and Legion is not meant for them.
            if !is_minion && !entity_ref.is_champion() {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            targets.push((id, is_minion));
        }

        for (id, is_minion) in targets {
            let scale = |value: i32| {
                if is_minion {
                    percent_of_i32(value, self.effect_minion_percent)
                } else {
                    value
                }
            };
            // Replace rather than skip-if-present, both within this tick, so the
            // ally never spends a stretch of the cycle without the bonus.
            ctx.entity_remove_buff(id, self.aura_buff);
            ctx.add_buff(
                id,
                &BuffV1 {
                    defence: scale(self.effect_bonus_defence),
                    magic_resistance: scale(self.effect_bonus_magic_resistance),
                    hp_regen: scale(self.effect_bonus_hp_regen),
                    ..BuffV1::timed(self.aura_buff, AURA_DURATION_TICKS)
                },
            );
        }

        self.refresh_cooldown = AURA_REFRESH_TICKS;
    }
}

impl Default for LocketOfTheIronSolari {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for LocketOfTheIronSolari {
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
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
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
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}
