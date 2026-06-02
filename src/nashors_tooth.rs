use mod_api::*;

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
        1400
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "wind_dagger".to_string(),
            "needlessly_large_rod".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_nashors_tooth".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: 135,
            attack_speed_mult: 35,
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
        // Nashor's on-hit magic damage
        let Some(player_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        let bonus_damage = 35 + entity_ref.stat().magic_power * 3 / 100; // 35 + 3% of AP as bonus magic damage
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
        2000
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
            attack_speed_mult: 50,
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
        // Nashor's on-hit magic damage
        let Some(player_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        let bonus_damage = 60 + entity_ref.stat().magic_power * 5 / 100; // 60 + 5% of AP as bonus magic damage
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
