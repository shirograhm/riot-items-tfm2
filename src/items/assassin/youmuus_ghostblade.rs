use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, is_enemy_champion, refresh_buff, ticks, ItemMeta};

// Haunt's movement speed while out of combat, and Wraith Step's burst. One
// name each for both tiers: a champion holds one Ghostblade at a time.
const HAUNT_BUFF: &str = "youmuus_ghostblade_haunt";
const WRAITH_STEP_BUFF: &str = "youmuus_ghostblade_wraith_step";
// Haunt is kept up as a short timed buff, put back when this little is left,
// so it never outlasts the carrier's return to combat by much.
const HAUNT_BUFF_TICKS: usize = 30;
const HAUNT_REFRESH_AT: usize = 10;

#[derive(Clone, Debug)]
pub struct YoumuusGhostblade {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    move_speed_mult: i32,
    effect_lethality: usize,
    effect_bonus_move_speed_mult: i32,
    effect_out_of_combat_seconds: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    // Ticks since the carrier last damaged or hit an enemy champion with an
    // Ability: time out of combat, as Opportunity counts it.
    idle_ticks: usize,
    // Ticks left on the Haunt buff this item last put up; 0 when it is down.
    haunt_left: usize,
    // Ticks until Wraith Step can go off again.
    wraith_step_cooldown: usize,
}

impl YoumuusGhostblade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "youmuus_ghostblade",
                &["serrated_dirk"],
                &["radiant_youmuus_ghostblade"],
            ),
            price: 650,
            attack: 40,
            move_speed_mult: 5,
            effect_lethality: 18,
            effect_bonus_move_speed_mult: 10,
            effect_out_of_combat_seconds: 5.0,
            effect_move_speed_mult: 20,
            effect_duration_seconds: 4.0,
            effect_cooldown_seconds: 45.0,
            // Non-vital stats (internals)
            idle_ticks: 0,
            haunt_left: 0,
            wraith_step_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_youmuus_ghostblade", &["youmuus_ghostblade"]),
            price: 1000,
            attack: 60,
            move_speed_mult: 8,
            effect_lethality: 18,
            effect_bonus_move_speed_mult: 15,
            effect_out_of_combat_seconds: 5.0,
            effect_move_speed_mult: 30,
            effect_duration_seconds: 4.0,
            effect_cooldown_seconds: 45.0,
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
                move_speed_mult,
                effect_lethality,
                effect_bonus_move_speed_mult,
                effect_out_of_combat_seconds,
                effect_move_speed_mult,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    // Combat with an enemy champion: restarts the out-of-combat timer and
    // takes Haunt down at once.
    fn note_combat(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        self.idle_ticks = 0;
        if self.haunt_left > 0 {
            ctx.entity_remove_buff(caster, HAUNT_BUFF);
            self.haunt_left = 0;
        }
    }

    // Wraith Step: a burst of movement speed on damaging an enemy champion,
    // then the cooldown.
    fn wraith_step(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        if self.wraith_step_cooldown > 0 {
            return;
        }
        self.wraith_step_cooldown = ticks(self.effect_cooldown_seconds);
        refresh_buff(
            ctx,
            caster,
            WRAITH_STEP_BUFF,
            &BuffV1 {
                move_speed_mult: self.effect_move_speed_mult,
                ..BuffV1::timed(WRAITH_STEP_BUFF, ticks(self.effect_duration_seconds))
            },
        );
    }
}

impl Default for YoumuusGhostblade {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for YoumuusGhostblade {
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
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.idle_ticks = 0;
        self.haunt_left = 0;
        self.wraith_step_cooldown = 0;
    }

    // Haunt: once out of combat long enough, keeps its movement speed up.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.idle_ticks = self.idle_ticks.saturating_add(1);
        self.wraith_step_cooldown = self.wraith_step_cooldown.saturating_sub(1);
        self.haunt_left = self.haunt_left.saturating_sub(1);
        if self.idle_ticks < ticks(self.effect_out_of_combat_seconds)
            || self.haunt_left > HAUNT_REFRESH_AT
        {
            return;
        }
        let entity = {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            if !champion_ref.is_alive() {
                return;
            }
            champion_ref.id()
        };
        refresh_buff(
            ctx,
            entity,
            HAUNT_BUFF,
            &BuffV1 {
                move_speed_mult: self.effect_bonus_move_speed_mult,
                ..BuffV1::timed(HAUNT_BUFF, HAUNT_BUFF_TICKS)
            },
        );
        self.haunt_left = HAUNT_BUFF_TICKS;
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        // Lethality on all damage except to turrets.
        if !target_ref.is_tower() {
            apply_lethality(ctx, caster, target, self.effect_lethality, damage);
        }

        if is_enemy_champion(ctx, caster, target) {
            self.note_combat(ctx, caster);
            self.wraith_step(ctx, caster);
        }
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally && is_enemy_champion(ctx, caster, target) {
            self.note_combat(ctx, caster);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
