use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::{BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

#[derive(Clone, Debug)]
pub struct AtmasReckoning {
    price: usize,
    hp: i32,
    crit_chance: i32,
    effect_stack_crit_chance: i32,
    effect_hp_per_stack: usize,
    effect_max_stacks: usize,
    refresh_cooldown: usize,
}

impl Default for AtmasReckoning {
    fn default() -> Self {
        Self {
            price: 1450,
            hp: 500,
            crit_chance: 20,
            effect_stack_crit_chance: 5,
            effect_hp_per_stack: 1000,
            effect_max_stacks: 5,
            refresh_cooldown: 0,
        }
    }
}

impl AtmasReckoning {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_stack_crit_chance: cfg
                .effect_stack_crit_chance
                .unwrap_or(d.effect_stack_crit_chance),
            effect_hp_per_stack: cfg.effect_hp_per_stack.unwrap_or(d.effect_hp_per_stack),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            refresh_cooldown: 0,
        }
    }
}

fn apply_big_hands(
    ctx: &mut GameCtx,
    player: usize,
    refresh_cooldown: &mut usize,
    stack_crit_chance: i32,
    hp_per_stack: usize,
    max_stacks: usize,
    buff_name: &str,
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

    let stacks = (champion_ref.hp().max / hp_per_stack.max(1)).min(max_stacks);
    let crit = stack_crit_chance * stacks as i32;
    if crit <= 0 {
        return;
    }

    let entity_id = champion_ref.id();
    ctx.add_buff(
        entity_id,
        BuffState {
            name: ArrayString::try_from(buff_name).unwrap(),
            duration: BuffType::Time {
                tick: BUFF_REFRESH_DURATION_TICKS,
            },
            crit_chance: crit,
            ..Default::default()
        },
    );
    *refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
}

impl ModItemInfo for AtmasReckoning {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "atmas_reckoning"
    }

    fn icon(&self) -> &str {
        "atmas_reckoning"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_atmas_reckoning".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            "atmas_reckoning_big_hands",
        );
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            "atmas_reckoning_big_hands",
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantAtmasReckoning {
    price: usize,
    hp: i32,
    crit_chance: i32,
    effect_stack_crit_chance: i32,
    effect_hp_per_stack: usize,
    effect_max_stacks: usize,
    refresh_cooldown: usize,
}

impl Default for RadiantAtmasReckoning {
    fn default() -> Self {
        Self {
            price: 2050,
            hp: 850,
            crit_chance: 25,
            effect_stack_crit_chance: 5,
            effect_hp_per_stack: 1000,
            effect_max_stacks: 5,
            refresh_cooldown: 0,
        }
    }
}

impl RadiantAtmasReckoning {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_stack_crit_chance: cfg
                .effect_stack_crit_chance
                .unwrap_or(d.effect_stack_crit_chance),
            effect_hp_per_stack: cfg.effect_hp_per_stack.unwrap_or(d.effect_hp_per_stack),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            refresh_cooldown: 0,
        }
    }
}

impl ModItemInfo for RadiantAtmasReckoning {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_atmas_reckoning"
    }

    fn icon(&self) -> &str {
        "radiant_atmas_reckoning"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["atmas_reckoning".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            "radiant_atmas_reckoning_big_hands",
        );
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        apply_big_hands(
            ctx,
            player,
            &mut self.refresh_cooldown,
            self.effect_stack_crit_chance,
            self.effect_hp_per_stack,
            self.effect_max_stacks,
            "radiant_atmas_reckoning_big_hands",
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}
