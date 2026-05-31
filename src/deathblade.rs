use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct DeathBlade;

impl ModItemInfo for DeathBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "deathblade"
    }

    fn icon(&self) -> &str {
        "t6_2"
    }

    fn price(&self) -> usize {
        1400
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bf_sword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_deathblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 80,
            attack_mult: 15,
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
pub struct RadiantDeathBlade;

impl ModItemInfo for RadiantDeathBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_deathblade"
    }

    fn icon(&self) -> &str {
        "t6_3"
    }

    fn price(&self) -> usize {
        2000
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["deathblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 120,
            attack_mult: 25,
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
