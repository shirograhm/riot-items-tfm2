use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, heal_shield_power, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

#[derive(Clone, Debug)]
pub struct Dawncore {
    meta: ItemMeta,
    // Shared by both variants, as Big Hands is: First Light follows the
    // carrier's Ability Haste, which is the same whichever tier it holds.
    first_light_buff: &'static str,
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    effect_heal_shield_power: f64,
    effect_stack_heal_shield_power: f64,
    effect_stack_magic_power: i32,
    effect_haste_per_stack: usize,
    // Non-vital stats (internals)
    stacks: usize,
    refresh_cooldown: usize,
}

impl Dawncore {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("dawncore", &["forbidden_idol"], &["radiant_dawncore"]),
            first_light_buff: "dawncore_first_light",
            price: 850,
            magic_power: 40,
            hp_regen: 3,
            effect_heal_shield_power: 16.0,
            effect_stack_heal_shield_power: 2.0,
            effect_stack_magic_power: 15,
            effect_haste_per_stack: 10,
            // Non-vital stats (internals)
            stacks: 0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dawncore", &["dawncore"]),
            price: 750,
            magic_power: 60,
            hp_regen: 4,
            effect_heal_shield_power: 16.0,
            effect_stack_heal_shield_power: 2.0,
            effect_stack_magic_power: 15,
            effect_haste_per_stack: 10,
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
                hp_regen,
                effect_heal_shield_power,
                effect_stack_heal_shield_power,
                effect_stack_magic_power,
                effect_haste_per_stack
            ]
        );
        self
    }

    // First Light: a step for every `effect_haste_per_stack` of the carrier's
    // Ability Haste, worth Ability Power as a buff and Heal and Shield Power
    // as this item's report ([`StableItem::update`]). The haste is the sum of
    // every buff's `skill_cooldown_mult`, item stat blocks included, which is
    // what the engine's own cooldowns read; First Light's grant adds none.
    // Re-added on the overlap cycle Big Hands uses, so it follows the haste up
    // and down.
    fn apply_first_light(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some((champion_id, haste)) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion_ref| {
                let haste: i32 = (0..champion_ref.buff_count())
                    .filter_map(|index| champion_ref.buff_at(index))
                    .map(|buff| buff.skill_cooldown_mult)
                    .sum();
                (champion_ref.id(), haste)
            })
        else {
            return;
        };

        self.stacks = haste.max(0) as usize / self.effect_haste_per_stack.max(1);
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
        let power = self.effect_stack_magic_power * self.stacks as i32;
        if power <= 0 {
            return;
        }
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                magic_power: power,
                ..BuffV1::timed(self.first_light_buff, BUFF_REFRESH_DURATION_TICKS)
            },
        );
    }
}

impl Default for Dawncore {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Dawncore {
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
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.stacks = 0;
        self.refresh_cooldown = 0;
        self.apply_first_light(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.apply_first_light(ctx, player);
        heal_shield_power::note(
            ctx,
            player,
            self.effect_heal_shield_power
                + self.effect_stack_heal_shield_power * self.stacks as f64,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::HpRegen, ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
