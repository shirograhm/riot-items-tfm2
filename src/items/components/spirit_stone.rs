use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct SpiritStone {
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    skill_cooldown_mult: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
}

impl Default for SpiritStone {
    fn default() -> Self {
        Self {
            price: 500,
            magic_power: 40,
            hp_regen: 4,
            skill_cooldown_mult: 5,
            effect_percent_bonus_damage: 15.0,
            effect_bonus_hp_percent_of_damage: 3.0,
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
                magic_power,
                hp_regen,
                skill_cooldown_mult,
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
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hunters_talisman".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["grezs_spectral_lantern".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            hp_regen: self.hp_regen,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
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
        vec![ItemTagV1::Ap, ItemTagV1::HpRegen, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
