use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, SelfCastWatch, SharedRiches};

/// Dream Maker — what World Atlas grows into for a support that keeps its
/// allies up: the passive it is named for, and the gold the Atlas line pays
/// ([`SharedRiches`]).
///
/// # Dream Maker
///
/// A skill the carrier casts on an allied champion (a heal, a shield or a
/// buff) blows that ally a Dream Bubble. The ally's next hit on an enemy
/// champion, a basic attack or an ability, pops it for bonus magic damage
/// dealt in the ally's own name, and only then does the cooldown start. A
/// bubble nobody pops is gone when its time is up, with no cooldown. There is
/// one bubble at a time.
///
/// # Who gets the bubble
///
/// The ally the skill was cast on, as Ardent Censer's Sanctify has it. A skill
/// the carrier casts on itself that heals around it (the Monk's) names no
/// ally, so the first one it is seen to heal gets it ([`SelfCastWatch`]).
///
/// # Telling that the ally hit a champion
///
/// An item only hears its own carrier's attacks, so the ally's are read back.
/// A player's `deal` statistic is their damage to champions: it going up
/// while the bubble is on means the ally landed something. The host
/// serialises the player's whole statistics for every read, so they are read
/// only on a tick where some enemy champion lost health or shield (a call
/// each to find out). The one that lost it is the one hit; of several, the
/// nearest to the ally, which is a guess when two were hit at once by
/// different champions.
#[derive(Clone, Debug)]
pub struct DreamMaker {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_duration_seconds: f64,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    /// The Dream Bubble out on an ally, if there is one.
    bubble: Option<Bubble>,
    /// Ticks until another can be blown.
    cooldown: usize,
    self_cast: SelfCastWatch,
    riches: SharedRiches,
}

/// The Dream Bubble: a buff with no stats, on the ally for as long as the
/// bubble lasts. It is also the name the `view_buffs` binding in
/// `view/effects.view_effects` draws the bubble under (`effects/dream_bubble`).
/// Shared by both variants.
const BUBBLE_BUFF: &str = "dream_maker_bubble";

/// A Dream Bubble on an ally.
#[derive(Clone, Debug)]
struct Bubble {
    ally: usize,
    /// The ally's player, whose statistics say when the ally has hit a champion.
    player: usize,
    /// Ticks until it is gone unpopped.
    remaining: usize,
    /// The ally's damage to champions, as last read.
    dealt: u64,
    /// (id, health and shield) of every living enemy champion, as of the last
    /// tick.
    enemies: Vec<(usize, usize)>,
}

/// The one statistic read, out of the player's whole document.
#[derive(serde::Deserialize)]
struct Dealing {
    deal: u64,
}

impl DreamMaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("dream_maker", &["runic_compass"], &["radiant_dream_maker"]),
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_duration_seconds: 3.0,
            effect_min_bonus_damage: 40,
            effect_max_bonus_damage: 150,
            effect_cooldown_seconds: 8.0,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            bubble: None,
            cooldown: 0,
            self_cast: SelfCastWatch::default(),
            riches: SharedRiches::default(),
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
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_cooldown_seconds,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    /// Whether a bubble can be blown: none is out and the cooldown has run.
    fn ready(&self) -> bool {
        self.bubble.is_none() && self.cooldown == 0
    }

    /// Level 1 gets `effect_min_bonus_damage` and level 12
    /// `effect_max_bonus_damage`, the eleven-step ramp Sheen's Spellblade
    /// uses. The level is the bubbled ally's.
    fn bonus_damage(&self, level: usize) -> usize {
        let per_level = (self
            .effect_max_bonus_damage
            .saturating_sub(self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }

    /// Blows a Dream Bubble to `ally`, if one can be blown.
    fn blow(&mut self, ctx: &mut StableSim<'_>, ally: usize) {
        if !self.ready() {
            return;
        }
        let Some(team) = ctx
            .get_entity(ally)
            .filter(|ally| ally.is_champion() && ally.is_alive())
            .map(|ally| ally.team())
        else {
            return;
        };
        let Some(player) = player_of(ctx, ally) else {
            return;
        };
        let Some(dealt) = dealt(ctx, player) else {
            return;
        };
        let remaining = ticks(self.effect_duration_seconds).max(1);
        refresh_buff(
            ctx,
            ally,
            BUBBLE_BUFF,
            &BuffV1::timed(BUBBLE_BUFF, remaining),
        );
        self.bubble = Some(Bubble {
            ally,
            player,
            remaining,
            dealt,
            enemies: enemy_champions(ctx, team),
        });
    }

    /// Watches the bubbled ally for a hit on an enemy champion, which pops the
    /// bubble, and counts the bubble down.
    fn run_bubble(&mut self, ctx: &mut StableSim<'_>) {
        let Some(mut bubble) = self.bubble.take() else {
            return;
        };
        let ally = ctx
            .get_entity(bubble.ally)
            .filter(|ally| ally.is_alive())
            .map(|ally| (ally.team(), ally.level()));
        let Some((team, level)) = ally else {
            // The ally died with it: the bubble goes with them.
            ctx.entity_remove_buff(bubble.ally, BUBBLE_BUFF);
            return;
        };

        // The enemy champions that lost health or shield since the last tick.
        let now = enemy_champions(ctx, team);
        let hit: Vec<usize> = now
            .iter()
            .filter(|&&(id, health)| {
                bubble
                    .enemies
                    .iter()
                    .any(|&(known, before)| known == id && health < before)
            })
            .map(|&(id, _)| id)
            .collect();
        bubble.enemies = now;

        if !hit.is_empty() {
            if let Some(total) = dealt(ctx, bubble.player) {
                let landed = total > bubble.dealt;
                bubble.dealt = total;
                let victim = hit
                    .into_iter()
                    .min_by_key(|&enemy| ctx.distance_sq(bubble.ally, enemy));
                if let (true, Some(victim)) = (landed, victim) {
                    ctx.entity_remove_buff(bubble.ally, BUBBLE_BUFF);
                    // Set before the damage goes out, not after: the hit runs
                    // the attack pipeline, and the cooldown is what the text
                    // says starts here.
                    self.cooldown = ticks(self.effect_cooldown_seconds);
                    ctx.deal_damage(
                        bubble.ally,
                        victim,
                        0,
                        self.bonus_damage(level),
                        AttackTypeV1::Item,
                    );
                    return;
                }
            }
        }

        bubble.remaining -= 1;
        if bubble.remaining == 0 {
            // Unpopped. The buff is timed to run out on this tick anyway.
            ctx.entity_remove_buff(bubble.ally, BUBBLE_BUFF);
            return;
        }
        self.bubble = Some(bubble);
    }
}

/// The player whose champion `entity` is: the statistics hang off the player.
fn player_of(ctx: &StableSim<'_>, entity: usize) -> Option<usize> {
    (0..ctx.player_count()).find_map(|index| {
        let player_ref = ctx.player_at(index)?;
        let champion_ref = player_ref.champion()?;
        (champion_ref.id() == entity).then(|| player_ref.id())
    })
}

/// The player's damage to champions, over the match.
fn dealt(ctx: &StableSim<'_>, player: usize) -> Option<u64> {
    let json = ctx.get_player(player)?.statistics_json("")?;
    let dealing: Dealing = serde_json::from_str(&json).ok()?;
    Some(dealing.deal)
}

/// (id, health and shield) of every living champion not on `team`.
fn enemy_champions(ctx: &StableSim<'_>, team: usize) -> Vec<(usize, usize)> {
    let mut enemies = Vec::new();
    for index in 0..ctx.champion_count() {
        let id = ctx.champion_id_at(index);
        let Some(enemy_ref) = ctx.get_entity(id) else {
            continue;
        };
        if !enemy_ref.is_alive() || !enemy_ref.is_champion() || enemy_ref.team() == team {
            continue;
        }
        enemies.push((id, enemy_ref.hp().0 + enemy_ref.shield()));
    }
    enemies
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
        self.blow(ctx, target);
    }

    /// Runs the cooldown, the watch on a self-cast and the bubble, and pays
    /// Shared Riches.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
        if let Some(&ally) = self.self_cast.poll(ctx).first() {
            self.blow(ctx, ally);
        }
        self.run_bubble(ctx);
        self.riches.update(
            ctx,
            player,
            self.effect_bonus_gold,
            self.effect_gold_interval_seconds,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
