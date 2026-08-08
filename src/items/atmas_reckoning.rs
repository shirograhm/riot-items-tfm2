use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta, BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

fn apply_big_hands(
    ctx: &mut StableSim<'_>,
    player: usize,
    refresh_cooldown: &mut usize,
    stack_crit_chance: i32,
    hp_per_stack: usize,
    max_stacks: usize,
    name: &str,
) {
    if *refresh_cooldown > 0 {
        *refresh_cooldown -= 1;
        return;
    }

    let Some(player_ref) = ctx.get_player(player) else {
        return;
    };
    let Some(champion_ref) = player_ref.champion() else {
        return;
    };

    let stacks = (champion_ref.hp().1 / hp_per_stack.max(1)).min(max_stacks);
    let crit = stack_crit_chance * stacks as i32;
    if crit <= 0 {
        return;
    }

    let entity_id = champion_ref.id();
    ctx.add_buff(
        entity_id,
        &BuffV1 {
            crit_chance: crit,
            ..BuffV1::timed(name, BUFF_REFRESH_DURATION_TICKS)
        },
    );
    *refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
}

#[derive(Clone, Debug)]
pub struct AtmasReckoning {
    meta: ItemMeta,
    big_hands_buff: &'static str,
    price: usize,
    hp: i32,
    crit_chance: i32,
    effect_stack_crit_chance: i32,
    effect_hp_per_stack: usize,
    effect_max_stacks: usize,
    refresh_cooldown: usize,
}

impl AtmasReckoning {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "atmas_reckoning",
                &["ring_of_reincarnation"],
                &["radiant_atmas_reckoning"],
            ),
            big_hands_buff: "atmas_reckoning_big_hands",
            price: 1450,
            hp: 500,
            crit_chance: 20,
            effect_stack_crit_chance: 5,
            effect_hp_per_stack: 1000,
            effect_max_stacks: 5,
            // Non-vital stats (internals)
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_atmas_reckoning", &["atmas_reckoning"]),
            big_hands_buff: "atmas_reckoning_big_hands",
            price: 2050,
            hp: 850,
            crit_chance: 25,
            effect_stack_crit_chance: 5,
            effect_hp_per_stack: 1000,
            effect_max_stacks: 5,
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
                crit_chance,
                effect_stack_crit_chance,
                effect_hp_per_stack,
                effect_max_stacks
            ]
        );
        self
    }
}

impl Default for AtmasReckoning {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for AtmasReckoning {
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
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            self.big_hands_buff,
        );
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            self.big_hands_buff,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
