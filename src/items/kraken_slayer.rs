use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct KrakenSlayer {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_max_percent_bonus: f64,
    effect_hp_percent_threshold: f64,
    effect_attack_interval: usize,
    // Qualifying basic attacks landed since the last proc.
    attack_count: usize,
}

impl Default for KrakenSlayer {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 45,
            attack_speed_mult: 25,
            move_speed_mult: 4,
            effect_bonus_flat_damage: 150,
            effect_max_percent_bonus: 75.0,
            effect_hp_percent_threshold: 25.0,
            effect_attack_interval: 3,
            attack_count: 0,
        }
    }
}

impl KrakenSlayer {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
            effect_attack_interval: cfg
                .effect_attack_interval
                .unwrap_or(d.effect_attack_interval),
            attack_count: 0,
        }
    }
}

fn bring_it_down_damage(
    ctx: &mut GameCtx,
    target: usize,
    flat: usize,
    max_percent_bonus: f64,
    hp_percent_threshold: f64,
) -> usize {
    let Some(target_ref) = ctx.get_entity(target) else {
        return 0;
    };
    let hp = target_ref.hp();
    if hp.max == 0 {
        return flat;
    }
    let hp_ratio = (hp.current as f64 / hp.max as f64).clamp(0.0, 1.0);
    let threshold = (hp_percent_threshold / 100.0).clamp(0.0, 1.0);
    let ratio = if threshold >= 1.0 {
        1.0
    } else {
        ((1.0 - hp_ratio) / (1.0 - threshold)).clamp(0.0, 1.0)
    };
    let scaling = 1.0 + (max_percent_bonus / 100.0) * ratio;
    (flat as f64 * scaling).round() as usize
}

fn tick_bring_it_down(
    ctx: &mut GameCtx,
    target: usize,
    attack_count: &mut usize,
    interval: usize,
    flat: usize,
    max_percent_bonus: f64,
    hp_percent_threshold: f64,
) -> usize {
    let is_tower = ctx
        .get_entity(target)
        .map(|t| t.is_tower())
        .unwrap_or(false);
    if is_tower {
        return 0;
    }

    *attack_count += 1;
    if *attack_count < interval.max(1) {
        return 0;
    }
    *attack_count = 0;
    bring_it_down_damage(ctx, target, flat, max_percent_bonus, hp_percent_threshold)
}

impl ModItemInfo for KrakenSlayer {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "kraken_slayer"
    }

    fn icon(&self) -> &str {
        "kraken_slayer"
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
        vec!["radiant_kraken_slayer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.attack_count = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let bonus = tick_bring_it_down(
            ctx,
            target,
            &mut self.attack_count,
            self.effect_attack_interval,
            self.effect_bonus_flat_damage,
            self.effect_max_percent_bonus,
            self.effect_hp_percent_threshold,
        );
        if bonus > 0 {
            ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantKrakenSlayer {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_max_percent_bonus: f64,
    effect_hp_percent_threshold: f64,
    effect_attack_interval: usize,
    attack_count: usize,
}

impl Default for RadiantKrakenSlayer {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 75,
            attack_speed_mult: 45,
            move_speed_mult: 4,
            effect_bonus_flat_damage: 150,
            effect_max_percent_bonus: 75.0,
            effect_hp_percent_threshold: 25.0,
            effect_attack_interval: 3,
            attack_count: 0,
        }
    }
}

impl RadiantKrakenSlayer {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
            effect_attack_interval: cfg
                .effect_attack_interval
                .unwrap_or(d.effect_attack_interval),
            attack_count: 0,
        }
    }
}

impl ModItemInfo for RadiantKrakenSlayer {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_kraken_slayer"
    }

    fn icon(&self) -> &str {
        "radiant_kraken_slayer"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["kraken_slayer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.attack_count = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let bonus = tick_bring_it_down(
            ctx,
            target,
            &mut self.attack_count,
            self.effect_attack_interval,
            self.effect_bonus_flat_damage,
            self.effect_max_percent_bonus,
            self.effect_hp_percent_threshold,
        );
        if bonus > 0 {
            ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
