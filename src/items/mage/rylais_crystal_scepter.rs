use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta};

/// Rimefrost's slow. The name is also the `view_buffs` binding in
/// `view/effects.view_effects` that draws frost at the slowed unit's feet
/// (`effects/rylais_frost`), so the picture is up exactly while the slow is.
/// The base item and its Radiant share it.
const SLOW_BUFF: &str = "rylais_crystal_scepter_slow";
/// The same slow on a minion, under a name of its own for frost drawn at a
/// minion's size (`effects/rylais_frost_small`).
const SMALL_SLOW_BUFF: &str = "rylais_crystal_scepter_slow_small";

#[derive(Clone, Debug)]
pub struct RylaisCrystalScepter {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
}

impl RylaisCrystalScepter {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "rylais_crystal_scepter",
                &["haunting_guise", "needlessly_large_rod"],
                &["radiant_rylais_crystal_scepter"],
            ),
            price: 700,
            hp: 150,
            magic_power: 65,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_rylais_crystal_scepter",
                &["rylais_crystal_scepter"],
            ),
            price: 950,
            hp: 200,
            magic_power: 100,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
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
                effect_slow_amount,
                effect_duration_seconds
            ]
        );
        self
    }

    /// Rimefrost: slows `target`, or starts the slow's time over. Never a
    /// turret.
    fn slow(&self, ctx: &mut StableSim<'_>, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        let name = if target_ref.is_minion() {
            SMALL_SLOW_BUFF
        } else {
            SLOW_BUFF
        };

        refresh_buff(
            ctx,
            target,
            name,
            &BuffV1 {
                move_speed_mult: -self.effect_slow_amount,
                ..BuffV1::timed(name, ticks(self.effect_duration_seconds))
            },
        );
    }
}

impl Default for RylaisCrystalScepter {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RylaisCrystalScepter {
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
        self.slow(ctx, target);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
