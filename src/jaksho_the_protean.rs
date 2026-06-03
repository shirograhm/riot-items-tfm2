use arrayvec::ArrayString;
use mod_api::*;

const JAKSHO_BUFF_DURATION: usize = 240;
const JAKSHO_MAX_STACKS: usize = 4;

#[derive(Default, Clone, Debug)]
pub struct JakshoTheProtean;

impl ModItemInfo for JakshoTheProtean {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "jaksho_the_protean"
    }

    fn icon(&self) -> &str {
        "t8_5"
    }

    fn price(&self) -> usize {
        1400
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["gatekeepers_armor".to_string(), "night_hood".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_jaksho_the_protean".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 300,
            defence: 40,
            magic_resistance: 65,
            ..Default::default()
        }
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
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "jaksho_the_protean_stack")
            .count();
        if stack_count < JAKSHO_MAX_STACKS {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: JAKSHO_BUFF_DURATION,
                    },
                    defence_mult: 6,
                    magic_resistance_mult: 6,
                    name: ArrayString::try_from("jaksho_the_protean_stack").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantJakshoTheProtean;

impl ModItemInfo for RadiantJakshoTheProtean {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_jaksho_the_protean"
    }

    fn icon(&self) -> &str {
        "t8_6"
    }

    fn price(&self) -> usize {
        2000
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["jaksho_the_protean".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 550,
            defence: 65,
            magic_resistance: 65,
            ..Default::default()
        }
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
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "radiant_jaksho_the_protean_stack")
            .count();
        if stack_count < JAKSHO_MAX_STACKS {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: JAKSHO_BUFF_DURATION,
                    },
                    defence_mult: 10,
                    magic_resistance_mult: 10,
                    name: ArrayString::try_from("radiant_jaksho_the_protean_stack").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
