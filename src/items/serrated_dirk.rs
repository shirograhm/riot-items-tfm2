use mod_api::*;

use crate::apply_lethality;
use crate::config::ItemConfig;

// Lethality: the wielder's basic attacks ignore a flat amount of the target's
// armor. The game mitigates physical damage by `100/(100+armor)`, so
// `apply_lethality` scales the outgoing attack up so that after the game applies
// armor the result matches `armor - lethality`. This only affects basic attacks
// (the one damage instance a mod can modify); ability damage is dealt by the game
// and can't be scaled here.

#[derive(Clone, Debug)]
pub struct SerratedDirk {
    price: usize,
    attack: i32,
    effect_lethality: usize,
}

impl Default for SerratedDirk {
    fn default() -> Self {
        Self {
            price: 800,
            attack: 45,
            effect_lethality: 10,
        }
    }
}

impl SerratedDirk {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
        }
    }
}

impl ModItemInfo for SerratedDirk {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "serrated_dirk"
    }

    fn icon(&self) -> &str {
        "serrated_dirk"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ironsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "hubris".to_string(),
            "bastionbreaker".to_string(),
            "serpents_fang".to_string(),
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
