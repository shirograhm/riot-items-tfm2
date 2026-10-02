use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, Annul, ItemMeta};

/// What this is built from, which is also the upgrade line the Annul cooldown
/// is noted under (`crate::upgrade_carry`). Both tiers take over the cooldown of
/// the item they replace.
const ANNUL_LINE: &str = "verdant_barrier";

// Annul: Grants a Spell Shield that blocks the next enemy Ability (40 second
// cooldown). The shield itself lives in `crate::vfx::annul`; Verdant Barrier's
// cooldown carries into it, and it carries into the Radiant.

#[derive(Clone, Debug)]
pub struct BansheesVeil {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    magic_resistance: i32,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    annul: Annul,
}

impl BansheesVeil {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "banshees_veil",
                &[ANNUL_LINE],
                &["radiant_banshees_veil"],
            ),
            price: 700,
            magic_power: 60,
            magic_resistance: 40,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            annul: Annul::on_line(ANNUL_LINE, true),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_banshees_veil", &["banshees_veil"]),
            price: 950,
            magic_power: 100,
            magic_resistance: 60,
            effect_cooldown_seconds: 40.0,
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
            [price, magic_power, magic_resistance, effect_cooldown_seconds]
        );
        self
    }
}

impl Default for BansheesVeil {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for BansheesVeil {
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
            magic_power: self.magic_power,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.annul.reset(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.annul.update(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        self.annul.on_damaged(
            ctx,
            player,
            entity,
            attacker,
            damage,
            attack_type,
            self.effect_cooldown_seconds,
        );
    }

    fn on_cc(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize, caster: usize) {
        self.annul.on_cc(ctx, player, caster, self.effect_cooldown_seconds);
    }

    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.annul.carry()
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.annul.resume(carry);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::MagicResistance]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
