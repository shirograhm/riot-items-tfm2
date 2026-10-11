use crate::config::ItemConfig;
use crate::{apply_config, Annul};
use mod_api_stable::*;

// Annul: Grants a Spell Shield that blocks the next enemy Ability (40 second
// cooldown). The shield itself lives in `crate::vfx::annul`; its cooldown carries
// into Banshee's Veil.

const NEXT: &str = "banshees_veil";
// The upgrade line the Annul cooldown is noted under (`crate::upgrade_carry`):
// this item's key, which Banshee's Veil and its Radiant note theirs under too.
const ANNUL_LINE: &str = "verdant_barrier";

#[derive(Clone, Debug)]
pub struct VerdantBarrier {
    price: usize,
    magic_power: i32,
    magic_resistance: i32,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    annul: Annul,
}

impl Default for VerdantBarrier {
    fn default() -> Self {
        Self {
            price: 400,
            magic_power: 35,
            magic_resistance: 25,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            annul: Annul::on_line(ANNUL_LINE, false),
        }
    }
}

impl VerdantBarrier {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [price, magic_power, magic_resistance, effect_cooldown_seconds]
        );
        item
    }
}

impl StableItem for VerdantBarrier {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "verdant_barrier".to_string()
    }

    fn icon(&self) -> String {
        "verdant_barrier".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![NEXT.to_string()]
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
        if next_key == NEXT {
            self.annul.carry()
        } else {
            0
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::MagicResistance]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
