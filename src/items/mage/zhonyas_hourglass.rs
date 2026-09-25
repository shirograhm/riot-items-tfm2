use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta};

// Time Stop: Falling below 25% health puts you in stasis for 2.5 seconds. While in
// stasis, you are untargetable, invulnerable, and unable to act (120 second
// cooldown).
//
// Stasis is built the way `guardian_angel` builds its revive: a self-banish makes
// the carrier untargetable and locks her inputs, and a `damaged_reduce: 100` buff
// covers damage already on its way (burns, projectiles in flight).

/// Invulnerability for the stasis and the tick after it. Shared by both tiers.
const STASIS_BUFF: &str = "zhonyas_hourglass_stasis";
/// A turning golden hourglass whose sand runs out over the stasis, on Guardian
/// Angel's ring. Bound in `view/effects.view_effects`; the sheet is drawn 2.5
/// seconds long, so it matches the default `effect_duration_seconds`.
const STASIS_EFFECT: &str = "riot_zhonyas_hourglass_stasis";

#[derive(Clone, Debug)]
pub struct ZhonyasHourglass {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    defence: i32,
    effect_hp_percent_threshold: f64,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    time_stop_cooldown: usize,
}

impl ZhonyasHourglass {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "zhonyas_hourglass",
                &["seekers_armguard"],
                &["radiant_zhonyas_hourglass"],
            ),
            price: 750,
            magic_power: 50,
            defence: 35,
            effect_hp_percent_threshold: 25.0,
            effect_duration_seconds: 2.5,
            effect_cooldown_seconds: 120.0,
            // Non-vital stats (internals)
            time_stop_cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zhonyas_hourglass", &["zhonyas_hourglass"]),
            price: 1050,
            magic_power: 80,
            defence: 50,
            effect_hp_percent_threshold: 25.0,
            effect_duration_seconds: 2.5,
            effect_cooldown_seconds: 120.0,
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
                magic_power,
                defence,
                effect_hp_percent_threshold,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for ZhonyasHourglass {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ZhonyasHourglass {
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
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.time_stop_cooldown = 0;
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.time_stop_cooldown = self.time_stop_cooldown.saturating_sub(1);
    }

    // The host resolves the hit before this runs, so "falling below" is read after
    // the fact, as in `steraks_gage`: stasis starts on the tick the carrier crosses
    // under the threshold. A hit that kills outright never gets here alive.
    //
    // An untargetable carrier is already banished (Guardian Angel's revive, or
    // this item's own stasis): a second banish would replace that one, and cut a
    // longer stasis short.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if self.time_stop_cooldown > 0 {
            return;
        }
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        if !entity_ref.is_alive() || !entity_ref.is_targetable() {
            return;
        }
        let (current_hp, max_hp) = entity_ref.hp();
        if current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }

        let stasis = ticks(self.effect_duration_seconds);
        ctx.add_buff(
            entity,
            &BuffV1 {
                damaged_reduce: 100,
                ..BuffV1::timed(STASIS_BUFF, stasis + 1)
            },
        );
        ctx.entity_clear_cc(entity);
        ctx.entity_banish(entity, entity, stasis, STASIS_EFFECT, "");
        self.time_stop_cooldown = ticks(self.effect_cooldown_seconds);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
