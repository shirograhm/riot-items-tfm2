use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, ticks, ItemMeta, AURA_DURATION_TICKS, AURA_REFRESH_TICKS,
    DISTANCE_UNITS_PER_RANGE,
};

/// Bandlepipes — Fanfare, a self-empower that turns into a short team aura.
///
/// # Why this is two buffs and not one
///
/// Fanfare is two different shapes of effect wearing one name. The movement
/// speed is a plain timed self-buff, set once when the trigger lands. The
/// attack speed is an *aura*: the tooltip says "while empowered, you and nearby
/// allied champions", so an ally who walks into range during the window has to
/// pick it up, and one who walks out has to lose it. A single buff applied at
/// trigger time would be a snapshot of whoever happened to be standing there.
///
/// So `fanfare_buff` is the timed self-buff and `anthem_buff` is re-applied on
/// the shared aura cycle ([`AURA_REFRESH_TICKS`]) for as long as
/// `fanfare_remaining` is non-zero, exactly as `LocketOfTheIronSolari` does for
/// Legion.
///
/// # The residue when Fanfare ends
///
/// `anthem_buff` outlives its last refresh by up to
/// `AURA_DURATION_TICKS - AURA_REFRESH_TICKS`, so the expiry tick clears it
/// explicitly rather than letting it drain. That clear can only reach who is in
/// range *now*, so an ally who left range mid-window keeps it for the remainder
/// of the buff — the same residue Legion has, and the reason the duration is
/// kept short.
///
/// # Both variants share the buff names
///
/// The passive is identical on base and Radiant (only the stat line grows), so
/// they deliberately share `fanfare_buff`/`anthem_buff`: same-name buffs stack,
/// and two instances granting the same amount would otherwise double it.
/// `ArdentCenser` shares `sanctify_buff` for the same reason.
#[derive(Clone, Debug)]
pub struct Bandlepipes {
    meta: ItemMeta,
    fanfare_buff: &'static str,
    anthem_buff: &'static str,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    skill_cooldown_mult: i32,
    effect_duration_seconds: f64,
    effect_move_speed_mult: i32,
    effect_attack_speed_mult: i32,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    /// Ticks of Fanfare left; zero means the aura is not running.
    fanfare_remaining: usize,
    /// Ticks until the aura re-applies, on the shared cycle.
    refresh_cooldown: usize,
}

impl Bandlepipes {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "bandlepipes",
                &["aegis_of_the_legion"],
                &["radiant_bandlepipes"],
            ),
            fanfare_buff: "bandlepipes_fanfare",
            anthem_buff: "bandlepipes_anthem",
            price: 500,
            hp: 100,
            defence: 20,
            magic_resistance: 30,
            skill_cooldown_mult: 15,
            effect_duration_seconds: 4.0,
            effect_move_speed_mult: 12,
            effect_attack_speed_mult: 20,
            effect_max_distance: 100,
            // Non-vital stats (internals)
            fanfare_remaining: 0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bandlepipes", &["bandlepipes"]),
            price: 750,
            hp: 200,
            defence: 30,
            magic_resistance: 50,
            skill_cooldown_mult: 20,
            // Fanfare itself is unchanged — Radiant buys the stat line only.
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
                defence,
                magic_resistance,
                skill_cooldown_mult,
                effect_duration_seconds,
                effect_move_speed_mult,
                effect_attack_speed_mult,
                effect_max_distance
            ]
        );
        self
    }

    /// The carrier plus every living allied champion inside the aura, carrier
    /// first. Towers and minions are allied entities too, and Fanfare is not
    /// meant for them.
    fn anthem_targets(
        &self,
        ctx: &StableSim<'_>,
        caster_id: usize,
        caster_team: usize,
    ) -> Vec<usize> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut targets = vec![caster_id];
        for index in 0..ctx.entity_count() {
            let Some(entity_ref) = ctx.entity_at(index) else {
                continue;
            };
            let id = entity_ref.id();
            if id == caster_id {
                continue;
            }
            if !entity_ref.is_alive()
                || entity_ref.team() != caster_team
                || !entity_ref.is_champion()
            {
                continue;
            }
            if ctx.distance_sq(caster_id, id) > range_sq {
                continue;
            }
            targets.push(id);
        }
        targets
    }

    /// The carrier's champion id and team, if it exists and is alive.
    fn caster(ctx: &StableSim<'_>, player: usize) -> Option<(usize, usize)> {
        let champion = ctx.get_player(player)?.champion()?;
        champion
            .is_alive()
            .then(|| (champion.id(), champion.team()))
    }

    fn apply_anthem(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }
        let Some((caster_id, caster_team)) = Self::caster(ctx, player) else {
            return;
        };
        for id in self.anthem_targets(ctx, caster_id, caster_team) {
            // Replace rather than skip-if-present, both within this tick, so an
            // ally never spends part of the cycle without the bonus.
            ctx.entity_remove_buff(id, self.anthem_buff);
            ctx.add_buff(
                id,
                &BuffV1 {
                    attack_speed_mult: self.effect_attack_speed_mult,
                    ..BuffV1::timed(self.anthem_buff, AURA_DURATION_TICKS)
                },
            );
        }
        self.refresh_cooldown = AURA_REFRESH_TICKS;
    }

    fn clear_anthem(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some((caster_id, caster_team)) = Self::caster(ctx, player) else {
            return;
        };
        for id in self.anthem_targets(ctx, caster_id, caster_team) {
            ctx.entity_remove_buff(id, self.anthem_buff);
        }
    }
}

impl Default for Bandlepipes {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Bandlepipes {
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
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.fanfare_remaining = 0;
        self.refresh_cooldown = 0;
    }

    // Fanfare. `is_ally` false is the SDK's flag for an enemy-targeted skill,
    // and `on_skill_hit` only ever fires for this carrier's own casts, so the
    // trigger needs no team gate of its own.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if is_ally {
            return;
        }
        let Some(is_champion) = ctx.get_entity(target).map(|t| t.is_champion()) else {
            return;
        };
        if !is_champion {
            return;
        }

        let duration = ticks(self.effect_duration_seconds);
        self.fanfare_remaining = duration;
        // A second cast inside the window refreshes rather than stacks: one
        // `entity_remove_buff` clears every copy, so a multi-hit cast cannot
        // leave two on the carrier.
        ctx.entity_remove_buff(caster, self.fanfare_buff);
        ctx.add_buff(
            caster,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(self.fanfare_buff, duration)
            },
        );
        // Start the aura on this tick instead of waiting out the cycle the
        // previous window left behind.
        self.refresh_cooldown = 0;
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.fanfare_remaining == 0 {
            return;
        }
        self.fanfare_remaining -= 1;
        if self.fanfare_remaining == 0 {
            self.clear_anthem(ctx, player);
            return;
        }
        self.apply_anthem(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::MoveSpeed,
            ItemTagV1::AttackSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
