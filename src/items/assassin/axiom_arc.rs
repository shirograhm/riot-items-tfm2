use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, total_lethality, ItemMeta};

#[derive(Clone, Debug)]
pub struct AxiomArc {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_ult_cooldown_mult: i32,
    effect_ult_cooldown_per_lethality: f64,
    /// Name of the buff Flux applies. Distinct per variant so the base and the
    /// Radiant version cannot be mistaken for one another.
    flux_buff: &'static str,
}

impl AxiomArc {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("axiom_arc", &["serrated_dirk"], &["radiant_axiom_arc"]),
            price: 1300,
            attack: 70,
            skill_cooldown_mult: 10,
            effect_ult_cooldown_mult: 10,
            effect_ult_cooldown_per_lethality: 0.2,
            flux_buff: "axiom_arc_flux",
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_axiom_arc", &["axiom_arc"]),
            price: 1900,
            attack: 105,
            skill_cooldown_mult: 15,
            flux_buff: "radiant_axiom_arc_flux",
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
                effect_ult_cooldown_mult,
                effect_ult_cooldown_per_lethality
            ]
        );
        self
    }
}

impl Default for AxiomArc {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for AxiomArc {
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

    // The flat CDR stat covers both bars, the way every other cooldown item in
    // this mod grants it. Flux's share is ultimate-only and is applied as a
    // buff instead, because it depends on the rest of the build.
    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // Flux. The wielder's lethality is fixed for the match — it comes from the
    // items they bought — so this is resolved once on spawn rather than ticked.
    // Guarded by `has_buff` the way `apply_adaptive_force` is: there is no
    // remove API and same-name buffs stack, so a second application on respawn
    // would double the reduction.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let lethality = total_lethality(ctx, player);

        let Some((champion_id, already_applied)) = ctx.get_player(player).and_then(|p| {
            let champion_ref = p.champion()?;
            Some((champion_ref.id(), has_buff(&champion_ref, self.flux_buff)))
        }) else {
            return;
        };

        if already_applied {
            return;
        }

        let bonus = self.effect_ult_cooldown_mult as f64
            + self.effect_ult_cooldown_per_lethality * lethality as f64;
        let buff = BuffV1 {
            ult_cooldown_mult: bonus.round() as i32,
            ..BuffV1::named(self.flux_buff)
        };
        ctx.add_buff(champion_id, &buff);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
