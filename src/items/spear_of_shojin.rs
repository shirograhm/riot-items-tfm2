use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_stacks, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct SpearOfShojin {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    stack_buff: &'static str,
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_stack_attack_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl SpearOfShojin {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("spear_of_shojin", &["phage"], &["radiant_spear_of_shojin"]),
            stack_buff: "spear_of_shojin_buff",
            price: 1400,
            hp: 350,
            attack: 35,
            skill_cooldown_mult: 10,
            effect_stack_attack_mult: 3,
            effect_max_stacks: 4,
            effect_duration_seconds: 5.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_spear_of_shojin", &["spear_of_shojin"]),
            stack_buff: "radiant_spear_of_shojin_buff",
            price: 2200,
            hp: 600,
            attack: 60,
            skill_cooldown_mult: 20,
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
                attack,
                skill_cooldown_mult,
                effect_stack_attack_mult,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }

    fn add_attack_stack(&self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = buff_stacks(&caster_ref, self.stack_buff);
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                &BuffV1 {
                    attack_mult: self.effect_stack_attack_mult,
                    ..BuffV1::timed(self.stack_buff, ticks(self.effect_duration_seconds))
                },
            );
        }
    }
}

impl Default for SpearOfShojin {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SpearOfShojin {
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
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, caster: usize, target: usize) {
        self.add_attack_stack(ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
