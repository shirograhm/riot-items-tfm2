use mod_api::*;

#[derive(Default, Clone, Debug)]
pub struct ExecutionersCalling;

impl ModItemInfo for ExecutionersCalling {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "executioners_calling"
    }

    fn icon(&self) -> &str {
        "t8_2"
    }

    fn price(&self) -> usize {
        500
    }

    fn tier(&self) -> usize {
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ironsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: 25,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HealReduce]
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
                duration: BuffType::Time { tick: 120 }, // ~2 seconds, adjust to taste
                heal_reduce: 25,                        // 25% healing reduction
                ..Default::default()
            },
        );
    }
}
