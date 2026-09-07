use crate::config::ItemConfig;
use crate::{apply_config, is_enemy_champion, ticks, TICKS_PER_SECOND};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HauntingGuise {
    price: usize,
    magic_power: i32,
    hp: i32,
    effect_stack_percent_damage: f64,
    effect_stacks_per_second: usize,
    effect_max_stacks: usize,
    effect_out_of_combat_seconds: f64,
    // Non-vital stats (internals)
    /// Ticks of combat left before Madness falls off. Refreshed to the full
    /// window on every trade with an enemy champion, counted down in `update`.
    combat_ticks: usize,
    /// Madness stacks held, and the sub-second progress toward the next one.
    madness: usize,
    stack_progress: usize,
}

impl Default for HauntingGuise {
    fn default() -> Self {
        Self {
            price: 950,
            magic_power: 60,
            hp: 200,
            effect_stack_percent_damage: 2.0,
            effect_stacks_per_second: 1,
            effect_max_stacks: 3,
            effect_out_of_combat_seconds: 3.0,
            // Non-vital stats (internals)
            combat_ticks: 0,
            madness: 0,
            stack_progress: 0,
        }
    }
}

impl HauntingGuise {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                magic_power,
                hp,
                effect_stack_percent_damage,
                effect_stacks_per_second,
                effect_max_stacks,
                effect_out_of_combat_seconds
            ]
        );
        item
    }

    /// Restarts the combat window when the wielder and `other` are on opposite
    /// sides and `other` is a champion. Called from both directions: being hit
    /// by an enemy champion is as much "in combat with" one as hitting it.
    fn note_combat(&mut self, ctx: &mut StableSim<'_>, entity: usize, other: usize) {
        if is_enemy_champion(ctx, entity, other) {
            self.combat_ticks = ticks(self.effect_out_of_combat_seconds);
        }
    }

    /// One tick of progress toward the next Madness stack. Counted in
    /// stacks-per-second over [`TICKS_PER_SECOND`] rather than in whole seconds
    /// so the cadence stays configurable.
    fn build_madness(&mut self) {
        if self.madness >= self.effect_max_stacks {
            self.madness = self.effect_max_stacks;
            self.stack_progress = 0;
            return;
        }
        let per_second = TICKS_PER_SECOND as usize;
        self.stack_progress += self.effect_stacks_per_second;
        while self.stack_progress >= per_second {
            self.stack_progress -= per_second;
            self.madness = (self.madness + 1).min(self.effect_max_stacks);
        }
    }

    /// What Madness currently multiplies outgoing damage by — `1.0` unstacked.
    fn madness_multiplier(&self) -> f64 {
        1.0 + (self.madness as f64 * self.effect_stack_percent_damage) / 100.0
    }
}

impl StableItem for HauntingGuise {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "haunting_guise".to_string()
    }

    fn icon(&self) -> String {
        "haunting_guise".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["fated_ashes".to_string(), "hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "riftmaker".to_string(),
            "liandrys_torment".to_string(),
            "bloodletters_curse".to_string(),
            "dusk_and_dawn".to_string(),
            "grezs_spectral_lantern".to_string(),
            "rylais_crystal_scepter".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.combat_ticks = 0;
        self.madness = 0;
        self.stack_progress = 0;
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        if self.combat_ticks == 0 {
            // Madness is lost outright on leaving combat, not decayed a stack
            // at a time: the bar restarts from zero on the next fight.
            self.madness = 0;
            self.stack_progress = 0;
            return;
        }
        self.combat_ticks -= 1;
        self.build_madness();
    }

    /// Fires for every hit the wielder lands, basic attack or ability, so
    /// Madness amplifies all of their damage rather than just their autos.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let is_tower = ctx
            .get_entity(target)
            .is_some_and(|target_ref| target_ref.is_tower());

        // Read before `note_combat`, so the hit that opens a fight is amplified
        // by the stacks already earned rather than by the second it starts.
        if !is_tower && self.madness > 0 {
            *damage = (*damage as f64 * self.madness_multiplier()).round() as usize;
        }

        self.note_combat(ctx, caster, target);
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally {
            self.note_combat(ctx, caster, target);
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        self.note_combat(ctx, entity, attacker);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
