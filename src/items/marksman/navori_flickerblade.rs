use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

// The cooldowns Transcendence shortens: the two basic abilities, never the
// ultimate.
const BASIC_ABILITIES: [SkillSlotV1; 2] = [SkillSlotV1::Skill, SkillSlotV1::Skill2];

// Navori Flickerblade: Transcendence. Every basic attack takes a share of
// what is left of the carrier's basic ability cooldowns off them, through the
// host's `entity_reduce_cooldown` (ABI level 10; an older host leaves the
// cooldowns alone).
#[derive(Clone, Debug)]
pub struct NavoriFlickerblade {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    crit_chance: i32,
    move_speed_mult: i32,
    effect_remaining_cooldown_percent: f64,
    // Non-vital stats (internals)
    // The tick of the last attack that cut the cooldowns. An attack's several
    // hits all land on one tick, and two attacks never do.
    cut_at: Option<usize>,
}

impl NavoriFlickerblade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "navori_flickerblade",
                &["twin_stormblade", "scouts_slingshot"],
                &["radiant_navori_flickerblade"],
            ),
            price: 700,
            attack_speed_mult: 35,
            crit_chance: 20,
            move_speed_mult: 4,
            effect_remaining_cooldown_percent: 15.0,
            // Non-vital stats (internals)
            cut_at: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_navori_flickerblade", &["navori_flickerblade"]),
            price: 1000,
            attack_speed_mult: 55,
            crit_chance: 25,
            move_speed_mult: 4,
            // Transcendence itself is unchanged — Radiant buys the stat line only.
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
                effect_remaining_cooldown_percent
            ]
        );
        self
    }
}

impl Default for NavoriFlickerblade {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for NavoriFlickerblade {
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

    // Transcendence: each basic attack, once, whatever it hits. A cooldown
    // that is ready, or a slot still locked by level, is left as it is.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack || self.effect_remaining_cooldown_percent <= 0.0 {
            return;
        }
        let tick = ctx.tick();
        if self.cut_at == Some(tick) {
            return;
        }
        self.cut_at = Some(tick);

        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let percent = self.effect_remaining_cooldown_percent;
        let cuts: Vec<(SkillSlotV1, usize)> = BASIC_ABILITIES
            .iter()
            .filter_map(|&slot| {
                let cooldown = caster_ref.cooldown(slot)?;
                let cut = percent_of(cooldown.remaining, percent);
                (cooldown.unlocked && cut > 0).then_some((slot, cut))
            })
            .collect();
        for (slot, cut) in cuts {
            ctx.entity_reduce_cooldown(caster, slot, cut);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::AttackSpeed,
            ItemTagV1::MoveSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
