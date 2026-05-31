use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct NeedlesslyLargeRod;

impl ModItemInfo for NeedlesslyLargeRod {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "needlessly_large_rod"
    }

    fn icon(&self) -> &str {
        "t6_9"
    }

    fn price(&self) -> usize {
        850
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["rabadons_deathcap".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 115,
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
