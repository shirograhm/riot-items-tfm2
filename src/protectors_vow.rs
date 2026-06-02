use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct ProtectorsVow;

impl ModItemInfo for ProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_7"
    }

    fn price(&self) -> usize {
        1350
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 350,
            defence: 50,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantProtectorsVow;

impl ModItemInfo for RadiantProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_8"
    }

    fn price(&self) -> usize {
        1950
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}
