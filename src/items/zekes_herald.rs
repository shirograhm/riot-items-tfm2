use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, buff_name, has_buff, ItemMeta, ADAPTIVE_FORCE_AD_RATIO,
    BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS, DISTANCE_UNITS_PER_RANGE,
};

#[derive(Clone, Debug)]
pub struct ZekesHerald {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    aura_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_adaptive_force: i32,
    effect_vamp: i32,
    effect_max_distance: usize,
    refresh_cooldown: usize,
}

impl ZekesHerald {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "zekes_herald",
                &["bandleglass_mirror"],
                &["radiant_zekes_herald"],
            ),
            aura_buff: "zekes_herald_aura",
            price: 1050,
            hp: 300,
            hp_regen: 3,
            magic_power: 35,
            skill_cooldown_mult: 10,
            effect_adaptive_force: 30,
            effect_vamp: 6,
            effect_max_distance: 80,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zekes_herald", &["zekes_herald"]),
            aura_buff: "radiant_zekes_herald_aura",
            price: 1500,
            hp: 500,
            hp_regen: 5,
            magic_power: 55,
            skill_cooldown_mult: 15,
            effect_adaptive_force: 50,
            effect_vamp: 10,
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
                effect_adaptive_force,
                effect_vamp,
                effect_max_distance
            ]
        );
        self
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
            if !entity_ref.is_alive() || entity_ref.team() != caster_team || id == caster_id {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            let prefers_ap = entity_ref.stat().magic_power > entity_ref.stat().attack;

            let is_already_affected: bool = has_buff(&entity_ref, self.aura_buff);
            if !is_already_affected {
                targets.push((id, prefers_ap));
            }
        }

        for (id, prefers_ap) in targets {
            let mut buff = BuffState {
                name: buff_name(self.aura_buff),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                vamp: self.effect_vamp,
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

impl Default for ZekesHerald {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for ZekesHerald {
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
