use crate::config::ItemConfig;
use crate::{apply_config, ticks};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HextechAlternator {
    price: usize,
    magic_power: i32,
    effect_bonus_flat_damage: usize,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    cooldown: usize,
}

impl Default for HextechAlternator {
    fn default() -> Self {
        Self {
            price: 800,
            magic_power: 100,
            effect_bonus_flat_damage: 65,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            cooldown: 0,
        }
    }
}

impl HextechAlternator {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                magic_power,
                effect_bonus_flat_damage,
                effect_cooldown_seconds
            ]
        );
        item
    }
}

impl StableItem for HextechAlternator {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hextech_alternator".to_string()
    }

    fn icon(&self) -> String {
        "hextech_alternator".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "hextech_gunblade".to_string(),
            "ludens_tempest".to_string(),
            "night_harvester".to_string(),
            "shadowflame".to_string(),
            "stormsurge".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
    }

    /// Revved.
    ///
    /// `Item` hits are turned away so the bolt cannot pay for itself: it is dealt
    /// through the engine and comes back around through this hook. The cooldown
    /// would bound that anyway, but the gate is what makes it impossible rather
    /// than merely brief.
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
        if !matches!(attack_type, AttackTypeV1::BaseAttack | AttackTypeV1::Skill) {
            return;
        }
        if self.cooldown > 0 {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        // Set before the damage goes out, not after: the hit re-enters the
        // attack pipeline, and a cooldown written afterwards would be written
        // over whatever that pipeline did in between.
        self.cooldown = ticks(self.effect_cooldown_seconds);
        ctx.deal_damage(
            caster,
            target,
            0,
            self.effect_bonus_flat_damage,
            AttackTypeV1::Item,
        );
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
