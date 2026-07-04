use arrayvec::ArrayString;
use core::fmt::Write;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

const REFRESH_LOCKOUT_TICKS: usize = 3;

#[derive(Clone, Debug)]
pub struct OverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    refresh_lockout: usize,
}

impl Default for OverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 25,
            hp: 400,
            effect_caster_hp_percent_attack: 2.5,
            refresh_lockout: 0,
        }
    }
}

impl OverlordsBloodmail {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_caster_hp_percent_attack: cfg
                .effect_caster_hp_percent_attack
                .unwrap_or(d.effect_caster_hp_percent_attack),
            refresh_lockout: 0,
        }
    }

    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
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

        let prefix = "overlords_bloodmail_tyranny_";
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

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
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
                attack: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for OverlordsBloodmail {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "overlords_bloodmail"
    }

    fn icon(&self) -> &str {
        "t11_0"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "soldiers_longsword".to_string(),
            "hardened_heart".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_overlords_bloodmail".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_tyranny(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_tyranny(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantOverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    // Ticks remaining during which `apply_tyranny` skips its presence check, so a
    // freshly added buff can't be duplicated before it becomes visible.
    refresh_lockout: usize,
}

impl Default for RadiantOverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 40,
            hp: 650,
            effect_caster_hp_percent_attack: 2.5,
            refresh_lockout: 0,
        }
    }
}

impl RadiantOverlordsBloodmail {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_caster_hp_percent_attack: cfg
                .effect_caster_hp_percent_attack
                .unwrap_or(d.effect_caster_hp_percent_attack),
            refresh_lockout: 0,
        }
    }

    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
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

        let prefix = "radiant_overlords_bloodmail_tyranny_";
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

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
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
                attack: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for RadiantOverlordsBloodmail {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_overlords_bloodmail"
    }

    fn icon(&self) -> &str {
        "t11_1"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["overlords_bloodmail".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_tyranny(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_tyranny(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
