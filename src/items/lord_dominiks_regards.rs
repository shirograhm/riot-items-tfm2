use mod_api::*;

use crate::config::ItemConfig;

// Giant Slayer: basic attacks deal a percentage of bonus damage that scales with
// the target's maximum health -- `effect_percent_bonus_damage` per
// `effect_hp_per_stack` HP, capped at `effect_max_percent_bonus`. Basic attacks
// are the only damage a mod can modify, so the amp is applied to `*damage` in
// `on_attack` (like diamond_tipped_spear's distance scaling). Stepwise per full
// HP bracket, matching the "for every 1000 maximum health" wording (see
// atmas_reckoning's Big Hands, which scales crit the same way).

/// Scales a basic attack's `damage` by Giant Slayer against `target`: one
/// `percent_per_step` bracket per `hp_per_step` of the target's maximum health,
/// capped at `max_percent`. Towers are skipped.
fn apply_giant_slayer(
    ctx: &mut GameCtx,
    target: usize,
    damage: &mut usize,
    percent_per_step: f64,
    hp_per_step: usize,
    max_percent: f64,
) {
    let Some(target_ref) = ctx.get_entity(target) else {
        return;
    };
    if target_ref.is_tower() {
        return;
    }
    let steps = (target_ref.hp().max / hp_per_step.max(1)) as f64;
    let bonus_percent = (percent_per_step * steps).min(max_percent);
    *damage = (*damage as f64 * (1.0 + bonus_percent / 100.0)).round() as usize;
}

#[derive(Clone, Debug)]
pub struct LordDominiksRegards {
    price: usize,
    attack: i32,
    crit_chance: i32,
    defence_penetration: usize,
    effect_percent_bonus_damage: f64,
    effect_hp_per_stack: usize,
    effect_max_percent_bonus: f64,
}

impl Default for LordDominiksRegards {
    fn default() -> Self {
        Self {
            price: 1450,
            attack: 45,
            crit_chance: 20,
            defence_penetration: 25,
            effect_percent_bonus_damage: 3.0,
            effect_hp_per_stack: 1000,
            effect_max_percent_bonus: 15.0,
        }
    }
}

impl LordDominiksRegards {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
            effect_percent_bonus_damage: cfg
                .effect_percent_bonus_damage
                .unwrap_or(d.effect_percent_bonus_damage),
            effect_hp_per_stack: cfg.effect_hp_per_stack.unwrap_or(d.effect_hp_per_stack),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
        }
    }
}

impl ModItemInfo for LordDominiksRegards {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "lord_dominiks_regards"
    }

    fn icon(&self) -> &str {
        "lord_dominiks_regards"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["last_whisper".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_lord_dominiks_regards".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_giant_slayer(
            ctx,
            target,
            damage,
            self.effect_percent_bonus_damage,
            self.effect_hp_per_stack,
            self.effect_max_percent_bonus,
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantLordDominiksRegards {
    price: usize,
    attack: i32,
    crit_chance: i32,
    defence_penetration: usize,
    effect_percent_bonus_damage: f64,
    effect_hp_per_stack: usize,
    effect_max_percent_bonus: f64,
}

impl Default for RadiantLordDominiksRegards {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 85,
            crit_chance: 25,
            defence_penetration: 35,
            effect_percent_bonus_damage: 3.0,
            effect_hp_per_stack: 1000,
            effect_max_percent_bonus: 15.0,
        }
    }
}

impl RadiantLordDominiksRegards {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
            effect_percent_bonus_damage: cfg
                .effect_percent_bonus_damage
                .unwrap_or(d.effect_percent_bonus_damage),
            effect_hp_per_stack: cfg.effect_hp_per_stack.unwrap_or(d.effect_hp_per_stack),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
        }
    }
}

impl ModItemInfo for RadiantLordDominiksRegards {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_lord_dominiks_regards"
    }

    fn icon(&self) -> &str {
        "radiant_lord_dominiks_regards"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["lord_dominiks_regards".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_giant_slayer(
            ctx,
            target,
            damage,
            self.effect_percent_bonus_damage,
            self.effect_hp_per_stack,
            self.effect_max_percent_bonus,
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
