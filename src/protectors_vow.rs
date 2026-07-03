use arrayvec::ArrayString;
use core::fmt::Write;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// Awe passive: while equipped, the wielder gains bonus max HP equal to
// `effect_bonus_flat_hp` plus `effect_caster_defence_percent_hp`% of their defence. The
// bonus must track defence gained mid-battle (levels, stacking armor buffs, etc.), but a
// live buff can't be mutated or removed, and re-adding a same-named buff stacks rather
// than replaces.
//
// So the bonus is granted as *permanent, incremental* buffs whose name encodes the
// cumulative HP granted so far (`protectors_vow_awe_<total>`). Each tick we recompute the
// target bonus from current defence, read back the largest total already granted from the
// buff names, and — only if the target is higher — add a permanent buff for the
// *difference*, tagged with the new total. The deltas sum to the target, and because the
// buffs never expire the HP never lapses: this removes the ~1-tick flicker the old
// expiring-buff approach had (especially visible here, since it flashes the HP bar).
// `refresh_lockout` still bridges the few ticks before a freshly added buff becomes
// visible to `buff_count()`, so a delta is never granted twice. Reading the running total
// from the buff names (rather than a field on `self`) also self-heals: if the buffs are
// cleared on respawn/dispel, the total reads back as 0 and the full bonus re-grants.
//
// Trade-off: defence is assumed monotonic within a battle (it only rises from levels /
// items / stacking buffs — enemy armor penetration doesn't lower `stat().defence`). If it
// ever dropped the bonus couldn't be lowered, but the alternative was a visible HP-bar
// flicker.
const REFRESH_LOCKOUT_TICKS: usize = 3;

#[derive(Clone, Debug)]
pub struct ProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    // Ticks remaining during which `apply_awe` skips its work, so a freshly added buff
    // can't be duplicated before it becomes visible.
    refresh_lockout: usize,
}

impl Default for ProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 350,
            defence: 50,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_lockout: 0,
        }
    }
}

impl ProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_lockout: 0,
        }
    }

    // Top up the Awe bonus. Recompute the target HP from the champion's *current* defence,
    // read back the largest cumulative total already granted (encoded in the buff names),
    // and add a permanent buff for the difference only when the target has grown.
    // `refresh_lockout` skips a few ticks after each add so a delta can't be granted twice
    // before the new buff is visible to `buff_count()`.
    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_lockout > 0 {
            self.refresh_lockout -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let prefix = "protectors_vow_awe_";
        let granted = (0..entity_ref.buff_count())
            .filter_map(|i| {
                entity_ref
                    .buff_at(i)
                    .name
                    .as_str()
                    .strip_prefix(prefix)
                    .and_then(|total| total.parse::<i32>().ok())
            })
            .max()
            .unwrap_or(0);

        let target = self.effect_bonus_flat_hp
            + percent_of(
                entity_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= granted {
            return;
        }

        let entity_id = entity_ref.id();
        let mut name = ArrayString::<64>::new();
        write!(&mut name, "{prefix}{target}").unwrap();

        ctx.add_buff(
            entity_id,
            BuffState {
                name,
                duration: BuffType::Permanent,
                hp: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for ProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        // Seed the bonus at spawn; the refresh lockout in `apply_awe` then guards against
        // a duplicate add before this one becomes visible.
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        // Top up the bonus as defence grows; also re-grants the full amount if the buffs
        // were cleared (respawn/dispel), since the running total then reads back as 0.
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    // Ticks remaining during which `apply_awe` skips its work, so a freshly added buff
    // can't be duplicated before it becomes visible.
    refresh_lockout: usize,
}

impl Default for RadiantProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1800,
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_lockout: 0,
        }
    }
}

impl RadiantProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_lockout: 0,
        }
    }

    // Top up the Awe bonus. Recompute the target HP from the champion's *current* defence,
    // read back the largest cumulative total already granted (encoded in the buff names),
    // and add a permanent buff for the difference only when the target has grown.
    // `refresh_lockout` skips a few ticks after each add so a delta can't be granted twice
    // before the new buff is visible to `buff_count()`.
    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_lockout > 0 {
            self.refresh_lockout -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let prefix = "radiant_protectors_vow_awe_";
        let granted = (0..entity_ref.buff_count())
            .filter_map(|i| {
                entity_ref
                    .buff_at(i)
                    .name
                    .as_str()
                    .strip_prefix(prefix)
                    .and_then(|total| total.parse::<i32>().ok())
            })
            .max()
            .unwrap_or(0);

        let target = self.effect_bonus_flat_hp
            + percent_of(
                entity_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= granted {
            return;
        }

        let entity_id = entity_ref.id();
        let mut name = ArrayString::<64>::new();
        write!(&mut name, "{prefix}{target}").unwrap();

        ctx.add_buff(
            entity_id,
            BuffState {
                name,
                duration: BuffType::Permanent,
                hp: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for RadiantProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        // Seed the bonus at spawn; the refresh lockout in `apply_awe` then guards against
        // a duplicate add before this one becomes visible.
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        // Top up the bonus as defence grows; also re-grants the full amount if the buffs
        // were cleared (respawn/dispel), since the running total then reads back as 0.
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}
