use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct TrinityForce {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl TrinityForce {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "trinity_force",
                &["sheen", "phage"],
                &["radiant_trinity_force"],
            ),
            price: 1400,
            hp: 200,
            attack: 30,
            attack_speed_mult: 15,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 33,
            effect_ad_percent_damage: 33.0,
            effect_cooldown_seconds: 3.5,
            spellblade_ready: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_trinity_force", &["trinity_force"]),
            price: 2000,
            hp: 333,
            attack: 33,
            attack_speed_mult: 33,
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
                hp,
                attack,
                attack_speed_mult,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ad_percent_damage,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for TrinityForce {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for TrinityForce {
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
            hp: self.hp,
            attack: self.attack,
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
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);
        self.spellblade_ready = false;

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
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
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AD,
            ItemTag::AS,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
