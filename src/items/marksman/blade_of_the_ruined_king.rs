use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta, PROC_DELAY_SECONDS};

#[derive(Clone, Debug)]
pub struct BladeOfTheRuinedKing {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    vamp: i32,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    /// `(ticks left, target, damage)` for hits that have not landed yet.
    pending_hits: Vec<(usize, usize, usize)>,
}

impl BladeOfTheRuinedKing {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "blade_of_the_ruined_king",
                &["hearthbound_axe", "ruinous_blade"],
                &["radiant_blade_of_the_ruined_king"],
            ),
            price: 1450,
            attack: 50,
            attack_speed_mult: 25,
            vamp: 5,
            effect_hp_percent_damage: 5.0,
            effect_minion_damage_cap: 50,
            // Non-vital state (internal)
            pending_hits: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_blade_of_the_ruined_king",
                &["blade_of_the_ruined_king"],
            ),
            price: 2100,
            attack: 60,
            attack_speed_mult: 50,
            vamp: 10,
            effect_hp_percent_damage: 5.0,
            effect_minion_damage_cap: 50,
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
                attack,
                attack_speed_mult,
                vamp,
                effect_hp_percent_damage,
                effect_minion_damage_cap,
            ]
        );
        self
    }
}

impl Default for BladeOfTheRuinedKing {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for BladeOfTheRuinedKing {
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
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            vamp: self.vamp,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        let mut bonus_damage = percent_of(target_ref.hp().0, self.effect_hp_percent_damage as f64);
        if !target_ref.is_champion() {
            bonus_damage = bonus_damage.clamp(0, self.effect_minion_damage_cap);
        }

        // Fixed here rather than re-read when it lands: the health the tooltip
        // promises a share of is the health the target had when it was hit.
        self.pending_hits
            .push((ticks(PROC_DELAY_SECONDS), target, bonus_damage));
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.pending_hits.clear();
    }

    /// Lands the hits whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.pending_hits.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion| champion.id())
        else {
            return;
        };

        // Collected first: the drain cannot deal damage while it still holds the
        // list, and `ctx` is needed for both.
        let mut landed = Vec::new();
        self.pending_hits.retain_mut(|(remaining, target, damage)| {
            *remaining = remaining.saturating_sub(1);
            if *remaining > 0 {
                return true;
            }
            landed.push((*target, *damage));
            false
        });

        for (target, damage) in landed {
            // The target can die inside the delay, and damage dealt to a corpse
            // still lands in the damage statistics.
            if ctx
                .get_entity(target)
                .is_some_and(|target_ref| target_ref.is_alive())
            {
                ctx.deal_damage(caster, target, damage, 0, AttackTypeV1::Item);
            }
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::AttackSpeed,
            ItemTagV1::Vamp,
            ItemTagV1::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
