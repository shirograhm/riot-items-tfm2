use crate::config::ItemConfig;
use crate::{apply_config, Transmutation, TransmutationNumbers};
use mod_api_stable::*;

// Philosopher's Medallion: Hunter's Talisman's step up to Philosopher's Stone,
// beside Bami's Cinder, with the Stone's Transmutation (`crate::Transmutation`).
#[derive(Clone, Debug)]
pub struct PhilosophersMedallion {
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_skill_cooldown_mult: i32,
    effect_bonus_hp_regen: i32,
    effect_duration_seconds: f64,
    effect_hp_percent_damage: f64,
    effect_burn_duration_seconds: f64,
    effect_burn_damage_cap: usize,
    // Non-vital stats (internals)
    transmutation: Transmutation,
}

impl Default for PhilosophersMedallion {
    fn default() -> Self {
        Self {
            price: 650,
            hp: 200,
            defence: 10,
            magic_resistance: 10,
            effect_skill_cooldown_mult: 5,
            effect_bonus_hp_regen: 15,
            effect_duration_seconds: 10.0,
            effect_hp_percent_damage: 5.0,
            effect_burn_duration_seconds: 3.0,
            effect_burn_damage_cap: 150,
            // Non-vital stats (internals)
            transmutation: Transmutation::default(),
        }
    }
}

impl PhilosophersMedallion {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                defence,
                magic_resistance,
                effect_skill_cooldown_mult,
                effect_bonus_hp_regen,
                effect_duration_seconds,
                effect_hp_percent_damage,
                effect_burn_duration_seconds,
                effect_burn_damage_cap
            ]
        );
        item
    }

    fn transmutation_numbers(&self) -> TransmutationNumbers {
        TransmutationNumbers {
            haste: self.effect_skill_cooldown_mult,
            hp_regen: self.effect_bonus_hp_regen,
            seconds: self.effect_duration_seconds,
            burn_percent: self.effect_hp_percent_damage,
            burn_seconds: self.effect_burn_duration_seconds,
            burn_cap_per_second: self.effect_burn_damage_cap,
        }
    }
}

impl StableItem for PhilosophersMedallion {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "philosophers_medallion".to_string()
    }

    fn icon(&self) -> String {
        "philosophers_medallion".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hunters_talisman".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["philosophers_stone".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.transmutation.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.transmutation.update(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        _entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let numbers = self.transmutation_numbers();
        self.transmutation.on_damaged(ctx, attacker, damage, &numbers);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        let numbers = self.transmutation_numbers();
        self.transmutation.on_kill(ctx, entity, victim, &numbers);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::HpRegen,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
