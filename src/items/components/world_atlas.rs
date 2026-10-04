use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

/// World Atlas — the support's starting item: a little health regen and gold
/// that comes in by itself ([`crate::SharedRiches`]). It grows into Runic Compass,
/// then one of the finished support items.
#[derive(Clone, Debug)]
pub struct WorldAtlas {
    price: usize,
    hp_regen: i32,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
}

impl Default for WorldAtlas {
    fn default() -> Self {
        Self {
            price: 250,
            hp_regen: 3,
            effect_bonus_gold: 2,
            effect_gold_interval_seconds: 5.0,
        }
    }
}

impl WorldAtlas {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp_regen,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        item
    }

    /// What Shared Riches pays a holder: this much gold, this often.
    /// [`crate::SharedRiches`] does the paying, from the match hook.
    pub(crate) fn shared_riches(&self) -> (usize, f64) {
        (self.effect_bonus_gold, self.effect_gold_interval_seconds)
    }
}

impl StableItem for WorldAtlas {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "world_atlas".to_string()
    }

    fn icon(&self) -> String {
        "world_atlas".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["runic_compass".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
