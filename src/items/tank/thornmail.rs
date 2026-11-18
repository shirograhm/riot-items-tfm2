use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_enemy_champion, percent_of, refresh_buff, ticks, ItemMeta};

// The Grievous Wounds every 40% item puts up, under the one name so two of
// them refresh rather than stack.
const GRIEVOUS_BUFF: &str = "40_percent_heal_cut";

// Thorns: when struck by a basic attack, deal magic damage to the attacker and,
// if it is a champion, apply Grievous Wounds. The game's own Thornmail (the
// item now drawn as Shield of Molten Stone) cannot run code, so this is the
// mod's own, built from Bramble Vest like League's.
#[derive(Clone, Debug)]
pub struct Thornmail {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_defence_percent_damage: f64,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Thornmail {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "thornmail",
                &["black_knights_heavy_plate"],
                &["radiant_thornmail"],
            ),
            price: 700,
            hp: 150,
            defence: 50,
            effect_bonus_flat_damage: 15,
            effect_caster_defence_percent_damage: 10.0,
            effect_heal_reduce: 40,
            effect_duration_seconds: 3.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_thornmail", &["thornmail"]),
            price: 1000,
            hp: 250,
            defence: 70,
            effect_bonus_flat_damage: 25,
            effect_caster_defence_percent_damage: 15.0,
            effect_heal_reduce: 40,
            effect_duration_seconds: 3.0,
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
                effect_bonus_flat_damage,
                effect_caster_defence_percent_damage,
                effect_heal_reduce,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for Thornmail {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Thornmail {
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
            ..Default::default()
        }
    }

    // Basic attacks only, as League's. The returned damage goes in as Item
    // damage, so two Thornmails never answer each other.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack || attacker == entity {
            return;
        }
        let Some(armor) = ctx.get_entity(entity).map(|me| me.stat().defence) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        if !attacker_ref.is_alive() || attacker_ref.is_tower() {
            return;
        }

        let thorns = self.effect_bonus_flat_damage
            + percent_of(armor, self.effect_caster_defence_percent_damage);
        let champion = is_enemy_champion(ctx, entity, attacker);
        ctx.deal_damage(entity, attacker, 0, thorns, AttackTypeV1::Item);

        if champion {
            refresh_buff(
                ctx,
                attacker,
                GRIEVOUS_BUFF,
                &BuffV1 {
                    heal_reduce: self.effect_heal_reduce,
                    ..BuffV1::timed(GRIEVOUS_BUFF, ticks(self.effect_duration_seconds))
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::HealReduce,
            ItemTagV1::ReflectDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}
