use arrayvec::ArrayString;
use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct GuinsoosRageblade;

impl ModItemInfo for GuinsoosRageblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "guinsoos_rageblade"
    }

    fn icon(&self) -> &str {
        "t9_5"
    }

    fn price(&self) -> usize {
        1350
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "soldiers_longsword".to_string(),
            "arcane_crystal".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_guinsoos_rageblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 30,
            magic_power: 30,
            attack_speed_mult: 30,
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
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "guinsoos_rageblade_buff")
            .count();
        if stack_count < 4 {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time { tick: 240 },
                    attack_speed_mult: 8,
                    name: ArrayString::try_from("guinsoos_rageblade_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
        // Basic attacks deal 30 bonus magic damage
        ctx.deal_damage(caster, target, 0, 30, AttackType::BaseAttack);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantGuinsoosRageblade;

impl ModItemInfo for RadiantGuinsoosRageblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_guinsoos_rageblade"
    }

    fn icon(&self) -> &str {
        "t9_6"
    }

    fn price(&self) -> usize {
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["guinsoos_rageblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 50,
            magic_power: 50,
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
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "radiant_guinsoos_rageblade_buff")
            .count();
        if stack_count < 4 {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time { tick: 240 },
                    attack_speed_mult: 8,
                    name: ArrayString::try_from("radiant_guinsoos_rageblade_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
        // Basic attacks deal 30 bonus magic damage
        ctx.deal_damage(caster, target, 0, 30, AttackType::BaseAttack);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
