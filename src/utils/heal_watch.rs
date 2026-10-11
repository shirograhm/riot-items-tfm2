//! Finds the allies the carrier healed, and by how much.
//!
//! The host reports an ally-targeted skill once, at the cast, and names only
//! the target the cast was aimed at: `apply_input` calls `on_skill_hit` right
//! after `try_skill` (read from the 0.5.6 SDK's IR). The heal has not landed
//! yet, its amount is never given, and a skill aimed at its own caster that
//! heals around them arrives as a plain self-cast with the allies it reaches
//! never named. The Monk's heal is one: `MonkSkillAction::casting_target` is
//! `AllyOnlySelf`.
//!
//! What the host does keep is who healed whom. A player's `heal` statistic
//! counts their healing of champions and `self_heal` the part of it that
//! landed on themselves (`GamePlayer::on_heal`, called with the health really
//! restored), so the difference going up means the carrier healed another
//! champion, and by that much. After a cast this watches for a short while: an
//! ally whose health rose, with that difference risen since it was last read,
//! was healed by the carrier.
//!
//! The statistics are the dear part. The host serialises the player's whole
//! document for every read, so they are read only on a tick where some ally's
//! health did rise; the health reads that decide it are a call each.
//!
//! Only heals leave that trace. Nothing is counted as a shield is given
//! (`Entity::add_shield` leaves the statistics alone; `shield_given` grows as
//! a shield absorbs damage) and a buff is not counted at all. Neither is an
//! ally at full health, whose health cannot rise.

use mod_api_stable::*;

// How long a cast is watched. The heal lands at the skill's `start_timing`
// (28 ticks into the Monk's), so this only has to outlast the slowest cast.
const WATCH_TICKS: usize = 90;

// The two statistics read, out of the player's whole document.
#[derive(serde::Deserialize)]
struct Healing {
    heal: u64,
    self_heal: u64,
}

// A heal the carrier landed on an allied champion.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AllyHeal {
    pub(crate) ally: usize,
    // The health restored.
    pub(crate) amount: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HealWatch {
    // Ticks left to watch; 0 when nothing is being watched.
    remaining: usize,
    caster: usize,
    player: usize,
    // The carrier's healing of other champions, as last read: at the cast,
    // or on the last tick an ally's health rose.
    healed_others: u64,
    // (id, health) of the carrier's living allied champions, as last read.
    allies: Vec<(usize, usize)>,
}

impl HealWatch {
    // Starts watching, from `on_skill_hit`: the carrier cast an ally-targeted
    // skill. The heal has not landed yet, so what is read here is the state
    // before it. A watch that is already running is only given more time, and
    // keeps what it last read.
    pub(crate) fn open(&mut self, ctx: &StableSim<'_>, caster: usize) {
        if self.is_open() && self.caster == caster {
            self.remaining = WATCH_TICKS;
            return;
        }
        self.close();
        let Some(player) = carrier_player(ctx, caster) else {
            return;
        };
        let Some(total) = healed_others(ctx, player) else {
            return;
        };
        self.caster = caster;
        self.player = player;
        self.healed_others = total;
        self.allies = allies(ctx, caster);
        self.remaining = WATCH_TICKS;
    }

    pub(crate) fn close(&mut self) {
        self.remaining = 0;
        self.allies.clear();
    }

    pub(crate) fn is_open(&self) -> bool {
        self.remaining > 0
    }

    // Called every tick, from `update`. Returns the heals the carrier landed
    // on allied champions since the last tick.
    pub(crate) fn poll(&mut self, ctx: &StableSim<'_>) -> Vec<AllyHeal> {
        if !self.is_open() {
            return Vec::new();
        }
        self.remaining -= 1;

        let carrier_alive = ctx
            .get_entity(self.caster)
            .is_some_and(|caster_ref| caster_ref.is_alive());
        if !carrier_alive {
            self.close();
            return Vec::new();
        }

        let now = allies(ctx, self.caster);
        let mut healed = Vec::new();
        for &(ally, hp) in &now {
            let before = self.allies.iter().find(|&&(id, _)| id == ally);
            let Some(&(_, before_hp)) = before else {
                continue;
            };
            if hp > before_hp {
                healed.push(AllyHeal {
                    ally,
                    amount: hp - before_hp,
                });
            }
        }
        self.allies = now;

        if !healed.is_empty() {
            let Some(total) = healed_others(ctx, self.player) else {
                self.close();
                return Vec::new();
            };
            let given = total.saturating_sub(self.healed_others);
            self.healed_others = total;
            // Health rises for other reasons too (regeneration, an ally's own
            // lifesteal). Whatever rose, the carrier gave no more than `given`.
            let risen: u64 = healed.iter().map(|heal| heal.amount as u64).sum();
            if risen > given {
                for heal in &mut healed {
                    heal.amount = (heal.amount as u64 * given / risen) as usize;
                }
                healed.retain(|heal| heal.amount > 0);
            }
        }

        if self.remaining == 0 {
            self.close();
        }
        healed
    }

    // Reads the state again, so that a heal the item itself has just given in
    // the carrier's name is not read back next tick as one of the carrier's.
    // Heals apply at once (`Entity::healed_inner`), so this sees them.
    pub(crate) fn resync(&mut self, ctx: &StableSim<'_>) {
        if !self.is_open() {
            return;
        }
        let Some(total) = healed_others(ctx, self.player) else {
            self.close();
            return;
        };
        self.healed_others = total;
        self.allies = allies(ctx, self.caster);
    }
}

// A [`HealWatch`] for the items that only ask whether a cast on the carrier
// reached an ally as well: it answers once for one cast, the way an
// ally-targeted skill is reported once.
#[derive(Clone, Debug, Default)]
pub(crate) struct SelfCastWatch(HealWatch);

impl SelfCastWatch {
    // Starts watching, from `on_skill_hit`: the carrier cast an ally-targeted
    // skill on themselves.
    pub(crate) fn open(&mut self, ctx: &StableSim<'_>, caster: usize) {
        self.0.open(ctx, caster);
    }

    pub(crate) fn close(&mut self) {
        self.0.close();
    }

    // Called every tick, from `update`. Returns the allied champions the
    // carrier healed since the last tick, which ends the watch.
    pub(crate) fn poll(&mut self, ctx: &StableSim<'_>) -> Vec<usize> {
        let healed = self.0.poll(ctx);
        if !healed.is_empty() {
            self.0.close();
        }
        healed.into_iter().map(|heal| heal.ally).collect()
    }
}

// The player whose champion `caster` is. `on_skill_hit` is handed the entity
// and the statistics hang off the player.
fn carrier_player(ctx: &StableSim<'_>, caster: usize) -> Option<usize> {
    (0..ctx.player_count()).find_map(|index| {
        let player_ref = ctx.player_at(index)?;
        let champion_ref = player_ref.champion()?;
        (champion_ref.id() == caster).then(|| player_ref.id())
    })
}

// The player's healing of champions other than their own, over the match.
fn healed_others(ctx: &StableSim<'_>, player: usize) -> Option<u64> {
    let json = ctx.get_player(player)?.statistics_json("")?;
    let healing: Healing = serde_json::from_str(&json).ok()?;
    Some(healing.heal.saturating_sub(healing.self_heal))
}

// (id, health) of every living allied champion other than the carrier.
fn allies(ctx: &StableSim<'_>, caster: usize) -> Vec<(usize, usize)> {
    let Some(caster_team) = ctx.get_entity(caster).map(|caster_ref| caster_ref.team()) else {
        return Vec::new();
    };

    let mut allies = Vec::new();
    for index in 0..ctx.champion_count() {
        let id = ctx.champion_id_at(index);
        if id == caster {
            continue;
        }
        let Some(ally_ref) = ctx.get_entity(id) else {
            continue;
        };
        if !ally_ref.is_alive() || !ally_ref.is_champion() || ally_ref.team() != caster_team {
            continue;
        }
        allies.push((id, ally_ref.hp().0));
    }
    allies
}
