use arrayvec::ArrayString;
use mod_api::*;

use crate::force_to_ad;
use crate::force_to_ap;

const MIRAGE_BLADE_FORCE_BUFF: i32 = 60;
const MIRAGE_BLADE_FORCE_BUFF_RADIANT: i32 = 100;
const MIRAGE_BLADE_MOVE_SPEED: i32 = 20;
const MIRAGE_BLADE_MOVE_SPEED_DURATION: usize = 120;

#[derive(Default, Clone, Debug)]
pub struct MirageBlade;

impl ModItemInfo for MirageBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "mirage_blade"
    }

    fn icon(&self) -> &str {
        "t9_7"
    }

    fn price(&self) -> usize {
        1500
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_mirage_blade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: 40,
            move_speed_mult: 10,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_bonus_damage");

        if !is_buff_applied {
            // Check adaptive-ness
            if entity_ref.stat().magic_power > entity_ref.stat().attack {
                ctx.add_buff(
                    entity_ref.id(),
                    BuffState {
                        duration: BuffType::Permanent,
                        magic_power: force_to_ap(MIRAGE_BLADE_FORCE_BUFF),
                        name: ArrayString::try_from("mirage_blade_bonus_damage").unwrap(),
                        ..Default::default()
                    },
                )
            } else {
                ctx.add_buff(
                    entity_ref.id(),
                    BuffState {
                        duration: BuffType::Permanent,
                        attack: force_to_ad(MIRAGE_BLADE_FORCE_BUFF),
                        name: ArrayString::try_from("mirage_blade_bonus_damage").unwrap(),
                        ..Default::default()
                    },
                )
            }
        }
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_move_speed");

        if !is_buff_applied {
            // Check adaptive-ness
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Time {
                        tick: MIRAGE_BLADE_MOVE_SPEED_DURATION,
                    },
                    move_speed_mult: MIRAGE_BLADE_MOVE_SPEED,
                    name: ArrayString::try_from("mirage_blade_move_speed").unwrap(),
                    ..Default::default()
                },
            )
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantMirageBlade;

impl ModItemInfo for RadiantMirageBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_mirage_blade"
    }

    fn icon(&self) -> &str {
        "t9_8"
    }

    fn price(&self) -> usize {
        2100
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["mirage_blade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: 65,
            move_speed_mult: 15,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_bonus_damage");

        if !is_buff_applied {
            // Check adaptive-ness
            if entity_ref.stat().magic_power > entity_ref.stat().attack {
                ctx.add_buff(
                    entity_ref.id(),
                    BuffState {
                        duration: BuffType::Permanent,
                        magic_power: force_to_ap(MIRAGE_BLADE_FORCE_BUFF_RADIANT),
                        name: ArrayString::try_from("mirage_blade_bonus_damage").unwrap(),
                        ..Default::default()
                    },
                )
            } else {
                ctx.add_buff(
                    entity_ref.id(),
                    BuffState {
                        duration: BuffType::Permanent,
                        attack: force_to_ad(MIRAGE_BLADE_FORCE_BUFF_RADIANT),
                        name: ArrayString::try_from("mirage_blade_bonus_damage").unwrap(),
                        ..Default::default()
                    },
                )
            }
        }
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "mirage_blade_move_speed");

        if !is_buff_applied {
            // Check adaptive-ness
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Time {
                        tick: MIRAGE_BLADE_MOVE_SPEED_DURATION,
                    },
                    move_speed_mult: MIRAGE_BLADE_MOVE_SPEED,
                    name: ArrayString::try_from("mirage_blade_move_speed").unwrap(),
                    ..Default::default()
                },
            )
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
