use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HuntersTalisman {
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
}

impl Default for HuntersTalisman {
    fn default() -> Self {
        Self {
            price: 500,
            magic_power: 20,
            hp_regen: 2,
            effect_percent_bonus_damage: 5.0,
            effect_bonus_hp_percent_of_damage: 1.0,
        }
    }
}

impl HuntersTalisman {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                magic_power,
                hp_regen,
                effect_percent_bonus_damage,
                effect_bonus_hp_percent_of_damage
            ]
        );
        item
    }
}

impl StableItem for HuntersTalisman {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hunters_talisman".to_string()
    }

    fn icon(&self) -> String {
        "hunters_talisman".to_string()
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
        vec!["spirit_stone".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    // Butcher. Only a basic attack's damage can be amplified — ability damage is
    // dealt by the game and never reaches a mod — so the bonus and the heal it
    // feeds both ride the auto-attack.
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
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !is_monster(&target_ref) {
            return;
        }

        *damage += percent_of(*damage, self.effect_percent_bonus_damage);
        let heal = percent_of(*damage, self.effect_bonus_hp_percent_of_damage);
        ctx.heal(caster, caster, heal);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
