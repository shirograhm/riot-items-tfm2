use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_stacks, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct BlackfireTorch {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    stack_buff: &'static str,
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_stack_magic_power: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl BlackfireTorch {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "blackfire_torch",
                &["staff_of_rapture"],
                &["radiant_blackfire_torch"],
            ),
            stack_buff: "blackfire_torch_buff",
            price: 1300,
            magic_power: 130,
            skill_cooldown_mult: 15,
            effect_stack_magic_power: 10,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_blackfire_torch", &["blackfire_torch"]),
            stack_buff: "radiant_blackfire_torch_buff",
            price: 1900,
            magic_power: 175,
            skill_cooldown_mult: 25,
            effect_stack_magic_power: 30,
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
                magic_power,
                skill_cooldown_mult,
                effect_stack_magic_power,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for BlackfireTorch {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for BlackfireTorch {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        _target: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = buff_stacks(&entity_ref, self.stack_buff);
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                &BuffV1 {
                    magic_power: self.effect_stack_magic_power,
                    ..BuffV1::timed(self.stack_buff, ticks(self.effect_duration_seconds))
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
