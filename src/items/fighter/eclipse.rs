use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Eclipse {
    meta: ItemMeta,
    mark_buff: &'static str,
    cooldown_buff: &'static str,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_hp_percent_damage: f64,
    effect_bonus_flat_shield: usize,
    effect_ad_percent_shield: f64,
    effect_duration_seconds: f64,
    effect_shield_seconds: f64,
    effect_cooldown_seconds: f64,
}

impl Eclipse {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("eclipse", &["caulfields_warhammer"], &["radiant_eclipse"]),
            mark_buff: "eclipse_mark",
            cooldown_buff: "eclipse_cooldown",
            price: 1300,
            attack: 55,
            skill_cooldown_mult: 15,
            effect_hp_percent_damage: 5.0,
            effect_bonus_flat_shield: 100,
            effect_ad_percent_shield: 15.0,
            effect_duration_seconds: 2.0,
            effect_shield_seconds: 2.0,
            effect_cooldown_seconds: 6.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_eclipse", &["eclipse"]),
            mark_buff: "radiant_eclipse_mark",
            cooldown_buff: "radiant_eclipse_cooldown",
            price: 1950,
            attack: 95,
            skill_cooldown_mult: 15,
            effect_hp_percent_damage: 8.0,
            effect_bonus_flat_shield: 120,
            effect_ad_percent_shield: 20.0,
            effect_duration_seconds: 2.0,
            effect_shield_seconds: 2.0,
            effect_cooldown_seconds: 6.0,
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
                attack,
                skill_cooldown_mult,
                effect_hp_percent_damage,
                effect_bonus_flat_shield,
                effect_ad_percent_shield,
                effect_duration_seconds,
                effect_shield_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for Eclipse {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Eclipse {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
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

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // `on_attack` rather than `on_skill_hit` because it is the one hook that
    // sees auto-attacks *and* ability hits, which is exactly the trigger set.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // Only the two trigger types the passive names. Leaving `Item` out is
        // what keeps the proc's own damage below from re-entering this hook,
        // and `Dot`/`Well` ticks from feeding the mark.
        if !matches!(attack_type, AttackTypeV1::BaseAttack | AttackTypeV1::Skill) {
            return;
        }

        // Everything read off an entity happens here, before the first `&mut
        // ctx` call: `StableEntity` borrows the sim immutably and the mutations
        // below need it back.
        let (marked, on_cooldown, target_max_hp, caster_attack) = {
            let Some(target_ref) = ctx.get_entity(target) else {
                return;
            };
            if !target_ref.is_champion() {
                return;
            }
            let Some(caster_ref) = ctx.get_entity(caster) else {
                return;
            };
            (
                has_buff(&target_ref, self.mark_buff),
                has_buff(&caster_ref, self.cooldown_buff),
                target_ref.hp().1,
                caster_ref.stat().attack,
            )
        };

        if !marked {
            // A hit that lands while the proc is still cooling down re-marks as
            // normal, so the mark is already up when the cooldown ends.
            ctx.add_buff(
                target,
                &BuffV1::timed(self.mark_buff, ticks(self.effect_duration_seconds)),
            );
            return;
        }

        if on_cooldown {
            return;
        }

        // Consume: one `entity_remove_buff` clears every instance, so the
        // duplicate marks a multi-hit cast can stack up (a fresh buff is not
        // visible for ~3 ticks, so the next hit inside that window re-marks
        // rather than reading it) all go at once.
        ctx.entity_remove_buff(target, self.mark_buff);

        let bonus_damage = percent_of(target_max_hp, self.effect_hp_percent_damage);
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackTypeV1::Item);

        let shield = self.effect_bonus_flat_shield
            + percent_of(caster_attack, self.effect_ad_percent_shield);
        ctx.entity_add_shield(caster, shield, ticks(self.effect_shield_seconds));

        ctx.add_buff(
            caster,
            &BuffV1::timed(self.cooldown_buff, ticks(self.effect_cooldown_seconds)),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
