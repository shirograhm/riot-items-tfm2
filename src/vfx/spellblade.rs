//! Spellblade: using an Ability empowers the next basic attack within
//! `WINDOW_SECONDS`. Shared by Sheen and everything built from it (Trinity
//! Force, Dusk and Dawn, Lich Bane, Essence Reaver, Bloodsong); each item owns
//! its numbers and deals its own bonus damage.
//!
//! No hook reports a cast, so one is read the way Zeke's Convergence reads its
//! ult: an ability's remaining cooldown going *up* between two ticks, since it
//! only ever counts down otherwise. That covers every ability, the ones that
//! hit nothing included. While the empowered attack is up the carrier holds
//! `SPARKS_BUFF`, and the attack that spends it plays a burst on its target.

use mod_api_stable::*;

use crate::{has_buff, refresh_buff, ticks, TICKS_PER_SECOND};

/// Starts when an empowered attack lands and keeps a cast from readying the
/// next one until it runs out. One name for every Spellblade item, so they
/// share it.
const COOLDOWN_BUFF: &str = "spellblade_cooldown";
/// How long a cast keeps the next basic attack empowered; an unused one is
/// lost. Another cast while it is up restarts it. The tooltips state it as a
/// fixed 10 seconds, so it is not read from config.
const WINDOW_SECONDS: f64 = 10.0;
/// Statless marker on a champion whose next basic attack is empowered. It is
/// the `view_buffs` binding in `view/effects.view_effects` that draws sparks
/// circling the champion's hands (`effects/spellblade_sparks`).
const SPARKS_BUFF: &str = "riot_spellblade";
/// Refreshed once a second while Spellblade is up, so it never lapses between
/// refreshes, and gone half a second after a missed one: the champion died,
/// or the item left.
const SPARKS_TICKS: usize = 90;
/// The `view_effects` burst that plays on the target of the empowered attack
/// (`effects/spellblade_proc`), in the sparks' colours.
const PROC_EFFECT: &str = "riot_spellblade_proc";

#[derive(Clone, Debug, Default)]
pub(crate) struct Spellblade {
    /// The next basic attack is empowered.
    ready: bool,
    /// Ticks left before an unused empowered attack is lost.
    window: usize,
    /// The carrier's remaining ability cooldowns (skill, skill2, ult) last
    /// tick. `None` until the first reading after a spawn, which is only a
    /// baseline.
    last_cooldowns: Option<(usize, usize, usize)>,
}

impl Spellblade {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    /// Readies Spellblade when the carrier casts, unless the last empowered
    /// attack's cooldown is still running; a cast while it is ready restarts
    /// the window instead. Keeps the sparks up while it is ready and takes
    /// them down when the window runs out. Call once per `update`.
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let cooldowns = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, skill, skill2, ult)| (skill, skill2, ult));
        let cast = matches!(
            (cooldowns, self.last_cooldowns),
            (Some(now), Some(before))
                if now.0 > before.0 || now.1 > before.1 || now.2 > before.2
        );
        self.last_cooldowns = cooldowns;

        self.window = self.window.saturating_sub(1);
        let expired = self.ready && self.window == 0;
        if expired {
            self.ready = false;
        }

        let Some((champion, cooling_down)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), has_buff(&c, COOLDOWN_BUFF)))
        else {
            return;
        };
        if cast && !cooling_down {
            if !self.ready {
                mark(ctx, champion);
            }
            self.ready = true;
            self.window = ticks(WINDOW_SECONDS);
        } else if expired {
            ctx.entity_remove_buff(champion, SPARKS_BUFF);
        } else if self.ready && ctx.tick() % TICKS_PER_SECOND as usize == 0 {
            mark(ctx, champion);
        }
    }

    /// Spends the empowered attack: starts the shared cooldown, takes the
    /// sparks off `caster` and plays the burst on `target`. For a Spellblade
    /// item's `on_attack`, alongside its bonus damage.
    pub(crate) fn spend(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        cooldown_seconds: f64,
    ) {
        self.ready = false;
        ctx.add_buff(
            caster,
            &BuffV1::timed(COOLDOWN_BUFF, ticks(cooldown_seconds)),
        );
        ctx.entity_remove_buff(caster, SPARKS_BUFF);
        ctx.play_view_effect(PROC_EFFECT, caster, &InputTargetV1::target(target), 0, 0, 0);
    }
}

/// Puts up (or keeps up) the sparks on `entity`. Replacing rather than adding
/// keeps one instance however many Spellblade items it holds, and the view does
/// not restart an animation whose buff was replaced in one tick.
fn mark(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        SPARKS_BUFF,
        &BuffV1::timed(SPARKS_BUFF, SPARKS_TICKS),
    );
}
