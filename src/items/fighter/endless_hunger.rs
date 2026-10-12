use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    add_stack, apply_config, percent_of_i32, ticks, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

// Famine's ability haste, re-added on a cycle so it follows the carrier's
// Attack Damage. The base item and its Radiant share both buff names.
const FAMINE_BUFF: &str = "endless_hunger_famine";
const FEAST_BUFF: &str = "endless_hunger_feast";

#[derive(Clone, Debug)]
pub struct EndlessHunger {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    vamp: i32,
    toughness: usize,
    effect_skill_cooldown_mult: i32,
    effect_ad_percent_haste: f64,
    effect_vamp: i32,
    effect_duration_seconds: f64,
    refresh_cooldown: usize,
}

impl EndlessHunger {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "endless_hunger",
                &["ruinous_blade", "bf_sword"],
                &["radiant_endless_hunger"],
            ),
            price: 700,
            attack: 40,
            vamp: 5,
            toughness: 20,
            effect_skill_cooldown_mult: 5,
            effect_ad_percent_haste: 5.0,
            effect_vamp: 15,
            effect_duration_seconds: 5.0,
            // Non-vital stats (internals)
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_endless_hunger", &["endless_hunger"]),
            price: 1000,
            attack: 60,
            vamp: 10,
            toughness: 25,
            effect_skill_cooldown_mult: 5,
            effect_ad_percent_haste: 5.0,
            effect_vamp: 15,
            effect_duration_seconds: 5.0,
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
                vamp,
                toughness,
                effect_skill_cooldown_mult,
                effect_ad_percent_haste,
                effect_vamp,
                effect_duration_seconds
            ]
        );
        self
    }

    // Famine. Same overlap cycle as Riftmaker's Infusion: a buff a little
    // longer than the period it is re-added on, so the haste never lapses
    // while it tracks the Attack Damage up and down.
    fn apply_famine(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some((champion_id, attack)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.stat().attack))
        else {
            return;
        };

        let haste = self.effect_skill_cooldown_mult
            + percent_of_i32(attack as i32, self.effect_ad_percent_haste);
        if haste <= 0 {
            return;
        }

        ctx.add_buff(
            champion_id,
            &BuffV1 {
                skill_cooldown_mult: haste,
                ..BuffV1::timed(FAMINE_BUFF, BUFF_REFRESH_DURATION_TICKS)
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }

    // One stack at most, refreshed: a double takedown in one tick still
    // grants the Omnivamp once.
    fn feast(&self, ctx: &mut StableSim<'_>, entity: usize) {
        add_stack(
            ctx,
            entity,
            &BuffV1 {
                vamp: self.effect_vamp,
                ..BuffV1::timed(FEAST_BUFF, ticks(self.effect_duration_seconds))
            },
            1,
        );
    }
}

impl Default for EndlessHunger {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for EndlessHunger {
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
            vamp: self.vamp,
            toughness: self.toughness,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_famine(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_famine(ctx, player);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        if ctx.get_entity(victim).is_some_and(|v| v.is_champion()) {
            self.feast(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.feast(ctx, entity);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::Vamp,
            ItemTagV1::Toughness,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
