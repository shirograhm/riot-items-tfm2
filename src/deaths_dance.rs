use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct DeathsDance {
    price: usize,
    attack: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_delayed_damage_percent: f64,
    effect_burn_hp_percent_cap: f64,
    effect_kill_heal_missing_percent: f64,
    accumulated_damage: i32,
    last_damaged_by: usize,
}

impl Default for DeathsDance {
    fn default() -> Self {
        Self {
            price: 1450,
            attack: 50,
            defence: 50,
            skill_cooldown_mult: 10,
            effect_delayed_damage_percent: 25.0,
            effect_burn_hp_percent_cap: 5.0,
            effect_kill_heal_missing_percent: 15.0,
            accumulated_damage: 0,
            last_damaged_by: 0,
        }
    }
}

impl DeathsDance {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_delayed_damage_percent: cfg
                .effect_delayed_damage_percent
                .unwrap_or(d.effect_delayed_damage_percent),
            effect_burn_hp_percent_cap: cfg
                .effect_burn_hp_percent_cap
                .unwrap_or(d.effect_burn_hp_percent_cap),
            effect_kill_heal_missing_percent: cfg
                .effect_kill_heal_missing_percent
                .unwrap_or(d.effect_kill_heal_missing_percent),
            accumulated_damage: d.accumulated_damage,
            last_damaged_by: d.last_damaged_by,
        }
    }
}

impl ModItemInfo for DeathsDance {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "deaths_dance"
    }

    fn icon(&self) -> &str {
        "deaths_dance"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["steel_sigil".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_deaths_dance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            damaged_reduce: self.effect_delayed_damage_percent as usize,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.accumulated_damage <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_burn_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "deaths_dance_burn");
        if is_burn_applied {
            return;
        }

        let entity = entity_ref.id();
        let per_second_cap =
            percent_of(entity_ref.hp().max, self.effect_burn_hp_percent_cap / 5.0) as i32;

        let tick_damage = self.accumulated_damage.min(per_second_cap);
        if tick_damage <= 0 {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 12 },
                name: ArrayString::try_from("deaths_dance_burn").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(
            self.last_damaged_by,
            entity,
            tick_damage as usize,
            0,
            AttackType::Item,
        );
        self.accumulated_damage -= tick_damage;
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
    ) {
        if attacker == entity {
            return;
        }

        self.accumulated_damage += percent_of(
            (damage as f64 * 4.0 / 3.0).round() as usize,
            self.effect_delayed_damage_percent,
        ) as i32;

        self.last_damaged_by = attacker
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        self.accumulated_damage = 0;

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let missing_hp = entity_ref.hp().max.saturating_sub(entity_ref.hp().current);
        let entity = entity_ref.id();
        let heal_amount = percent_of(missing_hp, self.effect_kill_heal_missing_percent);
        if heal_amount == 0 {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: heal_amount as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantDeathsDance {
    price: usize,
    attack: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_delayed_damage_percent: f64,
    effect_burn_hp_percent_cap: f64,
    effect_kill_heal_missing_percent: f64,
    accumulated_damage: i32,
    last_damaged_by: usize,
}

impl Default for RadiantDeathsDance {
    fn default() -> Self {
        Self {
            price: 2100,
            attack: 85,
            defence: 85,
            skill_cooldown_mult: 10,
            effect_delayed_damage_percent: 25.0,
            effect_burn_hp_percent_cap: 5.0,
            effect_kill_heal_missing_percent: 15.0,
            accumulated_damage: 0,
            last_damaged_by: 0,
        }
    }
}

impl RadiantDeathsDance {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_delayed_damage_percent: cfg
                .effect_delayed_damage_percent
                .unwrap_or(d.effect_delayed_damage_percent),
            effect_burn_hp_percent_cap: cfg
                .effect_burn_hp_percent_cap
                .unwrap_or(d.effect_burn_hp_percent_cap),
            effect_kill_heal_missing_percent: cfg
                .effect_kill_heal_missing_percent
                .unwrap_or(d.effect_kill_heal_missing_percent),
            accumulated_damage: d.accumulated_damage,
            last_damaged_by: d.last_damaged_by,
        }
    }
}

impl ModItemInfo for RadiantDeathsDance {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_deaths_dance"
    }

    fn icon(&self) -> &str {
        "radiant_deaths_dance"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["deaths_dance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            damaged_reduce: self.effect_delayed_damage_percent as usize,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.accumulated_damage <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_burn_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_deaths_dance_burn");
        if is_burn_applied {
            return;
        }

        let entity = entity_ref.id();
        let per_second_cap =
            percent_of(entity_ref.hp().max, self.effect_burn_hp_percent_cap / 5.0) as i32;

        let tick_damage = self.accumulated_damage.min(per_second_cap);
        if tick_damage <= 0 {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 12 },
                name: ArrayString::try_from("radiant_deaths_dance_burn").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(
            self.last_damaged_by,
            entity,
            tick_damage as usize,
            0,
            AttackType::Item,
        );
        self.accumulated_damage -= tick_damage;
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
    ) {
        if attacker == entity {
            return;
        }

        self.accumulated_damage += percent_of(
            (damage as f64 * 4.0 / 3.0).round() as usize,
            self.effect_delayed_damage_percent,
        ) as i32;

        self.last_damaged_by = attacker
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        self.accumulated_damage = 0;

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let missing_hp = entity_ref.hp().max.saturating_sub(entity_ref.hp().current);
        let entity = entity_ref.id();
        let heal_amount = percent_of(missing_hp, self.effect_kill_heal_missing_percent);
        if heal_amount == 0 {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: heal_amount as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
