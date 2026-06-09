use mod_api::*;

use crate::percent_of;

const BLADE_PERCENT_DAMAGE: f64 = 5.0;
const MAX_MINION_FLAT_DAMAGE: usize = 50;

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
        1450
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string(), "soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 50,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        // Don't apply this damage to towers
        if target_ref.is_tower() {
            return;
        }
        // Bonus damage equal to 5% of the target's current HP on hit
        let mut bonus_damage = percent_of(target_ref.hp().current, BLADE_PERCENT_DAMAGE);
        // If the target is not a champion, cap at 50 flat damage.
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, MAX_MINION_FLAT_DAMAGE);
        }
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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
        2100
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 60,
            attack_speed_mult: 50,
            vamp: 10,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        // Don't apply this damage to towers
        if target_ref.is_tower() {
            return;
        }
        // Bonus damage equal to 5% of the target's current HP on hit
        let mut bonus_damage = percent_of(target_ref.hp().current, BLADE_PERCENT_DAMAGE);
        // If the target is not a champion, cap at 50 flat damage.
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, MAX_MINION_FLAT_DAMAGE);
        }
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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
