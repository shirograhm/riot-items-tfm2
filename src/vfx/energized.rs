use mod_api_stable::*;

use crate::{has_buff, refresh_buff};

const CHARGED_BUFF: &str = "riot_energized";

const CHARGED_TICKS: usize = 60;
const MARK_EVERY_TICKS: usize = 20;
const STACK_EVERY_TICKS: usize = 12;
const ATTACK_STACKS: usize = 5;
const SHARE_LOCKOUT_TICKS: usize = 240;

#[derive(Clone, Debug, Default)]
pub(crate) struct Energized {
    stacks: usize,
    idle_ticks: usize,
    marked: bool,
    spent_at: Option<usize>,
    take_down: bool,
}

impl Energized {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn stacks(&self) -> usize {
        self.stacks
    }

    pub(crate) fn set_stacks(&mut self, stacks: usize) {
        self.stacks = stacks;
    }

    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize, max: usize) {
        if self.idle_ticks >= STACK_EVERY_TICKS {
            self.stacks = (self.stacks + 1).min(max);
            self.idle_ticks = 0;
        } else {
            self.idle_ticks += 1;
        }

        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id())
        else {
            return;
        };

        if self.take_down {
            self.take_down = false;
            ctx.entity_remove_buff(champion, CHARGED_BUFF);
        }
        if self.stacks >= max && (!self.marked || ctx.tick() % MARK_EVERY_TICKS == 0) {
            self.marked = true;
            refresh_buff(
                ctx,
                champion,
                CHARGED_BUFF,
                &BuffV1::timed(CHARGED_BUFF, CHARGED_TICKS),
            );
        }
    }

    pub(crate) fn is_charged(&self, ctx: &StableSim<'_>, caster: usize, max: usize) -> bool {
        if self.stacks >= max {
            return true;
        }
        let rested = self
            .spent_at
            .is_none_or(|at| ctx.tick().saturating_sub(at) >= SHARE_LOCKOUT_TICKS);
        rested
            && ctx
                .get_entity(caster)
                .is_some_and(|caster_ref| has_buff(&caster_ref, CHARGED_BUFF))
    }

    pub(crate) fn spend(&mut self, ctx: &StableSim<'_>) {
        self.stacks = 0;
        self.marked = false;
        self.spent_at = Some(ctx.tick());
        self.take_down = true;
    }

    pub(crate) fn basic_attack(&mut self, max: usize) {
        self.stacks = (self.stacks + ATTACK_STACKS).min(max);
    }
}
