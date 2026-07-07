use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct OblivionOrb {
    price: usize,
    magic_power: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Default for OblivionOrb {
    fn default() -> Self {
        Self {
            price: 500,
            magic_power: 50,
            effect_heal_reduce: 25,
            effect_duration_seconds: 2.0,
        }
    }
}

impl OblivionOrb {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for OblivionOrb {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "oblivion_orb"
    }

    fn icon(&self) -> &str {
        "t10_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["arcane_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["morellonomicon".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageType::AP {
            return;
        }

        let already_reduced = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "25_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: ArrayString::try_from("25_percent_heal_cut").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::HealReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
