use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, immolate_burn, mark_immolate, percent_of, ItemMeta, TICKS_PER_SECOND};

// Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies
// within 30 range. This effect deals 50% more damage to minions and monsters.

#[derive(Clone, Debug)]
pub struct PhilosophersStone {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
    effect_minion_bonus_percent: f64,
    // Non-vital stats (internals)
    until_next_burn: usize,
}

impl PhilosophersStone {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "philosophers_stone",
                &["aegis_of_the_legion", "bamis_cinder"],
                &["radiant_philosophers_stone"],
            ),
            price: 750,
            hp: 200,
            defence: 20,
            magic_resistance: 30,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_minion_bonus_percent: 50.0,
            // Non-vital stats (internals)
            until_next_burn: TICKS_PER_SECOND as usize,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_philosophers_stone", &["philosophers_stone"]),
            price: 1050,
            hp: 350,
            defence: 30,
            magic_resistance: 50,
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_minion_bonus_percent: 50.0,
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
                defence,
                magic_resistance,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_max_distance,
                effect_minion_bonus_percent
            ]
        );
        self
    }
}

impl Default for PhilosophersStone {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for PhilosophersStone {
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
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.until_next_burn = TICKS_PER_SECOND as usize;
        // Flames up from the first frame rather than the first burn.
        if let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        {
            mark_immolate(ctx, champion);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // Immolate, once a second while alive, the same burn as Bami's Cinder.
        self.until_next_burn = self.until_next_burn.saturating_sub(1);
        if self.until_next_burn > 0 {
            return;
        }
        self.until_next_burn = TICKS_PER_SECOND as usize;

        let Some((caster, team, max_hp)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.team(), c.hp().1))
        else {
            return;
        };

        mark_immolate(ctx, caster);
        let damage = self.effect_bonus_flat_damage
            + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let minion_damage = damage + percent_of(damage, self.effect_minion_bonus_percent);
        immolate_burn(
            ctx,
            caster,
            team,
            self.effect_max_distance,
            damage,
            minion_damage,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::DotDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
