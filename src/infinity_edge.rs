use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct InfinityEdge;

impl ModItemInfo for InfinityEdge {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "infinity_edge"
    }

    fn icon(&self) -> &str {
        "t6_0"
    }

    fn price(&self) -> usize {
        1300
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bf_sword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_infinity_edge".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 80,
            crit_chance: 20,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantInfinityEdge;

impl ModItemInfo for RadiantInfinityEdge {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_infinity_edge"
    }

    fn icon(&self) -> &str {
        "t6_1"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["infinity_edge".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 120,
            crit_chance: 45,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
