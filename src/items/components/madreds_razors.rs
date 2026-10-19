use crate::config::ItemConfig;
use crate::{apply_config, is_monster, ProcQueue};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct MadredsRazors {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_bonus_magic_damage: usize,
    effect_bonus_flat_heal: i32,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl Default for MadredsRazors {
    fn default() -> Self {
        Self {
            price: 650,
            attack: 25,
            attack_speed_mult: 15,
            effect_bonus_magic_damage: 25,
            effect_bonus_flat_heal: 5,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }
}

impl MadredsRazors {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                attack,
                attack_speed_mult,
                effect_bonus_magic_damage,
                effect_bonus_flat_heal
            ]
        );
        item
    }
}

impl StableItem for MadredsRazors {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "madreds_razors".to_string()
    }

    fn icon(&self) -> String {
        "madreds_razors".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hunters_machete".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["feral_flare".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        if !ctx
            .get_entity(target)
            .is_some_and(|target_ref| is_monster(&target_ref))
        {
            return;
        }
        self.procs.on_hit_magic(
            ctx,
            target,
            damage,
            damage_type,
            is_crit,
            self.effect_bonus_magic_damage,
        );
        ctx.heal(caster, caster, self.effect_bonus_flat_heal.max(0) as usize);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
