use arrayvec::ArrayString;
use mod_api::*;

const ARDENT_CENSER_BUFF_DURATION: usize = 360;
const ARDENT_CENSER_FORCE_BUFF: i32 = 60;
const ARDENT_CENSER_ATTACK_SPEED_BUFF: i32 = 20;

#[derive(Default, Clone, Debug)]
pub struct ArdentCenser;

impl ModItemInfo for ArdentCenser {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "ardent_censer"
    }

    fn icon(&self) -> &str {
        "t9_7"
    }

    fn price(&self) -> usize {
        1100
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_ardent_censer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 100,
            hp_regen: 20,
            skill_cooldown_mult: 10,
            move_speed_mult: 10,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, caster: Option<usize>, target: usize, _heal: usize) {
        // fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(caster_id) = caster else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster_id) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_attack_speed_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "ardent_censer_attack_speed_buff");
        let is_force_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "ardent_censer_force_buff");

        // If same team, apply attack speed and adaptive force
        if caster_ref.team() == target_ref.team() {
            if !is_force_buff_applied {
                if target_ref.stat().magic_power > target_ref.stat().attack {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: ARDENT_CENSER_BUFF_DURATION,
                            },
                            magic_power: ARDENT_CENSER_FORCE_BUFF * 2,
                            name: ArrayString::try_from("ardent_censer_force_buff").unwrap(),
                            ..Default::default()
                        },
                    )
                } else {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: ARDENT_CENSER_BUFF_DURATION,
                            },
                            attack: ARDENT_CENSER_FORCE_BUFF,
                            name: ArrayString::try_from("ardent_censer_force_buff").unwrap(),
                            ..Default::default()
                        },
                    )
                }
            }

            if !is_attack_speed_buff_applied {
                ctx.add_buff(
                    target,
                    BuffState {
                        duration: BuffType::Time {
                            tick: ARDENT_CENSER_BUFF_DURATION,
                        },
                        attack_speed_mult: ARDENT_CENSER_ATTACK_SPEED_BUFF,
                        name: ArrayString::try_from("ardent_censer_attack_speed_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantArdentCenser;

impl ModItemInfo for RadiantArdentCenser {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_ardent_censer"
    }

    fn icon(&self) -> &str {
        "t9_8"
    }

    fn price(&self) -> usize {
        1650
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ardent_censer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 150,
            hp_regen: 30,
            skill_cooldown_mult: 20,
            move_speed_mult: 20,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, caster: Option<usize>, target: usize, _heal: usize) {
        // fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(caster_id) = caster else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster_id) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_attack_speed_buff_applied = (0..target_ref.buff_count()).any(|i| {
            target_ref.buff_at(i).name.as_str() == "radiant_ardent_censer_attack_speed_buff"
        });
        let is_force_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "radiant_ardent_censer_force_buff");

        // If same team, apply attack speed and adaptive force
        if caster_ref.team() == target_ref.team() {
            if !is_force_buff_applied {
                if target_ref.stat().magic_power > target_ref.stat().attack {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: ARDENT_CENSER_BUFF_DURATION,
                            },
                            magic_power: ARDENT_CENSER_FORCE_BUFF * 2,
                            name: ArrayString::try_from("radiant_ardent_censer_force_buff")
                                .unwrap(),
                            ..Default::default()
                        },
                    )
                } else {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: ARDENT_CENSER_BUFF_DURATION,
                            },
                            attack: ARDENT_CENSER_FORCE_BUFF,
                            name: ArrayString::try_from("radiant_ardent_censer_force_buff")
                                .unwrap(),
                            ..Default::default()
                        },
                    )
                }
            }

            if !is_attack_speed_buff_applied {
                ctx.add_buff(
                    target,
                    BuffState {
                        duration: BuffType::Time {
                            tick: ARDENT_CENSER_BUFF_DURATION,
                        },
                        attack_speed_mult: ARDENT_CENSER_ATTACK_SPEED_BUFF,
                        name: ArrayString::try_from("radiant_ardent_censer_attack_speed_buff")
                            .unwrap(),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
