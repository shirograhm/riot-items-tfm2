use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct RabadonsDeathcap;

impl ModItemInfo for RabadonsDeathcap {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "rabadons_deathcap"
    }

    fn icon(&self) -> &str {
        "t6_5"
    }

    fn price(&self) -> usize {
        1500
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["needlessly_large_rod".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_rabadons_deathcap".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 165,
            magic_power_mult: 20,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantRabadonsDeathcap;

impl ModItemInfo for RadiantRabadonsDeathcap {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_rabadons_deathcap"
    }

    fn icon(&self) -> &str {
        "t6_6"
    }

    fn price(&self) -> usize {
        2250
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["rabadons_deathcap".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 220,
            magic_power_mult: 35,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
