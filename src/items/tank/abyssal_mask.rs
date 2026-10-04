use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, sized_range, ItemMeta, AURA_DURATION_TICKS, AURA_REFRESH_TICKS};

/// Abyssal Mask — Unmake, an aura that lowers the magic resistance of the
/// enemy champions around the carrier.
///
/// The same shape as `FrozenHeart`'s Winter's Caress: a short buff on every
/// enemy champion in range, put back on the shared aura cycle
/// ([`AURA_REFRESH_TICKS`]), so an enemy who walks out keeps it for at most
/// [`AURA_DURATION_TICKS`] and one who walks in has it within a cycle.
///
/// Riot's Unmake now reads "take 12% more magic damage". This is the older
/// form of it, the one the item was asked for with: less magic resistance.
///
/// # Both variants share the buff name
///
/// Same-name buffs stack, and re-applying is a remove followed by an add: one
/// `entity_remove_buff` clears every copy. So two Masks standing by the same
/// enemy leave one Unmake on them, not two.
#[derive(Clone, Debug)]
pub struct AbyssalMask {
    meta: ItemMeta,
    unmake_buff: &'static str,
    price: usize,
    hp: i32,
    magic_resistance: i32,
    skill_cooldown_mult: i32,
    effect_percent_mr_shred: i32,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    /// Ticks until the aura re-applies, on the shared cycle.
    refresh_cooldown: usize,
}

impl AbyssalMask {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("abyssal_mask", &["dusk_raven"], &["radiant_abyssal_mask"]),
            unmake_buff: "abyssal_mask_unmake",
            price: 700,
            hp: 150,
            magic_resistance: 50,
            skill_cooldown_mult: 10,
            effect_percent_mr_shred: 20,
            effect_max_distance: 50,
            // Non-vital stats (internals)
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_abyssal_mask", &["abyssal_mask"]),
            price: 950,
            hp: 250,
            magic_resistance: 75,
            skill_cooldown_mult: 15,
            // Unmake itself is unchanged — Radiant buys the stat line only.
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
                skill_cooldown_mult,
                effect_percent_mr_shred,
                effect_max_distance
            ]
        );
        self
    }

    fn apply_aura(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(caster) = player_ref.champion() else {
            return;
        };
        if !caster.is_alive() {
            return;
        }
        let caster_id = caster.id();
        let caster_team = caster.team();

        let range = sized_range(ctx, caster_id, self.effect_max_distance);
        let range_sq = range * range;

        let mut targets: Vec<usize> = Vec::new();
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive()
                || !entity_ref.is_champion()
                || entity_ref.team() == caster_team
            {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            targets.push(id);
        }

        for id in targets {
            // Both halves land in the same tick, so the target is never
            // observed without the buff.
            ctx.entity_remove_buff(id, self.unmake_buff);
            ctx.add_buff(
                id,
                &BuffV1 {
                    magic_resistance_mult: -self.effect_percent_mr_shred,
                    ..BuffV1::timed(self.unmake_buff, AURA_DURATION_TICKS)
                },
            );
        }

        self.refresh_cooldown = AURA_REFRESH_TICKS;
    }
}

impl Default for AbyssalMask {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for AbyssalMask {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_aura(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_aura(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::MrDebuff,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::MagicResistance
    }
}
