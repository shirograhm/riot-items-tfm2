use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, percent_of, ItemMeta, ProcQueue};

/// The Shield Reaver bonus this hit earned, or zero when the passive does not
/// apply. The shield is read here, at the moment of the hit, so the proc that
/// lands a moment later is the one the attack earned rather than one judged
/// against whatever shield the target has by then.
fn shield_reaver(
    ctx: &mut StableSim<'_>,
    caster: usize,
    target: usize,
    flat: usize,
    ad_percent: f64,
) -> usize {
    let shielded_champion = ctx
        .get_entity(target)
        .map(|t| t.is_champion() && t.shield() > 0)
        .unwrap_or(false);
    if !shielded_champion {
        return 0;
    }
    let caster_ad = ctx.get_entity(caster).map(|c| c.stat().attack).unwrap_or(0);
    flat + percent_of(caster_ad, ad_percent)
}

#[derive(Clone, Debug)]
pub struct SerpentsFang {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl SerpentsFang {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "serpents_fang",
                &["serrated_dirk"],
                &["radiant_serpents_fang"],
            ),
            price: 1200,
            attack: 60,
            effect_lethality: 15,
            effect_bonus_flat_damage: 50,
            effect_ad_percent_damage: 10.0,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_serpents_fang", &["serpents_fang"]),
            price: 1800,
            attack: 100,
            effect_lethality: 15,
            effect_bonus_flat_damage: 85,
            effect_ad_percent_damage: 15.0,
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
                effect_lethality,
                effect_bonus_flat_damage,
                effect_ad_percent_damage
            ]
        );
        self
    }
}

impl Default for SerpentsFang {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SerpentsFang {
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
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_target_tower = target_ref.is_tower();

        if !is_target_tower {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }

        let bonus = shield_reaver(
            ctx,
            caster,
            target,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        self.procs.push_physical(ctx, target, bonus);
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    /// Lands the Shield Reaver damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::ShieldBreak]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
