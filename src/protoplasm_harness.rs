use mod_api::*;

use crate::percent_of;

#[derive(Clone, Debug)]
pub struct ProtoplasmHarness {
    healing_active: bool,
    healing_remaining_ticks: usize,
    heal_per_tick: usize,
    cooldown_remaining: usize,
}

impl Default for ProtoplasmHarness {
    fn default() -> Self {
        Self {
            healing_active: false,
            healing_remaining_ticks: 0,
            heal_per_tick: 0,
            cooldown_remaining: 0,
        }
    }
}

impl ModItemInfo for ProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_3"
    }

    fn price(&self) -> usize {
        1000
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 400,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        // Only trigger if not already healing and cooldown is ready
        if !self.healing_active && self.cooldown_remaining == 0 {
            if let Some(entity_ref) = ctx.get_entity(entity) {
                let hp = entity_ref.hp();
                let hp_threshold = percent_of(hp.max, 30.0);

                // If current HP is below 30% of max HP
                if hp.current < hp_threshold {
                    let missing_hp = hp.max - hp.current;
                    let heal_total = percent_of(missing_hp, 50.0);

                    // 3 seconds = 180 ticks (at 60 ticks/second)
                    self.healing_remaining_ticks = 180;
                    self.heal_per_tick = heal_total / 180;
                    self.healing_active = true;
                    // 30 seconds cooldown = 1800 ticks (at 60 ticks/second)
                    self.cooldown_remaining = 1800;
                }
            }
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.healing_active && self.healing_remaining_ticks > 0 {
            ctx.heal(player, player, self.heal_per_tick);
            self.healing_remaining_ticks -= 1;

            if self.healing_remaining_ticks == 0 {
                self.healing_active = false;
            }
        }

        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtoplasmHarness {
    healing_active: bool,
    healing_remaining_ticks: usize,
    heal_per_tick: usize,
    cooldown_remaining: usize,
}

impl Default for RadiantProtoplasmHarness {
    fn default() -> Self {
        Self {
            healing_active: false,
            healing_remaining_ticks: 0,
            heal_per_tick: 0,
            cooldown_remaining: 0,
        }
    }
}

impl ModItemInfo for RadiantProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_4"
    }

    fn price(&self) -> usize {
        1600
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 800,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        // Only trigger if not already healing and cooldown is ready
        if !self.healing_active && self.cooldown_remaining == 0 {
            if let Some(entity_ref) = ctx.get_entity(entity) {
                let hp = entity_ref.hp();
                let hp_threshold = percent_of(hp.max, 30.0);

                // If current HP is below 30% of max HP
                if hp.current < hp_threshold {
                    let missing_hp = hp.max - hp.current;
                    let heal_total = percent_of(missing_hp, 75.0); // 75% for radiant version

                    // 3 seconds = 180 ticks (at 60 ticks/second)
                    self.healing_remaining_ticks = 180;
                    self.heal_per_tick = heal_total / 180;
                    self.healing_active = true;
                    // 30 seconds cooldown = 1800 ticks (at 60 ticks/second)
                    self.cooldown_remaining = 1800;
                }
            }
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.healing_active && self.healing_remaining_ticks > 0 {
            ctx.heal(player, player, self.heal_per_tick);
            self.healing_remaining_ticks -= 1;

            if self.healing_remaining_ticks == 0 {
                self.healing_active = false;
            }
        }

        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }
    }
}
