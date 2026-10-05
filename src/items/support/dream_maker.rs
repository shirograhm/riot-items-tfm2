use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, SelfCastWatch, ADAPTIVE_FORCE_AD_RATIO};

/// Dream Maker — what World Atlas grows into for a support that keeps its
/// allies up: the passive it is named for, and the gold the Atlas line pays
/// ([`crate::SharedRiches`]).
///
/// # Dream Maker
///
/// A skill the carrier casts on an allied champion (a heal, a shield or a
/// buff) blows that ally a Dream Bubble. For as long as the bubble lasts the
/// ally takes less damage from abilities and has Adaptive Force: ability
/// power for an ally with more of that than attack damage, attack damage at
/// [`ADAPTIVE_FORCE_AD_RATIO`] for any other, the way Diamond Tipped Spear's
/// Pierce reads it.
///
/// One cast, one bubble: while it lasts no other is blown, and when it ends
/// the cooldown starts. Both variants share the buff name, so two Dream
/// Makers blowing on one ally leave one bubble, not two.
///
/// # Who gets the bubble
///
/// The nearest allied champion the cast reached. A skill cast on an ally
/// names only that ally, so that is the one, as Ardent Censer's Sanctify has
/// it. A skill the carrier casts on itself that heals around it (the Monk's)
/// names no ally: of the ones it is seen to heal ([`SelfCastWatch`]), the
/// nearest to the carrier gets it.
#[derive(Clone, Debug)]
pub struct DreamMaker {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_duration_seconds: f64,
    effect_skill_damaged_reduce: usize,
    effect_adaptive_force: i32,
    effect_cooldown_seconds: f64,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    /// Ticks the bubble blown last still has to run; zero when none is out.
    bubble: usize,
    /// Ticks until another can be blown, counted from when the last one ended.
    cooldown: usize,
    self_cast: SelfCastWatch,
}

/// The Dream Bubble: the buff that carries what it grants. It is also the name
/// the `view_buffs` binding in `view/effects.view_effects` draws the bubble
/// under (`effects/dream_bubble`).
const BUBBLE_BUFF: &str = "dream_maker_bubble";

impl DreamMaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("dream_maker", &["runic_compass"], &["radiant_dream_maker"]),
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_duration_seconds: 5.0,
            effect_skill_damaged_reduce: 8,
            effect_adaptive_force: 20,
            effect_cooldown_seconds: 8.0,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            bubble: 0,
            cooldown: 0,
            self_cast: SelfCastWatch::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dream_maker", &["dream_maker"]),
            price: 750,
            hp: 300,
            hp_regen: 5,
            // Dream Maker itself is unchanged — Radiant buys the stat line only.
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
                effect_duration_seconds,
                effect_skill_damaged_reduce,
                effect_adaptive_force,
                effect_cooldown_seconds,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    /// Whether a bubble can be blown: none is out and the cooldown has run.
    fn ready(&self) -> bool {
        self.bubble == 0 && self.cooldown == 0
    }

    /// Blows a Dream Bubble to `ally`, if one can be blown, and starts its time.
    fn blow_to(&mut self, ctx: &mut StableSim<'_>, ally: usize) {
        if self.ready() && self.blow(ctx, ally) {
            self.bubble = ticks(self.effect_duration_seconds).max(1);
        }
    }

    /// Blows a Dream Bubble to `ally`, in place of any it already has (another
    /// Dream Maker's). Whether it was blown: not to an ally that is no living
    /// champion.
    fn blow(&self, ctx: &mut StableSim<'_>, ally: usize) -> bool {
        let Some(favors_ap) = ctx
            .get_entity(ally)
            .filter(|ally| ally.is_champion() && ally.is_alive())
            .map(|ally| {
                let stat = ally.stat();
                stat.magic_power > stat.attack
            })
        else {
            return false;
        };
        let force = self.effect_adaptive_force;
        let (attack, magic_power) = if favors_ap {
            (0, force)
        } else {
            ((force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32, 0)
        };
        refresh_buff(
            ctx,
            ally,
            BUBBLE_BUFF,
            &BuffV1 {
                skill_damaged_reduce: self.effect_skill_damaged_reduce,
                attack,
                magic_power,
                ..BuffV1::timed(BUBBLE_BUFF, ticks(self.effect_duration_seconds))
            },
        );
        true
    }

    /// What Shared Riches pays a holder: this much gold, this often.
    /// [`crate::SharedRiches`] does the paying, from the match hook.
    pub(crate) fn shared_riches(&self) -> (usize, f64) {
        (self.effect_bonus_gold, self.effect_gold_interval_seconds)
    }
}

impl Default for DreamMaker {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DreamMaker {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.self_cast.close();
        // A bubble still out when the carrier died ended long ago. Left
        // standing it would run out its time after the respawn (no `update`
        // runs for a dead carrier) and only then start the cooldown.
        self.bubble = 0;
        self.cooldown = 0;
    }

    // Dream Maker's trigger. `is_ally` is the SDK's flag for an ally-targeted
    // skill — a heal, shield or buff — and `on_skill_hit` only ever fires for
    // this carrier's own casts.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally || !self.ready() {
            return;
        }
        // Self-casts count as ally-targeted, and the bubble only ever goes to
        // someone else. A self-cast may still have healed an ally, as the
        // Monk's heal does around them: that is watched for in `update`.
        if target == caster {
            self.self_cast.open(ctx, caster);
            return;
        }
        self.blow_to(ctx, target);
    }

    /// Runs the bubble's time and the cooldown that starts as it ends, and the
    /// watch on a self-cast.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.bubble > 0 {
            self.bubble -= 1;
            if self.bubble == 0 {
                self.cooldown = ticks(self.effect_cooldown_seconds);
            }
        } else {
            self.cooldown = self.cooldown.saturating_sub(1);
        }
        // Of the allies a self-cast healed, the nearest to the carrier.
        let healed = self.self_cast.poll(ctx);
        if !healed.is_empty() {
            let carrier = ctx
                .get_player(player)
                .and_then(|player_ref| player_ref.champion())
                .map(|champion_ref| champion_ref.id());
            let nearest = carrier.and_then(|carrier| {
                healed
                    .into_iter()
                    .min_by_key(|&ally| ctx.distance_sq(carrier, ally))
            });
            if let Some(ally) = nearest {
                self.blow_to(ctx, ally);
            }
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
