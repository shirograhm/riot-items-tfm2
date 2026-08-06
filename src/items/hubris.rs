use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, count_takedowns, mark_enemy_champion, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Hubris {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
    takedown_marks: Vec<(usize, usize)>,
}

impl Hubris {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("hubris", &["serrated_dirk"], &["radiant_hubris"]),
            price: 1300,
            attack: 70,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            eminence_stacks: 0,
            takedown_marks: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hubris", &["hubris"]),
            price: 1950,
            attack: 115,
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
                attack,
                skill_cooldown_mult,
                effect_lethality,
                effect_bonus_flat_attack,
                effect_stack_attack,
                effect_duration_seconds
            ]
        );
        self
    }

    fn eminence_bonus_ad(&self) -> i32 {
        self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32
    }

    // Grants `count` Eminence stacks to the wielder (a growing, decaying AD buff per
    // stack), matching the "12 (+3 per stack)" payout using the pre-increment count.

    fn grant_eminence(&mut self, ctx: &mut StableSim<'_>, player: usize, count: usize) {
        if count == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        for _ in 0..count {
            let ad = self.eminence_bonus_ad();
            ctx.add_buff(
                champion_id,
                &BuffV1 {
                    attack: ad,
                    ..BuffV1::timed("", ticks(self.effect_duration_seconds))
                },
            );
            self.eminence_stacks += 1;
        }
    }
}

impl Default for Hubris {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Hubris {
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
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.eminence_stacks = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let takedowns = count_takedowns(&mut self.takedown_marks, ctx);
        self.grant_eminence(ctx, player, takedowns);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally {
            mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
