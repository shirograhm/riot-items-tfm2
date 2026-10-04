use crate::apply_config;
use crate::config::ItemConfig;
use crate::SharedRiches;
use mod_api_stable::*;

/// Runic Compass — World Atlas's upgrade, and the last step before a finished
/// support item: health on top of the regen, and the gold comes in faster
/// ([`SharedRiches`]). It grows into every finished item of the World Atlas
/// line.
#[derive(Clone, Debug)]
pub struct RunicCompass {
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    riches: SharedRiches,
}

impl Default for RunicCompass {
    fn default() -> Self {
        Self {
            price: 450,
            hp: 150,
            hp_regen: 3,
            effect_bonus_gold: 3,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            riches: SharedRiches::default(),
        }
    }
}

impl RunicCompass {
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

impl StableItem for RunicCompass {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "runic_compass".to_string()
    }

    fn icon(&self) -> String {
        "runic_compass".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["world_atlas".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "bloodsong".to_string(),
            "celestial_opposition".to_string(),
            "dream_maker".to_string(),
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
