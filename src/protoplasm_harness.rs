use arrayvec::ArrayString;
use mod_api::*;

use crate::percent_of;

const PROTOPLASM_HARNESS_THRESHOLD: f64 = 40.0; // 40% HP threshold for activation
const PROTOPLASM_HARNESS_BUFF_DURATION: usize = 360; // 6 seconds in ticks
const PROTOPLASM_HARNESS_HP_PERCENT_BOOST: f64 = 25.0; // 25% HP boost
const PROTOPLASM_HARNESS_COOLDOWN: usize = 1800; // 30 seconds in ticks

#[derive(Default, Clone, Debug)]
pub struct ProtoplasmHarness;

impl ModItemInfo for ProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_3"
    }

    fn price(&self) -> usize {
        1000
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 400,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::CooltimeReduce, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let has_harness_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "protoplasm_harness_buff");
        let has_cooldown_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "protoplasm_harness_cooldown_buff");
        let hp_threshold = percent_of(entity_ref.hp().max, PROTOPLASM_HARNESS_THRESHOLD);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().current <= hp_threshold) {
            // Bonus HP granted is 400 + 25% of max HP
            let bonus_hp =
                400 + percent_of(entity_ref.hp().max, PROTOPLASM_HARNESS_HP_PERCENT_BOOST) as i32;
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: PROTOPLASM_HARNESS_BUFF_DURATION,
                    },
                    hp: bonus_hp,
                    name: ArrayString::try_from("protoplasm_harness_buff").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: PROTOPLASM_HARNESS_COOLDOWN,
                    },
                    name: ArrayString::try_from("protoplasm_harness_cooldown_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantProtoplasmHarness;

impl ModItemInfo for RadiantProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_4"
    }

    fn price(&self) -> usize {
        1600
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 800,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::CooltimeReduce, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let has_harness_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_protoplasm_harness_buff");
        let has_cooldown_buff: bool = (0..entity_ref.buff_count()).any(|i| {
            entity_ref.buff_at(i).name.as_str() == "radiant_protoplasm_harness_cooldown_buff"
        });
        let hp_threshold = percent_of(entity_ref.hp().max, PROTOPLASM_HARNESS_THRESHOLD);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().current <= hp_threshold) {
            // Bonus HP granted is 600 + 25% of max HP
            let bonus_hp =
                600 + percent_of(entity_ref.hp().max, PROTOPLASM_HARNESS_HP_PERCENT_BOOST) as i32;
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: PROTOPLASM_HARNESS_BUFF_DURATION,
                    },
                    hp: bonus_hp,
                    name: ArrayString::try_from("radiant_protoplasm_harness_buff").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: PROTOPLASM_HARNESS_COOLDOWN,
                    },
                    name: ArrayString::try_from("radiant_protoplasm_harness_cooldown_buff")
                        .unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}
