use crate::config::ItemConfig;
use crate::{apply_config, Eternity};
use mod_api_stable::*;

/// Catalyst of Aeons: health, a little Ability Power and Ability Haste, and
/// Eternity ([`crate::Eternity`]): a heal for every Ability cast. It grows
/// into Rod of Ages, which keeps Eternity.
#[derive(Clone, Debug)]
pub struct CatalystOfAeons {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_heal: usize,
    effect_max_heal: usize,
    // Non-vital stats (internals)
    eternity: Eternity,
}

impl Default for CatalystOfAeons {
    fn default() -> Self {
        Self {
            price: 500,
            hp: 100,
            magic_power: 20,
            skill_cooldown_mult: 5,
            effect_min_heal: 15,
            effect_max_heal: 40,
            // Non-vital stats (internals)
            eternity: Eternity::default(),
        }
    }
}

impl CatalystOfAeons {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                magic_power,
                skill_cooldown_mult,
                effect_min_heal,
                effect_max_heal
            ]
        );
        item
    }
}

impl StableItem for CatalystOfAeons {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "catalyst_of_aeons".to_string()
    }

    fn icon(&self) -> String {
        "catalyst_of_aeons".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string(), "spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["rod_of_ages".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.eternity.reset();
    }

    /// Eternity's heal, for an Ability cast since the last tick.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.eternity
            .update(ctx, player, self.effect_min_heal, self.effect_max_heal);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
