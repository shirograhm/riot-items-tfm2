use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct Heartsteel {
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_cooldown_seconds: usize,
    accumulated_bonus_hp: i32,
}

impl Default for Heartsteel {
    fn default() -> Self {
        Self {
            price: 1500,
            hp: 500,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_cooldown_seconds: 15,
            accumulated_bonus_hp: 0,
        }
    }
}

impl Heartsteel {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_caster_hp_percent_damage: cfg
                .effect_caster_hp_percent_damage
                .unwrap_or(d.effect_caster_hp_percent_damage),
            effect_bonus_hp_percent_of_damage: cfg
                .effect_bonus_hp_percent_of_damage
                .unwrap_or(d.effect_bonus_hp_percent_of_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            accumulated_bonus_hp: d.accumulated_bonus_hp,
        }
    }
}

impl ModItemInfo for Heartsteel {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "heartsteel"
    }

    fn icon(&self) -> &str {
        "t10_1"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_heartsteel".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_bonus_hp <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            entity_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                hp: self.accumulated_bonus_hp,
                name: ArrayString::try_from("heartsteel_stack").unwrap(),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "heartsteel_cooldown");
        if is_cooldown_ticking {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.hp().max, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;

        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_cooldown_seconds * 60,
                },
                name: ArrayString::try_from("heartsteel_cooldown").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Permanent,
                hp: bonus_hp,
                name: ArrayString::try_from("heartsteel_stack").unwrap(),
                ..Default::default()
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantHeartsteel {
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_cooldown_seconds: usize,
    accumulated_bonus_hp: i32,
}

impl Default for RadiantHeartsteel {
    fn default() -> Self {
        Self {
            price: 2100,
            hp: 800,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_cooldown_seconds: 15,
            accumulated_bonus_hp: 0,
        }
    }
}

impl RadiantHeartsteel {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_caster_hp_percent_damage: cfg
                .effect_caster_hp_percent_damage
                .unwrap_or(d.effect_caster_hp_percent_damage),
            effect_bonus_hp_percent_of_damage: cfg
                .effect_bonus_hp_percent_of_damage
                .unwrap_or(d.effect_bonus_hp_percent_of_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            accumulated_bonus_hp: d.accumulated_bonus_hp,
        }
    }
}

impl ModItemInfo for RadiantHeartsteel {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_heartsteel"
    }

    fn icon(&self) -> &str {
        "t10_2"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["heartsteel".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_bonus_hp <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            entity_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                hp: self.accumulated_bonus_hp,
                name: ArrayString::try_from("radiant_heartsteel_stack").unwrap(),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "radiant_heartsteel_cooldown");
        if is_cooldown_ticking {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.hp().max, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;

        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_cooldown_seconds * 60,
                },
                name: ArrayString::try_from("radiant_heartsteel_cooldown").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Permanent,
                hp: bonus_hp,
                name: ArrayString::try_from("radiant_heartsteel_stack").unwrap(),
                ..Default::default()
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
