use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, DOT_TICK_RATE};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct FatedAshes {
    price: usize,
    magic_power: i32,
    effect_bonus_flat_damage: usize,
    effect_duration_seconds: f64,
    effect_minion_percent: f64,
    /// Targets burning now, as `(entity, ticks left, ticks until the next tick)`.
    burns: Vec<(usize, usize, usize)>,
}

impl Default for FatedAshes {
    fn default() -> Self {
        Self {
            price: 500,
            magic_power: 50,
            effect_bonus_flat_damage: 15,
            effect_duration_seconds: 3.0,
            effect_minion_percent: 400.0,
            burns: Vec::new(),
        }
    }
}

impl FatedAshes {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                magic_power,
                effect_bonus_flat_damage,
                effect_duration_seconds,
                effect_minion_percent
            ]
        );
        item
    }

    fn duration_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds).max(DOT_TICK_RATE)
    }

    fn instance_count(&self) -> usize {
        (self.duration_ticks() / DOT_TICK_RATE).max(1)
    }

    /// One tick's worth of the burn, or `None` if the target is not there to
    /// take it.
    ///
    /// The stated total is what a champion takes over the full duration; a minion
    /// or monster takes the boosted share of it. Entity ids are recycled slots, so
    /// the target is re-read every tick rather than trusted from when it was lit.
    fn instance_damage(&self, ctx: &mut StableSim<'_>, id: usize) -> Option<usize> {
        let entity_ref = ctx.get_entity(id)?;
        if !entity_ref.is_alive() {
            return None;
        }
        let total = if entity_ref.is_champion() {
            self.effect_bonus_flat_damage
        } else {
            percent_of(self.effect_bonus_flat_damage, self.effect_minion_percent)
        };
        Some((total as f64 / self.instance_count() as f64).round() as usize)
    }

    fn apply_burn(&mut self, target: usize) {
        let duration = self.duration_ticks();
        match self.burns.iter_mut().find(|(id, _, _)| *id == target) {
            Some(burn) => burn.1 = duration,
            None => self.burns.push((target, duration, DOT_TICK_RATE)),
        }
    }

    fn tick_burns(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let mut kept = Vec::with_capacity(self.burns.len());
        for (id, remaining, until_next) in std::mem::take(&mut self.burns) {
            let remaining = remaining.saturating_sub(1);
            let mut until_next = until_next.saturating_sub(1);
            if until_next == 0 {
                let Some(damage) = self.instance_damage(ctx, id) else {
                    continue;
                };
                ctx.deal_damage(caster, id, 0, damage, AttackTypeV1::Item);
                until_next = DOT_TICK_RATE;
            }
            if remaining > 0 {
                kept.push((id, remaining, until_next));
            }
        }
        self.burns = kept;
    }
}

impl StableItem for FatedAshes {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "fated_ashes".to_string()
    }

    fn icon(&self) -> String {
        "fated_ashes".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        1
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["haunting_guise".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.burns.clear();
    }

    /// Inflame. Lights whatever the carrier's ability lands on.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if is_ally {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        self.apply_burn(target);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.burns.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| champion.id())
        else {
            // A dead carrier burns nobody, and holding the list for its next life
            // would relight ids that belong to the fight that has moved on.
            self.burns.clear();
            return;
        };
        self.tick_burns(ctx, caster);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::DotDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
