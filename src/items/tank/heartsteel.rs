use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Heartsteel {
    meta: ItemMeta,
    stack_buff: &'static str,
    cooldown_buff: &'static str,
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_cooldown_seconds: f64,
    accumulated_bonus_hp: i32,
}

impl Heartsteel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "heartsteel",
                &["ring_of_reincarnation"],
                &["radiant_heartsteel"],
            ),
            stack_buff: "heartsteel_stack",
            cooldown_buff: "heartsteel_cooldown",
            price: 1500,
            hp: 500,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 12.0,
            effect_cooldown_seconds: 20.0,
            // Non-vital stats (internals)
            accumulated_bonus_hp: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_heartsteel", &["heartsteel"]),
            stack_buff: "heartsteel_stack",
            cooldown_buff: "heartsteel_cooldown",
            price: 2100,
            hp: 800,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 12.0,
            effect_cooldown_seconds: 20.0,
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
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_bonus_hp_percent_of_damage,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for Heartsteel {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Heartsteel {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            &BuffV1 {
                hp: self.accumulated_bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        let is_cooldown_ticking = has_buff(&caster_ref, self.cooldown_buff);
        if is_cooldown_ticking {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.hp().1, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;

        ctx.add_buff(
            caster,
            &BuffV1::timed(self.cooldown_buff, ticks(self.effect_cooldown_seconds)),
        );
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackTypeV1::Item);
        ctx.add_buff(
            caster,
            &BuffV1 {
                hp: bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    /// Colossal Consumption's banked HP is permanent, so it crosses the Radiant
    /// upgrade. Only the counter moves: the HP already granted this life is
    /// sitting on the champion as `heartsteel_stack` buffs, and `on_spawn`
    /// re-applies it from the carried total on the next respawn. The cast round
    /// trips exactly for every `i32`, so the total needs no clamping.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_bonus_hp as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_bonus_hp = carry as i32;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
