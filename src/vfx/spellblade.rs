use mod_api_stable::*;

use std::cell::Cell;
use std::sync::Mutex;

use crate::{has_buff, percent_of, refresh_buff, ticks, TICKS_PER_SECOND};

const WINDOW_SECONDS: f64 = 10.0;
const SPARKS_TICKS: usize = 90;

const SPARKS_BUFF: &str = "riot_spellblade";
const PROC_EFFECT: &str = "riot_spellblade_proc";
const COOLDOWN_BUFF: &str = "spellblade_cooldown";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SpellbladeBonus {
    pub(crate) flat: usize,
    pub(crate) per_level: usize,
    pub(crate) ad_percent: f64,
    pub(crate) ap_percent: f64,
    pub(crate) crit_percent: f64,
}

impl SpellbladeBonus {
    pub(crate) fn by_level(min: usize, max: usize) -> Self {
        Self {
            flat: min,
            per_level: (max.saturating_sub(min) as f64 / 11.0).round() as usize,
            ..Self::default()
        }
    }

    // The bonus on `carrier`'s stats as they are now.
    pub(crate) fn of(&self, carrier: &StableEntity<'_, '_>) -> usize {
        let stat = carrier.stat();
        self.flat
            + carrier.level().saturating_sub(1) * self.per_level
            + percent_of(stat.attack, self.ad_percent)
            + percent_of(stat.magic_power, self.ap_percent)
            + percent_of(stat.crit_chance, self.crit_percent)
    }
}

static BONUSES: Mutex<Vec<(&'static str, SpellbladeBonus)>> = Mutex::new(Vec::new());

thread_local! {
    static CLAIMED: Cell<Option<(usize, usize)>> = Cell::new(None);
}

fn held(ctx: &StableSim<'_>, player: usize) -> Vec<(&'static str, SpellbladeBonus)> {
    let Some(keys) = ctx.get_player(player).map(|p| p.item_keys()) else {
        return Vec::new();
    };
    let Ok(bonuses) = BONUSES.lock() else {
        return Vec::new();
    };
    keys.iter()
        .filter_map(|held| {
            bonuses
                .iter()
                .find(|(key, _)| *key == held.as_str())
                .copied()
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Spellblade {
    ready: bool,
    window: usize,
    last_cooldowns: Option<(usize, usize, usize)>,
    holder: usize,
    charged: Vec<&'static str>,
}

impl Spellblade {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    pub(crate) fn note_bonus(key: &'static str, bonus: SpellbladeBonus) {
        if let Ok(mut bonuses) = BONUSES.lock() {
            bonuses.retain(|&(noted, _)| noted != key);
            bonuses.push((key, bonus));
        }
    }

    pub(crate) fn wins(&mut self, ctx: &StableSim<'_>, caster: usize, key: &str) -> bool {
        let swing = (ctx.tick(), caster);
        let wins = self
            .strongest(ctx, caster)
            .is_none_or(|strongest| strongest == key)
            && CLAIMED.with(|claimed| claimed.replace(Some(swing)) != Some(swing));
        if !wins {
            self.ready = false;
        }
        wins
    }

    fn strongest(&self, ctx: &StableSim<'_>, caster: usize) -> Option<&'static str> {
        let carrier = ctx.get_entity(caster)?;
        let mut best: Option<(&'static str, usize)> = None;
        for (key, bonus) in held(ctx, self.holder) {
            if !self.charged.contains(&key) {
                continue;
            }
            let amount = bonus.of(&carrier);
            if best.is_none_or(|(_, top)| amount > top) {
                best = Some((key, amount));
            }
        }
        best.map(|(key, _)| key)
    }

    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        CLAIMED.with(|claimed| claimed.set(None));
        self.holder = player;
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
            self.charged = held(ctx, player).into_iter().map(|(key, _)| key).collect();
        } else if expired {
            ctx.entity_remove_buff(champion, SPARKS_BUFF);
        } else if self.ready && ctx.tick() % TICKS_PER_SECOND as usize == 0 {
            mark(ctx, champion);
        }
    }

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

fn mark(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        SPARKS_BUFF,
        &BuffV1::timed(SPARKS_BUFF, SPARKS_TICKS),
    );
}
