use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct GrezsSpectralLantern {
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    skill_cooldown_mult: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
}

impl Default for GrezsSpectralLantern {
    fn default() -> Self {
        Self {
            price: 800,
            magic_power: 60,
            hp_regen: 6,
            skill_cooldown_mult: 10,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_hp_percent_of_damage: 4.0,
        }
    }
}

impl GrezsSpectralLantern {
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

impl StableItem for GrezsSpectralLantern {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "grezs_spectral_lantern".to_string()
    }

    fn icon(&self) -> String {
        "grezs_spectral_lantern".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_stone".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["spirit_of_the_spectral_wraith".to_string()]
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
