use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_tiered_adaptive_force, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct MirageBlade {
    meta: ItemMeta,
    adaptive_force_buff: &'static str,
    // The previous tier's buff and its Adaptive Force: on a holder of that item
    // this one only tops up the difference. `None` on the base variant.
    upgrades_from: Option<(&'static str, i32)>,
    price: usize,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    adaptive_force: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl MirageBlade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "mirage_blade",
                &["scouts_slingshot"],
                &["radiant_mirage_blade"],
            ),
            adaptive_force_buff: "mirage_blade_adaptive_force",
            upgrades_from: None,
            price: 750,
            attack_speed_mult: 35,
            move_speed_mult: 10,
            adaptive_force: 30,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_mirage_blade", &["mirage_blade"]),
            adaptive_force_buff: "radiant_mirage_blade_adaptive_force",
            upgrades_from: Some(("mirage_blade_adaptive_force", 30)),
            price: 1050,
            attack_speed_mult: 60,
            move_speed_mult: 15,
            adaptive_force: 50,
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
                move_speed_mult,
                adaptive_force,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for MirageBlade {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for MirageBlade {
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
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        apply_tiered_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            self.adaptive_force_buff,
            self.upgrades_from,
        );
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        apply_tiered_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            self.adaptive_force_buff,
            self.upgrades_from,
        );
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        player: usize,
        _entity: usize,
        _victim: usize,
    ) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();

        let is_buff_applied = has_buff(&champion_ref, "mirage_blade_move_speed");
        if is_buff_applied {
            ctx.entity_remove_buff(champion_id, "mirage_blade_move_speed");
        }
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(
                    "mirage_blade_move_speed",
                    ticks(self.effect_duration_seconds),
                )
            },
        )
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
