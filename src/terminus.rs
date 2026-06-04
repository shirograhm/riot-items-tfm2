use arrayvec::ArrayString;
use mod_api::*;

const TERMINUS_BUFF_DURATION: usize = 240;
const TERMINUS_MAX_STACKS: usize = 4;
const TERMINUS_PEN_PER_STACK: usize = 4;

#[derive(Default, Clone, Debug)]
pub struct Terminus {
    flip_flop: bool,
}

impl ModItemInfo for Terminus {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "terminus"
    }

    fn icon(&self) -> &str {
        "t8_7"
    }

    fn price(&self) -> usize {
        1300
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string(), "wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_terminus".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 30,
            attack_speed_mult: 35,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.flip_flop = true;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let defenses_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| caster_entity_ref.buff_at(i).name.as_str() == "terminus_armor_pen_buff")
            .count();
        let resistance_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str() == "terminus_magic_resistance_pen_buff"
            })
            .count();
        if self.flip_flop {
            if defenses_stack_count < TERMINUS_MAX_STACKS {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: TERMINUS_BUFF_DURATION,
                        },
                        defence_penetration: TERMINUS_PEN_PER_STACK,
                        name: ArrayString::try_from("terminus_armor_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = false;
        } else {
            if resistance_stack_count < TERMINUS_MAX_STACKS {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: TERMINUS_BUFF_DURATION,
                        },
                        magic_resistance_penetration: TERMINUS_PEN_PER_STACK,
                        name: ArrayString::try_from("terminus_magic_resistance_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::DefensePenetration,
            ItemTag::MRPenetration,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantTerminus {
    flip_flop: bool,
}

impl ModItemInfo for RadiantTerminus {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_terminus"
    }

    fn icon(&self) -> &str {
        "t8_8"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["terminus".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 50,
            attack_speed_mult: 60,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.flip_flop = true;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let defenses_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str() == "radiant_terminus_armor_pen_buff"
            })
            .count();
        let resistance_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str()
                    == "radiant_terminus_magic_resistance_pen_buff"
            })
            .count();
        if self.flip_flop {
            if defenses_stack_count < TERMINUS_MAX_STACKS {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: TERMINUS_BUFF_DURATION,
                        },
                        defence_penetration: TERMINUS_PEN_PER_STACK,
                        name: ArrayString::try_from("radiant_terminus_armor_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = false;
        } else {
            if resistance_stack_count < TERMINUS_MAX_STACKS {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: TERMINUS_BUFF_DURATION,
                        },
                        magic_resistance_penetration: TERMINUS_PEN_PER_STACK,
                        name: ArrayString::try_from("radiant_terminus_magic_resistance_pen_buff")
                            .unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::DefensePenetration,
            ItemTag::MRPenetration,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
