use mod_api::*;

use crate::percent_of;

#[derive(Default, Clone, Debug)]
pub struct NashorsTooth;

impl ModItemInfo for NashorsTooth {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "nashors_tooth"
    }

    fn icon(&self) -> &str {
        "t8_3"
    }

    fn price(&self) -> usize {
        1450
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "needlessly_large_rod".to_string(),
            "wind_dagger".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_nashors_tooth".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 115,
            attack_speed_mult: 25,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let bonus_damage = 35
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.stat().magic_power, 3.0))
                .unwrap_or(0);
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantNashorsTooth;

impl ModItemInfo for RadiantNashorsTooth {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_nashors_tooth"
    }

    fn icon(&self) -> &str {
        "t8_4"
    }

    fn price(&self) -> usize {
        2050
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["nashors_tooth".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 180,
            attack_speed_mult: 40,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let bonus_damage = 50
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.stat().magic_power, 5.0))
                .unwrap_or(0);
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
