use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct BladeOfTheRuinedKing;

impl ModItemInfo for BladeOfTheRuinedKing {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "blade_of_the_ruined_king"
    }

    fn icon(&self) -> &str {
        "t7_4"
    }

    fn price(&self) -> usize {
        1400
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string(), "ruinous_blade".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 50,
            attack_speed_mult: 25,
            vamp: 10,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantBladeOfTheRuinedKing;

impl ModItemInfo for RadiantBladeOfTheRuinedKing {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_blade_of_the_ruined_king"
    }

    fn icon(&self) -> &str {
        "t7_5"
    }

    fn price(&self) -> usize {
        2000
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 50,
            attack_speed_mult: 30,
            vamp: 10,
            base_attack_enemy_max_hp_damage: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::Vamp,
            ItemTag::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
