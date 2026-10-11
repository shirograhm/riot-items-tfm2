use std::collections::HashMap;

use super::base::{Base, Family, Member};
use super::fields;
use super::state::{Change, Moved, Patch, State};

// Items a patch nerfs at most.
const MAX_NERFS: usize = 4;
// And buffs at most.
const MAX_BUFFS: usize = 5;
// The score an item has to reach, either way, to be touched.
const THRESHOLD: f64 = 0.6;

const MIN_GAMES: u32 = 15;

// How far from what was wanted a change may come out: no less than this
// much of it, no more than this much of it. A step over the second is made
// up for, down to somewhere between the two.
const NET_SPAN: (f64, f64) = (0.5, 2.0);
// The furthest a number's smallest step is looked for, as a share of the
// number.
const FAR: f64 = 0.6;
// The most a number is moved to make up for another, in hundredths of it.
const MAKE_UP_FAR: u32 = 30;

// Imaginary even games added to every item's record.
const PRIOR_GAMES: f64 = 24.0;
// Win rate over (or under) 50% that is worth one point of score: four
// points of win rate.
const WIN_UNIT: f64 = 0.04;
// Points of score for being held twice as often as the average chosen
// item, and taken for being held half as often.
const PLAY_WEIGHT: f64 = 0.6;
// How many doublings of that count at most.
const PLAY_SPAN: f64 = 2.0;

// What a patch moves a number by, as a share of it, for an item whose
// score is two points. Less for a lower score and more for a higher one,
// within [`RATE_SPAN`].
const RATE: f64 = 0.06;
const RATE_SPAN: (f64, f64) = (0.6, 1.5);
// The score from which an item has two of its numbers moved, not one.
const TWO_FIELDS_FROM: f64 = 2.0;

// How one item did: times it was held at the end of a match, and how many
// of those were on the winning side.
#[derive(Clone, Copy, Default)]
pub(crate) struct Tally {
    pub games: u32,
    pub wins: u32,
}

// A small generator of its own, seeded from the patch and the item, so the
// same save makes the same patch from the same matches.
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
    // Where equal scores fall: differently every patch, the same for the
    // same patch.
    tiebreak: u64,
}

// What each tier of `family` reads at `from` and at `to`, for the tiers
// whose number is not the same at both.
fn moves(family: &Family, field: &str, from: f64, to: f64) -> Vec<Moved> {
    family
        .members
        .iter()
        .filter_map(|member| {
            let unpatched = *member.values.get(field)?;
            let whole = member.whole.contains(field);
            let before = fields::quantize(&member.key, field, unpatched, unpatched * from, whole);
            let after = fields::quantize(&member.key, field, unpatched, unpatched * to, whole);
            (before != after).then(|| Moved {
                key: member.key.clone(),
                before,
                after,
            })
        })
        .collect()
}

// One number of an item moved, in every tier it moves in.
struct Step {
    field: String,
    // [`State::ratios`] for the field once this is in.
    ratio: f64,
    members: Vec<Moved>,
    // How far the first tier's number went, as a share of its unpatched
    // value and counted as strength: plus for a stronger item, so minus for
    // a longer cooldown.
    power: f64,
    // Some tier's number moved by more than [`fields::MAX_SINGLE_CHANGE`]
    // of what it was.
    big: bool,
}

// The tier a change is measured on and has to move: the first that has the
// field, the legendary. It was the last, the radiant, while every number
// went in small steps and the two moved together. In steps of five and
// fifty the radiant's bigger number takes a step first, and a change that
// stopped there would be the radiant's alone, patch after patch.
fn lead<'a>(family: &'a Family, field: &str) -> Option<&'a Member> {
    family
        .members
        .iter()
        .find(|member| member.values.contains_key(field))
}

// How much stronger than unpatched a number is at `value`, as a share of
// its unpatched `base`.
fn strength(rule: fields::Rule, base: f64, value: f64) -> f64 {
    let share = value / base - 1.0;
    if rule.lower_is_stronger {
        -share
    } else {
        share
    }
}

// Whether a tier's number may be where `moved` puts it: inside the band of
// its unpatched value, or one step from it. The one step is for the number
// whose first step is already outside (10 Ability Haste in fives, the 33 of
// Trinity Force in tens): it has that step each way and no more.
fn in_bounds(family: &Family, field: &str, moved: &Moved) -> bool {
    let Some(member) = family.members.iter().find(|member| member.key == moved.key) else {
        return false;
    };
    let Some(&base) = member.values.get(field) else {
        return false;
    };
    let step = fields::step(&member.key, field, base, member.whole.contains(field));
    (fields::MIN_RATIO - 1e-9..=fields::MAX_RATIO + 1e-9).contains(&(moved.after / base))
        || (moved.after - base).abs() <= step + 1e-9
}

// `field` of `family` made stronger or weaker by the least that moves the
// legendary's number, looked for from `from` to `to` (shares of the number,
// a hundredth at a time). Nothing where no step is to be had: none that
// near, or the number is at its bounds.
//
// Of the ratios that give the legendary that one step, it is the one that
// moves the most tiers: the radiant's number comes along where it has a
// step to take before the legendary's has a second.
fn nudge(
    state: &State,
    id: &str,
    family: &Family,
    field: &str,
    stronger: bool,
    from: f64,
    to: f64,
) -> Option<Step> {
    let rule = fields::rule(field)?;
    let current = state.ratio(id, field);
    let lead = lead(family, field)?;
    let base = *lead.values.get(field)?;
    let up = stronger != rule.lower_is_stronger;
    let first = ((from * 100.0).round() as u32).max(1);
    let last = (to * 100.0).round() as u32;
    // The step so far, and what it makes the legendary's number.
    let mut found: Option<(Step, f64)> = None;
    for hundredth in first..=last {
        let share = f64::from(hundredth) / 100.0;
        let ratio = if up {
            current * (1.0 + share)
        } else {
            current * (1.0 - share)
        };
        // An unsigned stat is never patched under what the game holds.
        if ratio <= 0.0 || (rule.unsigned && ratio < 1.0 - 1e-9) {
            break;
        }
        let members = moves(family, field, current, ratio);
        let Some((before, after)) = members
            .iter()
            .find(|moved| moved.key == lead.key)
            .map(|moved| (moved.before, moved.after))
        else {
            continue;
        };
        if found.as_ref().is_some_and(|(_, settled)| *settled != after) {
            break;
        }
        if !members.iter().all(|moved| in_bounds(family, field, moved)) {
            break;
        }
        if found
            .as_ref()
            .is_some_and(|(step, _)| members.len() <= step.members.len())
        {
            continue;
        }
        let power = strength(rule, base, after) - strength(rule, base, before);
        let big = members.iter().any(|moved| {
            ((moved.after - moved.before) / moved.before).abs() > fields::MAX_SINGLE_CHANGE + 1e-9
        });
        found = Some((
            Step {
                field: field.to_string(),
                ratio,
                members,
                power,
                big,
            },
            after,
        ));
    }
    found.map(|(step, _)| step)
}

// The move of `field` that best makes up for a step of `got` where `rate`
// was wanted: `stronger` is the way it goes, against the step. Best is what
// leaves the two nearest `rate` together, and nothing is returned where no
// move of the field leaves them within [`NET_SPAN`] of it. The move itself
// is never a big one ([`Step::big`]).
fn counter(
    state: &State,
    id: &str,
    family: &Family,
    field: &str,
    stronger: bool,
    got: f64,
    rate: f64,
) -> Option<Step> {
    let mut best: Option<(f64, Step)> = None;
    for hundredth in 1..=MAKE_UP_FAR {
        let share = f64::from(hundredth) / 100.0;
        let Some(step) = nudge(state, id, family, field, stronger, share, share) else {
            continue;
        };
        if step.big {
            break;
        }
        let net = got - step.power.abs();
        // Further would only take more of the step back.
        if net < rate * NET_SPAN.0 - 1e-9 {
            break;
        }
        if net > rate * NET_SPAN.1 + 1e-9 {
            continue;
        }
        let miss = (net - rate).abs();
        if best.as_ref().is_none_or(|(least, _)| miss < *least - 1e-12) {
            best = Some((miss, step));
        }
    }
    best.map(|(_, step)| step)
}

// The item's total: how far each of its numbers is from its unpatched
// value, as a share of it and counted as strength, all added up. With the
// numbers as the state has them, but for the fields `plan` moves.
//
// Read off the legendary, like every change.
fn total(state: &State, id: &str, family: &Family, plan: &[&Step]) -> f64 {
    let mut seen: Vec<&str> = Vec::new();
    let mut sum = 0.0;
    for member in &family.members {
        for (field, &base) in &member.values {
            if seen.contains(&field.as_str()) {
                continue;
            }
            seen.push(field.as_str());
            let Some(rule) = fields::rule(field) else {
                continue;
            };
            let ratio = plan
                .iter()
                .find(|step| step.field == *field)
                .map_or_else(|| state.ratio(id, field), |step| step.ratio);
            let value = fields::quantize(
                &member.key,
                field,
                base,
                base * ratio,
                member.whole.contains(field),
            );
            sum += strength(rule, base, value);
        }
    }
    sum
}

// Whether the item is still inside its band with `more` on top of `plan`:
// its total no more than [`fields::MAX_RATIO`] after a buff, no less than
// [`fields::MIN_RATIO`] after a nerf.
fn fits(state: &State, id: &str, family: &Family, plan: &[Step], more: &[Step], buff: bool) -> bool {
    let all: Vec<&Step> = plan.iter().chain(more).collect();
    let total = total(state, id, family, &all);
    if buff {
        total <= fields::MAX_RATIO - 1.0 + 1e-9
    } else {
        total >= fields::MIN_RATIO - 1.0 - 1e-9
    }
}

// `first`, and ahead of it the other end of the range it is an end of, at
// the same ratio: both ends of a range, or neither.
fn with_partner(state: &State, id: &str, family: &Family, first: Step) -> Vec<Step> {
    let partner = fields::partner(&first.field)
        .and_then(|partner| Some((partner, fields::rule(partner)?, lead(family, partner)?)));
    let Some((partner, rule, lead)) = partner else {
        return vec![first];
    };
    let from = state.ratio(id, partner);
    let members = moves(family, partner, from, first.ratio);
    let power = members
        .iter()
        .find(|moved| moved.key == lead.key)
        .zip(lead.values.get(partner))
        .map_or(0.0, |(moved, &base)| {
            strength(rule, base, moved.after) - strength(rule, base, moved.before)
        });
    vec![
        Step {
            field: partner.to_string(),
            ratio: first.ratio,
            members,
            power,
            big: false,
        },
        first,
    ]
}

// The changes to one item: one or two of its numbers, picked at random
// among those that can move, the other end of a range with it, and what
// makes up for a step that is more than was wanted.
//
// A number moves by its smallest step. Twice what was wanted or less, the
// step stands as it is. More, and another number of the item is moved the
// other way ([`counter`]): then a step may even be a big one, over a
// quarter of the number, which nothing else lets through. A number that
// finds nothing to make up for it is passed over, and once every number
// has been asked that way, whatever is still wanted is taken as it used to
// be: any step that is no big one, however far over.
//
// Every row a change brings is the item's, under its buff or its nerf: what
// makes up for a buff is a number going down in the list of buffs.
fn changes_for(state: &State, judged: &Judged<'_>, buff: bool, number: u32) -> Vec<Change> {
    let size = judged.score.abs();
    let rate = RATE * (size / 2.0).clamp(RATE_SPAN.0, RATE_SPAN.1);
    let wanted = if size >= TWO_FIELDS_FROM { 2 } else { 1 };
    let (id, family) = (judged.id, judged.family);

    let mut candidates: Vec<&str> = Vec::new();
    for member in &family.members {
        for field in member.values.keys() {
            if !candidates.contains(&field.as_str()) {
                candidates.push(field);
            }
        }
    }
    candidates.sort_unstable();
    Rng::seeded(number, id).shuffle(&mut candidates);

    let mut plan: Vec<Step> = Vec::new();
    let mut picked = 0;
    // The numbers that come out near what was wanted, by themselves or
    // made up for.
    for &field in &candidates {
        if picked == wanted {
            break;
        }
        if plan.iter().any(|step| step.field == field) {
            continue;
        }
        let Some(first) = nudge(state, id, family, field, buff, rate, FAR) else {
            continue;
        };
        let got = first.power.abs();
        let over = got > rate * NET_SPAN.1 + 1e-9;
        if !over && first.big {
            continue;
        }
        let mut steps = with_partner(state, id, family, first);
        if over {
            // Not an end of a range: its other end would have to follow.
            let made_up = candidates
                .iter()
                .copied()
                .filter(|other| fields::partner(other).is_none())
                .filter(|other| !plan.iter().chain(&steps).any(|step| step.field == *other))
                .filter_map(|other| counter(state, id, family, other, !buff, got, rate))
                .min_by(|a, b| {
                    let miss = |step: &Step| (got - step.power.abs() - rate).abs();
                    miss(a).total_cmp(&miss(b))
                });
            let Some(made_up) = made_up else {
                continue;
            };
            steps.push(made_up);
        }
        if !fits(state, id, family, &plan, &steps, buff) {
            continue;
        }
        plan.extend(steps);
        picked += 1;
    }
    // And for what is still wanted, a step as it used to be taken.
    for &field in &candidates {
        if picked == wanted {
            break;
        }
        if plan.iter().any(|step| step.field == field) {
            continue;
        }
        let far = (rate * 3.0).min(fields::MAX_SINGLE_CHANGE);
        let Some(first) = nudge(state, id, family, field, buff, rate, far) else {
            continue;
        };
        if first.big {
            continue;
        }
        let steps = with_partner(state, id, family, first);
        if !fits(state, id, family, &plan, &steps, buff) {
            continue;
        }
        plan.extend(steps);
        picked += 1;
    }

    plan.into_iter()
        .map(|step| Change {
            family: id.to_string(),
            field: step.field,
            buff,
            ratio: step.ratio,
            members: step.members,
            games: judged.tally.games,
            wins: judged.tally.wins,
        })
        .collect()
}

// The patch for the matches of `version`: `tallies` by item key.
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
        .filter(|item| item.tally.games > MIN_GAMES)
        .take_while(|item| item.score >= THRESHOLD)
        .take(MAX_NERFS)
    {
        changes.extend(changes_for(state, item, false, number));
    }
    for item in judged
        .iter()
        .rev()
        .filter(|item| item.tally.games > MIN_GAMES)
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
        ..Patch::default()
    }
}
