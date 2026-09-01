use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, percent_of, ticks, ItemMeta, ProcQueue};

#[derive(Clone, Debug)]
pub struct VoltaicCyclosword {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_lethality: usize,
    effect_hp_percent_damage: f64,
    effect_minion_damage_cap: usize,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    energized_stacks: usize,
    energized_update_tick: usize,
    /// Ticks left on Firmament's bonus lethality
    firmament_ticks: usize,
    procs: ProcQueue,
}

impl VoltaicCyclosword {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "voltaic_cyclosword",
                &["serrated_dirk"],
                &["radiant_voltaic_cyclosword"],
            ),
            price: 1300,
            attack: 60,
            skill_cooldown_mult: 10,
            effect_lethality: 12,
            effect_bonus_lethality: 6,
            effect_hp_percent_damage: 6.0,
            effect_minion_damage_cap: 200,
            effect_max_stacks: 100,
            effect_duration_seconds: 4.0,
            // Non-vital stats (internals)
            energized_stacks: 0,
            energized_update_tick: 0,
            firmament_ticks: 0,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_voltaic_cyclosword", &["voltaic_cyclosword"]),
            price: 1950,
            attack: 100,
            skill_cooldown_mult: 15,
            effect_lethality: 12,
            effect_bonus_lethality: 10,
            effect_hp_percent_damage: 10.0,
            effect_minion_damage_cap: 200,
            effect_max_stacks: 100,
            effect_duration_seconds: 4.0,
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
                skill_cooldown_mult,
                effect_lethality,
                effect_bonus_lethality,
                effect_hp_percent_damage,
                effect_minion_damage_cap,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }

    fn active_lethality(&self) -> usize {
        if self.firmament_ticks > 0 {
            self.effect_lethality + self.effect_bonus_lethality
        } else {
            self.effect_lethality
        }
    }
}

impl Default for VoltaicCyclosword {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for VoltaicCyclosword {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.energized_stacks = 0;
        self.energized_update_tick = 0;
        self.firmament_ticks = 0;
        self.procs.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        if self.energized_stacks >= self.effect_max_stacks {
            let mut bonus_damage = percent_of(target_ref.hp().0, self.effect_hp_percent_damage);
            if !target_ref.is_champion() {
                bonus_damage = bonus_damage.min(self.effect_minion_damage_cap);
            }

            // The percentage is taken off the health the target had when the
            // charged swing landed, not the health it has once the proc
            // arrives, so the number matches the hit that earned it.
            self.procs.push_physical(ctx, target, bonus_damage);
            self.firmament_ticks = ticks(self.effect_duration_seconds);
            self.energized_stacks = 0;
        }

        apply_lethality(ctx, caster, target, self.active_lethality(), damage);

        // Gain 5 energized stacks on base attacks, up to the max stacks
        if attack_type == AttackTypeV1::BaseAttack {
            self.energized_stacks = (self.energized_stacks + 5).min(self.effect_max_stacks);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);

        self.firmament_ticks = self.firmament_ticks.saturating_sub(1);

        // Add 1 energized stack per 0.2 seconds
        if self.energized_update_tick >= 12 {
            self.energized_stacks = (self.energized_stacks + 1).min(self.effect_max_stacks);
            self.energized_update_tick = 0;
        } else {
            self.energized_update_tick += 1;
        }
    }

    /// The Energized meter carries across the Radiant upgrade, so buying it
    /// mid-fight does not throw away a nearly full bar.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.energized_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.energized_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::HpPercentDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
