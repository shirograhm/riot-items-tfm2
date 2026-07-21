use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct WitsEnd {
    price: usize,
    attack_speed_mult: i32,
    magic_resistance: i32,
    toughness: usize,
    effect_bonus_magic_damage: usize,
    on_hit_cooldown_seconds: f64,
}

impl Default for WitsEnd {
    fn default() -> Self {
        Self {
            price: 1400,
            attack_speed_mult: 40,
            magic_resistance: 80,
            toughness: 20,
            effect_bonus_magic_damage: 45,
            on_hit_cooldown_seconds: 0.5,
        }
    }
}

impl WitsEnd {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            toughness: cfg.toughness.unwrap_or(d.toughness),
            effect_bonus_magic_damage: cfg
                .effect_bonus_magic_damage
                .unwrap_or(d.effect_bonus_magic_damage),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

fn apply_fray(
    ctx: &mut GameCtx,
    caster: usize,
    target: usize,
    magic_damage: usize,
    cooldown_ticks: usize,
    cooldown_buff: &str,
) {
    let Some(target_ref) = ctx.get_entity(target) else {
        return;
    };
    if target_ref.is_tower() {
        return;
    }
    let is_cooldown_ticking =
        (0..target_ref.buff_count()).any(|i| target_ref.buff_at(i).name.as_str() == cooldown_buff);
    if is_cooldown_ticking {
        return;
    }
    ctx.add_buff(
        target,
        BuffState {
            duration: BuffType::Time {
                tick: cooldown_ticks,
            },
            name: cooldown_buff.try_into().unwrap(),
            ..Default::default()
        },
    );
    ctx.deal_damage(caster, target, 0, magic_damage, AttackType::BaseAttack);
}

impl ModItemInfo for WitsEnd {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "wits_end"
    }

    fn icon(&self) -> &str {
        "wits_end"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["scouts_slingshot".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_wits_end".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            magic_resistance: self.magic_resistance,
            toughness: self.toughness,
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
        apply_fray(
            ctx,
            caster,
            target,
            self.effect_bonus_magic_damage,
            (self.on_hit_cooldown_seconds * 60.0).round() as usize,
            "wits_end_on_hit_cooldown",
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AS, ItemTag::MagicResistance, ItemTag::Toughness]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantWitsEnd {
    price: usize,
    attack_speed_mult: i32,
    magic_resistance: i32,
    toughness: usize,
    effect_bonus_magic_damage: usize,
    on_hit_cooldown_seconds: f64,
}

impl Default for RadiantWitsEnd {
    fn default() -> Self {
        Self {
            price: 2000,
            attack_speed_mult: 65,
            magic_resistance: 130,
            toughness: 30,
            effect_bonus_magic_damage: 45,
            on_hit_cooldown_seconds: 0.5,
        }
    }
}

impl RadiantWitsEnd {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            toughness: cfg.toughness.unwrap_or(d.toughness),
            effect_bonus_magic_damage: cfg
                .effect_bonus_magic_damage
                .unwrap_or(d.effect_bonus_magic_damage),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantWitsEnd {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_wits_end"
    }

    fn icon(&self) -> &str {
        "radiant_wits_end"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["wits_end".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            magic_resistance: self.magic_resistance,
            toughness: self.toughness,
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
        apply_fray(
            ctx,
            caster,
            target,
            self.effect_bonus_magic_damage,
            (self.on_hit_cooldown_seconds * 60.0).round() as usize,
            "wits_end_on_hit_cooldown",
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AS, ItemTag::MagicResistance, ItemTag::Toughness]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
