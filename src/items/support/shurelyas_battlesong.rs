use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, sized_range, ticks, ItemMeta};

// Inspire's speed: shared by both tiers, so two carriers refresh it rather
// than stacking it. Also the `view_buffs` binding of the pale gold swirl at
// the feet (docs/vfx/make_shurelyas_inspire.py); rename both together.
const INSPIRE_BUFF: &str = "shurelyas_battlesong_inspire";
// The ring that sweeps out from the carrier to the passive's 100 range as it
// goes off, drawn under the units (docs/vfx/make_shurelyas_speech.py).
const SPEECH_EFFECT: &str = "riot_shurelyas_speech";

// Shurelya's Battlesong: League's Inspire made a passive (no active):
// damaging an enemy champion at or under the health threshold speeds up the
// carrier and every allied champion near them, on a cooldown.
#[derive(Clone, Debug)]
pub struct ShurelyasBattlesong {
    meta: ItemMeta,
    price: usize,
    hp_regen: i32,
    magic_power: i32,
    move_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_hp_percent_threshold: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_max_distance: usize,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    cooldown: usize,
}

impl ShurelyasBattlesong {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "shurelyas_battlesong",
                &["bandleglass_mirror"],
                &["radiant_shurelyas_battlesong"],
            ),
            price: 750,
            hp_regen: 1,
            magic_power: 30,
            move_speed_mult: 4,
            skill_cooldown_mult: 15,
            effect_hp_percent_threshold: 50.0,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 4.0,
            effect_max_distance: 100,
            effect_cooldown_seconds: 55.0,
            // Non-vital stats (internals)
            cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_shurelyas_battlesong", &["shurelyas_battlesong"]),
            price: 750,
            hp_regen: 2,
            magic_power: 45,
            move_speed_mult: 6,
            skill_cooldown_mult: 20,
            effect_move_speed_mult: 30,
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
                hp_regen,
                magic_power,
                move_speed_mult,
                skill_cooldown_mult,
                effect_hp_percent_threshold,
                effect_move_speed_mult,
                effect_duration_seconds,
                effect_max_distance,
                effect_cooldown_seconds
            ]
        );
        self
    }

    // Inspire: the carrier and every living allied champion within range of
    // them. Champions only, as Locket's Devotion: speed is no use to a minion.
    fn inspire(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let caster_team = caster_ref.team();
        let range = sized_range(ctx, caster, self.effect_max_distance);
        let range_sq = range * range;

        let mut targets = vec![caster];
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            if id == caster {
                continue;
            }
            let Some(ally_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !ally_ref.is_alive() || ally_ref.team() != caster_team {
                continue;
            }
            if ctx.distance_sq(caster, id) > range_sq {
                continue;
            }
            targets.push(id);
        }

        let speed = BuffV1 {
            move_speed_mult: self.effect_move_speed_mult,
            ..BuffV1::timed(INSPIRE_BUFF, ticks(self.effect_duration_seconds))
        };
        ctx.play_view_effect(
            SPEECH_EFFECT,
            caster,
            &InputTargetV1::target(caster),
            0,
            0,
            0,
        );
        for id in targets {
            refresh_buff(ctx, id, INSPIRE_BUFF, &speed);
        }
        self.cooldown = ticks(self.effect_cooldown_seconds);
    }
}

impl Default for ShurelyasBattlesong {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ShurelyasBattlesong {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            move_speed_mult: self.move_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if self.cooldown > 0 {
            return;
        }
        // The carrier's own damage: attacks, abilities and their damage over
        // time. Other items' procs are left out.
        let own_damage = matches!(
            attack_type,
            AttackTypeV1::BaseAttack
                | AttackTypeV1::Skill
                | AttackTypeV1::Dot
                | AttackTypeV1::DotIgnoreShield
        );
        if !own_damage {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || !target_ref.is_alive() {
            return;
        }
        let (current_hp, max_hp) = target_ref.hp();
        if current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if target_ref.team() == caster_ref.team() {
            return;
        }
        self.inspire(ctx, caster);
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::MoveSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
