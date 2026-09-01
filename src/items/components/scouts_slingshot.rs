use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks};

#[derive(Clone, Debug)]
pub struct ScoutsSlingshot {
    price: usize,
    attack_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_cooldown_seconds: f64,
}

impl Default for ScoutsSlingshot {
    fn default() -> Self {
        Self {
            price: 800,
            attack_speed_mult: 30,
            effect_bonus_flat_damage: 40,
            effect_cooldown_seconds: 20.0,
        }
    }
}

impl ScoutsSlingshot {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                attack_speed_mult,
                effect_bonus_flat_damage,
                effect_cooldown_seconds
            ]
        );
        item
    }
}

impl StableItem for ScoutsSlingshot {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "scouts_slingshot".to_string()
    }

    fn icon(&self) -> String {
        "scouts_slingshot".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "diamond_tipped_spear".to_string(),
            "guinsoos_rageblade".to_string(),
            "mirage_blade".to_string(),
            "kraken_slayer".to_string(),
            "wits_end".to_string(),
            "experimental_hexplate".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack_speed_mult: self.attack_speed_mult,
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
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // Damaging an enemy champion deals 40 bonus magic damage (20 second cooldown).
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let is_cooldown_ticking = has_buff(&caster_ref, "scouts_slingshot_cooldown");

        if !is_cooldown_ticking {
            ctx.add_buff(
                caster,
                &BuffV1::timed(
                    "scouts_slingshot_cooldown",
                    ticks(self.effect_cooldown_seconds),
                ),
            );
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_flat_damage,
                AttackTypeV1::Item,
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
