use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, ticks, ItemMeta, DOT_TICK_RATE};

/// Statless marker on a unit Torment is burning, monster as much as
/// champion: the `view_buffs` binding of the same name in
/// `view/effects.view_effects` draws a fire at its feet for as long as the
/// marker is up. One name for both tiers and every carrier, so a unit two of
/// them burn shows one fire. The art is drawn to a champion's size.
const BURN_BUFF: &str = "riot_liandrys_burn";
/// The same marker for a minion, bound to a fire its size: a minion stands
/// about 13 px wide and tall against a champion's 21 to 36, its feet 7 px
/// under it against 12, and the champion's fire swallowed it.
const SMALL_BURN_BUFF: &str = "riot_liandrys_burn_small";
/// How much longer than the burn the marker is put up for, which is also how
/// long it is left alone before it is put up again. A damage-over-time skill
/// starts the burn over on every one of its ticks, and replacing a buff that
/// often is work for nothing: the marker would restart its animation as often
/// if the view ever took a replaced buff for a new one. So the flames may
/// outlast the burn by this much, a fifth of a second.
const MARKER_SLACK_TICKS: usize = DOT_TICK_RATE;

/// One unit this carrier's Torment is burning.
#[derive(Clone, Copy, Debug)]
struct Burn {
    target: usize,
    /// Ticks the burn has left.
    remaining: usize,
    /// Ticks until its next damage instance.
    until_next: usize,
    /// Ticks the flames' marker has left. Never less than `remaining`.
    marker: usize,
    /// Which marker the flames are: the small one on a minion. Kept here
    /// because the unit may be gone by the time its flames are to come down.
    flames: &'static str,
}

#[derive(Clone, Debug)]
pub struct LiandrysTorment {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_duration_seconds: f64,
    burns: Vec<Burn>,
}

impl LiandrysTorment {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "liandrys_torment",
                &["haunting_guise"],
                &["radiant_liandrys_torment"],
            ),
            price: 700,
            hp: 200,
            magic_power: 40,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
            burns: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_liandrys_torment", &["liandrys_torment"]),
            price: 1000,
            hp: 300,
            magic_power: 75,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
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
                hp,
                magic_power,
                effect_hp_percent_damage,
                effect_minion_damage_cap,
                effect_duration_seconds
            ]
        );
        self
    }

    fn duration_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds).max(DOT_TICK_RATE)
    }

    fn instance_count(&self) -> usize {
        (self.duration_ticks() / DOT_TICK_RATE).max(1)
    }

    fn instance_damage(&self, ctx: &mut StableSim<'_>, id: usize) -> Option<usize> {
        let entity_ref = ctx.get_entity(id)?;
        if !entity_ref.is_alive() {
            return None;
        }
        let total = percent_of(entity_ref.hp().1, self.effect_hp_percent_damage);
        let mut per_instance = total as f64 / self.instance_count() as f64;
        if !entity_ref.is_champion() {
            per_instance = per_instance.min(self.effect_minion_damage_cap as f64);
        }
        Some(per_instance.round() as usize)
    }

    /// Starts the burn on `target`, or starts its time over, and puts the
    /// `flames` up with it.
    fn apply_burn(&mut self, ctx: &mut StableSim<'_>, target: usize, flames: &'static str) {
        let duration = self.duration_ticks();
        let index = match self.burns.iter().position(|burn| burn.target == target) {
            Some(index) => index,
            None => {
                self.burns.push(Burn {
                    target,
                    remaining: 0,
                    until_next: DOT_TICK_RATE,
                    marker: 0,
                    flames,
                });
                self.burns.len() - 1
            }
        };
        let burn = &mut self.burns[index];
        burn.remaining = duration;
        // Up for the whole burn from one call, not kept alive instance by
        // instance, and only put up again once the burn would outlast it. The
        // price is that the flames outlast a burn cut short by its carrier's
        // death, by up to the burn's own length.
        if burn.marker < duration {
            burn.marker = duration + MARKER_SLACK_TICKS;
            let marker = burn.marker;
            refresh_buff(ctx, target, flames, &BuffV1::timed(flames, marker));
        }
    }

    fn tick_burns(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let mut kept = Vec::with_capacity(self.burns.len());
        for mut burn in std::mem::take(&mut self.burns) {
            burn.remaining = burn.remaining.saturating_sub(1);
            burn.until_next = burn.until_next.saturating_sub(1);
            burn.marker = burn.marker.saturating_sub(1);
            if burn.until_next == 0 {
                let Some(damage) = self.instance_damage(ctx, burn.target) else {
                    // Dead or gone, and its flames go with the burn: minions
                    // die burning all the time, and a marker left to run out
                    // would sit on the body, or on whatever takes its slot.
                    ctx.entity_remove_buff(burn.target, burn.flames);
                    continue;
                };
                ctx.deal_damage(caster, burn.target, 0, damage, AttackTypeV1::Item);
                burn.until_next = DOT_TICK_RATE;
            }
            if burn.remaining > 0 {
                kept.push(burn);
            }
        }
        self.burns = kept;
    }
}

impl Default for LiandrysTorment {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for LiandrysTorment {
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
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.burns.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.burns.is_empty() {
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_champion) = player_ref.champion() else {
            return;
        };
        let player_champion_id = player_champion.id();

        self.tick_burns(ctx, player_champion_id);
    }

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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        // Ability damage is a skill's hit and every tick of a skill's damage
        // over time, which the engine reports as a kind of its own. Only the
        // hit used to count, so a burning or poisoning skill started Torment
        // once and never kept it going, the way an ability that hits again
        // does.
        let ability = matches!(
            attack_type,
            AttackTypeV1::Skill | AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield
        );
        if target_ref.is_tower() || !ability {
            return;
        }
        let flames = if target_ref.is_minion() {
            SMALL_BURN_BUFF
        } else {
            BURN_BUFF
        };

        self.apply_burn(ctx, target, flames);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
