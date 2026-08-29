use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct ArdentCenser {
    meta: ItemMeta,
    /// Shared by both variants: Sanctify is a state on the ally rather than
    /// a per-carrier stack, and the two variants grant the same amount.
    sanctify_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_attack_speed_mult: i32,
    effect_enemy_max_hp_damage: usize,
    effect_duration_seconds: f64,
}

impl ArdentCenser {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "ardent_censer",
                &["bandleglass_mirror"],
                &["radiant_ardent_censer"],
            ),
            sanctify_buff: "ardent_censer_sanctify",
            price: 1000,
            hp: 200,
            hp_regen: 2,
            magic_power: 45,
            skill_cooldown_mult: 5,
            move_speed_mult: 5,
            effect_attack_speed_mult: 20,
            effect_enemy_max_hp_damage: 2,
            effect_duration_seconds: 6.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_ardent_censer", &["ardent_censer"]),
            price: 1700,
            hp: 400,
            hp_regen: 4,
            magic_power: 90,
            skill_cooldown_mult: 5,
            move_speed_mult: 5,
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
                magic_power,
                skill_cooldown_mult,
                move_speed_mult,
                effect_attack_speed_mult,
                effect_enemy_max_hp_damage,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for ArdentCenser {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ArdentCenser {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    // Sanctify. `is_ally` is the SDK's flag for an ally-targeted skill — a
    // heal, shield or buff — which is exactly the trigger, and `on_skill_hit`
    // only ever fires for this carrier's own casts. Re-applying is a remove
    // followed by an add rather than a `has_buff` gate: refreshing means
    // replacing the instance, and one `entity_remove_buff` clears every copy,
    // so a multi-hit cast cannot leave two on the same ally.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        // Self-casts count as ally-targeted, so the carrier has to be ruled
        // out explicitly: Sanctify only ever lands on someone else.
        if !is_ally || target == caster {
            return;
        }
        let Some(is_champion) = ctx.get_entity(target).map(|t| t.is_champion()) else {
            return;
        };
        if !is_champion {
            return;
        }

        ctx.entity_remove_buff(target, self.sanctify_buff);
        ctx.add_buff(
            target,
            &BuffV1 {
                attack_speed_mult: self.effect_attack_speed_mult,
                base_attack_enemy_max_hp_damage: self.effect_enemy_max_hp_damage,
                ..BuffV1::timed(self.sanctify_buff, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::MoveSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
