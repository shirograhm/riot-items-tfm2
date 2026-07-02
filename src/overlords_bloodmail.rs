use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// Tyranny passive: while equipped, the wielder gains bonus Attack Damage equal to
// `effect_caster_hp_percent_attack`% of their maximum health. `update` maintains
// this as a short (30-tick) buff that is re-applied once it lapses, so the bonus
// tracks the champion's current maximum health (see mirage_blade.rs for the same
// maintained-buff pattern).

#[derive(Clone, Debug)]
pub struct OverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
}

impl Default for OverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 55,
            hp: 250,
            effect_caster_hp_percent_attack: 2.5,
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
        }
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
        // TODO: fill in the build recipe (component item keys).
        vec![]
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

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let has_buff = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "overlords_bloodmail_tyranny");
        if has_buff {
            return;
        }

        let bonus_attack =
            percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
        let entity_id = entity_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                duration: BuffType::Time { tick: 30 },
                attack: bonus_attack,
                name: ArrayString::try_from("overlords_bloodmail_tyranny").unwrap(),
                ..Default::default()
            },
        );
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
}

impl Default for RadiantOverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 105,
            hp: 450,
            effect_caster_hp_percent_attack: 2.5,
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
        }
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

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let has_buff = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_overlords_bloodmail_tyranny");
        if has_buff {
            return;
        }

        let bonus_attack =
            percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
        let entity_id = entity_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                duration: BuffType::Time { tick: 30 },
                attack: bonus_attack,
                name: ArrayString::try_from("radiant_overlords_bloodmail_tyranny").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
