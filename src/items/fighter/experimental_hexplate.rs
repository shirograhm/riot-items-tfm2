use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, Elapsed, ItemMeta};
const OVERDRIVE_BUFF: &str = "experimental_hexplate_overdrive";

#[derive(Clone, Debug)]
pub struct ExperimentalHexplate {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    ult_cooldown_mult: i32,
    effect_attack_speed_mult: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    last_ult_cooldown: Option<usize>,
    cooldown_ticks: usize,
    /// Steps the cooldown by the time gone by, so it runs through a death.
    clock: Elapsed,
}

impl ExperimentalHexplate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "experimental_hexplate",
                &["ring_of_reincarnation", "scouts_slingshot"],
                &["radiant_experimental_hexplate"],
            ),
            price: 600,
            hp: 150,
            attack_speed_mult: 30,
            move_speed_mult: 0,
            ult_cooldown_mult: 15,
            effect_attack_speed_mult: 32,
            effect_move_speed_mult: 16,
            effect_duration_seconds: 8.0,
            effect_cooldown_seconds: 32.0,
            // Non-vital stats (internals)
            last_ult_cooldown: None,
            cooldown_ticks: 0,
            clock: Elapsed::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_experimental_hexplate", &["experimental_hexplate"]),
            price: 950,
            hp: 200,
            attack_speed_mult: 50,
            move_speed_mult: 5,
            ult_cooldown_mult: 25,
            effect_attack_speed_mult: 32,
            effect_move_speed_mult: 16,
            effect_duration_seconds: 8.0,
            effect_cooldown_seconds: 32.0,
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
                attack_speed_mult,
                move_speed_mult,
                ult_cooldown_mult,
                effect_attack_speed_mult,
                effect_move_speed_mult,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for ExperimentalHexplate {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ExperimentalHexplate {
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
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    // The cooldown keeps running through a death, like an item cooldown in
    // League; the first cooldown reading of the new life is only a baseline.
    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.last_ult_cooldown = None;
    }

    // Overdrive. A cast shows up as the ult cooldown going *up* between two
    // ticks, the same reading Zeke's Convergence uses.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // By the time gone by, not by one: no `update` runs for a dead carrier,
        // and the cooldown is to keep running through the death.
        let gone = self.clock.since_last(ctx);
        self.cooldown_ticks = self.cooldown_ticks.saturating_sub(gone);

        let ult_cooldown = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, _, _, ult)| ult);
        let cast = matches!(
            (ult_cooldown, self.last_ult_cooldown),
            (Some(now), Some(before)) if now > before
        );
        self.last_ult_cooldown = ult_cooldown;
        if !cast || self.cooldown_ticks > 0 {
            return;
        }

        let Some(carrier) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        refresh_buff(
            ctx,
            carrier,
            OVERDRIVE_BUFF,
            &BuffV1 {
                attack_speed_mult: self.effect_attack_speed_mult,
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(OVERDRIVE_BUFF, ticks(self.effect_duration_seconds))
            },
        );
        // From the cast, not from the end of Overdrive.
        self.cooldown_ticks = ticks(self.effect_cooldown_seconds);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        let mut tags = vec![ItemTagV1::Hp, ItemTagV1::AttackSpeed];
        if self.move_speed_mult > 0 {
            tags.push(ItemTagV1::MoveSpeed);
        }
        tags.push(ItemTagV1::CooltimeReduce);
        tags
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
