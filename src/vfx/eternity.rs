use mod_api_stable::*;

use std::cell::Cell;

const LEVEL_STEPS: f64 = 11.0;

thread_local! {
    static HEALED: Cell<Option<(u64, usize, usize)>> = const { Cell::new(None) };
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Eternity {
    last_cooldowns: Option<(usize, usize, usize)>,
}

impl Eternity {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn update(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        min_heal: usize,
        max_heal: usize,
    ) {
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
        if !cast {
            return;
        }

        let Some((champion, level)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.level()))
        else {
            return;
        };
        let this_cast = (ctx.seed(), ctx.tick(), champion);
        if HEALED.replace(Some(this_cast)) == Some(this_cast) {
            return;
        }

        let steps = (level.saturating_sub(1) as f64).min(LEVEL_STEPS);
        let span = max_heal.saturating_sub(min_heal) as f64;
        let heal = min_heal + (span * steps / LEVEL_STEPS).round() as usize;
        ctx.heal(champion, champion, heal);
    }
}
