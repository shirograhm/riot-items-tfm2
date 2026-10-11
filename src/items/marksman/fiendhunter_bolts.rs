use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, ticks, Elapsed, ItemMeta};

// Opening Barrage's attack speed. The base item and its Radiant share the
// name, so an upgrade mid-window replaces it rather than stacking a second one.
const BARRAGE_BUFF: &str = "fiendhunter_bolts_barrage";

#[derive(Clone, Debug)]
pub struct FiendhunterBolts {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    crit_chance: i32,
    move_speed_mult: i32,
    ult_cooldown_mult: i32,
    effect_attack_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_max_stacks: usize,
    effect_percent_bonus_damage: f64,
    effect_damage_conversion: f64,
    effect_cooldown_seconds: f64,
    last_ult_cooldown: Option<usize>,
    window_ticks: usize,
    empowered_attacks: usize,
    cooldown_ticks: usize,
    // Steps the cooldown by the time gone by, so it runs through a death.
    clock: Elapsed,
}

impl FiendhunterBolts {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "fiendhunter_bolts",
                &["twin_stormblade", "scouts_slingshot"],
                &["radiant_fiendhunter_bolts"],
            ),
            price: 700,
            attack_speed_mult: 35,
            crit_chance: 20,
            move_speed_mult: 4,
            ult_cooldown_mult: 15,
            effect_attack_speed_mult: 50,
            effect_duration_seconds: 8.0,
            effect_max_stacks: 3,
            effect_percent_bonus_damage: 60.0,
            effect_damage_conversion: 15.0,
            effect_cooldown_seconds: 45.0,
            // Non-vital stats (internals)
            last_ult_cooldown: None,
            window_ticks: 0,
            empowered_attacks: 0,
            cooldown_ticks: 0,
            clock: Elapsed::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_fiendhunter_bolts", &["fiendhunter_bolts"]),
            price: 1000,
            attack_speed_mult: 55,
            crit_chance: 25,
            move_speed_mult: 4,
            ult_cooldown_mult: 25,
            effect_attack_speed_mult: 50,
            effect_duration_seconds: 8.0,
            effect_max_stacks: 3,
            effect_percent_bonus_damage: 60.0,
            effect_damage_conversion: 15.0,
            effect_cooldown_seconds: 45.0,
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
                attack_speed_mult,
                crit_chance,
                move_speed_mult,
                ult_cooldown_mult,
                effect_attack_speed_mult,
                effect_duration_seconds,
                effect_max_stacks,
                effect_percent_bonus_damage,
                effect_damage_conversion,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for FiendhunterBolts {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for FiendhunterBolts {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            move_speed_mult: self.move_speed_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    // The cooldown keeps running through a death, like an item cooldown in
    // League; only the open window and its empowered attacks are dropped.
    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.last_ult_cooldown = None;
        self.window_ticks = 0;
        self.empowered_attacks = 0;
    }

    // Opening Barrage. A cast shows up as the ult cooldown going *up* between
    // two ticks, the same reading Zeke's Convergence uses; the first reading
    // after a spawn is only a baseline.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // By the time gone by, not by one: no `update` runs for a dead carrier,
        // and the cooldown is to keep running through the death.
        let gone = self.clock.since_last(ctx);
        self.cooldown_ticks = self.cooldown_ticks.saturating_sub(gone);
        self.window_ticks = self.window_ticks.saturating_sub(1);
        if self.window_ticks == 0 {
            self.empowered_attacks = 0;
        }

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
        let duration = ticks(self.effect_duration_seconds);
        refresh_buff(
            ctx,
            carrier,
            BARRAGE_BUFF,
            &BuffV1 {
                attack_speed_mult: self.effect_attack_speed_mult,
                ..BuffV1::timed(BARRAGE_BUFF, duration)
            },
        );
        self.window_ticks = duration;
        self.empowered_attacks = self.effect_max_stacks;
        self.cooldown_ticks = ticks(self.effect_cooldown_seconds);
    }

    // An empowered attack that did not crit is made one at the item's own
    // rate. One that did keeps the engine's crit (already 200% when it gets
    // here) and adds the true damage instead. The true damage lands as an
    // `Item` hit, which this `BaseAttack` gate keeps from spending another
    // charge.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if self.empowered_attacks == 0 || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        self.empowered_attacks -= 1;

        if !is_crit {
            let ratio = 1.0 + self.effect_percent_bonus_damage / 100.0;
            *damage = (*damage as f64 * ratio).round() as usize;
            return;
        }

        let true_damage = percent_of(*damage, self.effect_damage_conversion);
        if true_damage > 0 {
            ctx.deal_damage_typed(
                caster,
                target,
                true_damage,
                DamageTypeV1::Fixed,
                AttackTypeV1::Item,
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::AttackSpeed,
            ItemTagV1::MoveSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
