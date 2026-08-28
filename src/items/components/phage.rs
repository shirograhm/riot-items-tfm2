use crate::config::ItemConfig;
use crate::{apply_config, has_buff, is_enemy_champion, ticks};
use mod_api_stable::*;

const RAGE_BUFF: &str = "phage_rage";

#[derive(Clone, Debug)]
pub struct Phage {
    price: usize,
    hp: i32,
    attack: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl Default for Phage {
    fn default() -> Self {
        Self {
            price: 950,
            hp: 200,
            attack: 30,
            effect_move_speed_mult: 5,
            effect_duration_seconds: 2.0,
        }
    }
}

impl Phage {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                attack,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        item
    }

    fn grant_rage(&mut self, ctx: &mut StableSim<'_>, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if has_buff(&caster_ref, RAGE_BUFF) {
            ctx.entity_remove_buff(caster, RAGE_BUFF);
        }
        ctx.add_buff(
            caster,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(RAGE_BUFF, ticks(self.effect_duration_seconds))
            },
        );
    }
}

impl StableItem for Phage {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "phage".to_string()
    }

    fn icon(&self) -> String {
        "phage".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "hardened_heart".to_string(),
            "soldiers_longsword".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "frozen_mallet".to_string(),
            "overlords_bloodmail".to_string(),
            "black_cleaver".to_string(),
            "steraks_gage".to_string(),
            "trinity_force".to_string(),
            "spear_of_shojin".to_string(),
            "sundered_sky".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            attack: self.attack,
            ..Default::default()
        }
    }

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
        if attack_type == AttackTypeV1::BaseAttack && is_enemy_champion(ctx, caster, target) {
            self.grant_rage(ctx, caster, target);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Hp, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
