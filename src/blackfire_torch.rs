use arrayvec::ArrayString;
use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct BlackfireTorch;

impl ModItemInfo for BlackfireTorch {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "blackfire_torch"
    }

    fn icon(&self) -> &str {
        "t7_6"
    }

    fn price(&self) -> usize {
        1400
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["staff_of_rapture".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_blackfire_torch".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 130,
            skill_cooldown_mult: 15,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "blackfire_torch_buff")
            .count();
        if stack_count < 4 {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time { tick: 240 },
                    magic_power: 10,
                    name: ArrayString::try_from("blackfire_torch_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantBlackfireTorch;

impl ModItemInfo for RadiantBlackfireTorch {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_blackfire_torch"
    }

    fn icon(&self) -> &str {
        "t7_7"
    }

    fn price(&self) -> usize {
        2000
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blackfire_torch".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 160,
            skill_cooldown_mult: 25,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "radiant_blackfire_torch_buff")
            .count();
        if stack_count < 4 {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time { tick: 240 },
                    magic_power: 30,
                    name: ArrayString::try_from("radiant_blackfire_torch_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
