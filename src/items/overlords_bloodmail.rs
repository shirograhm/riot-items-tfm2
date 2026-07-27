use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, buff_name, percent_of, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

#[derive(Clone, Debug)]
pub struct OverlordsBloodmail {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    tyranny_buff: &'static str,
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_attack: f64,
    refresh_cooldown: usize,
}

impl OverlordsBloodmail {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "overlords_bloodmail",
                &["phage"],
                &["radiant_overlords_bloodmail"],
            ),
            tyranny_buff: "overlords_bloodmail_tyranny",
            price: 1400,
            attack: 25,
            hp: 400,
            effect_caster_hp_percent_attack: 2.5,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_overlords_bloodmail", &["overlords_bloodmail"]),
            tyranny_buff: "radiant_overlords_bloodmail_tyranny",
            price: 2000,
            attack: 40,
            hp: 650,
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
            [price, attack, hp, effect_caster_hp_percent_attack]
        );
        self
    }

    fn apply_tyranny(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let target = percent_of(champion_ref.hp().max, self.effect_caster_hp_percent_attack) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = champion_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: buff_name(self.tyranny_buff),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                attack: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for OverlordsBloodmail {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for OverlordsBloodmail {
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
            attack: self.attack,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_tyranny(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_tyranny(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
