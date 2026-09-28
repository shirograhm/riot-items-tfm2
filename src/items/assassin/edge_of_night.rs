use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, Annul, ItemMeta};

// Annul: Grants a Spell Shield that blocks the next enemy Ability (40 second
// cooldown). The shield itself lives in `crate::vfx::annul`, shared with Banshee's
// Veil.

#[derive(Clone, Debug)]
pub struct EdgeOfNight {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    hp: i32,
    effect_lethality: usize,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    annul: Annul,
}

impl EdgeOfNight {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "edge_of_night",
                &["serrated_dirk"],
                &["radiant_edge_of_night"],
            ),
            price: 750,
            attack: 35,
            hp: 100,
            effect_lethality: 15,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            annul: Annul::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_edge_of_night", &["edge_of_night"]),
            price: 1000,
            attack: 60,
            hp: 150,
            effect_lethality: 15,
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
            [price, attack, hp, effect_lethality, effect_cooldown_seconds]
        );
        self
    }
}

impl Default for EdgeOfNight {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for EdgeOfNight {
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
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if ctx.get_entity(target).is_some_and(|t| !t.is_tower()) {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.annul.reset();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.annul.update(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        self.annul.on_damaged(
            ctx,
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
        vec![ItemTagV1::Ad, ItemTagV1::Hp, ItemTagV1::DefensePenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
