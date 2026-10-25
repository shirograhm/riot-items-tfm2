use crate::config::ItemConfig;
use crate::{apply_config, heal_shield_power, is_monster, percent_of, ProcQueue};
use mod_api_stable::*;

// Hunter's Machete: a jungle starter, Feral Flare's through Madred's Razors.
// Nail adds bonus physical damage to basic attacks on monsters, and its
// Omnivamp only counts against monsters, so it is a heal on every hit on one
// rather than the `vamp` stat.
#[derive(Clone, Debug)]
pub struct HuntersMachete {
    price: usize,
    effect_bonus_flat_damage: usize,
    effect_vamp: i32,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl Default for HuntersMachete {
    fn default() -> Self {
        Self {
            price: 250,
            effect_bonus_flat_damage: 15,
            effect_vamp: 7,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }
}

impl HuntersMachete {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [price, effect_bonus_flat_damage, effect_vamp]
        );
        item
    }
}

impl StableItem for HuntersMachete {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hunters_machete".to_string()
    }

    fn icon(&self) -> String {
        "hunters_machete".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["madreds_razors".to_string()]
    }

    // No stats of its own: everything it gives is its passives.
    fn stat(&self) -> BuffV1 {
        BuffV1::default()
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    // Nail on a basic attack, then Omnivamp off the whole hit. A Nail bonus
    // that had to be queued (a crit, a magic swing) comes back as its own
    // `Item` hit and is healed from then.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if !ctx
            .get_entity(target)
            .is_some_and(|target_ref| is_monster(&target_ref))
        {
            return;
        }
        if attack_type == AttackTypeV1::BaseAttack {
            self.procs.on_hit_physical(
                ctx,
                target,
                damage,
                damage_type,
                is_crit,
                self.effect_bonus_flat_damage,
            );
        }
        let heal = percent_of(*damage, self.effect_vamp.max(0) as f64);
        if heal > 0 {
            // Sustain, not a cast heal: Heal and Shield Power leaves it alone.
            heal_shield_power::unraised(|| ctx.heal(caster, caster, heal));
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Vamp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
