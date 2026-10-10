use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct UnendingDespair {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
    // Non-vital stats (internals)
    /// The carrier's remaining ability cooldowns (skill, skill2, ult) last
    /// tick. `None` until the first reading after a spawn, which is only a
    /// baseline.
    last_cooldowns: Option<(usize, usize, usize)>,
}

impl UnendingDespair {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "unending_despair",
                &["ring_of_reincarnation"],
                &["radiant_unending_despair"],
            ),
            price: 750,
            hp: 250,
            defence: 15,
            effect_bonus_flat_heal: 15,
            effect_caster_hp_percent_heal: 1.5,
            // Non-vital stats (internals)
            last_cooldowns: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_unending_despair", &["unending_despair"]),
            price: 1050,
            hp: 350,
            defence: 25,
            effect_bonus_flat_heal: 20,
            effect_caster_hp_percent_heal: 2.5,
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
                defence,
                effect_bonus_flat_heal,
                effect_caster_hp_percent_heal
            ]
        );
        self
    }
}

impl Default for UnendingDespair {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for UnendingDespair {
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
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.last_cooldowns = None;
    }

    /// Anguish answers to the cast, whatever it hits. No hook reports one, so
    /// it is read the way Eternity reads it: an ability's remaining cooldown
    /// going up between two ticks.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let cooldowns = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, skill, skill2, ult)| (skill, skill2, ult));
        let cast = matches!(
            (cooldowns, self.last_cooldowns),
            (Some(now), Some(before))
                if now.0 > before.0 || now.1 > before.1 || now.2 > before.2
        );
        self.last_cooldowns = cooldowns;
        if !cast {
            return;
        }

        let Some((caster, max_hp)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.hp().1))
        else {
            return;
        };

        let heal_amount = self.effect_bonus_flat_heal as usize
            + percent_of(max_hp, self.effect_caster_hp_percent_heal);

        ctx.heal(caster, caster, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Defense, ItemTagV1::Vamp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
