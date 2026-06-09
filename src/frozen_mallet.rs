use arrayvec::ArrayString;
use mod_api::*;

use crate::percent_of;

const FROZEN_MALLET_SLOW_DURATION: usize = 120;
const FROZEN_MALLET_SLOW_PERCENT: i32 = -25;

#[derive(Default, Clone, Debug)]
pub struct FrozenMallet;

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
        1300
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
            hp: 450,
            attack: 45,
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

        // Don't apply buff or damage to towers
        if target_ref.is_tower() {
            return;
        }

        // Mallet slow debuff
        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "frozen_mallet_slow");

        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: FROZEN_MALLET_SLOW_DURATION,
                    },
                    move_speed_mult: FROZEN_MALLET_SLOW_PERCENT,
                    name: ArrayString::try_from("frozen_mallet_slow").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantFrozenMallet;

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
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["frozen_mallet".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 600,
            attack: 60,
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
        let bonus_damage = 20 + percent_of(caster_ref.hp().max, 3.0);

        // Don't apply buff or damage to towers
        if target_ref.is_tower() {
            return;
        }

        // Mallet slow debuff
        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "frozen_mallet_slow");

        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: FROZEN_MALLET_SLOW_DURATION,
                    },
                    move_speed_mult: FROZEN_MALLET_SLOW_PERCENT,
                    name: ArrayString::try_from("frozen_mallet_slow").unwrap(),
                    ..Default::default()
                },
            );
        }

        // Mallet on attack damage: 20 + 3% of caster's max HP
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
