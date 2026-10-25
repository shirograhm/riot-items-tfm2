use crate::config::ItemConfig;
use crate::{apply_config, heal_shield_power, is_monster, percent_of, ticks, DOT_TICK_RATE};
use mod_api_stable::*;

// Hunter's Talisman: a jungle starter, Grez's Spectral Lantern's through
// Spirit Stone and Philosopher's Stone's through Philosopher's Medallion.
// Tooth burns a monster it damages for a share of its maximum health, the
// burn starting over on each new hit; Omnivamp against monsters is a heal on
// every hit on one, the burn's included.
#[derive(Clone, Debug)]
pub struct HuntersTalisman {
    price: usize,
    effect_hp_percent_damage: f64,
    effect_duration_seconds: f64,
    effect_vamp: i32,
    // Monsters burning now, as `(entity, ticks left, ticks until the next tick)`.
    burns: Vec<(usize, usize, usize)>,
}

impl Default for HuntersTalisman {
    fn default() -> Self {
        Self {
            price: 250,
            effect_hp_percent_damage: 5.0,
            effect_duration_seconds: 5.0,
            effect_vamp: 7,
            burns: Vec::new(),
        }
    }
}

impl HuntersTalisman {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                effect_hp_percent_damage,
                effect_duration_seconds,
                effect_vamp
            ]
        );
        item
    }

    fn duration_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds).max(DOT_TICK_RATE)
    }

    fn instance_count(&self) -> usize {
        (self.duration_ticks() / DOT_TICK_RATE).max(1)
    }

    // One tick of the burn, or `None` if the monster is gone: an even share
    // of the whole burn, which is a share of its maximum health. Re-read every
    // tick, since entity ids are recycled.
    fn instance_damage(&self, ctx: &mut StableSim<'_>, id: usize) -> Option<usize> {
        let monster = ctx.get_entity(id)?;
        if !monster.is_alive() {
            return None;
        }
        let total = percent_of(monster.hp().1, self.effect_hp_percent_damage);
        Some((total as f64 / self.instance_count() as f64).round() as usize)
    }

    fn apply_burn(&mut self, target: usize) {
        let duration = self.duration_ticks();
        match self.burns.iter_mut().find(|(id, _, _)| *id == target) {
            Some(burn) => burn.1 = duration,
            None => self.burns.push((target, duration, DOT_TICK_RATE)),
        }
    }

    fn tick_burns(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let mut kept = Vec::with_capacity(self.burns.len());
        for (id, remaining, until_next) in std::mem::take(&mut self.burns) {
            let remaining = remaining.saturating_sub(1);
            let mut until_next = until_next.saturating_sub(1);
            if until_next == 0 {
                let Some(damage) = self.instance_damage(ctx, id) else {
                    continue;
                };
                ctx.deal_damage(caster, id, 0, damage, AttackTypeV1::Item);
                until_next = DOT_TICK_RATE;
            }
            if remaining > 0 {
                kept.push((id, remaining, until_next));
            }
        }
        self.burns = kept;
    }
}

impl StableItem for HuntersTalisman {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hunters_talisman".to_string()
    }

    fn icon(&self) -> String {
        "hunters_talisman".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "spirit_stone".to_string(),
            "philosophers_medallion".to_string(),
        ]
    }

    // No stats of its own: everything it gives is its passives.
    fn stat(&self) -> BuffV1 {
        BuffV1::default()
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.burns.clear();
    }

    // Omnivamp off every hit on a monster; Tooth starts or restarts the burn
    // on any but an `Item` hit, which the burn's own ticks are (they would
    // keep it alight forever).
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !ctx
            .get_entity(target)
            .is_some_and(|target_ref| is_monster(&target_ref))
        {
            return;
        }
        let heal = percent_of(*damage, self.effect_vamp.max(0) as f64);
        if heal > 0 {
            // Sustain, not a cast heal: Heal and Shield Power leaves it alone.
            heal_shield_power::unraised(|| ctx.heal(caster, caster, heal));
        }
        if attack_type != AttackTypeV1::Item {
            self.apply_burn(target);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.burns.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| champion.id())
        else {
            // A dead carrier burns nothing, and holding the list for its next
            // life would relight ids that may belong to other units by then.
            self.burns.clear();
            return;
        };
        self.tick_burns(ctx, caster);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::DotDamage, ItemTagV1::Vamp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}
