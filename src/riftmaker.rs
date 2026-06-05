use arrayvec::ArrayString;
use mod_api::*;

use crate::percent_of;

const RIFTMAKER_BUFF_TICK_DURATION: usize = 300;
const RIFTMAKER_BUFF_MAX_STACKS: usize = 3;
const RIFTMAKER_BUFF_POWER_PER_STACK_PERCENT: f64 = 3.0;

#[derive(Default, Clone, Debug)]
pub struct Riftmaker;

impl ModItemInfo for Riftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_2"
    }

    fn price(&self) -> usize {
        1300
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "spirit_crystal".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 400,
            magic_power: 75,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_power = ctx
            .get_entity(caster)
            .map(|e| percent_of(e.hp().max, RIFTMAKER_BUFF_POWER_PER_STACK_PERCENT))
            .unwrap_or(0);

        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| caster_ref.buff_at(i).name.as_str() == "riftmaker_magic_power_buff")
            .count();
        if stack_count < RIFTMAKER_BUFF_MAX_STACKS {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: RIFTMAKER_BUFF_TICK_DURATION,
                    },
                    magic_power: bonus_power as i32,
                    name: ArrayString::try_from("riftmaker_magic_power_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantRiftmaker;

impl ModItemInfo for RadiantRiftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_3"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 600,
            magic_power: 150,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_power = ctx
            .get_entity(caster)
            .map(|e| percent_of(e.hp().max, RIFTMAKER_BUFF_POWER_PER_STACK_PERCENT))
            .unwrap_or(0);

        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| {
                caster_ref.buff_at(i).name.as_str() == "radiant_riftmaker_magic_power_buff"
            })
            .count();
        if stack_count < RIFTMAKER_BUFF_MAX_STACKS {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: RIFTMAKER_BUFF_TICK_DURATION,
                    },
                    magic_power: bonus_power as i32,
                    name: ArrayString::try_from("radiant_riftmaker_magic_power_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
