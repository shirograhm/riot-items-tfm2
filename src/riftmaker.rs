use mod_api::*;

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
        1400
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
            magic_power: 60,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let hp = entity_ref.hp();
        let bonus_damage = hp.max * 6 / 100; // 6% max hp
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::HpPercentDamage]
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
        2100
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 500,
            magic_power: 100,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let hp = entity_ref.hp();
        let bonus_damage = hp.max * 6 / 100; // 6% max hp
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::HpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
