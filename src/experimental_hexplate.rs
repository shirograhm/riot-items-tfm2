use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct ExperimentalHexplate;

impl ModItemInfo for ExperimentalHexplate {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "experimental_hexplate"
    }

    fn icon(&self) -> &str {
        "t8_0"
    }

    fn price(&self) -> usize {
        1200
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string(), "wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_experimental_hexplate".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 350,
            attack_speed_mult: 35,
            ult_cooldown_mult: 15,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AS, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantExperimentalHexplate;

impl ModItemInfo for RadiantExperimentalHexplate {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_experimental_hexplate"
    }

    fn icon(&self) -> &str {
        "t8_1"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["experimental_hexplate".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 500,
            attack_speed_mult: 50,
            move_speed_mult: 5,
            ult_cooldown_mult: 25,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AS,
            ItemTag::MoveSpeed,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
