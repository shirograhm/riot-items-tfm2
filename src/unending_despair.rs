use mod_api::*;

use crate::percent_of;

#[derive(Default, Clone, Debug)]
pub struct UnendingDespair;

impl ModItemInfo for UnendingDespair {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "unending_despair"
    }

    fn icon(&self) -> &str {
        "t9_1"
    }

    fn price(&self) -> usize {
        1450
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "steel_armor".to_string(),
            "ring_of_reincarnation".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_unending_despair".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 450,
            defence: 30,
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
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantUnendingDespair;

impl ModItemInfo for RadiantUnendingDespair {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_unending_despair"
    }

    fn icon(&self) -> &str {
        "t9_2"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["unending_despair".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 700,
            defence: 50,
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
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
