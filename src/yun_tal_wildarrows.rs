use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct YunTalWildarrows {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_stack_crit_chance: i32,
    effect_max_stacks: usize,
    effect_flurry_attack_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    // Runtime state: total Practice stacks earned, re-applied on each spawn so the
    // permanent crit chance persists across combat rounds (see heartsteel.rs).
    accumulated_stacks: usize,
}

impl Default for YunTalWildarrows {
    fn default() -> Self {
        Self {
            price: 1500,
            attack: 65,
            attack_speed_mult: 20,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
            accumulated_stacks: 0,
        }
    }
}

impl YunTalWildarrows {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_stack_crit_chance: cfg
                .effect_stack_crit_chance
                .unwrap_or(d.effect_stack_crit_chance),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_flurry_attack_speed_mult: cfg
                .effect_flurry_attack_speed_mult
                .unwrap_or(d.effect_flurry_attack_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            accumulated_stacks: d.accumulated_stacks,
        }
    }
}

impl ModItemInfo for YunTalWildarrows {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "yun_tal_wildarrows"
    }

    fn icon(&self) -> &str {
        "yun_tal_wildarrows"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["noonquiver".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_yun_tal_wildarrows".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    // Practice: permanent crit chance earned so far is re-applied each spawn.
    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_stacks == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                crit_chance: self.accumulated_stacks as i32 * self.effect_stack_crit_chance,
                name: ArrayString::try_from("yun_tal_practice").unwrap(),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let is_flurry_on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "yun_tal_flurry_cooldown");

        // Practice: dealing physical damage grants permanent crit chance, capped.
        if damage_type == DamageType::AD && self.accumulated_stacks < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Permanent,
                    crit_chance: self.effect_stack_crit_chance,
                    name: ArrayString::try_from("yun_tal_practice").unwrap(),
                    ..Default::default()
                },
            );
            self.accumulated_stacks += 1;
        }

        // Flurry: on attack, gain a burst of attack speed on an internal cooldown.
        if !is_flurry_on_cooldown {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    attack_speed_mult: self.effect_flurry_attack_speed_mult,
                    name: ArrayString::try_from("yun_tal_flurry").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_cooldown_seconds * 60.0) as usize,
                    },
                    name: ArrayString::try_from("yun_tal_flurry_cooldown").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantYunTalWildarrows {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_stack_crit_chance: i32,
    effect_max_stacks: usize,
    effect_flurry_attack_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    accumulated_stacks: usize,
}

impl Default for RadiantYunTalWildarrows {
    fn default() -> Self {
        Self {
            price: 2200,
            attack: 80,
            attack_speed_mult: 50,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
            accumulated_stacks: 0,
        }
    }
}

impl RadiantYunTalWildarrows {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_stack_crit_chance: cfg
                .effect_stack_crit_chance
                .unwrap_or(d.effect_stack_crit_chance),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_flurry_attack_speed_mult: cfg
                .effect_flurry_attack_speed_mult
                .unwrap_or(d.effect_flurry_attack_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
            accumulated_stacks: d.accumulated_stacks,
        }
    }
}

impl ModItemInfo for RadiantYunTalWildarrows {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_yun_tal_wildarrows"
    }

    fn icon(&self) -> &str {
        "radiant_yun_tal_wildarrows"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["yun_tal_wildarrows".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.accumulated_stacks == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            BuffState {
                duration: BuffType::Permanent,
                crit_chance: self.accumulated_stacks as i32 * self.effect_stack_crit_chance,
                name: ArrayString::try_from("radiant_yun_tal_practice").unwrap(),
                ..Default::default()
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let is_flurry_on_cooldown = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == "radiant_yun_tal_flurry_cooldown");

        // Practice: dealing physical damage grants permanent crit chance, capped.
        if damage_type == DamageType::AD && self.accumulated_stacks < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Permanent,
                    crit_chance: self.effect_stack_crit_chance,
                    name: ArrayString::try_from("radiant_yun_tal_practice").unwrap(),
                    ..Default::default()
                },
            );
            self.accumulated_stacks += 1;
        }

        // Flurry: on attack, gain a burst of attack speed on an internal cooldown.
        if !is_flurry_on_cooldown {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    attack_speed_mult: self.effect_flurry_attack_speed_mult,
                    name: ArrayString::try_from("radiant_yun_tal_flurry").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_cooldown_seconds * 60.0) as usize,
                    },
                    name: ArrayString::try_from("radiant_yun_tal_flurry_cooldown").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
