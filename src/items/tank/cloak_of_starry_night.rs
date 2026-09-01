use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta, BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

#[derive(Clone, Debug)]
pub struct CloakOfStarryNight {
    meta: ItemMeta,
    limitless_buff: &'static str,
    price: usize,
    hp: i32,
    magic_resistance: i32,
    magic_resistance_mult: i32,
    effect_skill_damaged_reduce: usize,
    effect_magic_resistance_per_reduce: usize,
    effect_max_skill_damaged_reduce: usize,
    // Non-vital stats (internals)
    refresh_cooldown: usize,
}

impl CloakOfStarryNight {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "cloak_of_starry_night",
                &["dusk_raven"],
                &["radiant_cloak_of_starry_night"],
            ),
            limitless_buff: "cloak_of_starry_night_limitless",
            price: 1450,
            hp: 350,
            magic_resistance: 100,
            magic_resistance_mult: 25,
            effect_skill_damaged_reduce: 5,
            effect_magic_resistance_per_reduce: 25,
            effect_max_skill_damaged_reduce: 25,
            // Non-vital stats (internals)
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_cloak_of_starry_night",
                &["cloak_of_starry_night"],
            ),
            price: 2100,
            hp: 500,
            magic_resistance: 200,
            magic_resistance_mult: 25,
            effect_skill_damaged_reduce: 5,
            effect_magic_resistance_per_reduce: 25,
            effect_max_skill_damaged_reduce: 25,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                hp,
                magic_resistance,
                magic_resistance_mult,
                effect_skill_damaged_reduce,
                effect_magic_resistance_per_reduce,
                effect_max_skill_damaged_reduce
            ]
        );
        self
    }

    /// Limitless as the Stars. The magic resistance multiplier is a plain stat,
    /// but the skill damage reduction it feeds has to track that resistance as
    /// buffs and levels move it, so it is granted as a re-applied timed buff the
    /// way `Overlord's Bloodmail` grants Tyranny.
    ///
    /// Read first, then remove-and-replace: the buff carries no resistance of
    /// its own, so the number it is solved from is never one it produced.
    fn apply_limitless(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let entity_id = champion_ref.id();
        let magic_resistance = champion_ref.stat().magic_resistance;

        let scaled = if self.effect_magic_resistance_per_reduce == 0 {
            0
        } else {
            magic_resistance / self.effect_magic_resistance_per_reduce
        };
        let reduce = (self.effect_skill_damaged_reduce + scaled)
            .min(self.effect_max_skill_damaged_reduce);

        // Same-name buffs stack, so the old one goes before the new one lands.
        ctx.entity_remove_buff(entity_id, self.limitless_buff);
        ctx.add_buff(
            entity_id,
            &BuffV1 {
                skill_damaged_reduce: reduce,
                ..BuffV1::timed(self.limitless_buff, BUFF_REFRESH_DURATION_TICKS)
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for CloakOfStarryNight {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for CloakOfStarryNight {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_resistance: self.magic_resistance,
            magic_resistance_mult: self.magic_resistance_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_limitless(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_limitless(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::MagicResistance]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::MagicResistance
    }
}
