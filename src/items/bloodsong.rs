use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Bloodsong {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    vulnerable_buff: &'static str,
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    spellblade_ready: bool,
}

impl Bloodsong {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "bloodsong",
                &["sheen", "bandleglass_mirror"],
                &["radiant_bloodsong"],
            ),
            vulnerable_buff: "bloodsong_vulnerable",
            price: 1050,
            attack_speed_mult: 15,
            hp: 250,
            hp_regen: 2,
            magic_power: 20,
            skill_cooldown_mult: 10,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 125,
            effect_cooldown_seconds: 3.5,
            effect_damaged_amplify: 8,
            effect_duration_seconds: 4.0,
            spellblade_ready: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bloodsong", &["bloodsong"]),
            vulnerable_buff: "radiant_bloodsong_vulnerable",
            price: 1500,
            hp: 450,
            hp_regen: 4,
            magic_power: 40,
            skill_cooldown_mult: 20,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                attack_speed_mult,
                hp,
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_cooldown_seconds,
                effect_damaged_amplify,
                effect_duration_seconds
            ]
        );
        self
    }

    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }
}

impl Default for Bloodsong {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Bloodsong {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
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
        let on_cooldown = has_buff(&caster_ref, "spellblade_cooldown");
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

        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: ticks(self.effect_cooldown_seconds),
                },
                name: ArrayString::try_from("spellblade_cooldown").unwrap(),
                ..Default::default()
            },
        );

        // Increase the target's damage taken, but only while it is an enemy
        // champion and does not already carry the debuff (so it never stacks
        // past the configured amplification).
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let already_vulnerable = has_buff(&target_ref, self.vulnerable_buff);
        if !already_vulnerable {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    damaged_amplify: self.effect_damaged_amplify,
                    name: ArrayString::try_from(self.vulnerable_buff).unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::HPRegen,
            ItemTag::AP,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
