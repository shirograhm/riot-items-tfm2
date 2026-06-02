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
        1350
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
            hp: 450,
            magic_power: 100,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(player_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(player_champ_ref) = player_ref.champion() else {
            return;
        };
        // Heal the player for 55 + 1% of their max HP on skill hit
        let hp = player_champ_ref.hp();
        let heal_amount = 55 + (hp.max * 1 / 100);
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
        1950
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
        let Some(player_ref) = ctx.get_player(caster) else {
            return;
        };
        let Some(player_champ_ref) = player_ref.champion() else {
            return;
        };
        // Heal the player for 90 + 3% of their max HP on skill hit
        let hp = player_champ_ref.hp();
        let heal_amount = 90 + (hp.max * 3 / 100);
        ctx.heal(caster, caster, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
