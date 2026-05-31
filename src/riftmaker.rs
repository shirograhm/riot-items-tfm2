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

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_champ_ref) = player_ref.champion() else {
            return;
        };
        // Make sure the killed unit is another champion, not a minion or monster
        let Some(target_ref) = ctx.get_player(entity) else {
            return;
        };
        let Some(_target_champ_ref) = target_ref.champion() else {
            return;
        };
        // Heal the player for 50 + 2% of their max HP
        let hp = player_champ_ref.hp();
        let heal_amount = 50 + (hp.max * 2 / 100);
        ctx.heal(player, entity, heal_amount);
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

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_champ_ref) = player_ref.champion() else {
            return;
        };
        // Make sure the killed unit is another champion, not a minion or monster
        let Some(target_ref) = ctx.get_player(entity) else {
            return;
        };
        let Some(_target_champ_ref) = target_ref.champion() else {
            return;
        };
        // Heal the player for 100 + 5% of their max HP
        let hp = player_champ_ref.hp();
        let heal_amount = 100 + (hp.max * 5 / 100);
        ctx.heal(player, entity, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
