use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, has_buff, keep_spellblade, mark_spellblade, percent_of, spend_spellblade, ticks,
    ItemMeta, ProcQueue,
};

#[derive(Clone, Debug)]
pub struct EssenceReaver {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    crit_chance: i32,
    effect_ad_percent_damage: f64,
    effect_crit_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
    procs: ProcQueue,
}

impl EssenceReaver {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "essence_reaver",
                &["sheen", "noonquiver"],
                &["radiant_essence_reaver"],
            ),
            price: 800,
            attack: 30,
            attack_speed_mult: 20,
            skill_cooldown_mult: 10,
            crit_chance: 20,
            effect_ad_percent_damage: 125.0,
            effect_crit_percent_damage: 100.0,
            effect_cooldown_seconds: 1.5,
            // Non-vital stats (internals)
            spellblade_ready: false,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_essence_reaver", &["essence_reaver"]),
            price: 1000,
            attack: 50,
            attack_speed_mult: 30,
            skill_cooldown_mult: 15,
            crit_chance: 25,
            effect_ad_percent_damage: 125.0,
            effect_crit_percent_damage: 100.0,
            effect_cooldown_seconds: 1.5,
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
                attack_speed_mult,
                skill_cooldown_mult,
                crit_chance,
                effect_ad_percent_damage,
                effect_crit_percent_damage,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for EssenceReaver {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for EssenceReaver {
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
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade_ready = false;
        self.procs.clear();
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || is_ally {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = has_buff(&caster_ref, "spellblade_cooldown");
        if !on_cooldown {
            self.spellblade_ready = true;
            mark_spellblade(ctx, caster);
        }
    }

    // The crit term reads like Hamstringer's "(+100% crit)": each point of
    // critical strike chance adds one point of bonus damage at 100%.
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
        if !self.spellblade_ready || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stat = caster_ref.stat();
        let bonus_damage = percent_of(stat.attack, self.effect_ad_percent_damage)
            + percent_of(stat.crit_chance, self.effect_crit_percent_damage);

        self.procs.push_physical(ctx, target, bonus_damage);
        ctx.add_buff(
            caster,
            &BuffV1::timed("spellblade_cooldown", ticks(self.effect_cooldown_seconds)),
        );
        self.spellblade_ready = false;
        spend_spellblade(ctx, caster, target);
    }

    /// Lands the Spellblade damage whose delay has run out, and keeps the
    /// Spellblade sparks up while it is ready.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
        keep_spellblade(ctx, player, self.spellblade_ready);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
