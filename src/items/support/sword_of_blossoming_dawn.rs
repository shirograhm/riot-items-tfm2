use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, DISTANCE_UNITS_PER_RANGE};

#[derive(Clone, Debug)]
pub struct SwordOfBlossomingDawn {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_heal: usize,
    effect_max_heal: usize,
    effect_ad_percent_heal: f64,
    effect_ap_percent_heal: f64,
    effect_max_distance: usize,
}

impl SwordOfBlossomingDawn {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "sword_of_blossoming_dawn",
                &["forbidden_idol"],
                &["radiant_sword_of_blossoming_dawn"],
            ),
            price: 1000,
            attack_speed_mult: 20,
            hp: 200,
            magic_power: 40,
            skill_cooldown_mult: 10,
            effect_min_heal: 15,
            effect_max_heal: 60,
            effect_ad_percent_heal: 7.0,
            effect_ap_percent_heal: 7.0,
            effect_max_distance: 100,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_sword_of_blossoming_dawn",
                &["sword_of_blossoming_dawn"],
            ),
            price: 1700,
            attack_speed_mult: 35,
            hp: 350,
            magic_power: 70,
            skill_cooldown_mult: 15,
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
                attack_speed_mult,
                hp,
                magic_power,
                skill_cooldown_mult,
                effect_min_heal,
                effect_max_heal,
                effect_ad_percent_heal,
                effect_ap_percent_heal,
                effect_max_distance
            ]
        );
        self
    }

    /// Level 1 pays `effect_min_heal` and level 12 pays `effect_max_heal`,
    /// the same eleven-step ramp `bloodsong` uses for Spellblade.
    fn heal_amount(&self, level: usize, attack: usize, magic_power: usize) -> usize {
        let per_level =
            ((self.effect_max_heal - self.effect_min_heal) as f64 / 11.0).round() as usize;
        self.effect_min_heal
            + level.saturating_sub(1) * per_level
            + percent_of(attack, self.effect_ad_percent_heal)
            + percent_of(magic_power, self.effect_ap_percent_heal)
    }
}

impl Default for SwordOfBlossomingDawn {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SwordOfBlossomingDawn {
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
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // Basic attacks heal the most wounded ally in range. "Most wounded" is the
    // lowest fraction of maximum health rather than the lowest number, so a
    // chipped tank does not outrank a nearly dead carry; distance only breaks
    // ties. The carrier is not a candidate, matching the other ally-facing
    // items here (`zekes_herald`, `locket_of_the_iron_solari`).
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some((level, attack, magic_power, caster_team)) =
            ctx.get_entity(caster).map(|caster_ref| {
                let stat = caster_ref.stat();
                (
                    caster_ref.level(),
                    stat.attack,
                    stat.magic_power,
                    caster_ref.team(),
                )
            })
        else {
            return;
        };

        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut best: Option<(usize, f64, u64)> = None;
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
            let distance = ctx.distance_sq(caster, id);
            if distance > range_sq {
                continue;
            }
            let (current, max) = ally_ref.hp();
            if max == 0 {
                continue;
            }
            let wounded = current as f64 / max as f64;
            let better = best.is_none_or(|(_, best_wounded, best_distance)| {
                wounded < best_wounded || (wounded == best_wounded && distance < best_distance)
            });
            if better {
                best = Some((id, wounded, distance));
            }
        }

        let Some((ally, _, _)) = best else {
            return;
        };
        let heal = self.heal_amount(level, attack, magic_power);
        ctx.heal(caster, ally, heal);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
