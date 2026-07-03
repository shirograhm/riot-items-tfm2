use arrayvec::ArrayString;
use core::fmt::Write;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

// Tyranny passive: while equipped, the wielder gains bonus Attack Damage equal to
// `effect_caster_hp_percent_attack`% of their maximum health. The bonus must track
// max HP gained mid-battle (levels, Heartsteel, etc.), but a live buff can't be
// mutated or removed, and re-adding a same-named buff stacks rather than replaces.
//
// So the bonus is granted as *permanent, incremental* buffs whose name encodes the
// cumulative AD granted so far (`overlords_bloodmail_tyranny_<total>`). Each tick we
// recompute the target bonus from current max HP, read back the largest total already
// granted from the buff names, and — only if the target is higher — add a permanent
// buff for the *difference*, tagged with the new total. The deltas sum to the target,
// and because the buffs never expire the AD never lapses: this removes the ~1-tick
// flicker the old expiring-buff approach had. `refresh_lockout` still bridges the few
// ticks before a freshly added buff becomes visible to `buff_count()`, so a delta is
// never granted twice. Reading the running total from the buff names (rather than a
// field on `self`) also self-heals: if the buffs are cleared on respawn/dispel, the
// total reads back as 0 and the full bonus re-grants.
//
// Trade-off: max HP is assumed monotonic within a battle (it only rises from levels /
// items). If it ever dropped the bonus couldn't be lowered — but mid-battle max-HP
// loss doesn't occur, and the alternative was a visible flicker.
const REFRESH_LOCKOUT_TICKS: usize = 3;

#[derive(Clone, Debug)]
pub struct OverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    // Ticks remaining during which `apply_tyranny` skips its presence check, so a
    // freshly added buff can't be duplicated before it becomes visible.
    refresh_lockout: usize,
}

impl Default for OverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 25,
            hp: 400,
            effect_caster_hp_percent_attack: 1.5,
            refresh_lockout: 0,
        }
    }
}

impl OverlordsBloodmail {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_caster_hp_percent_attack: cfg
                .effect_caster_hp_percent_attack
                .unwrap_or(d.effect_caster_hp_percent_attack),
            refresh_lockout: 0,
        }
    }

    // Top up the Tyranny bonus. Recompute the target AD from the champion's *current*
    // max HP, read back the largest cumulative total already granted (encoded in the
    // buff names), and add a permanent buff for the difference only when the target has
    // grown. `refresh_lockout` skips a few ticks after each add so a delta can't be
    // granted twice before the new buff is visible to `buff_count()`.
    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
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

        let prefix = "overlords_bloodmail_tyranny_";
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

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
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
                attack: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for OverlordsBloodmail {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "overlords_bloodmail"
    }

    fn icon(&self) -> &str {
        "t11_0"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ironsword".to_string(), "ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_overlords_bloodmail".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        // Seed the bonus at spawn; the refresh lockout in `apply_tyranny` then guards
        // against a duplicate add before this one becomes visible.
        self.apply_tyranny(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        // Top up the bonus as max HP grows; also re-grants the full amount if the buffs
        // were cleared (respawn/dispel), since the running total then reads back as 0.
        self.apply_tyranny(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantOverlordsBloodmail {
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    // Ticks remaining during which `apply_tyranny` skips its presence check, so a
    // freshly added buff can't be duplicated before it becomes visible.
    refresh_lockout: usize,
}

impl Default for RadiantOverlordsBloodmail {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 40,
            hp: 650,
            effect_caster_hp_percent_attack: 1.5,
            refresh_lockout: 0,
        }
    }
}

impl RadiantOverlordsBloodmail {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            effect_caster_hp_percent_attack: cfg
                .effect_caster_hp_percent_attack
                .unwrap_or(d.effect_caster_hp_percent_attack),
            refresh_lockout: 0,
        }
    }

    // Top up the Tyranny bonus. Recompute the target AD from the champion's *current*
    // max HP, read back the largest cumulative total already granted (encoded in the
    // buff names), and add a permanent buff for the difference only when the target has
    // grown. `refresh_lockout` skips a few ticks after each add so a delta can't be
    // granted twice before the new buff is visible to `buff_count()`.
    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
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

        let prefix = "radiant_overlords_bloodmail_tyranny_";
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

        let target = percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
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
                attack: target - granted,
                ..Default::default()
            },
        );
        self.refresh_lockout = REFRESH_LOCKOUT_TICKS;
    }
}

impl ModItemInfo for RadiantOverlordsBloodmail {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_overlords_bloodmail"
    }

    fn icon(&self) -> &str {
        "t11_1"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["overlords_bloodmail".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        // Seed the bonus at spawn; the refresh lockout in `apply_tyranny` then guards
        // against a duplicate add before this one becomes visible.
        self.apply_tyranny(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        // Top up the bonus as max HP grows; also re-grants the full amount if the buffs
        // were cleared (respawn/dispel), since the running total then reads back as 0.
        self.apply_tyranny(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
