use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct ImperialMandate {
    meta: ItemMeta,
    /// Shared by both variants, the way `bloodsong` shares its own: Vulnerable
    /// is a state on the target rather than a per-carrier stack, and the two
    /// variants amplify by the same amount.
    vulnerable_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
}

impl ImperialMandate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "imperial_mandate",
                &["bandleglass_mirror"],
                &["radiant_imperial_mandate"],
            ),
            vulnerable_buff: "imperial_mandate_vulnerable",
            price: 1100,
            hp: 250,
            hp_regen: 2,
            magic_power: 50,
            skill_cooldown_mult: 5,
            effect_damaged_amplify: 7,
            effect_duration_seconds: 4.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_imperial_mandate", &["imperial_mandate"]),
            price: 1650,
            hp: 350,
            hp_regen: 3,
            magic_power: 100,
            skill_cooldown_mult: 10,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_damaged_amplify,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for ImperialMandate {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ImperialMandate {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // Command. Re-marking is a remove followed by an add rather than a
    // `has_buff` gate: extending the duration means replacing the instance, and
    // one `entity_remove_buff` clears every copy, so a multi-hit cast cannot
    // leave two amplifiers on the same target. It also sidesteps the ~3 tick
    // delay before a fresh buff becomes visible to `has_buff`, which would
    // otherwise let a fast second hit stack a duplicate.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if is_ally {
            return;
        }
        // Read before mutating: `StableEntity` holds an immutable borrow of the
        // sim that the calls below need back.
        let is_champion = {
            let Some(target_ref) = ctx.get_entity(target) else {
                return;
            };
            target_ref.is_champion()
        };
        if !is_champion {
            return;
        }

        ctx.entity_remove_buff(target, self.vulnerable_buff);
        ctx.add_buff(
            target,
            &BuffV1 {
                damaged_amplify: self.effect_damaged_amplify,
                ..BuffV1::timed(self.vulnerable_buff, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
