use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

const BURN_TICK_RATE: usize = 12;

#[derive(Clone, Debug)]
pub struct LiandrysTorment {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_duration_seconds: f64,
    refresh_cooldown: usize,
}

impl Default for LiandrysTorment {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 350,
            magic_power: 75,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
            refresh_cooldown: 0,
        }
    }
}

impl LiandrysTorment {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_hp_percent_damage: cfg
                .effect_hp_percent_damage
                .unwrap_or(d.effect_hp_percent_damage),
            effect_minion_damage_cap: cfg
                .effect_minion_damage_cap
                .unwrap_or(d.effect_minion_damage_cap),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            refresh_cooldown: 0,
        }
    }
}

impl ModItemInfo for LiandrysTorment {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "liandrys_torment"
    }

    fn icon(&self) -> &str {
        "liandrys_torment"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["haunting_guise".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_liandrys_torment".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
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
        let caster_team = caster.team();

        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() == caster_team {
                continue;
            }

            let is_burning = (0..entity_ref.buff_count())
                .any(|i| entity_ref.buff_at(i).name.as_str() == "liandrys_torment_burn");
            let amount_to_burn = percent_of(entity_ref.hp().max, self.effect_hp_percent_damage);
            let mut bonus_damage = amount_to_burn as f64
                / (self.effect_duration_seconds * 60.0 / BURN_TICK_RATE as f64);
            if !entity_ref.is_champion() {
                bonus_damage = bonus_damage.clamp(0.0, self.effect_minion_damage_cap as f64);
            }

            if is_burning {
                ctx.deal_damage(
                    player,
                    id,
                    0,
                    bonus_damage.round() as usize,
                    AttackType::Item,
                );
                self.refresh_cooldown = BURN_TICK_RATE;
            }
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "liandrys_torment_burn");
        if is_cooldown_ticking {
            return;
        }
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("liandrys_torment_burn").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantLiandrysTorment {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_duration_seconds: f64,
    refresh_cooldown: usize,
}

impl Default for RadiantLiandrysTorment {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 550,
            magic_power: 150,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
            refresh_cooldown: 0,
        }
    }
}

impl RadiantLiandrysTorment {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_hp_percent_damage: cfg
                .effect_hp_percent_damage
                .unwrap_or(d.effect_hp_percent_damage),
            effect_minion_damage_cap: cfg
                .effect_minion_damage_cap
                .unwrap_or(d.effect_minion_damage_cap),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            refresh_cooldown: 0,
        }
    }
}

impl ModItemInfo for RadiantLiandrysTorment {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_liandrys_torment"
    }

    fn icon(&self) -> &str {
        "radiant_liandrys_torment"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["liandrys_torment".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
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
        let caster_team = caster.team();

        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() == caster_team {
                continue;
            }

            let is_burning = (0..entity_ref.buff_count())
                .any(|i| entity_ref.buff_at(i).name.as_str() == "liandrys_torment_burn");
            let amount_to_burn = percent_of(entity_ref.hp().max, self.effect_hp_percent_damage);
            let mut bonus_damage = amount_to_burn as f64
                / (self.effect_duration_seconds * 60.0 / BURN_TICK_RATE as f64);
            if !entity_ref.is_champion() {
                bonus_damage = bonus_damage.clamp(0.0, self.effect_minion_damage_cap as f64);
            }

            if is_burning {
                ctx.deal_damage(
                    player,
                    id,
                    0,
                    bonus_damage.round() as usize,
                    AttackType::Item,
                );
                self.refresh_cooldown = BURN_TICK_RATE;
            }
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "liandrys_torment_burn");
        if is_cooldown_ticking {
            return;
        }
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("liandrys_torment_burn").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
