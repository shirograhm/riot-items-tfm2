use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct MortalReminder;

impl ModItemInfo for MortalReminder {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "mortal_reminder"
    }

    fn icon(&self) -> &str {
        "t7_8"
    }

    fn price(&self) -> usize {
        1100
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "executioners_calling".to_string(),
            "wind_dagger".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 55,
            attack_speed_mult: 25,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::HealReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time { tick: 180 },
                heal_reduce: 40,
                ..Default::default()
            },
        );
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantMortalReminder;

impl ModItemInfo for RadiantMortalReminder {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_mortal_reminder"
    }

    fn icon(&self) -> &str {
        "t7_9"
    }

    fn price(&self) -> usize {
        1750
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 70,
            attack_speed_mult: 40,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::HealReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time { tick: 180 },
                heal_reduce: 40,
                ..Default::default()
            },
        );
    }
}
