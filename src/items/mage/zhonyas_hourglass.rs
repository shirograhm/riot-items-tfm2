use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta};

// Time Stop: Falling below 20% health puts you in stasis for 2.5 seconds. While in
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
/// The activation sound: `sound/sfx/riot_zhonyas_stasis.sound_info`, mapped
/// into `asset/base/sound/sfx` by `mod.override_info`, which is where sound
/// names are looked up. The clip is trimmed so its first hit lands on the
/// activation and its closing chime on the default 2.5 second stasis ending;
/// the untrimmed source is `sfx/zhonyas.wav`.
const STASIS_SFX: &str = "riot_zhonyas_stasis";

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
    /// A hit took the carrier under the threshold this tick; stasis waits for
    /// `update` to see whether she survived the rest of the tick's damage.
    time_stop_pending: bool,
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
            effect_hp_percent_threshold: 20.0,
            effect_duration_seconds: 2.5,
            effect_cooldown_seconds: 120.0,
            // Non-vital stats (internals)
            time_stop_cooldown: 0,
            time_stop_pending: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zhonyas_hourglass", &["zhonyas_hourglass"]),
            price: 1050,
            magic_power: 80,
            defence: 50,
            effect_hp_percent_threshold: 20.0,
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
        self.time_stop_pending = false;
    }

    // Time Stop itself. It waits here rather than firing from `on_damaged`
    // because a hit that takes the carrier under the threshold can be followed,
    // in the same tick, by one that kills her: stasis started on the first played
    // its hourglass and sound over a death (2026-09-26). By the time this runs
    // the tick's damage is in, so a carrier who died gets nothing, and the
    // cooldown is not spent.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.time_stop_cooldown = self.time_stop_cooldown.saturating_sub(1);
        if !std::mem::take(&mut self.time_stop_pending) || self.time_stop_cooldown > 0 {
            return;
        }
        let Some(entity) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive() && c.hp().0 > 0 && c.is_targetable())
            .map(|c| c.id())
        else {
            return;
        };

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
        ctx.play_sfx(STASIS_SFX, entity, &InputTargetV1::target(entity));
        self.time_stop_cooldown = ticks(self.effect_cooldown_seconds);
    }

    // The host resolves the hit before this runs, so "falling below" is read after
    // the fact, as in `steraks_gage`: the carrier crosses under the threshold on
    // this tick. A hit that kills outright never gets here alive; one that is
    // followed by a killing blow does, which is why stasis itself waits for
    // `update`.
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
        if current_hp == 0 || current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }
        self.time_stop_pending = true;
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
