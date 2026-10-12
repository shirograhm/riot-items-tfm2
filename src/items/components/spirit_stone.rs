use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, ProcQueue};

#[derive(Clone, Debug)]
pub struct SpiritStone {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl Default for SpiritStone {
    fn default() -> Self {
        Self {
            price: 400,
            hp: 100,
            magic_power: 20,
            effect_percent_bonus_damage: 10.0,
            effect_bonus_hp_percent_of_damage: 2.0,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }
}

impl SpiritStone {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                magic_power,
                effect_percent_bonus_damage,
                effect_bonus_hp_percent_of_damage
            ]
        );
        item
    }
}

impl StableItem for SpiritStone {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "spirit_stone".to_string()
    }

    fn icon(&self) -> String {
        "spirit_stone".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["grezs_spectral_lantern".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    // Butcher, the same passive as Grez's Spectral Lantern's: autos and
    // skills alike, with the heal taken off the whole hit, bonus included.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // The bonus is dealt through the engine, so it comes back around as an
        // `Item` hit. Without this it would butcher itself, forever.
        if attack_type == AttackTypeV1::Item {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !is_monster(&target_ref) {
            return;
        }

        let bonus = percent_of(*damage, self.effect_percent_bonus_damage);
        let heal = percent_of(*damage + bonus, self.effect_bonus_hp_percent_of_damage);

        // The heal lands with the hit; only the bonus damage waits, so it
        // reads as its own number on the monster.
        self.procs.push_magic(ctx, target, bonus);
        ctx.heal(caster, caster, heal);
    }

    // Lands the Butcher bonus whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
