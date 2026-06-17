use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct BladeOfTheRuinedKing {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,

    has_cooldown: bool,
    effect_cooldown_seconds: usize,
}

impl Default for BladeOfTheRuinedKing {
    fn default() -> Self {
        Self {
            price: 1450,
            attack: 50,
            attack_speed_mult: 25,
            effect_hp_percent_damage: 5.0,
            effect_minion_damage_cap: 50,

            has_cooldown: true,
            effect_cooldown_seconds: 1,
        }
    }
}

impl BladeOfTheRuinedKing {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_hp_percent_damage: cfg
                .effect_hp_percent_damage
                .unwrap_or(d.effect_hp_percent_damage),
            effect_minion_damage_cap: cfg
                .effect_minion_damage_cap
                .unwrap_or(d.effect_minion_damage_cap),

            has_cooldown: cfg.has_cooldown.unwrap_or(d.has_cooldown),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for BladeOfTheRuinedKing {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "blade_of_the_ruined_king"
    }

    fn icon(&self) -> &str {
        "t7_4"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wind_dagger".to_string(), "soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let mut bonus_damage = percent_of(
            target_ref.hp().current,
            self.effect_hp_percent_damage as f64,
        );
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, self.effect_minion_damage_cap);
        }

        if self.has_cooldown {
            let is_cooldown_ticking = (0..caster_ref.buff_count()).any(|i| {
                caster_ref.buff_at(i).name.as_str() == "blade_of_the_ruined_king_cooldown"
            });
            if !is_cooldown_ticking {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: self.effect_cooldown_seconds * 60,
                        },
                        name: ArrayString::try_from("blade_of_the_ruined_king_cooldown").unwrap(),
                        ..Default::default()
                    },
                );
                ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
            }
        } else {
            ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::HpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBladeOfTheRuinedKing {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    vamp: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,

    has_cooldown: bool,
    effect_cooldown_seconds: usize,
}

impl Default for RadiantBladeOfTheRuinedKing {
    fn default() -> Self {
        Self {
            price: 2100,
            attack: 60,
            attack_speed_mult: 50,
            vamp: 10,
            effect_hp_percent_damage: 5.0,
            effect_minion_damage_cap: 50,

            has_cooldown: true,
            effect_cooldown_seconds: 1,
        }
    }
}

impl RadiantBladeOfTheRuinedKing {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            vamp: cfg.vamp.unwrap_or(d.vamp),
            effect_hp_percent_damage: cfg
                .effect_hp_percent_damage
                .unwrap_or(d.effect_hp_percent_damage),
            effect_minion_damage_cap: cfg
                .effect_minion_damage_cap
                .unwrap_or(d.effect_minion_damage_cap),

            has_cooldown: cfg.has_cooldown.unwrap_or(d.has_cooldown),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantBladeOfTheRuinedKing {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_blade_of_the_ruined_king"
    }

    fn icon(&self) -> &str {
        "t7_5"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blade_of_the_ruined_king".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            vamp: self.vamp,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let mut bonus_damage = percent_of(
            target_ref.hp().current,
            self.effect_hp_percent_damage as f64,
        );
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, self.effect_minion_damage_cap);
        }

        if self.has_cooldown {
            let is_cooldown_ticking = (0..caster_ref.buff_count()).any(|i| {
                caster_ref.buff_at(i).name.as_str() == "radiant_blade_of_the_ruined_king_cooldown"
            });
            if !is_cooldown_ticking {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: self.effect_cooldown_seconds * 60,
                        },
                        name: ArrayString::try_from("radiant_blade_of_the_ruined_king_cooldown")
                            .unwrap(),
                        ..Default::default()
                    },
                );
                ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
            }
        } else {
            ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::Vamp,
            ItemTag::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
