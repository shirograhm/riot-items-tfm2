use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, buff_name, has_buff, percent_of_i32, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS, DISTANCE_UNITS_PER_RANGE,
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
            effect_bonus_defence: 10,
            effect_bonus_magic_resistance: 20,
            effect_bonus_hp_regen: 4,
            effect_minion_percent: 150.0,
            effect_max_distance: 80,
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

    /// Grants Legion to every living ally in range that is not already carrying
    /// it. Minions take the same bonuses scaled by `effect_minion_percent`, so
    /// the scan walks all entities rather than just the champion table.
    fn apply_aura(&mut self, ctx: &mut GameCtx, player: usize) {
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
            if has_buff(&entity_ref, self.aura_buff) {
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
            ctx.add_buff(
                id,
                BuffState {
                    name: buff_name(self.aura_buff),
                    duration: BuffType::Time {
                        tick: BUFF_REFRESH_DURATION_TICKS,
                    },
                    defence: scale(self.effect_bonus_defence),
                    magic_resistance: scale(self.effect_bonus_magic_resistance),
                    hp_regen: scale(self.effect_bonus_hp_regen),
                    ..Default::default()
                },
            );
        }

        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for LocketOfTheIronSolari {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for LocketOfTheIronSolari {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_aura(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_aura(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::Defense,
            ItemTag::MagicResistance,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}
