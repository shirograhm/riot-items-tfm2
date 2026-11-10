use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::vfx::reach_ring::{self, RING_REACHES};
use crate::{apply_config, refresh_buff, Energized, ItemMeta, ProcQueue};

// Sharpshooter: while fully Energized the carrier has `effect_bonus_range`
// more attack range, and the basic attack that spends the charge deals bonus
// magic damage on-hit. A turret hit spends the charge without the damage, so
// the reach cannot be held for sieging.
//
// One timed buff carries the range and draws a ring at the new reach
// ([`reach_ring`]; `effects/sharpshooter_ring_<reach>`, bound in
// `view/effects.view_effects`).

// The ring buff for each of [`RING_REACHES`].
const RINGS: [&str; RING_REACHES.len()] = [
    "riot_sharpshooter_40",
    "riot_sharpshooter_50",
    "riot_sharpshooter_60",
    "riot_sharpshooter_70",
    "riot_sharpshooter_80",
    "riot_sharpshooter_90",
    "riot_sharpshooter_100",
    "riot_sharpshooter_110",
];
// Kept up like the Energized marker: a 1 s buff renewed every 1/3 s.
const BUFF_TICKS: usize = 60;
const REFRESH_EVERY_TICKS: usize = 20;

#[derive(Clone, Debug)]
pub struct RapidFirecannon {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    crit_chance: i32,
    move_speed_mult: i32,
    effect_max_stacks: usize,
    effect_bonus_magic_damage: usize,
    effect_bonus_range: usize,
    // Non-vital stats (internals)
    energized: Energized,
    procs: ProcQueue,
    // The Sharpshooter buff on the carrier, while it is up.
    sharpshooter: Option<&'static str>,
}

impl RapidFirecannon {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "rapid_firecannon",
                &["twin_stormblade", "scouts_slingshot"],
                &["radiant_rapid_firecannon"],
            ),
            price: 700,
            attack_speed_mult: 35,
            crit_chance: 20,
            move_speed_mult: 4,
            effect_max_stacks: 100,
            effect_bonus_magic_damage: 40,
            effect_bonus_range: 15,
            // Non-vital stats (internals)
            energized: Energized::default(),
            procs: ProcQueue::new(),
            sharpshooter: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_rapid_firecannon", &["rapid_firecannon"]),
            price: 950,
            attack_speed_mult: 65,
            crit_chance: 25,
            move_speed_mult: 4,
            effect_max_stacks: 100,
            effect_bonus_magic_damage: 60,
            effect_bonus_range: 20,
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
                attack_speed_mult,
                crit_chance,
                move_speed_mult,
                effect_max_stacks,
                effect_bonus_magic_damage,
                effect_bonus_range
            ]
        );
        self
    }

    // Keeps the range up while charged, renewing it on a cycle inside its
    // duration, and takes it down when the charge is gone.
    fn keep_sharpshooter(&mut self, ctx: &mut StableSim<'_>, champion: usize, charged: bool) {
        if !charged {
            if let Some(name) = self.sharpshooter.take() {
                ctx.entity_remove_buff(champion, name);
            }
            return;
        }
        let fresh = self.sharpshooter.is_none();
        if !fresh && ctx.tick() % REFRESH_EVERY_TICKS != 0 {
            return;
        }
        let name = match self.sharpshooter {
            Some(name) => name,
            None => {
                // Another tier's ring may linger from an upgrade mid-charge.
                reach_ring::clear_rings(ctx, champion, &RINGS);
                reach_ring::ring_for(ctx, champion, self.effect_bonus_range, &RINGS)
            }
        };
        let buff = reach_ring::range_buff(name, self.effect_bonus_range, BUFF_TICKS);
        refresh_buff(ctx, champion, name, &buff);
        self.sharpshooter = Some(name);
    }
}

impl Default for RapidFirecannon {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RapidFirecannon {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.energized.reset();
        self.procs.clear();
        self.sharpshooter = None;
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        // Base attacks only: the bonus lands as part of the swing or as `Item`
        // damage, which must not spend or build the meter.
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(tower) = ctx.get_entity(target).map(|t| t.is_tower()) else {
            return;
        };

        if self
            .energized
            .is_charged(ctx, caster, self.effect_max_stacks)
        {
            self.energized.spend(ctx);
            // Down at once, not on the next update: the reach is for this
            // swing only.
            self.keep_sharpshooter(ctx, caster, false);
            if !tower {
                self.procs.on_hit_magic(
                    ctx,
                    target,
                    damage,
                    damage_type,
                    is_crit,
                    self.effect_bonus_magic_damage,
                );
            }
        }

        self.energized.basic_attack(self.effect_max_stacks);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
        self.energized.update(ctx, player, self.effect_max_stacks);

        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id())
        else {
            return;
        };
        let charged = self
            .energized
            .is_charged(ctx, champion, self.effect_max_stacks);
        self.keep_sharpshooter(ctx, champion, charged);
    }

    // The Energized meter carries across the Radiant upgrade, so buying it
    // mid-fight does not throw away a nearly full bar.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.energized.stacks() as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.energized
                .set_stacks((carry as usize).min(self.effect_max_stacks));
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::AttackSpeed,
            ItemTagV1::AttackRange,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
