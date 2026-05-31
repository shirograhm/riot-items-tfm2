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
        1350
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
            magic_power: 135,
            skill_cooldown_mult: 15,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce, ItemTag::DotDamage]
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
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blackfire_torch".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 180,
            skill_cooldown_mult: 20,
            dot_amplify: 20,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce, ItemTag::DotDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
