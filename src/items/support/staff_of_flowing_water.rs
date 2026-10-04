use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, SelfCastWatch};

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
    self_cast: SelfCastWatch,
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
            self_cast: SelfCastWatch::default(),
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

    /// Rapids on `target` and on the carrier, refreshed rather than stacked.
    /// A cast that reaches several allies comes through here once per ally in
    /// the same tick, so the carrier's copy is refreshed only on the first of
    /// them: one application per cast, however many allies it reached.
    fn grant_rapids(&mut self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        let buff = self.rapids();
        refresh_buff(ctx, target, self.rapids_buff, &buff);

        let tick = ctx.tick();
        if self.carrier_refreshed_tick != Some(tick) {
            refresh_buff(ctx, caster, self.rapids_buff, &buff);
            self.carrier_refreshed_tick = Some(tick);
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
        self.self_cast.close();
    }

    // Rapids. Same trigger as `ardent_censer`'s Sanctify: `is_ally` marks an
    // ally-targeted skill — a heal, shield or buff — and self-casts count as
    // one, so the carrier is ruled out as a target. A self-cast may still have
    // healed an ally, as the Monk's heal does around them: that is watched for
    // in `update`.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally {
            return;
        }
        if target == caster {
            self.self_cast.open(ctx, caster);
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || !target_ref.is_alive() {
            return;
        }

        self.grant_rapids(ctx, caster, target);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let healed = self.self_cast.poll(ctx);
        if healed.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion_ref| champion_ref.id())
        else {
            return;
        };

        for target in healed {
            self.grant_rapids(ctx, caster, target);
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
