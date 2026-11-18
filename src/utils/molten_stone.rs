//! Shield of Molten Stone's Immovable as the Earth: a % armor buff on every
//! champion holding either tier, kept up from the match hook like Sunfire
//! Cape's Immolate. The item is the game's own, so it cannot run code itself;
//! its stat block keeps the flat stats and Plating, and the % armor lives here
//! (`effect_defence_mult` in `config.json`, read by the DLL only).

use mod_api_stable::*;

use crate::apply_config;
use crate::config::ItemConfig;

// The game's keys for the two tiers.
pub(crate) const MOLTEN_STONE_KEY: &str = "eternal_iron_plate";
pub(crate) const RADIANT_MOLTEN_STONE_KEY: &str = "impregnable_fortress";

// One buff whatever is held; same-name buffs stack in the engine, so it is
// only ever removed and re-added whole.
const BUFF: &str = "immovable_as_the_earth";

// Ticks between two looks, as `patches::live`: a buff just added does not
// show among an entity's buffs for a few ticks.
const CHECK_TICKS: usize = 20;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Immovable {
    effect_defence_mult: i32,
}

impl Default for Immovable {
    fn default() -> Self {
        Self {
            effect_defence_mult: 20,
        }
    }
}

impl Immovable {
    pub(crate) fn with_config(cfg: &ItemConfig) -> Self {
        let mut immovable = Self::default();
        apply_config!(immovable, cfg, [effect_defence_mult]);
        immovable
    }
}

// Keeps every living champion's [`BUFF`] at the strongest tier it holds, and
// takes it off one that holds neither.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>, base: &Immovable, radiant: &Immovable) {
    if sim.tick() % CHECK_TICKS != 0 || sim.is_end() {
        return;
    }
    // (champion, the % it should hold, whether it holds any buff now)
    let mut fixes: Vec<(usize, i32, bool)> = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let Some(champion) = player.champion().filter(|champion| champion.is_alive()) else {
            continue;
        };
        let keys = player.item_keys();
        let held = |key: &str| keys.iter().any(|held| held == key);
        let want = [
            (MOLTEN_STONE_KEY, base.effect_defence_mult),
            (RADIANT_MOLTEN_STONE_KEY, radiant.effect_defence_mult),
        ]
        .iter()
        .filter(|(key, _)| held(key))
        .map(|(_, percent)| *percent)
        .max()
        .unwrap_or(0);
        let mut count = 0usize;
        let mut right = false;
        for slot in 0..champion.buff_count() {
            let Some(buff) = champion.buff_at(slot) else {
                continue;
            };
            if buff.name() == BUFF {
                count += 1;
                right = buff.defence_mult == want;
            }
        }
        if (want != 0 && count == 1 && right) || (want == 0 && count == 0) {
            continue;
        }
        fixes.push((champion.id(), want, count > 0));
    }
    for (champion, want, held) in fixes {
        if held {
            sim.entity_remove_buff(champion, BUFF);
        }
        if want != 0 {
            sim.add_buff(
                champion,
                &BuffV1 {
                    defence_mult: want,
                    ..BuffV1::named(BUFF)
                },
            );
        }
    }
}
