use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    ADAPTIVE_FORCE_AD_RATIO, BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS,
    DISTANCE_UNITS_PER_RANGE,
};

// The aura grants Adaptive Force to every allied champion in range as a
// fixed-duration buff, re-applied on a slightly shorter cycle than it lasts so a
// fresh buff is always in place before the previous one expires (same pattern as
// protectors_vow's Awe). An ally who leaves range simply stops being refreshed
// and the buff expires ~1s later; during the ~2-tick overlap the bonus is briefly
// doubled, which is harmless for a flat combat stat. Same-name buffs stack and
// there is no removal API, so this cyclic refresh is the only way to make an aura
// that tracks position up AND down.

#[derive(Clone, Debug)]
pub struct ZekesHerald {
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_adaptive_force: i32,
    effect_max_distance: usize,
    refresh_cooldown: usize,
}

impl Default for ZekesHerald {
    fn default() -> Self {
        Self {
            price: 1050,
            hp: 300,
            hp_regen: 3,
            magic_power: 30,
            skill_cooldown_mult: 10,
            effect_adaptive_force: 30,
            effect_max_distance: 80,
            refresh_cooldown: 0,
        }
    }
}

impl ZekesHerald {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_adaptive_force: cfg.effect_adaptive_force.unwrap_or(d.effect_adaptive_force),
            effect_max_distance: cfg.effect_max_distance.unwrap_or(d.effect_max_distance),
            refresh_cooldown: 0,
        }
    }

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

        // Collect targets first: `get_entity` borrows `ctx` immutably, while
        // `add_buff` below needs it mutably. `prefers_ap` is decided per ally so
        // each recipient gets the adaptive stat it actually favors.
        let mut targets: Vec<(usize, bool)> = Vec::new();
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() != caster_team {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            let prefers_ap = entity_ref.stat().magic_power > entity_ref.stat().attack;
            targets.push((id, prefers_ap));
        }

        for (id, prefers_ap) in targets {
            let mut buff = BuffState {
                name: "zekes_herald_aura".try_into().unwrap(),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                ..Default::default()
            };
            if prefers_ap {
                buff.magic_power = self.effect_adaptive_force;
            } else {
                buff.attack =
                    (self.effect_adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32;
            }
            ctx.add_buff(id, buff);
        }

        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl ModItemInfo for ZekesHerald {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "zekes_herald"
    }

    fn icon(&self) -> &str {
        "zekes_herald"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bandleglass_mirror".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_zekes_herald".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
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
            ItemTag::AD,
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantZekesHerald {
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_adaptive_force: i32,
    effect_max_distance: usize,
    refresh_cooldown: usize,
}

impl Default for RadiantZekesHerald {
    fn default() -> Self {
        Self {
            price: 1500,
            hp: 500,
            hp_regen: 5,
            magic_power: 50,
            skill_cooldown_mult: 15,
            effect_adaptive_force: 50,
            effect_max_distance: 80,
            refresh_cooldown: 0,
        }
    }
}

impl RadiantZekesHerald {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_adaptive_force: cfg.effect_adaptive_force.unwrap_or(d.effect_adaptive_force),
            effect_max_distance: cfg.effect_max_distance.unwrap_or(d.effect_max_distance),
            refresh_cooldown: 0,
        }
    }

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
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() != caster_team {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            let prefers_ap = entity_ref.stat().magic_power > entity_ref.stat().attack;
            targets.push((id, prefers_ap));
        }

        for (id, prefers_ap) in targets {
            let mut buff = BuffState {
                name: "radiant_zekes_herald_aura".try_into().unwrap(),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                ..Default::default()
            };
            if prefers_ap {
                buff.magic_power = self.effect_adaptive_force;
            } else {
                buff.attack =
                    (self.effect_adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32;
            }
            ctx.add_buff(id, buff);
        }

        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl ModItemInfo for RadiantZekesHerald {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_zekes_herald"
    }

    fn icon(&self) -> &str {
        "radiant_zekes_herald"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["zekes_herald".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
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
            ItemTag::AD,
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
