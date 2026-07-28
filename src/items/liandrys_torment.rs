use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta};

/// Ticks between burn damage instances. A burn deals its total in
/// `duration / BURN_TICK_RATE` equal instances.
const BURN_TICK_RATE: usize = 12;

#[derive(Clone, Debug)]
pub struct LiandrysTorment {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_duration_seconds: f64,
    /// Active burns, as `(entity_id, ticks_remaining, ticks_until_next_damage)`
    burns: Vec<(usize, usize, usize)>,
}

impl LiandrysTorment {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "liandrys_torment",
                &["haunting_guise"],
                &["radiant_liandrys_torment"],
            ),
            price: 1400,
            hp: 350,
            magic_power: 75,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
            burns: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_liandrys_torment", &["liandrys_torment"]),
            price: 2000,
            hp: 550,
            magic_power: 150,
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
                effect_hp_percent_damage,
                effect_minion_damage_cap,
                effect_duration_seconds
            ]
        );
        self
    }

    /// How long a burn lasts, in ticks.
    fn duration_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds).max(BURN_TICK_RATE)
    }

    /// How many damage instances one burn is split into.
    fn instance_count(&self) -> usize {
        (self.duration_ticks() / BURN_TICK_RATE).max(1)
    }

    /// Damage for a single instance against `id`, or `None` once the target is dead
    /// or gone — a burn that can no longer land is dropped rather than tracked.
    fn instance_damage(&self, ctx: &mut GameCtx, id: usize) -> Option<usize> {
        let entity_ref = ctx.get_entity(id)?;
        if !entity_ref.is_alive() {
            return None;
        }
        let total = percent_of(entity_ref.hp().max, self.effect_hp_percent_damage);
        let mut per_instance = total as f64 / self.instance_count() as f64;
        if !entity_ref.is_champion() {
            per_instance = per_instance.min(self.effect_minion_damage_cap as f64);
        }
        Some(per_instance.round() as usize)
    }

    /// Burns `target` for the full duration, refreshing a burn already running on
    /// it. A refresh resets only the remaining duration, never the tick cadence:
    /// re-arming the cadence on every hit would let an attack rate faster than
    /// `BURN_TICK_RATE` push the next instance out of reach indefinitely.
    fn apply_burn(&mut self, target: usize) {
        let duration = self.duration_ticks();
        match self.burns.iter_mut().find(|(id, _, _)| *id == target) {
            Some(burn) => burn.1 = duration,
            None => self.burns.push((target, duration, BURN_TICK_RATE)),
        }
    }

    /// Ages every burn by one tick, dealing damage to the ones that came due and
    /// dropping those that expired or lost their target.
    fn tick_burns(&mut self, ctx: &mut GameCtx, caster: usize) {
        let mut kept = Vec::with_capacity(self.burns.len());
        for (id, remaining, until_next) in std::mem::take(&mut self.burns) {
            let remaining = remaining.saturating_sub(1);
            let mut until_next = until_next.saturating_sub(1);
            if until_next == 0 {
                let Some(damage) = self.instance_damage(ctx, id) else {
                    continue;
                };
                ctx.deal_damage(caster, id, 0, damage, AttackType::Item);
                until_next = BURN_TICK_RATE;
            }
            if remaining > 0 {
                kept.push((id, remaining, until_next));
            }
        }
        self.burns = kept;
    }
}

impl Default for LiandrysTorment {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for LiandrysTorment {
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
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.burns.clear();
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.burns.is_empty() {
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_champion) = player_ref.champion() else {
            return;
        };
        let player_champion_id = player_champion.id();

        self.tick_burns(ctx, player_champion_id);
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        self.apply_burn(target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
