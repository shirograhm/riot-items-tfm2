use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::BUFF_REFRESH_DURATION_TICKS;

#[derive(Clone, Debug)]
pub struct EchoesOfHelia {
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_stacks: usize,
    effect_max_stacks: usize,
    charge_stored: usize,
}

impl Default for EchoesOfHelia {
    fn default() -> Self {
        Self {
            price: 1100,
            hp: 250,
            hp_regen: 4,
            magic_power: 45,
            skill_cooldown_mult: 15,
            effect_min_stacks: 120,
            effect_max_stacks: 450,
            charge_stored: 0,
        }
    }
}

impl EchoesOfHelia {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_min_stacks: cfg.effect_min_stacks.unwrap_or(d.effect_min_stacks),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            charge_stored: 0,
        }
    }
}

impl ModItemInfo for EchoesOfHelia {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "echoes_of_helia"
    }

    fn icon(&self) -> &str {
        "echoes_of_helia"
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
        vec!["radiant_echoes_of_helia".to_string()]
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

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_player(caster) else {
            return;
        };

        let stack_gain = (*damage as f64 * 0.3) as usize;
        let limit_per_level = (self.effect_max_stacks - self.effect_min_stacks) as f64 / 11.0;
        let max_limit =
            self.effect_min_stacks + (caster_ref.level() - 1) * limit_per_level.round() as usize;

        if self.charge_stored + stack_gain > max_limit {
            self.charge_stored = max_limit;
        } else {
            self.charge_stored += stack_gain;
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(champion_ref) = caster_ref.champion() else {
            return;
        };

        let caster_team = caster_ref.team();
        let caster_champion_id = champion_ref.id();

        let mut nearest_id = usize::MAX;
        let mut nearest_dist = u64::MAX;

        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };

            if ctx.distance_sq(caster_champion_id, id) < nearest_dist
                && entity_ref.team() == caster_team
                && entity_ref.is_alive()
            {
                nearest_id = id;
                nearest_dist = ctx.distance_sq(caster_champion_id, id)
            }
        }
        let buff = BuffState {
            name: ArrayString::try_from("echoes_of_helia_healing").unwrap(),
            duration: BuffType::Time {
                tick: BUFF_REFRESH_DURATION_TICKS,
            },
            hp_regen: self.charge_stored as i32,
            ..Default::default()
        };
        ctx.add_buff(nearest_id, buff);

        self.charge_stored = 0;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::AP,
            ItemTag::CooltimeReduce,
            ItemTag::Vamp,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantEchoesOfHelia {
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_stacks: usize,
    effect_max_stacks: usize,
    charge_stored: usize,
}

impl Default for RadiantEchoesOfHelia {
    fn default() -> Self {
        Self {
            price: 1500,
            hp: 450,
            hp_regen: 6,
            magic_power: 65,
            skill_cooldown_mult: 20,
            effect_min_stacks: 120,
            effect_max_stacks: 450,
            charge_stored: 0,
        }
    }
}

impl RadiantEchoesOfHelia {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_min_stacks: cfg.effect_min_stacks.unwrap_or(d.effect_min_stacks),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            charge_stored: 0,
        }
    }
}

impl ModItemInfo for RadiantEchoesOfHelia {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_echoes_of_helia"
    }

    fn icon(&self) -> &str {
        "radiant_echoes_of_helia"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["echoes_of_helia".to_string()]
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

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_player(caster) else {
            return;
        };

        let stack_gain = (*damage as f64 * 0.3) as usize;
        let limit_per_level = (self.effect_max_stacks - self.effect_min_stacks) as f64 / 11.0;
        let max_limit =
            self.effect_min_stacks + (caster_ref.level() - 1) * limit_per_level.round() as usize;

        if self.charge_stored + stack_gain > max_limit {
            self.charge_stored = max_limit;
        } else {
            self.charge_stored += stack_gain;
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(champion_ref) = caster_ref.champion() else {
            return;
        };

        let caster_team = caster_ref.team();
        let caster_champion_id = champion_ref.id();

        let mut nearest_id = usize::MAX;
        let mut nearest_dist = u64::MAX;

        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };

            if ctx.distance_sq(caster_champion_id, id) < nearest_dist
                && entity_ref.team() == caster_team
                && entity_ref.is_alive()
            {
                nearest_id = id;
                nearest_dist = ctx.distance_sq(caster_champion_id, id)
            }
        }
        let buff = BuffState {
            name: ArrayString::try_from("echoes_of_helia_healing").unwrap(),
            duration: BuffType::Time {
                tick: BUFF_REFRESH_DURATION_TICKS,
            },
            hp_regen: self.charge_stored as i32,
            ..Default::default()
        };
        ctx.add_buff(nearest_id, buff);

        self.charge_stored = 0;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::AP,
            ItemTag::CooltimeReduce,
            ItemTag::Vamp,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
