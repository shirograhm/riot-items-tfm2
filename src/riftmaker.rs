use mod_api::*;

use crate::percent_of;

#[derive(Default, Clone, Debug)]
pub struct Riftmaker;

impl ModItemInfo for Riftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_2"
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
            "spirit_crystal".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 400,
            magic_power: 75,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        // Heal the player for 10 + 1% of their max HP on skill hit
        let heal_amount = 10
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.hp().max, 1.0))
                .unwrap_or(0);
        ctx.heal(caster, caster, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantRiftmaker;

impl ModItemInfo for RadiantRiftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_3"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 600,
            magic_power: 150,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        // Heal the player for 25 + 3% of their max HP on skill hit
        let heal_amount = 25
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.hp().max, 3.0))
                .unwrap_or(0);
        ctx.heal(caster, caster, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
