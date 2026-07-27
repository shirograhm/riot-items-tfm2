use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Heartsteel {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    stack_buff: &'static str,
    cooldown_buff: &'static str,
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_cooldown_seconds: f64,
    accumulated_bonus_hp: i32,
}

impl Heartsteel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "heartsteel",
                &["ring_of_reincarnation"],
                &["radiant_heartsteel"],
            ),
            stack_buff: "heartsteel_stack",
            cooldown_buff: "heartsteel_cooldown",
            price: 1500,
            hp: 500,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 15.0,
            effect_cooldown_seconds: 15.0,
            accumulated_bonus_hp: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_heartsteel", &["heartsteel"]),
            stack_buff: "radiant_heartsteel_stack",
            cooldown_buff: "radiant_heartsteel_cooldown",
            price: 2100,
            hp: 800,
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
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_bonus_hp_percent_of_damage,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for Heartsteel {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Heartsteel {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_bonus_hp <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                hp: self.accumulated_bonus_hp,
                name: buff_name(self.stack_buff),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let is_cooldown_ticking = has_buff(&caster_ref, self.cooldown_buff);
        if is_cooldown_ticking {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.hp().max, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;

        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: ticks(self.effect_cooldown_seconds),
                },
                name: buff_name(self.cooldown_buff),
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Permanent,
                hp: bonus_hp,
                name: buff_name(self.stack_buff),
                ..Default::default()
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
