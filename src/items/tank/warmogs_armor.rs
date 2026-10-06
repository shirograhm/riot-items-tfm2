use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, percent_of, ticks, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS, TICKS_PER_SECOND,
};

/// Warmog's Heart heals in a pulse every half second, each one half of the
/// per-second share, so the rate the tooltip states is unchanged.
const REGEN_PERIOD_TICKS: usize = 30;

#[derive(Clone, Debug)]
pub struct WarmogsArmor {
    meta: ItemMeta,
    move_speed_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    regen_cooldown: usize,
    move_speed_cooldown: usize,
    /// Ticks left before Warmog's Heart is live again; every hit taken
    /// restarts it at `effect_duration_seconds`.
    damaged_ticks: usize,
}

impl WarmogsArmor {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "warmogs_armor",
                &["ring_of_reincarnation"],
                &["radiant_warmogs_armor"],
            ),
            move_speed_buff: "warmogs_armor_move_speed",
            price: 750,
            hp: 300,
            hp_regen: 3,
            effect_caster_hp_percent_heal: 3.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6.0,
            // Non-vital stats (internals)
            regen_cooldown: 0,
            move_speed_cooldown: 0,
            damaged_ticks: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_warmogs_armor", &["warmogs_armor"]),
            move_speed_buff: "warmogs_armor_move_speed",
            price: 1050,
            hp: 500,
            hp_regen: 5,
            effect_caster_hp_percent_heal: 3.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6.0,
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
                effect_caster_hp_percent_heal,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        self
    }

    fn apply_passive(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // Warmog's Heart is suppressed while the holder has taken damage recently.
        if self.damaged_ticks > 0 {
            self.damaged_ticks -= 1;
            return;
        }

        let (entity, max_hp) = {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            (champion_ref.id(), champion_ref.hp().1)
        };

        // Heal a share of maximum health every `REGEN_PERIOD_TICKS`. A direct
        // heal rather than an `hp_regen` buff: the engine applies regen on its
        // own schedule, and a heal is counted like every other item's.
        if self.regen_cooldown == 0 {
            let share =
                self.effect_caster_hp_percent_heal * REGEN_PERIOD_TICKS as f64 / TICKS_PER_SECOND;
            let heal = percent_of(max_hp, share);
            if heal > 0 {
                ctx.heal(entity, entity, heal);
            }
            // The pulse lands on the tick the countdown reaches 0.
            self.regen_cooldown = REGEN_PERIOD_TICKS - 1;
        } else {
            self.regen_cooldown -= 1;
        }

        // ...and grant movement speed as a fixed-duration buff.
        if self.move_speed_cooldown == 0 {
            ctx.add_buff(
                entity,
                &BuffV1 {
                    move_speed_mult: self.effect_move_speed_mult,
                    ..BuffV1::timed(self.move_speed_buff, BUFF_REFRESH_DURATION_TICKS)
                },
            );
            self.move_speed_cooldown = BUFF_REFRESH_PERIOD_TICKS;
        } else {
            self.move_speed_cooldown -= 1;
        }
    }
}

impl Default for WarmogsArmor {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for WarmogsArmor {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.regen_cooldown = 0;
        self.move_speed_cooldown = 0;
        self.damaged_ticks = 0;
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    // Counted on the item rather than kept as a buff on the carrier: a buff
    // replaced on every hit is not visible to `has_buff` for ~3 ticks, and
    // `update` took each of those gaps for the timer having run out.
    fn on_damaged(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _player: usize,
        _entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        self.damaged_ticks = ticks(self.effect_duration_seconds);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
