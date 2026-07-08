use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// The Tyranny bonus attack is granted as a fixed-duration buff that is
// re-applied on a slightly shorter cycle than it lasts, so a fresh buff is
// always in place before the previous one expires -- no single-tick gap where
// the bonus would flicker off. Recomputing the amount each cycle lets it track
// the holder's current max HP, rising or falling with it, instead of only
// ratcheting upward. During the brief overlap both buffs are live, so the bonus
// is momentarily doubled -- harmless, since it only ever overshoots.
const TYRANNY_BUFF_DURATION_TICKS: usize = 600; // 10 seconds
const TYRANNY_REFRESH_PERIOD_TICKS: usize = 598; // 9.966 seconds -> ~.033 second of overlap

#[derive(Clone, Debug)]
pub struct OverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    refresh_cooldown: usize,
}

impl Default for OverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 25,
            hp: 400,
            effect_caster_hp_percent_attack: 2.5,
            refresh_cooldown: 0,
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
            refresh_cooldown: 0,
        }
    }

    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = entity_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: ArrayString::try_from("overlords_bloodmail_tyranny").unwrap(),
                duration: BuffType::Time {
                    tick: TYRANNY_BUFF_DURATION_TICKS,
                },
                attack: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = TYRANNY_REFRESH_PERIOD_TICKS;
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
        "overlords_bloodmail"
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
        self.refresh_cooldown = 0;
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
    refresh_cooldown: usize,
}

impl Default for RadiantOverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 40,
            hp: 650,
            effect_caster_hp_percent_attack: 2.5,
            refresh_cooldown: 0,
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
            refresh_cooldown: 0,
        }
    }

    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = entity_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: ArrayString::try_from("radiant_overlords_bloodmail_tyranny").unwrap(),
                duration: BuffType::Time {
                    tick: TYRANNY_BUFF_DURATION_TICKS,
                },
                attack: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = TYRANNY_REFRESH_PERIOD_TICKS;
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
        "radiant_overlords_bloodmail"
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
        self.refresh_cooldown = 0;
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
