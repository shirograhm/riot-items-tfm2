use crate::apply_config;
use crate::config::ItemConfig;
use crate::SharedRiches;
use mod_api_stable::*;

/// Bounty of Worlds — the last step of the World Atlas line before a finished
/// support item: more health and regen, and more gold again
/// ([`SharedRiches`]). It grows into Bloodsong, Solstice Sleigh or Zaz'Zak's
/// Realmspike.
#[derive(Clone, Debug)]
pub struct BountyOfWorlds {
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    riches: SharedRiches,
}

impl Default for BountyOfWorlds {
    fn default() -> Self {
        Self {
            price: 400,
            hp: 100,
            hp_regen: 4,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            riches: SharedRiches::default(),
        }
    }
}

impl BountyOfWorlds {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                hp_regen,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        item
    }
}

impl StableItem for BountyOfWorlds {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "bounty_of_worlds".to_string()
    }

    fn icon(&self) -> String {
        "bounty_of_worlds".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["runic_compass".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "bloodsong".to_string(),
            "solstice_sleigh".to_string(),
            "zazzaks_realmspike".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn update(&mut self, sim: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.riches.update(
            sim,
            player,
            self.effect_bonus_gold,
            self.effect_gold_interval_seconds,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
