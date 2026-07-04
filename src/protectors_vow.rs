use arrayvec::ArrayString;
use core::fmt::Write;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

const REFRESH_LOCKOUT_TICKS: usize = 3;

#[derive(Clone, Debug)]
pub struct ProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    refresh_lockout: usize,
}

impl Default for ProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 350,
            defence: 50,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_lockout: 0,
        }
    }
}

impl ProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_lockout: 0,
        }
    }

    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_lockout > 0 {
            self.refresh_lockout -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let prefix = "protectors_vow_awe_";
        let granted = (0..entity_ref.buff_count())
            .filter_map(|i| {
                entity_ref
                    .buff_at(i)
                    .name
                    .as_str()
                    .strip_prefix(prefix)
                    .and_then(|total| total.parse::<i32>().ok())
            })
            .max()
            .unwrap_or(0);

        let target = self.effect_bonus_flat_hp
            + percent_of(
                entity_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= granted {
            return;
        }

        let entity_id = entity_ref.id();
        let mut name = ArrayString::<64>::new();
        write!(&mut name, "{prefix}{target}").unwrap();

        ctx.add_buff(
            entity_id,
            BuffState {
                name,
                duration: BuffType::Permanent,
                hp: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for ProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    refresh_lockout: usize,
}

impl Default for RadiantProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1800,
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_lockout: 0,
        }
    }
}

impl RadiantProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_lockout: 0,
        }
    }

    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_lockout > 0 {
            self.refresh_lockout -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let prefix = "radiant_protectors_vow_awe_";
        let granted = (0..entity_ref.buff_count())
            .filter_map(|i| {
                entity_ref
                    .buff_at(i)
                    .name
                    .as_str()
                    .strip_prefix(prefix)
                    .and_then(|total| total.parse::<i32>().ok())
            })
            .max()
            .unwrap_or(0);

        let target = self.effect_bonus_flat_hp
            + percent_of(
                entity_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= granted {
            return;
        }

        let entity_id = entity_ref.id();
        let mut name = ArrayString::<64>::new();
        write!(&mut name, "{prefix}{target}").unwrap();

        ctx.add_buff(
            entity_id,
            BuffState {
                name,
                duration: BuffType::Permanent,
                hp: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for RadiantProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}
