use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct ExecutionersCalling {
    price: usize,
    attack: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: usize,
}

impl Default for ExecutionersCalling {
    fn default() -> Self {
        Self {
            price: 500,
            attack: 25,
            effect_heal_reduce: 25,
            effect_duration_seconds: 2,
        }
    }
}

impl ExecutionersCalling {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for ExecutionersCalling {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "executioners_calling"
    }

    fn icon(&self) -> &str {
        "t8_2"
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_duration_seconds * 60,
                },
                heal_reduce: self.effect_heal_reduce,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HealReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
