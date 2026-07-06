use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct Shadowflame {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for Shadowflame {
    fn default() -> Self {
        Self {
            price: 1350,
            magic_power: 115,
            magic_resistance_penetration: 15,
            effect_hp_percent_threshold: 0.3,
        }
    }
}

impl Shadowflame {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
        }
    }
}

impl ModItemInfo for Shadowflame {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "shadowflame"
    }

    fn icon(&self) -> &str {
        "t11_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["needlessly_large_rod".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_shadowflame".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let current_hp_percent = target_ref.hp().current as f64 / target_ref.hp().max as f64;
        if current_hp_percent < self.effect_hp_percent_threshold && damage_type == DamageType::AP {
            *damage = (*damage as f64 * 1.2) as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::MRPenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantShadowflame {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for RadiantShadowflame {
    fn default() -> Self {
        Self {
            price: 1800,
            magic_power: 210,
            magic_resistance_penetration: 15,
            effect_hp_percent_threshold: 0.3,
        }
    }
}

impl RadiantShadowflame {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
        }
    }
}

impl ModItemInfo for RadiantShadowflame {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_shadowflame"
    }

    fn icon(&self) -> &str {
        "t11_9"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["shadowflame".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let current_hp_percent = target_ref.hp().current as f64 / target_ref.hp().max as f64;
        if current_hp_percent < self.effect_hp_percent_threshold && damage_type == DamageType::AP {
            *damage = (*damage as f64 * 1.2) as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::MRPenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
