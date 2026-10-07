use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, Spellblade};

/// Bloodsong — what World Atlas grows into, by way of Runic Compass:
/// Spellblade, a mark that makes its target take more damage, and the gold
/// the Atlas line pays ([`crate::SharedRiches`]).
#[derive(Clone, Debug)]
pub struct Bloodsong {
    meta: ItemMeta,
    vulnerable_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    spellblade: Spellblade,
}

impl Bloodsong {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("bloodsong", &["runic_compass"], &["radiant_bloodsong"]),
            vulnerable_buff: "bloodsong_vulnerable",
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 125,
            effect_cooldown_seconds: 1.5,
            effect_damaged_amplify: 7,
            effect_duration_seconds: 4.0,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            spellblade: Spellblade::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bloodsong", &["bloodsong"]),
            vulnerable_buff: "bloodsong_vulnerable",
            price: 750,
            hp: 300,
            hp_regen: 5,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 125,
            effect_cooldown_seconds: 1.5,
            effect_damaged_amplify: 7,
            effect_duration_seconds: 4.0,
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
                hp_regen,
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_cooldown_seconds,
                effect_damaged_amplify,
                effect_duration_seconds,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }

    /// What Shared Riches pays a holder: this much gold, this often.
    /// [`crate::SharedRiches`] does the paying, from the match hook.
    pub(crate) fn shared_riches(&self) -> (usize, f64) {
        (self.effect_bonus_gold, self.effect_gold_interval_seconds)
    }
}

impl Default for Bloodsong {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Bloodsong {
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
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade.reset();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !self.spellblade.is_ready() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.spellblade_damage(caster_ref.level());
        self.spellblade
            .spend(ctx, caster, target, self.effect_cooldown_seconds);

        // Vulnerable goes on ahead of the bonus, which can land as a hit of its own.
        if ctx.get_entity(target).is_some_and(|t| t.is_champion()) {
            refresh_buff(
                ctx,
                target,
                self.vulnerable_buff,
                &BuffV1 {
                    damaged_amplify: self.effect_damaged_amplify,
                    ..BuffV1::timed(self.vulnerable_buff, ticks(self.effect_duration_seconds))
                },
            );
        }
        Spellblade::on_hit_magic(ctx, caster, target, damage, damage_type, bonus_damage);
    }

    /// Watches for the cast that readies Spellblade.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.spellblade.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
