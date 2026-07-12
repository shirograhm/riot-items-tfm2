use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

// Spellblade: after landing an Ability on an enemy champion, the next basic
// attack deals bonus damage, gated by a self cooldown-buff. `spellblade_ready`
// is charged in `on_skill_hit` and consumed in `on_attack` (self state, since
// buffs cannot be removed to "spend" the charge).

#[derive(Clone, Debug)]
pub struct Sheen {
    price: usize,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl Default for Sheen {
    fn default() -> Self {
        Self {
            price: 500,
            attack_speed_mult: 15,
            skill_cooldown_mult: 10,
            effect_min_bonus_damage: 30,
            effect_max_bonus_damage: 85,
            effect_cooldown_seconds: 1.5,
            spellblade_ready: false,
        }
    }
}

impl Sheen {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_min_bonus_damage: cfg
                .effect_min_bonus_damage
                .unwrap_or(d.effect_min_bonus_damage),
            effect_max_bonus_damage: cfg
                .effect_max_bonus_damage
                .unwrap_or(d.effect_max_bonus_damage),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            spellblade_ready: false,
        }
    }

    // Bonus damage scales linearly from min (level 1) to max (level 12).
    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }
}

impl ModItemInfo for Sheen {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "sheen"
    }

    fn icon(&self) -> &str {
        "sheen"
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
        vec!["trinity_force".to_string(), "dusk_and_dawn".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.spellblade_ready = false;
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "sheen_spellblade_cooldown");
        if !on_cooldown {
            self.spellblade_ready = true;
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
        if !self.spellblade_ready {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.spellblade_damage(caster_ref.level());
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0).round() as usize,
                },
                name: ArrayString::try_from("sheen_spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AS, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
