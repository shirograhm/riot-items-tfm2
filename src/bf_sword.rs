use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct BFSword;

impl ModItemInfo for BFSword {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bf_sword"
    }

    fn icon(&self) -> &str {
        "t6_4"
    }

    fn price(&self) -> usize {
        800
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["infinity_edge".to_string(), "deathblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 65,
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
