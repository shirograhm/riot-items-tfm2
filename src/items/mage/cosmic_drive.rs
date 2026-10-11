use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{add_stack, apply_config, is_enemy_champion, ticks, ItemMeta};

// Spelldance's movement speed. The base item and its Radiant share the name,
// so an upgrade mid-buff replaces it rather than stacking a second one.
const SPELLDANCE_BUFF: &str = "cosmic_drive_spelldance";

#[derive(Clone, Debug)]
pub struct CosmicDrive {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl CosmicDrive {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "cosmic_drive",
                &["hextech_alternator", "winged_moonplate"],
                &["radiant_cosmic_drive"],
            ),
            price: 850,
            hp: 150,
            magic_power: 50,
            skill_cooldown_mult: 10,
            move_speed_mult: 4,
            effect_move_speed_mult: 8,
            effect_duration_seconds: 4.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_cosmic_drive", &["cosmic_drive"]),
            price: 1000,
            hp: 250,
            magic_power: 75,
            skill_cooldown_mult: 15,
            move_speed_mult: 4,
            effect_move_speed_mult: 8,
            effect_duration_seconds: 4.0,
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
                magic_power,
                skill_cooldown_mult,
                move_speed_mult,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for CosmicDrive {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for CosmicDrive {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    // Spelldance. Any hit counts - abilities, basic attacks and item procs
    // alike - as long as it lands as magic or true damage on an enemy champion.
    // One stack at most: the engine caps it and restarts its duration, which
    // also holds when an ability hits several champions in the same tick.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if *damage == 0 || !matches!(damage_type, DamageTypeV1::Ap | DamageTypeV1::Fixed) {
            return;
        }
        if !is_enemy_champion(ctx, caster, target) {
            return;
        }

        add_stack(
            ctx,
            caster,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(SPELLDANCE_BUFF, ticks(self.effect_duration_seconds))
            },
            1,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
