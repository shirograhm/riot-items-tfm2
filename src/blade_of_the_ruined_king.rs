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
            vamp: 5,
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
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let hp = entity_ref.hp();
        let bonus_damage = hp.current * 5 / 100; // 5% current hp
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
        1950
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
            attack_speed_mult: 35,
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
        damage_type: DamageType,
    ) {
        // Only proc on base attacks, not skills or dots
        if damage_type != DamageType::AD {
            return;
        }
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let hp = entity_ref.hp();
        let bonus_damage = hp.current * 8 / 100; // 8% current hp
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
