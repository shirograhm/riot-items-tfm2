use crate::config::ItemConfig;
use crate::{apply_config, heal_shield_power};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct ForbiddenIdol {
    price: usize,
    hp_regen: i32,
    effect_heal_shield_power: f64,
}

impl Default for ForbiddenIdol {
    fn default() -> Self {
        Self {
            price: 500,
            hp_regen: 2,
            effect_heal_shield_power: 8.0,
        }
    }
}

impl ForbiddenIdol {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, hp_regen, effect_heal_shield_power]);
        item
    }
}

impl StableItem for ForbiddenIdol {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "forbidden_idol".to_string()
    }

    fn icon(&self) -> String {
        "forbidden_idol".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["rejuvenation_bead".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "sword_of_blossoming_dawn".to_string(),
            "ardent_censer".to_string(),
            "staff_of_flowing_water".to_string(),
            "mikaels_blessing".to_string(),
            "dawncore".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    // Heal and shield power, reported every tick.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        heal_shield_power::note(ctx, player, self.effect_heal_shield_power);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
