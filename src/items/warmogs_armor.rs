use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, has_buff, percent_of, ticks, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

const REGEN_PERIOD_TICKS: usize = 60;

#[derive(Clone, Debug)]
pub struct WarmogsArmor {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant items keep
    // independent stacks.
    recently_damaged_buff: &'static str,
    move_speed_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    regen_cooldown: usize,
    move_speed_cooldown: usize,
}

impl WarmogsArmor {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "warmogs_armor",
                &["ring_of_reincarnation"],
                &["radiant_warmogs_armor"],
            ),
            recently_damaged_buff: "warmogs_armor_recently_damaged",
            move_speed_buff: "warmogs_armor_move_speed",
            price: 1450,
            hp: 600,
            hp_regen: 6,
            effect_caster_hp_percent_heal: 3.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6.0,
            regen_cooldown: 0,
            move_speed_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_warmogs_armor", &["warmogs_armor"]),
            recently_damaged_buff: "radiant_warmogs_armor_recently_damaged",
            move_speed_buff: "radiant_warmogs_armor_move_speed",
            price: 2100,
            hp: 1000,
            hp_regen: 10,
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
        let (entity, max_hp, recently_damaged) = {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            let recently_damaged = has_buff(&champion_ref, self.recently_damaged_buff);
            (champion_ref.id(), champion_ref.hp().1, recently_damaged)
        };

        // Warmog's Heart is suppressed while the holder has taken damage recently.
        if recently_damaged {
            return;
        }

        // Regenerate a share of maximum health every second.
        if self.regen_cooldown == 0 {
            let heal = percent_of(max_hp, self.effect_caster_hp_percent_heal) as i32;
            if heal > 0 {
                ctx.add_buff(
                    entity,
                    &BuffV1 {
                        hp_regen: heal,
                        ..BuffV1::timed("", 60)
                    },
                );
            }
            self.regen_cooldown = REGEN_PERIOD_TICKS;
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
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        if attacker == entity {
            return;
        }
        ctx.add_buff(
            entity,
            &BuffV1::timed(self.recently_damaged_buff, ticks(self.effect_duration_seconds)),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
