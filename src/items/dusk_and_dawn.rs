use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct DuskAndDawn {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_caster_ap_percent_heal: f64,
    effect_caster_hp_percent_heal: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl DuskAndDawn {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "dusk_and_dawn",
                &["sheen", "haunting_guise"],
                &["radiant_dusk_and_dawn"],
            ),
            price: 1400,
            hp: 200,
            magic_power: 60,
            attack_speed_mult: 15,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 3.5,
            // Non-vital stats (internals)
            spellblade_ready: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dusk_and_dawn", &["dusk_and_dawn"]),
            price: 2000,
            hp: 300,
            magic_power: 100,
            attack_speed_mult: 25,
            skill_cooldown_mult: 20,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 3.5,
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
                magic_power,
                attack_speed_mult,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_caster_ap_percent_heal,
                effect_caster_hp_percent_heal,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for DuskAndDawn {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DuskAndDawn {
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
            hp: self.hp,
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade_ready = false;
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        _is_ally: bool,
    ) {
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

    fn on_base_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
    ) {
        if !self.spellblade_ready {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);
        let heal_amount = percent_of(
            caster_ref.stat().magic_power,
            self.effect_caster_ap_percent_heal,
        ) + percent_of(caster_ref.hp().1, self.effect_caster_hp_percent_heal);

        ctx.deal_damage(caster, target, 0, bonus_damage, AttackTypeV1::Item);
        ctx.heal(caster, caster, heal_amount);

        self.spellblade_ready = false;
        ctx.add_buff(
            caster,
            &BuffV1::timed("spellblade_cooldown", ticks(self.effect_cooldown_seconds)),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
