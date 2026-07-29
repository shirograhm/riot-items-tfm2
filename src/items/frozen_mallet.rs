use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, percent_of, ticks, try_proc_on_hit, ItemMeta};

#[derive(Clone, Debug)]
pub struct FrozenMallet {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
    // Icathia's Curse, the radiant-only on-hit bonus. Zero on the base variant,
    // which skips the effect entirely.
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    on_hit_cooldown_seconds: f64,
}

impl FrozenMallet {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("frozen_mallet", &["phage"], &["radiant_frozen_mallet"]),
            price: 1400,
            hp: 400,
            attack: 40,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
            effect_bonus_flat_damage: 0,
            effect_caster_hp_percent_damage: 0.0,
            on_hit_cooldown_seconds: 0.75,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_frozen_mallet", &["frozen_mallet"]),
            price: 2000,
            hp: 600,
            attack: 60,
            effect_bonus_flat_damage: 20,
            effect_caster_hp_percent_damage: 3.0,
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
                attack,
                effect_slow_amount,
                effect_duration_seconds,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for FrozenMallet {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for FrozenMallet {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        let already_slowed = has_buff(&target_ref, "frozen_mallet_slow");

        // Icathia's Curse. `bonus_damage` is 0 on the base variant, which
        // short-circuits before the cooldown marker is ever stamped.
        let caster_hp_max = ctx.get_entity(caster).map(|c| c.hp().max).unwrap_or(0);
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_hp_max, self.effect_caster_hp_percent_damage);
        if bonus_damage > 0
            && try_proc_on_hit(
                ctx,
                target,
                "frozen_mallet_on_hit_cooldown",
                self.on_hit_cooldown_seconds,
            )
        {
            ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        }

        // Rime, shared by both variants: the slow does not stack with itself.
        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    move_speed_mult: -self.effect_slow_amount,
                    name: buff_name("frozen_mallet_slow"),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AD,
            ItemTag::MyHpPercentDamage,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
