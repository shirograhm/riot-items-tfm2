use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct RejuvenationBead {
    price: usize,
    hp_regen: i32,
}

impl Default for RejuvenationBead {
    fn default() -> Self {
        Self {
            price: 150,
            hp_regen: 2,
        }
    }
}

impl RejuvenationBead {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, hp_regen]);
        item
    }
}

impl StableItem for RejuvenationBead {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "rejuvenation_bead".to_string()
    }

    fn icon(&self) -> String {
        "rejuvenation_bead".to_string()
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
        vec!["forbidden_idol".to_string()]
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
