use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks};

#[derive(Clone, Debug)]
pub struct ExecutionersCalling {
    price: usize,
    attack: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Default for ExecutionersCalling {
    fn default() -> Self {
        Self {
            price: 1300,
            attack: 45,
            effect_heal_reduce: 25,
            effect_duration_seconds: 2.0,
        }
    }
}

impl ExecutionersCalling {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [price, attack, effect_heal_reduce, effect_duration_seconds]
        );
        item
    }
}

impl StableItem for ExecutionersCalling {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "executioners_calling".to_string()
    }

    fn icon(&self) -> String {
        "executioners_calling".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ironsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageTypeV1::Ad {
            return;
        }

        let already_reduced = has_buff(&entity_ref, "25_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                &BuffV1 {
                    heal_reduce: self.effect_heal_reduce,
                    ..BuffV1::timed("25_percent_heal_cut", ticks(self.effect_duration_seconds))
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::HealReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
