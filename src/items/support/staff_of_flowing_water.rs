use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct StaffOfFlowingWater {
    meta: ItemMeta,
    /// Shared by both variants: Rapids is a state on whoever holds it, and the
    /// two variants grant the same amount.
    rapids_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_magic_power: i32,
    effect_skill_cooldown_mult: i32,
    effect_duration_seconds: f64,
    // Non-vital stats (internals)
    carrier_refreshed_tick: Option<usize>,
}

impl StaffOfFlowingWater {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "staff_of_flowing_water",
                &["bandleglass_mirror", "forbidden_idol"],
                &["radiant_staff_of_flowing_water"],
            ),
            rapids_buff: "staff_of_flowing_water_rapids",
            price: 550,
            hp: 100,
            hp_regen: 1,
            magic_power: 30,
            skill_cooldown_mult: 10,
            effect_magic_power: 25,
            effect_skill_cooldown_mult: 10,
            effect_duration_seconds: 3.0,
            // Non-vital stats (internals)
            carrier_refreshed_tick: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_staff_of_flowing_water",
                &["staff_of_flowing_water"],
            ),
            price: 750,
            hp: 150,
            hp_regen: 2,
            magic_power: 50,
            skill_cooldown_mult: 15,
            effect_magic_power: 25,
            effect_skill_cooldown_mult: 10,
            effect_duration_seconds: 3.0,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_magic_power,
                effect_skill_cooldown_mult,
                effect_duration_seconds
            ]
        );
        self
    }

    fn rapids(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.effect_magic_power,
            skill_cooldown_mult: self.effect_skill_cooldown_mult,
            ..BuffV1::timed(self.rapids_buff, ticks(self.effect_duration_seconds))
        }
    }
}

impl Default for StaffOfFlowingWater {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for StaffOfFlowingWater {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.carrier_refreshed_tick = None;
    }

    // Rapids. Same trigger as `ardent_censer`'s Sanctify: `is_ally` marks an
    // ally-targeted skill — a heal, shield or buff — and self-casts count as
    // one, so the carrier is ruled out as a target explicitly.
    //
    // The target and the carrier both get the buff, refreshed rather than
    // stacked. A cast that lands on several allies arrives here once per ally
    // in the same tick, so the carrier's copy is refreshed only on the first of
    // them: one application per cast, however many allies it reached.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally || target == caster {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || !target_ref.is_alive() {
            return;
        }

        let buff = self.rapids();
        refresh_buff(ctx, target, self.rapids_buff, &buff);

        let tick = ctx.tick();
        if self.carrier_refreshed_tick != Some(tick) {
            refresh_buff(ctx, caster, self.rapids_buff, &buff);
            self.carrier_refreshed_tick = Some(tick);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
