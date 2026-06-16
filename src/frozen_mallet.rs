use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct FrozenMallet {
    price: usize,
    hp: i32,
    attack: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: usize,
}

impl Default for FrozenMallet {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 450,
            attack: 45,
            effect_slow_amount: 25,
            effect_duration_seconds: 2,
        }
    }
}

impl FrozenMallet {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_slow_amount: cfg.effect_slow_amount.unwrap_or(d.effect_slow_amount),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for FrozenMallet {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "frozen_mallet"
    }

    fn icon(&self) -> &str {
        "t7_0"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "soldiers_longsword".to_string(),
            "ring_of_reincarnation".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_frozen_mallet".to_string()]
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
        _caster: usize,
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
        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "frozen_mallet_slow");
        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    move_speed_mult: -self.effect_slow_amount,
                    name: ArrayString::try_from("frozen_mallet_slow").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantFrozenMallet {
    price: usize,
    hp: i32,
    attack: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: usize,
    effect_bonus_flat_damage: i32,
    effect_caster_hp_percent_damage: f64,
}

impl Default for RadiantFrozenMallet {
    fn default() -> Self {
        Self {
            price: 1900,
            hp: 600,
            attack: 60,
            effect_slow_amount: 25,
            effect_duration_seconds: 2,
            effect_bonus_flat_damage: 20,
            effect_caster_hp_percent_damage: 3.0,
        }
    }
}

impl RadiantFrozenMallet {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_slow_amount: cfg.effect_slow_amount.unwrap_or(d.effect_slow_amount),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_caster_hp_percent_damage: cfg
                .effect_caster_hp_percent_damage
                .unwrap_or(d.effect_caster_hp_percent_damage),
        }
    }
}

impl ModItemInfo for RadiantFrozenMallet {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_frozen_mallet"
    }

    fn icon(&self) -> &str {
        "t7_1"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["frozen_mallet".to_string()]
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
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.effect_bonus_flat_damage as usize
            + percent_of(
                caster_ref.hp().max,
                self.effect_caster_hp_percent_damage as f64,
            );

        if target_ref.is_tower() {
            return;
        }

        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "frozen_mallet_slow");
        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    move_speed_mult: -self.effect_slow_amount,
                    name: ArrayString::try_from("frozen_mallet_slow").unwrap(),
                    ..Default::default()
                },
            );
        }

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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
