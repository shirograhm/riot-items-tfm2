use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, percent_of, ticks, DISTANCE_UNITS_PER_RANGE, DOT_TICK_RATE, TICKS_PER_SECOND,
};

#[derive(Clone, Debug)]
pub struct BamisCinder {
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_minion_percent: f64,
    effect_max_distance: usize,
    effect_out_of_combat_seconds: f64,
    // Non-vital stats (internals)
    /// Ticks of combat left before Immolate goes out. Refreshed to the full
    /// window on every hit taken or dealt, counted down in `update`.
    combat_ticks: usize,
    /// Ticks until the next scorch lands on everyone in range.
    until_next_tick: usize,
}

impl Default for BamisCinder {
    fn default() -> Self {
        Self {
            price: 800,
            hp: 300,
            effect_bonus_flat_damage: 15,
            effect_minion_percent: 200.0,
            effect_max_distance: 35,
            effect_out_of_combat_seconds: 3.0,
            // Non-vital stats (internals)
            combat_ticks: 0,
            until_next_tick: DOT_TICK_RATE,
        }
    }
}

impl BamisCinder {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                effect_bonus_flat_damage,
                effect_minion_percent,
                effect_max_distance,
                effect_out_of_combat_seconds
            ]
        );
        item
    }

    /// Relights the aura. Called from both directions: damage taken keeps
    /// Immolate burning exactly as damage dealt does.
    fn note_combat(&mut self) {
        self.combat_ticks = ticks(self.effect_out_of_combat_seconds);
    }

    /// How many scorches make up one second, so the stated per-second damage is
    /// what a champion standing in the aura for a second actually takes.
    fn ticks_per_second(&self) -> usize {
        ((TICKS_PER_SECOND as usize) / DOT_TICK_RATE).max(1)
    }

    /// One scorch's worth of damage for `id`, or `None` if it is not there to
    /// take it. Entity ids are recycled slots, so the target is re-read every
    /// tick rather than trusted from when the aura lit.
    fn tick_damage(&self, ctx: &mut StableSim<'_>, id: usize) -> Option<usize> {
        let entity_ref = ctx.get_entity(id)?;
        if !entity_ref.is_alive() {
            return None;
        }
        let per_second = if entity_ref.is_champion() {
            self.effect_bonus_flat_damage
        } else {
            percent_of(self.effect_bonus_flat_damage, self.effect_minion_percent)
        };
        Some((per_second as f64 / self.ticks_per_second() as f64).round() as usize)
    }

    fn nearby_enemies(&self, ctx: &StableSim<'_>, caster: usize, caster_team: usize) -> Vec<usize> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut targets = Vec::new();
        for index in 0..ctx.entity_count() {
            let Some(entity_ref) = ctx.entity_at(index) else {
                continue;
            };
            let id = entity_ref.id();
            // Towers are enemy entities too, and Immolate is not meant for them.
            if !entity_ref.is_alive() || entity_ref.is_tower() || entity_ref.team() == caster_team {
                continue;
            }
            if ctx.distance_sq(caster, id) > range_sq {
                continue;
            }
            targets.push(id);
        }
        targets
    }
}

impl StableItem for BamisCinder {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "bamis_cinder".to_string()
    }

    fn icon(&self) -> String {
        "bamis_cinder".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        // Sunfire Cape, under the key the game keeps it by.
        vec!["hourglass_of_eternity".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.combat_ticks = 0;
        self.until_next_tick = DOT_TICK_RATE;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.combat_ticks == 0 {
            return;
        }
        self.combat_ticks -= 1;

        self.until_next_tick = self.until_next_tick.saturating_sub(1);
        if self.until_next_tick > 0 {
            return;
        }
        self.until_next_tick = DOT_TICK_RATE;

        let Some((caster, caster_team)) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| (champion.id(), champion.team()))
        else {
            // A dead carrier burns nobody, and the aura relights from scratch
            // on their next fight.
            self.combat_ticks = 0;
            return;
        };

        for id in self.nearby_enemies(ctx, caster, caster_team) {
            let Some(damage) = self.tick_damage(ctx, id) else {
                continue;
            };
            if damage == 0 {
                continue;
            }
            ctx.deal_damage(caster, id, 0, damage, AttackTypeV1::Item);
        }
    }

    fn on_attack(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // The scorch is dealt through the engine, so it comes back around as an
        // `Item` hit. Without this the aura would relight itself, forever: one
        // fight would leave Immolate burning for the rest of the match.
        if attack_type == AttackTypeV1::Item {
            return;
        }
        self.note_combat();
    }

    fn on_skill_hit(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        _target: usize,
        is_ally: bool,
    ) {
        if !is_ally {
            self.note_combat();
        }
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _player: usize,
        _entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        self.note_combat();
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::DotDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
