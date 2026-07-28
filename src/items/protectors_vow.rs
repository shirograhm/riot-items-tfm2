use mod_api::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, buff_name, percent_of, ItemMeta, BUFF_REFRESH_DURATION_TICKS,
    BUFF_REFRESH_PERIOD_TICKS,
};

#[derive(Clone, Debug)]
pub struct ProtectorsVow {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant items keep
    // independent stacks.
    awe_buff: &'static str,
    price: usize,
    hp: i32,
    defence: i32,
    // Radiant-only. Zero on the base variant, which reduces no cooldowns and
    // correspondingly does not carry the `CooltimeReduce` tag.
    skill_cooldown_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    refresh_cooldown: usize,
}

impl ProtectorsVow {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "protectors_vow",
                &["ring_of_reincarnation", "gatekeepers_armor"],
                &["radiant_protectors_vow"],
            ),
            awe_buff: "protectors_vow_awe",
            price: 1300,
            hp: 350,
            defence: 50,
            skill_cooldown_mult: 0,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_protectors_vow", &["protectors_vow"]),
            awe_buff: "radiant_protectors_vow_awe",
            price: 1800,
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
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
                defence,
                skill_cooldown_mult,
                effect_bonus_flat_hp,
                effect_caster_defence_percent_hp
            ]
        );
        self
    }

    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
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

        let target = self.effect_bonus_flat_hp
            + percent_of(
                champion_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = champion_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: buff_name(self.awe_buff),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                hp: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl Default for ProtectorsVow {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for ProtectorsVow {
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
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        let mut tags = vec![ItemTag::HP, ItemTag::Defense];
        if self.skill_cooldown_mult > 0 {
            tags.push(ItemTag::CooltimeReduce);
        }
        tags
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}
