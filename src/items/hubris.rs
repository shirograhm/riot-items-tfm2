use arrayvec::ArrayString;
use mod_api::*;

use crate::apply_lethality;
use crate::config::ItemConfig;

// Lethality (see serrated_dirk) plus Eminence: each kill adds a permanent stack
// and grants bonus Attack Damage that scales with the stack count for a while.
// The AD is granted as a fixed-duration Time buff on kill; because same-name
// buffs stack, a kill streak accumulates AD that each decays after its window,
// while the permanent `eminence_stacks` count keeps growing the per-kill payout.

#[derive(Clone, Debug)]
pub struct Hubris {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
}

impl Default for Hubris {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 75,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            eminence_stacks: 0,
        }
    }
}

impl Hubris {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_attack: cfg
                .effect_bonus_flat_attack
                .unwrap_or(d.effect_bonus_flat_attack),
            effect_stack_attack: cfg.effect_stack_attack.unwrap_or(d.effect_stack_attack),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            eminence_stacks: 0,
        }
    }

    fn eminence_bonus_ad(&self) -> i32 {
        self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32
    }
}

impl ModItemInfo for Hubris {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "hubris"
    }

    fn icon(&self) -> &str {
        "hubris"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serrated_dirk".to_string(), "soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_hubris".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.eminence_stacks = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        self.eminence_stacks += 1;
        ctx.add_buff(
            champion_id,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0).round() as usize,
                },
                attack: self.eminence_bonus_ad(),
                name: ArrayString::try_from("hubris_eminence").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantHubris {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_attack: i32,
    effect_stack_attack: i32,
    effect_duration_seconds: f64,
    eminence_stacks: usize,
}

impl Default for RadiantHubris {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 120,
            skill_cooldown_mult: 15,
            effect_lethality: 18,
            effect_bonus_flat_attack: 12,
            effect_stack_attack: 3,
            effect_duration_seconds: 90.0,
            eminence_stacks: 0,
        }
    }
}

impl RadiantHubris {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_attack: cfg
                .effect_bonus_flat_attack
                .unwrap_or(d.effect_bonus_flat_attack),
            effect_stack_attack: cfg.effect_stack_attack.unwrap_or(d.effect_stack_attack),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            eminence_stacks: 0,
        }
    }

    fn eminence_bonus_ad(&self) -> i32 {
        self.effect_bonus_flat_attack + self.effect_stack_attack * self.eminence_stacks as i32
    }
}

impl ModItemInfo for RadiantHubris {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_hubris"
    }

    fn icon(&self) -> &str {
        "radiant_hubris"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hubris".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.eminence_stacks = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize, _entity: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        self.eminence_stacks += 1;
        ctx.add_buff(
            champion_id,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0).round() as usize,
                },
                attack: self.eminence_bonus_ad(),
                name: ArrayString::try_from("radiant_hubris_eminence").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
