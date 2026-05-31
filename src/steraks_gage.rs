use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct SteraksGage;

impl ModItemInfo for SteraksGage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "steraks_gage"
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
            "ring_of_reincarnation".to_string(),
            "soldiers_longsword".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_steraks_gage".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 450,
            attack: 35,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantSteraksGage;

impl ModItemInfo for RadiantSteraksGage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_steraks_gage"
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
        vec!["steraks_gage".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 600,
            attack: 40,
            self_max_hp_damage: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD, ItemTag::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
