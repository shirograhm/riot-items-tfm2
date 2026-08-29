use crate::config::ItemConfig;
use crate::{apply_config, is_monster};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HuntersMachete {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_bonus_magic_damage: usize,
    effect_bonus_flat_heal: i32,
}

impl Default for HuntersMachete {
    fn default() -> Self {
        Self {
            price: 500,
            attack: 10,
            attack_speed_mult: 5,
            effect_bonus_magic_damage: 5,
            effect_bonus_flat_heal: 3,
        }
    }
}

impl HuntersMachete {
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

impl StableItem for HuntersMachete {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hunters_machete".to_string()
    }

    fn icon(&self) -> String {
        "hunters_machete".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["madreds_razors".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    // Maim. Monsters only at this tier, so the smite-like sustain is worth
    // nothing in a teamfight; `Feral Flare` is where it starts applying to
    // every basic attack.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !is_monster(&target_ref) {
            return;
        }

        ctx.deal_damage(
            caster,
            target,
            0,
            self.effect_bonus_magic_damage,
            AttackTypeV1::Item,
        );
        ctx.heal(caster, caster, self.effect_bonus_flat_heal.max(0) as usize);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
