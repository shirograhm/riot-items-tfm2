use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, percent_of, ticks, ItemMeta};

const BURN_TICK_RATE: usize = 12;

#[derive(Clone, Debug)]
pub struct LiandrysTorment {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_duration_seconds: f64,
    refresh_cooldown: usize,
}

impl LiandrysTorment {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "liandrys_torment",
                &["haunting_guise"],
                &["radiant_liandrys_torment"],
            ),
            price: 1400,
            hp: 350,
            magic_power: 75,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 40,
            effect_duration_seconds: 3.0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_liandrys_torment", &["liandrys_torment"]),
            price: 2000,
            hp: 550,
            magic_power: 150,
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
}

impl Default for LiandrysTorment {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for LiandrysTorment {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(caster) = player_ref.champion() else {
            return;
        };
        let caster_team = caster.team();

        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            let Some(entity_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !entity_ref.is_alive() || entity_ref.team() == caster_team {
                continue;
            }

            let is_burning = has_buff(&entity_ref, "liandrys_torment_burn");
            let amount_to_burn = percent_of(entity_ref.hp().max, self.effect_hp_percent_damage);
            let mut bonus_damage = amount_to_burn as f64
                / (self.effect_duration_seconds * 60.0 / BURN_TICK_RATE as f64);
            if !entity_ref.is_champion() {
                bonus_damage = bonus_damage.clamp(0.0, self.effect_minion_damage_cap as f64);
            }

            if is_burning {
                ctx.deal_damage(
                    player,
                    id,
                    0,
                    bonus_damage.round() as usize,
                    AttackType::Item,
                );
                self.refresh_cooldown = BURN_TICK_RATE;
            }
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = has_buff(&target_ref, "liandrys_torment_burn");
        if is_cooldown_ticking {
            return;
        }
        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: ticks(self.effect_duration_seconds),
                },
                name: buff_name("liandrys_torment_burn"),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
