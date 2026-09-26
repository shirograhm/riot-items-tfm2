use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

// Witch's Path: Killing a unit grants 0.5 armor, up to a maximum of 15.
//
// The kill count lives on the item, which survives death; the armor it buys is a
// short buff re-applied while the item is held. A permanent buff would outlive
// the item: after the upgrade into Zhonya's Hourglass (whose flat armor already
// covers the full passive) it would stay on the champion until the next death.
#[derive(Clone, Debug)]
pub struct SeekersArmguard {
    witchs_path_buff: &'static str,
    price: usize,
    magic_power: i32,
    defence: i32,
    effect_stack_defence: f64,
    effect_max_stacks: usize,
    // Non-vital stats (internals)
    stacks: usize,
    refresh_cooldown: usize,
}

impl Default for SeekersArmguard {
    fn default() -> Self {
        Self {
            witchs_path_buff: "seekers_armguard_witchs_path",
            price: 500,
            magic_power: 30,
            defence: 20,
            effect_stack_defence: 0.5,
            effect_max_stacks: 30,
            // Non-vital stats (internals)
            stacks: 0,
            refresh_cooldown: 0,
        }
    }
}

impl SeekersArmguard {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                magic_power,
                defence,
                effect_stack_defence,
                effect_max_stacks
            ]
        );
        item
    }

    /// Armor is whole points, so half-point steps land on every second kill.
    fn bonus_defence(&self) -> i32 {
        (self.stacks as f64 * self.effect_stack_defence).floor() as i32
    }

    /// Replaces rather than adds, so a new kill shows at once instead of
    /// overlapping the previous total for the rest of its duration.
    fn apply_witchs_path(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let bonus = self.bonus_defence();
        if bonus <= 0 {
            return;
        }
        let Some(champion_id) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id())
        else {
            return;
        };
        refresh_buff(
            ctx,
            champion_id,
            self.witchs_path_buff,
            &BuffV1 {
                defence: bonus,
                ..BuffV1::timed(self.witchs_path_buff, BUFF_REFRESH_DURATION_TICKS)
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl StableItem for SeekersArmguard {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "seekers_armguard".to_string()
    }

    fn icon(&self) -> String {
        "seekers_armguard".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "spirit_crystal".to_string(),
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["zhonyas_hourglass".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_witchs_path(ctx, player);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }
        self.apply_witchs_path(ctx, player);
    }

    // Any unit but a turret: minions, monsters and champions all count.
    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        player: usize,
        _entity: usize,
        victim: usize,
    ) {
        if self.stacks >= self.effect_max_stacks {
            return;
        }
        let Some(is_tower) = ctx.get_entity(victim).map(|v| v.is_tower()) else {
            return;
        };
        if is_tower {
            return;
        }
        let before = self.bonus_defence();
        self.stacks += 1;
        if self.bonus_defence() != before {
            self.apply_witchs_path(ctx, player);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
