use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta};

// Lifeline: Falling below 30% health grants a shield for 3 seconds that absorbs
// 330 - 605 (based on level) damage (90 second cooldown).
#[derive(Clone, Debug)]
pub struct ImmortalShieldbow {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_hp_percent_threshold: f64,
    effect_min_shield: usize,
    effect_max_shield: usize,
    effect_shield_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    lifeline_cooldown: usize,
}

impl ImmortalShieldbow {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "immortal_shieldbow",
                &["noonquiver"],
                &["radiant_immortal_shieldbow"],
            ),
            price: 750,
            attack: 45,
            crit_chance: 20,
            effect_hp_percent_threshold: 30.0,
            effect_min_shield: 330,
            effect_max_shield: 605,
            effect_shield_seconds: 3.0,
            effect_cooldown_seconds: 90.0,
            // Non-vital stats (internals)
            lifeline_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_immortal_shieldbow", &["immortal_shieldbow"]),
            price: 1050,
            attack: 65,
            crit_chance: 25,
            effect_hp_percent_threshold: 30.0,
            effect_min_shield: 330,
            effect_max_shield: 605,
            effect_shield_seconds: 3.0,
            effect_cooldown_seconds: 90.0,
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
                attack,
                crit_chance,
                effect_hp_percent_threshold,
                effect_min_shield,
                effect_max_shield,
                effect_shield_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    // Scales linearly from min (level 1) to max (level 12).
    fn shield_amount(&self, level: usize) -> usize {
        let per_level =
            ((self.effect_max_shield - self.effect_min_shield) as f64 / 11.0).round() as usize;
        self.effect_min_shield + level.saturating_sub(1) * per_level
    }
}

impl Default for ImmortalShieldbow {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ImmortalShieldbow {
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
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.lifeline_cooldown = 0;
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.lifeline_cooldown = self.lifeline_cooldown.saturating_sub(1);
    }

    // The host resolves the hit before this runs, so "falling below" is read
    // after the fact, as in `steraks_gage` and `locket_of_the_iron_solari`: the
    // shield lands on the tick the carrier crosses under the threshold. The
    // cooldown is an item-side tick counter, like Locket's Devotion, so a fast
    // second hit cannot slip past a buff the host has not shown yet. A lethal
    // hit arrives with the carrier already dead; shielding the corpse would
    // only burn the cooldown.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if self.lifeline_cooldown > 0 {
            return;
        }
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        if !entity_ref.is_alive() {
            return;
        }
        let (current_hp, max_hp) = entity_ref.hp();
        if current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }

        let shield = self.shield_amount(entity_ref.level());
        if shield == 0 {
            return;
        }

        ctx.entity_add_shield(entity, shield, ticks(self.effect_shield_seconds));
        self.lifeline_cooldown = ticks(self.effect_cooldown_seconds);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Shield]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
