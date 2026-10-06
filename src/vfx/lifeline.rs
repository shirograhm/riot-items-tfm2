use mod_api_stable::*;

use crate::refresh_buff;

const SHIELD_BUFF: &str = "riot_lifeline";
const SETTLE_TICKS: usize = 3;

#[derive(Clone, Debug, Default)]
pub(crate) struct Lifeline {
    remaining: usize,
    settle: usize,
}

impl Lifeline {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn raise(&mut self, ctx: &mut StableSim<'_>, entity: usize, duration_ticks: usize) {
        refresh_buff(
            ctx,
            entity,
            SHIELD_BUFF,
            &BuffV1::timed(SHIELD_BUFF, duration_ticks),
        );
        self.remaining = duration_ticks;
        self.settle = SETTLE_TICKS;
    }

    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.remaining == 0 {
            return;
        }
        self.remaining -= 1;
        if self.settle > 0 {
            self.settle -= 1;
            return;
        }

        let Some((champion, shield)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.shield()))
        else {
            return;
        };
        if shield == 0 {
            self.remaining = 0;
            ctx.entity_remove_buff(champion, SHIELD_BUFF);
        }
    }
}
