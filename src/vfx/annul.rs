use mod_api_stable::*;

use crate::{refresh_buff, ticks, upgrade_carry};

const ANNUL_BUFF: &str = "riot_annul";

#[derive(Clone, Debug, Default)]
pub(crate) struct Annul {
    cooldown: usize,
    up: bool,
    popped: bool,
    line: &'static str,
    built_up: bool,
    inherited: bool,
}

// Flags packed above the cooldown in [`Annul::carry`].
const CARRY_UP: u64 = 1 << 62;
const CARRY_POPPED: u64 = 1 << 63;

fn skill_damaged_reduce(entity: &StableEntity<'_, '_>) -> (usize, Option<usize>) {
    let mut others = 0;
    let mut own = None;
    for buff in (0..entity.buff_count()).filter_map(|i| entity.buff_at(i)) {
        if buff.name() == ANNUL_BUFF {
            own = Some(buff.skill_damaged_reduce);
        } else {
            others += buff.skill_damaged_reduce;
        }
    }
    (others, own)
}

impl Annul {
    pub(crate) fn on_line(line: &'static str, built_up: bool) -> Self {
        Self {
            line,
            built_up,
            ..Self::default()
        }
    }

    pub(crate) fn reset(&mut self, ctx: &StableSim<'_>, player: usize) {
        self.cooldown = 0;
        self.up = false;
        self.popped = false;
        self.inherited = true;
        upgrade_carry::note(self.line, ctx, player, 0);
    }

    pub(crate) fn carry(&self) -> u64 {
        let mut carry = self.cooldown as u64;
        if self.up {
            carry |= CARRY_UP;
        }
        if self.popped {
            carry |= CARRY_POPPED;
        }
        carry
    }

    pub(crate) fn resume(&mut self, carry: u64) {
        self.cooldown = (carry & !(CARRY_UP | CARRY_POPPED)) as usize;
        self.up = carry & CARRY_UP != 0;
        self.popped = carry & CARRY_POPPED != 0;
    }

    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if !std::mem::replace(&mut self.inherited, true) && self.built_up {
            if let Some((_, ready_at)) = upgrade_carry::latest(self.line, ctx, player) {
                let left = (ready_at as usize).saturating_sub(ctx.tick());
                self.cooldown = self.cooldown.max(left);
            }
        }
        let champion = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.is_alive()));
        if self.popped {
            if let Some((entity, _)) = champion {
                ctx.entity_remove_buff(entity, ANNUL_BUFF);
                self.popped = false;
            }
            return;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        if self.cooldown > 0 {
            return;
        }
        let Some((entity, true)) = champion else {
            return;
        };
        let Some((others, own)) = ctx.get_entity(entity).map(|c| skill_damaged_reduce(&c)) else {
            return;
        };

        let reduce = 100usize.saturating_sub(others);
        self.up = true;
        if own == Some(reduce) {
            return;
        }
        refresh_buff(
            ctx,
            entity,
            ANNUL_BUFF,
            &BuffV1 {
                skill_damaged_reduce: reduce,
                cc_immune: true,
                ..BuffV1::named(ANNUL_BUFF)
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        attack_type: AttackTypeV1,
        cooldown_seconds: f64,
    ) {
        let blocked = match attack_type {
            AttackTypeV1::Skill | AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield => true,
            AttackTypeV1::Item => damage == 0,
            _ => false,
        };
        if blocked {
            self.pop(ctx, player, entity, attacker, cooldown_seconds);
        }
    }

    pub(crate) fn on_cc(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        caster: usize,
        cooldown_seconds: f64,
    ) {
        let Some(entity) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        self.pop(ctx, player, entity, caster, cooldown_seconds);
    }

    fn pop(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        source: usize,
        cooldown_seconds: f64,
    ) {
        if !self.up {
            return;
        }
        let Some(team) = ctx.get_entity(entity).map(|e| e.team()) else {
            return;
        };
        if ctx.get_entity(source).is_some_and(|s| s.team() == team) {
            return;
        }
        self.up = false;
        self.popped = true;
        self.cooldown = ticks(cooldown_seconds);
        let ready_at = ctx.tick() + self.cooldown;
        upgrade_carry::note(self.line, ctx, player, ready_at as u64);
    }
}
