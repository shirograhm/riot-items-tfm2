use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_monster, item_counts, percent_of, refresh_buff, ticks, ItemMeta,
    DOT_TICK_RATE,
};

// League's Blackfire Torch (patch 14.19):
// Baleful Blaze: Dealing Ability damage burns enemies for 60 (+6% AP) magic
// damage over 3 seconds. Monsters take 60 bonus magic damage over the burn.
// Blackfire: Gain 4% Ability Power for each enemy champion and monster burning
// from Baleful Blaze.
//
// The carrier's Blackfire, one buff whose `magic_power_mult` is the share for
// every unit burning now. Short, and put up again while anything burns, so it
// lapses with the last burn, or with the carrier (no `update` runs while dead).
const BLACKFIRE_BUFF: &str = "blackfire_torch_blackfire";
const BLACKFIRE_TICKS: usize = 30;
// Put up again when this little is left.
const BLACKFIRE_RENEW_TICKS: usize = 10;
// The worthless Blackfire that replaces it when the last burn ends
// ([`BlackfireTorch::keep_blackfire`]).
const BLACKFIRE_CLEAR_TICKS: usize = 2;

// Statless markers on a burning unit: the `view_buffs` of the same names draw a
// small dark black-purple fire at its feet. One small fire for every unit,
// only placed at its feet: a champion's 12 px under the unit, a minion's 8, a
// jungle camp's 18, the Serpen's 37 and an epic monster's 55.
const FLAMES: &str = "riot_blackfire_burn";
const MINION_FLAMES: &str = "riot_blackfire_burn_minion";
const MONSTER_FLAMES: &str = "riot_blackfire_burn_monster";
const SERPEN_FLAMES: &str = "riot_blackfire_burn_serpen";
const EPIC_FLAMES: &str = "riot_blackfire_burn_epic";
// How much longer than the burn its fire is put up for, and how long it is
// left before being put up again (a damage-over-time skill restarts the burn
// every tick; Liandry's flames do the same).
const FLAMES_SLACK_TICKS: usize = DOT_TICK_RATE;

// One unit Baleful Blaze is burning.
#[derive(Clone, Copy, Debug)]
struct Burn {
    target: usize,
    // Ticks the burn has left.
    remaining: usize,
    // Ticks until its next damage tick.
    until_next: usize,
    // A monster: takes the bonus damage, and counts for Blackfire.
    monster: bool,
    // Counts for Blackfire: a champion or a monster.
    blackfire: bool,
    // Ticks its fire's marker has left. Never less than `remaining`.
    marker: usize,
    // Which fire, by the unit's size. Kept here because the unit may be gone
    // by the time its fire is to come down.
    flames: &'static str,
}

// The fire that sits at this unit's feet.
fn flames_for(unit: &StableEntity<'_, '_>) -> &'static str {
    if unit.is_minion() {
        return MINION_FLAMES;
    }
    match unit.kind() {
        Some(EntityKindV1::Epic) => EPIC_FLAMES,
        Some(EntityKindV1::Serpen) => SERPEN_FLAMES,
        Some(EntityKindV1::Jungle) => MONSTER_FLAMES,
        // Champions, summons, and a monster on a host too old to say which.
        _ if is_monster(unit) && unit.kind().is_none() => MONSTER_FLAMES,
        _ => FLAMES,
    }
}

#[derive(Clone, Debug)]
pub struct BlackfireTorch {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_bonus_magic_damage: usize,
    effect_burn_duration_seconds: f64,
    effect_stack_magic_power_mult: i32,
    // Non-vital stats (internals)
    burns: Vec<Burn>,
    // Blackfire as last put up: the units it counted, and its ticks left.
    blackfire_count: usize,
    blackfire_left: usize,
}

impl BlackfireTorch {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "blackfire_torch",
                &["staff_of_rapture", "needlessly_large_rod"],
                &["radiant_blackfire_torch"],
            ),
            price: 650,
            magic_power: 65,
            skill_cooldown_mult: 15,
            effect_bonus_flat_damage: 60,
            effect_ap_percent_damage: 6.0,
            effect_bonus_magic_damage: 60,
            effect_burn_duration_seconds: 3.0,
            effect_stack_magic_power_mult: 4,
            burns: Vec::new(),
            blackfire_count: 0,
            blackfire_left: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_blackfire_torch", &["blackfire_torch"]),
            price: 950,
            magic_power: 90,
            skill_cooldown_mult: 25,
            effect_bonus_flat_damage: 90,
            effect_ap_percent_damage: 9.0,
            effect_bonus_magic_damage: 90,
            effect_burn_duration_seconds: 3.0,
            effect_stack_magic_power_mult: 6,
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
                magic_power,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_bonus_magic_damage,
                effect_burn_duration_seconds,
                effect_stack_magic_power_mult
            ]
        );
        self
    }

    fn duration_ticks(&self) -> usize {
        ticks(self.effect_burn_duration_seconds).max(DOT_TICK_RATE)
    }

    fn instance_count(&self) -> usize {
        (self.duration_ticks() / DOT_TICK_RATE).max(1)
    }

    // Starts the burn on `target`, or starts its time over, and puts its fire
    // up with it.
    fn apply_burn(
        &mut self,
        ctx: &mut StableSim<'_>,
        target: usize,
        monster: bool,
        blackfire: bool,
        flames: &'static str,
    ) {
        let duration = self.duration_ticks();
        let index = match self.burns.iter().position(|burn| burn.target == target) {
            Some(index) => index,
            None => {
                self.burns.push(Burn {
                    target,
                    remaining: 0,
                    until_next: DOT_TICK_RATE,
                    monster,
                    blackfire,
                    marker: 0,
                    flames,
                });
                self.burns.len() - 1
            }
        };
        let burn = &mut self.burns[index];
        burn.remaining = duration;
        // Put up for the whole burn in one call, and again only once the burn
        // would outlast it.
        if burn.marker < duration {
            burn.marker = duration + FLAMES_SLACK_TICKS;
            let marker = burn.marker;
            refresh_buff(ctx, target, flames, &BuffV1::timed(flames, marker));
        }
    }

    // One damage tick: an even share of the whole burn, at the carrier's
    // Ability Power as it is now (Blackfire's included).
    fn instance_damage(&self, caster_ap: usize, monster: bool) -> usize {
        let mut total = self.effect_bonus_flat_damage
            + percent_of(caster_ap, self.effect_ap_percent_damage);
        if monster {
            total += self.effect_bonus_magic_damage;
        }
        (total as f64 / self.instance_count() as f64).round() as usize
    }

    fn tick_burns(&mut self, ctx: &mut StableSim<'_>, caster: usize, caster_ap: usize) {
        let mut kept = Vec::with_capacity(self.burns.len());
        for mut burn in std::mem::take(&mut self.burns) {
            // Re-read every tick, since entity ids are recycled.
            if !ctx
                .get_entity(burn.target)
                .is_some_and(|target_ref| target_ref.is_alive())
            {
                // Dead or gone, and its fire goes with the burn rather than
                // sitting on the body or whatever takes its slot.
                ctx.entity_remove_buff(burn.target, burn.flames);
                continue;
            }
            burn.remaining = burn.remaining.saturating_sub(1);
            burn.until_next = burn.until_next.saturating_sub(1);
            burn.marker = burn.marker.saturating_sub(1);
            if burn.until_next == 0 {
                let damage = self.instance_damage(caster_ap, burn.monster);
                if damage > 0 {
                    ctx.deal_damage(caster, burn.target, 0, damage, AttackTypeV1::Item);
                }
                burn.until_next = DOT_TICK_RATE;
            }
            if burn.remaining > 0 {
                kept.push(burn);
            }
        }
        self.burns = kept;
    }

    // Blackfire for the units burning now: put up when the count changes or
    // the buff is about to run out, taken down when nothing counts.
    fn keep_blackfire(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let count = self.burns.iter().filter(|burn| burn.blackfire).count();
        self.blackfire_left = self.blackfire_left.saturating_sub(1);
        if count == 0 {
            if self.blackfire_count != 0 {
                // Swapped for a worthless one rather than only removed: the
                // engine appears to work a buff's `magic_power_mult` out again
                // only when a buff is ADDED, so a bare removal left the AP up
                // until the next champion hit put Blackfire back (user, 11-10).
                refresh_buff(
                    ctx,
                    caster,
                    BLACKFIRE_BUFF,
                    &BuffV1::timed(BLACKFIRE_BUFF, BLACKFIRE_CLEAR_TICKS),
                );
            }
            self.blackfire_count = 0;
            self.blackfire_left = 0;
            return;
        }
        if count == self.blackfire_count && self.blackfire_left > BLACKFIRE_RENEW_TICKS {
            return;
        }
        refresh_buff(
            ctx,
            caster,
            BLACKFIRE_BUFF,
            &BuffV1 {
                magic_power_mult: self.effect_stack_magic_power_mult * count as i32,
                ..BuffV1::timed(BLACKFIRE_BUFF, BLACKFIRE_TICKS)
            },
        );
        self.blackfire_count = count;
        self.blackfire_left = BLACKFIRE_TICKS;
    }
}

impl Default for BlackfireTorch {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for BlackfireTorch {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // A new life burns nothing yet, and its Blackfire starts from none.
    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.burns.clear();
        self.blackfire_count = 0;
        self.blackfire_left = 0;
    }

    // Baleful Blaze. Ability damage sets it off, a tick of damage over time
    // included; the burn's own ticks (Item damage) never relight it.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !matches!(
            attack_type,
            AttackTypeV1::Skill | AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield
        ) {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_alive() || target_ref.is_tower() {
            return;
        }
        // Summons (ghouls, bears and the like) are neither champions nor
        // monsters: they burn like minions.
        let monster = is_monster(&target_ref)
            && !matches!(target_ref.kind(), Some(EntityKindV1::Summon));
        let blackfire = monster || target_ref.is_champion();
        let flames = flames_for(&target_ref);
        self.apply_burn(ctx, target, monster, blackfire, flames);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let Some((caster, caster_ap)) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| (champion.id(), champion.stat().magic_power))
        else {
            // A dead carrier burns nobody, and holding the list for its next
            // life would relight ids that belong to the fight that has moved on.
            self.burns.clear();
            return;
        };
        if !self.burns.is_empty() {
            self.tick_burns(ctx, caster, caster_ap);
        }
        self.keep_blackfire(ctx, caster);
        // The units Blackfire counts, for the number on the item's icon.
        let count = self.blackfire_count as u64;
        item_counts::note_in_combat(ctx, player, self.meta.key, || count);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::CooltimeReduce, ItemTagV1::DotDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
