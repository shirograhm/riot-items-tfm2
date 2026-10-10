//! Choosing what one patch changes: which items, which of their numbers,
//! and by how much, from how the items did on the game version that just
//! ended.
//!
//! An item is judged on two things, as the game's own champion patches are.
//! Its win rate, pulled toward an even 50% by [`PRIOR_GAMES`] imaginary games
//! so that three wins out of three do not read as an item that never loses.
//! And how often it was held, against the other items builds choose between.
//! The two add up to one score; the highest are nerfed and the lowest buffed,
//! an item nobody held among them.
//!
//! Every number here is this mod's pick (2026-10-10), untuned.

use std::collections::HashMap;

use super::base::{Base, Family};
use super::fields;
use super::state::{Change, Moved, Patch, State};

/// Items a patch nerfs at most.
const MAX_NERFS: usize = 4;
/// And buffs at most.
const MAX_BUFFS: usize = 5;
/// The score an item has to reach, either way, to be touched.
const THRESHOLD: f64 = 0.6;

/// Imaginary even games added to every item's record.
const PRIOR_GAMES: f64 = 24.0;
/// Win rate over (or under) 50% that is worth one point of score: four
/// points of win rate.
const WIN_UNIT: f64 = 0.04;
/// Points of score for being held twice as often as the average chosen
/// item, and taken for being held half as often.
const PLAY_WEIGHT: f64 = 0.6;
/// How many doublings of that count at most.
const PLAY_SPAN: f64 = 2.0;

/// What a patch moves a number by, as a share of it, for an item whose
/// score is two points. Less for a lower score and more for a higher one,
/// within [`RATE_SPAN`].
const RATE: f64 = 0.06;
const RATE_SPAN: (f64, f64) = (0.6, 1.5);
/// The score from which an item has two of its numbers moved, not one.
const TWO_FIELDS_FROM: f64 = 2.0;

/// How one item did: times it was held at the end of a match, and how many
/// of those were on the winning side.
#[derive(Clone, Copy, Default)]
pub(crate) struct Tally {
    pub games: u32,
    pub wins: u32,
}

/// A small generator of its own, seeded from the patch and the item, so the
/// same save makes the same patch from the same matches.
struct Rng(u64);

impl Rng {
    fn seeded(number: u32, text: &str) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(number).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for byte in text.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(hash)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            let pick = (self.next() % (last as u64 + 1)) as usize;
            items.swap(last, pick);
        }
    }
}

struct Judged<'a> {
    id: &'a str,
    family: &'a Family,
    tally: Tally,
    score: f64,
    /// Where equal scores fall: differently every patch, the same for the
    /// same patch.
    tiebreak: u64,
}

/// What each tier of `family` reads at `from` and at `to`, for the tiers
/// whose number is not the same at both.
fn moves(family: &Family, field: &str, from: f64, to: f64) -> Vec<Moved> {
    family
        .members
        .iter()
        .filter_map(|member| {
            let unpatched = *member.values.get(field)?;
            let whole = member.whole.contains(field);
            let before = fields::quantize(unpatched, unpatched * from, whole);
            let after = fields::quantize(unpatched, unpatched * to, whole);
            (before != after).then(|| Moved {
                key: member.key.clone(),
                before,
                after,
            })
        })
        .collect()
}

/// One field of `family` buffed or nerfed by about `rate`, or nothing where
/// it cannot be: at its floor or cap, or a number whose smallest step is a
/// bigger change than a patch makes.
fn change(
    state: &State,
    id: &str,
    family: &Family,
    field: &str,
    buff: bool,
    rate: f64,
    tally: Tally,
) -> Option<Change> {
    let rule = fields::rule(field)?;
    let current = state.ratio(id, field);
    // The tier builds end at: the last one that has the field.
    let lead = family
        .members
        .iter()
        .rev()
        .find(|member| member.values.contains_key(field))?;
    let up = buff != rule.lower_is_stronger;
    let floor = if rule.unsigned {
        1.0
    } else {
        fields::MIN_RATIO
    };
    // A small whole number does not move for a few percent: the rate is
    // raised until the lead tier's number does, as far as a patch may go.
    for factor in [1.0, 1.5, 2.0, 3.0] {
        let step = rate * factor;
        if step > fields::MAX_SINGLE_CHANGE {
            break;
        }
        let ratio = if up {
            current * (1.0 + step)
        } else {
            current * (1.0 - step)
        };
        if ratio < floor - 1e-9 || ratio > fields::MAX_RATIO + 1e-9 {
            break;
        }
        let members = moves(family, field, current, ratio);
        if !members.iter().any(|moved| moved.key == lead.key) {
            continue;
        }
        let jumps = members
            .iter()
            .any(|moved| ((moved.after - moved.before) / moved.before).abs() > fields::MAX_SINGLE_CHANGE + 1e-9);
        if jumps {
            return None;
        }
        return Some(Change {
            family: id.to_string(),
            field: field.to_string(),
            buff,
            ratio,
            members,
            games: tally.games,
            wins: tally.wins,
        });
    }
    None
}

/// The changes to one item: one or two of its numbers, picked at random
/// among those that can move, and the other end of a range with it.
fn changes_for(state: &State, judged: &Judged<'_>, buff: bool, number: u32) -> Vec<Change> {
    let size = judged.score.abs();
    let rate = RATE * (size / 2.0).clamp(RATE_SPAN.0, RATE_SPAN.1);
    let wanted = if size >= TWO_FIELDS_FROM { 2 } else { 1 };

    let mut candidates: Vec<&str> = Vec::new();
    for member in &judged.family.members {
        for field in member.values.keys() {
            if !candidates.contains(&field.as_str()) {
                candidates.push(field);
            }
        }
    }
    candidates.sort_unstable();
    Rng::seeded(number, judged.id).shuffle(&mut candidates);

    let mut out: Vec<Change> = Vec::new();
    let mut picked = 0;
    for field in candidates {
        if picked == wanted {
            break;
        }
        if out.iter().any(|change| change.field == field) {
            continue;
        }
        let Some(first) = change(state, judged.id, judged.family, field, buff, rate, judged.tally) else {
            continue;
        };
        // Both ends of a range by the same ratio, or neither.
        if let Some(partner) = fields::partner(field) {
            let has_partner = judged
                .family
                .members
                .iter()
                .any(|member| member.values.contains_key(partner));
            if has_partner {
                let from = state.ratio(judged.id, partner);
                let members = moves(judged.family, partner, from, first.ratio);
                out.push(Change {
                    family: judged.id.to_string(),
                    field: partner.to_string(),
                    buff,
                    ratio: first.ratio,
                    members,
                    games: judged.tally.games,
                    wins: judged.tally.wins,
                });
            }
        }
        out.push(first);
        picked += 1;
    }
    out
}

/// The patch for the matches of `version`: `tallies` by item key.
pub(crate) fn decide(
    base: &Base,
    state: &State,
    tallies: &HashMap<String, Tally>,
    matches: u32,
    number: u32,
    version: &str,
) -> Patch {
    let of = |family: &Family| {
        family.members.iter().fold(Tally::default(), |sum, member| {
            let tally = tallies.get(&member.key).copied().unwrap_or_default();
            Tally {
                games: sum.games + tally.games,
                wins: sum.wins + tally.wins,
            }
        })
    };
    // How often a chosen item that was held at all was held, on average.
    let held: Vec<f64> = base
        .families
        .values()
        .filter(|family| family.chosen)
        .map(|family| f64::from(of(family).games))
        .filter(|games| *games > 0.0)
        .collect();
    let usual = if held.is_empty() {
        0.0
    } else {
        held.iter().sum::<f64>() / held.len() as f64
    };

    let mut judged: Vec<Judged<'_>> = base
        .families
        .iter()
        .map(|(id, family)| {
            let tally = of(family);
            let games = f64::from(tally.games);
            let win = (f64::from(tally.wins) + PRIOR_GAMES / 2.0) / (games + PRIOR_GAMES) - 0.5;
            let play = if family.chosen && usual > 0.0 {
                ((games + 1.0) / (usual + 1.0))
                    .log2()
                    .clamp(-PLAY_SPAN, PLAY_SPAN)
            } else {
                0.0
            };
            Judged {
                id,
                family,
                tally,
                score: win / WIN_UNIT + PLAY_WEIGHT * play,
                tiebreak: Rng::seeded(number, id).next(),
            }
        })
        .collect();
    judged.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.tiebreak.cmp(&b.tiebreak))
    });

    let mut changes = Vec::new();
    for item in judged
        .iter()
        .take_while(|item| item.score >= THRESHOLD)
        .take(MAX_NERFS)
    {
        changes.extend(changes_for(state, item, false, number));
    }
    for item in judged
        .iter()
        .rev()
        .take_while(|item| item.score <= -THRESHOLD)
        .take(MAX_BUFFS)
    {
        changes.extend(changes_for(state, item, true, number));
    }
    Patch {
        number,
        version: version.to_string(),
        matches,
        changes,
    }
}
