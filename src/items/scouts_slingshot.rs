use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

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
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for ScoutsSlingshot {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "scouts_slingshot"
    }

    fn icon(&self) -> &str {
        "scouts_slingshot"
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
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
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

        let is_cooldown_ticking = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "scouts_slingshot_cooldown");

        if !is_cooldown_ticking {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                    },
                    name: ArrayString::try_from("scouts_slingshot_cooldown").unwrap(),
                    ..Default::default()
                },
            );
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_flat_damage,
                AttackType::Item,
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
