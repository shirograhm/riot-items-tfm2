use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta};

// Annul: Grants a Spell Shield that blocks the next enemy Ability (40 second
// cooldown).
//
// Item hooks run after a hit has resolved, so the block has to be in place
// before the ability lands. While Annul is up the carrier holds `ANNUL_BUFF`:
// `skill_damaged_reduce` takes ability damage to nothing and `cc_immune` stops
// its crowd control. Basic attacks are reduced by a separate stat
// (`base_attack_damaged_reduce`), so they land in full and never pop it.
//
// The first enemy ability to reach the carrier pops the shield, and the buff
// comes off on the next `update` rather than in the hook: an ability that
// deals its damage before its stun would otherwise lose the immunity between
// the two. Everything else landing in that same tick is blocked with it.

/// Also the `view_buffs` name in `view/effects.view_effects` that draws the
/// shield bubble (`effects/banshees_veil_shield`) for as long as it is up.
const ANNUL_BUFF: &str = "banshees_veil_annul";

#[derive(Clone, Debug)]
pub struct BansheesVeil {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    magic_resistance: i32,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    annul_cooldown: usize,
    /// The Spell Shield is on the carrier and has not been spent.
    annul_up: bool,
    /// Popped this tick; `update` takes the buff off this entity.
    annul_popped: Option<usize>,
}

impl BansheesVeil {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "banshees_veil",
                &["hextech_alternator"],
                &["radiant_banshees_veil"],
            ),
            price: 700,
            magic_power: 60,
            magic_resistance: 40,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            annul_cooldown: 0,
            annul_up: false,
            annul_popped: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_banshees_veil", &["banshees_veil"]),
            price: 950,
            magic_power: 100,
            magic_resistance: 60,
            effect_cooldown_seconds: 40.0,
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
            [price, magic_power, magic_resistance, effect_cooldown_seconds]
        );
        self
    }

    /// Spends the shield on a hit from `source`, if it is up and `source` is
    /// not on the carrier's team. A source the host no longer knows (a caster
    /// who died with the spell in flight) still counts: the hit was blocked.
    fn annul(&mut self, ctx: &mut StableSim<'_>, entity: usize, source: usize) {
        if !self.annul_up {
            return;
        }
        let Some(team) = ctx.get_entity(entity).map(|e| e.team()) else {
            return;
        };
        if ctx.get_entity(source).is_some_and(|s| s.team() == team) {
            return;
        }
        self.annul_up = false;
        self.annul_popped = Some(entity);
        self.annul_cooldown = ticks(self.effect_cooldown_seconds);
    }
}

impl Default for BansheesVeil {
    fn default() -> Self {
        Self::base()
    }
}

/// The carrier's ability damage reduction from every other buff, and the
/// shield's own if it is on, so the shield only fills the room left under 100%.
fn other_skill_damaged_reduce(entity: &StableEntity<'_, '_>) -> (usize, Option<usize>) {
    let mut others = 0;
    let mut own = None;
    for buff in (0..entity.buff_count()).filter_map(|i| entity.buff_at(i)) {
        if buff.name() == ANNUL_BUFF {
            own = Some(buff.skill_damaged_reduce);
        } else {
            others += buff.skill_damaged_reduce;
        }
    }
    (others, own)
}

impl StableItem for BansheesVeil {
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
            magic_power: self.magic_power,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.annul_cooldown = 0;
        self.annul_up = false;
        self.annul_popped = None;
    }

    // Takes a popped shield off, then keeps a ready one on the carrier. The
    // buff is re-checked every tick rather than added once: death can strip
    // it, and its size follows the carrier's other ability damage reduction
    // (Cloak of Starry Night's grows with magic resistance).
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if let Some(entity) = self.annul_popped.take() {
            ctx.entity_remove_buff(entity, ANNUL_BUFF);
            return;
        }
        self.annul_cooldown = self.annul_cooldown.saturating_sub(1);
        if self.annul_cooldown > 0 {
            return;
        }
        let Some((entity, others, own)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| {
                let (others, own) = other_skill_damaged_reduce(&c);
                (c.id(), others, own)
            })
        else {
            return;
        };

        let reduce = 100usize.saturating_sub(others);
        self.annul_up = true;
        if own == Some(reduce) {
            return;
        }
        refresh_buff(
            ctx,
            entity,
            ANNUL_BUFF,
            &BuffV1 {
                skill_damaged_reduce: reduce,
                cc_immune: true,
                ..BuffV1::named(ANNUL_BUFF)
            },
        );
    }

    // Abilities pop the shield. A burn or an item proc only does once it
    // reaches the carrier as 0 damage: that is the shield having eaten it, and
    // it must not go on eating them for free. Anything that still hurts went
    // past the shield and leaves it up.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let blocked = match attack_type {
            AttackTypeV1::Skill => true,
            AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield | AttackTypeV1::Item => damage == 0,
            _ => false,
        };
        if blocked {
            self.annul(ctx, entity, attacker);
        }
    }

    // A pure crowd-control ability deals no damage, so this is what spends
    // the shield on one. Whether the host still reports CC that `cc_immune`
    // turned away is not known yet.
    fn on_cc(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize, caster: usize) {
        let Some(entity) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        self.annul(ctx, entity, caster);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::MagicResistance]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
