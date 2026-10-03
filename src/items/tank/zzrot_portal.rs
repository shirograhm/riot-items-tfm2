use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_enemy_champion, percent_of, ticks, ItemMeta, VOIDSPAWN_UNIT};

// Void Gate: Damaging an enemy champion summons a Voidspawn for 6 seconds
// (20 second cooldown). The Voidspawn has 30% of your maximum health, armor and
// magic resistance, and its attacks deal 15 + 2% of your maximum health as
// physical damage.
//
// League's item opens a gate with an active, and the gate spawns Voidspawn
// while it stands. Items have no actives here, so the gate opens itself: a hit
// on an enemy champion, on a cooldown.
//
// The Voidspawn is the engine's own summon. `spawn_unit` builds the unit the
// Necromancer's ghoul is - it lives for its duration, chases the nearest enemy
// and attacks it - and gives it the name passed in, which is what its art is
// looked up by (`VOIDSPAWN_UNIT`).

/// How fast a Voidspawn moves: the ghoul's own speed, well above a champion's
/// 900 or so, so it reaches a fight it was not summoned on top of.
const VOIDSPAWN_MOVE_SPEED: usize = 1600;

#[derive(Clone, Debug)]
pub struct ZzRotPortal {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    defence: i32,
    magic_resistance: i32,
    effect_cooldown_seconds: f64,
    effect_duration_seconds: f64,
    effect_summon_stat_percent: f64,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    // Non-vital stats (internals)
    /// Ticks until the gate can open again.
    cooldown: usize,
    /// A hit has opened the gate; the Voidspawn comes out on the next `update`.
    /// Spawning from inside the attack hook would add an entity to the match
    /// while the engine is still resolving the hit that called it.
    gate_open: bool,
}

impl ZzRotPortal {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "zzrot_portal",
                &["aegis_of_the_legion"],
                &["radiant_zzrot_portal"],
            ),
            price: 700,
            hp: 100,
            hp_regen: 2,
            defence: 30,
            magic_resistance: 40,
            effect_cooldown_seconds: 20.0,
            effect_duration_seconds: 6.0,
            effect_summon_stat_percent: 30.0,
            effect_bonus_flat_damage: 15,
            effect_caster_hp_percent_damage: 2.0,
            // Non-vital stats (internals)
            cooldown: 0,
            gate_open: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zzrot_portal", &["zzrot_portal"]),
            price: 1000,
            hp: 200,
            hp_regen: 3,
            defence: 45,
            magic_resistance: 60,
            effect_cooldown_seconds: 20.0,
            effect_duration_seconds: 6.0,
            effect_summon_stat_percent: 40.0,
            effect_bonus_flat_damage: 25,
            effect_caster_hp_percent_damage: 3.0,
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
                hp_regen,
                defence,
                magic_resistance,
                effect_cooldown_seconds,
                effect_duration_seconds,
                effect_summon_stat_percent,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage
            ]
        );
        self
    }
}

impl Default for ZzRotPortal {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ZzRotPortal {
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
            hp_regen: self.hp_regen,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
        self.gate_open = false;
    }

    /// Void Gate: the hit that opens it. Autos and skills only, so nothing an
    /// item deals on its own (Immolate, a proc) opens the gate, and a Voidspawn
    /// cannot open the next one.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !matches!(attack_type, AttackTypeV1::BaseAttack | AttackTypeV1::Skill) {
            return;
        }
        if self.cooldown > 0 || self.gate_open {
            return;
        }
        if is_enemy_champion(ctx, caster, target) {
            self.gate_open = true;
        }
    }

    /// Counts the cooldown down, and lets the Voidspawn out of an open gate.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
        if !std::mem::take(&mut self.gate_open) {
            return;
        }

        // A carrier that died on the hit summons nothing, and keeps the gate
        // off cooldown for its next life.
        let Some((carrier, team, (x, y), max_hp, carrier_stat)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.team(), c.pos(), c.hp().1, c.stat()))
        else {
            return;
        };

        let share = |value: usize| percent_of(value, self.effect_summon_stat_percent);
        let stat = StatV1 {
            attack: self.effect_bonus_flat_damage
                + percent_of(max_hp, self.effect_caster_hp_percent_damage),
            hp: share(max_hp),
            defence: share(carrier_stat.defence),
            magic_resistance: share(carrier_stat.magic_resistance),
            move_speed: VOIDSPAWN_MOVE_SPEED,
            ..Default::default()
        };

        // On cooldown whether or not the host spawned anything: a host without
        // `spawn_unit` would otherwise be asked again on every hit.
        self.cooldown = ticks(self.effect_cooldown_seconds);
        ctx.spawn_unit(
            VOIDSPAWN_UNIT,
            carrier,
            team,
            x,
            y,
            ticks(self.effect_duration_seconds) as u64,
            &stat,
            // A plain melee swing, once a second; the damage is all in `attack`.
            &UnitAttackV1::default(),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}
