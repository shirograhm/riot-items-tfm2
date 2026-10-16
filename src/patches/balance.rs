use std::collections::HashMap;

use super::base::{Base, Family, Member};
use super::fields;
use super::state::{Change, Moved, Patch, State};

// The score an item has to reach, either way, to be touched.
const THRESHOLD: f64 = 0.6;

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

// How seldom a component that is built into one finished item only is
// patched in that item's place: in one patch out of this many of those the
// item is over the mark in.
const ALONE_ONE_IN: u64 = 4;

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
    // How far its changes go: its score without the sign, and for a
    // component the middle of the scores it stands in for.
    size: f64,
    // Where it falls among equal scores, ahead of the tiebreak: a component
    // comes before the items it stands in for, whichever end the changes
    // are picked from.
    rank: u8,
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
// field. A patch plans every tier by itself ([`changes_for`]), so that is
// the one tier there is.
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
//
// A number the player gave a least or a most of its own (`State::limits`)
// is held to those and not to the band: see [`State::within`].
fn in_bounds(state: &State, family: &Family, field: &str, moved: &Moved) -> bool {
    let Some(member) = family.members.iter().find(|member| member.key == moved.key) else {
        return false;
    };
    let Some(&base) = member.values.get(field) else {
        return false;
    };
    if let Some(within) = state.within(&moved.key, field, moved.before, moved.after) {
        return within;
    }
    let step = fields::step(&member.key, field, base, member.whole.contains(field));
    let tuning = super::tuning::tuning();
    (tuning.min_ratio - 1e-9..=tuning.max_ratio + 1e-9).contains(&(moved.after / base))
        || (moved.after - base).abs() <= step + 1e-9
}

// `field` of `family` made stronger or weaker by the least that moves its
// number, looked for from `from` to `to` (shares of the number, a hundredth
// at a time). Nothing where no step is to be had: none that near, or the
// number is at its bounds.
//
// `family` is one tier as a patch plans it. Given more, the number that has
// to move is the first tier's ([`lead`]), and of the ratios that give it
// that one step the one is taken that moves the most tiers.
fn nudge(
    state: &State,
    id: &str,
    family: &Family,
    field: &str,
    stronger: bool,
    from: f64,
    to: f64,
) -> Option<Step> {
    let rule = fields::rule_for(id, field)?;
    // A number the player's settings hold still, or the other end of a range
    // that one is an end of.
    let frozen = &super::tuning::tuning().frozen;
    if frozen.contains(field) || fields::partner(field).is_some_and(|other| frozen.contains(other)) {
        return None;
    }
    // Or one the player has protected on this item, in the Item Patcher
    // Settings window.
    if state.protected(id, field)
        || fields::partner(field).is_some_and(|other| state.protected(id, other))
    {
        return None;
    }
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
        // The wrong way: a number set by hand can stand between two steps,
        // and the nearest step to a little less of it is then the one above.
        if members
            .iter()
            .any(|moved| (moved.after.abs() > moved.before.abs()) != up)
        {
            continue;
        }
        if found.as_ref().is_some_and(|(_, settled)| *settled != after) {
            break;
        }
        if !members.iter().all(|moved| in_bounds(state, family, field, moved)) {
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
// Of the one tier being planned, like every change.
fn total(state: &State, id: &str, family: &Family, plan: &[&Step]) -> f64 {
    let mut seen: Vec<&str> = Vec::new();
    let mut sum = 0.0;
    for member in &family.members {
        for (field, &base) in &member.values {
            if seen.contains(&field.as_str()) {
                continue;
            }
            seen.push(field.as_str());
            let Some(rule) = fields::rule_for(&member.key, field) else {
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
// its total no more than the band's top after a buff, no less than its
// bottom after a nerf (`tuning`).
fn fits(state: &State, id: &str, family: &Family, plan: &[Step], more: &[Step], buff: bool) -> bool {
    let all: Vec<&Step> = plan.iter().chain(more).collect();
    let total = total(state, id, family, &all);
    let tuning = super::tuning::tuning();
    if buff {
        total <= tuning.max_ratio - 1.0 + 1e-9
    } else {
        total >= tuning.min_ratio - 1.0 - 1e-9
    }
}

// `first`, and ahead of it the other end of the range it is an end of, at
// the same ratio: both ends of a range, or neither.
fn with_partner(state: &State, id: &str, family: &Family, first: Step) -> Vec<Step> {
    let partner = fields::partner(&first.field)
        .and_then(|partner| Some((partner, fields::rule_for(id, partner)?, lead(family, partner)?)));
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

// The steps a patch takes on one tier of an item: one or two of its
// numbers, picked at random among those that can move, the other end of a
// range with it, and what makes up for a step that is more than was wanted.
//
// A number moves by its smallest step. Twice what was wanted or less, the
// step stands as it is. More, and another number of the tier is moved the
// other way ([`counter`]): then a step may even be a big one, over a
// quarter of the number, which nothing else lets through. A number that
// finds nothing to make up for it is passed over, and once every number
// has been asked that way, whatever is still wanted is taken as it used to
// be: any step that is no big one, however far over.
//
// `tier` is the item with that one tier in it, and `id` the tier's own key,
// which its ratios are kept under. `seed` is what orders the numbers that
// are tried, and `allowed` is asked of every step before it is taken, with
// the plan so far.
#[allow(clippy::too_many_arguments)]
fn plan_for(
    state: &State,
    id: &str,
    tier: &Family,
    seed: (u32, &str),
    buff: bool,
    rate: f64,
    wanted: usize,
    allowed: &dyn Fn(&[Step], &[Step]) -> bool,
) -> Vec<Step> {
    let family = tier;
    let mut candidates: Vec<&str> = Vec::new();
    for member in &family.members {
        for field in member.values.keys() {
            if !candidates.contains(&field.as_str()) {
                candidates.push(field);
            }
        }
    }
    candidates.sort_unstable();
    Rng::seeded(seed.0, seed.1).shuffle(&mut candidates);

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
        if !fits(state, id, family, &plan, &steps, buff) || !allowed(&plan, &steps) {
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
        if !fits(state, id, family, &plan, &steps, buff) || !allowed(&plan, &steps) {
            continue;
        }
        plan.extend(steps);
        picked += 1;
    }
    plan
}

// The tiers of an item that a patch moves, by their place among its tiers.
//
// A patch does not have to move every tier of an item. A tier is moved
// where its own record says what the item's does: its holders' win share,
// taken with the same imaginary even games as the item's, on the item's
// side of even by half of what an item needs to be touched. Where no
// tier's record says that much, which is what an item touched for how
// often it is built comes to, the tier that was held the most is moved, the
// first of them where they were held as much.
fn tiers_to_move(family: &Family, tallies: &HashMap<String, Tally>, buff: bool) -> Vec<usize> {
    let of = |member: &Member| tallies.get(&member.key).copied().unwrap_or_default();
    let lean = |tally: Tally| {
        let share = (f64::from(tally.wins) + PRIOR_GAMES / 2.0) / (f64::from(tally.games) + PRIOR_GAMES);
        (share - 0.5) / WIN_UNIT
    };
    let side = if buff { -1.0 } else { 1.0 };
    let agreeing: Vec<usize> = family
        .members
        .iter()
        .enumerate()
        .filter(|(_, member)| lean(of(member)) * side >= THRESHOLD / 2.0)
        .map(|(at, _)| at)
        .collect();
    if !agreeing.is_empty() {
        return agreeing;
    }
    (0..family.members.len())
        .max_by_key(|&at| (of(&family.members[at]).games, std::cmp::Reverse(at)))
        .into_iter()
        .collect()
}

// Whether `steps`, taken on the tier at `at`, would turn the order of two
// tiers round in one of their numbers: a radiant left with less attack
// damage than its legendary, or a longer cooldown. Asked only of numbers
// the two tiers do not have alike unpatched. The other tiers are read as
// this patch has left them (`landed`: tier, field, what it reads), or as
// the state has them.
fn crosses(
    state: &State,
    family: &Family,
    at: usize,
    steps: &[Step],
    landed: &[(usize, String, f64)],
) -> bool {
    let Some(mine) = family.members.get(at) else {
        return false;
    };
    steps.iter().any(|step| {
        let field = step.field.as_str();
        let (Some(&my_base), Some(after)) = (
            mine.values.get(field),
            step.members.first().map(|moved| moved.after),
        ) else {
            return false;
        };
        family.members.iter().enumerate().any(|(other_at, other)| {
            let Some(&their_base) = other.values.get(field) else {
                return false;
            };
            if other_at == at || their_base == my_base {
                return false;
            }
            let theirs = landed
                .iter()
                .rev()
                .find(|(tier, moved, _)| *tier == other_at && moved == field)
                .map_or_else(
                    || {
                        fields::quantize(
                            &other.key,
                            field,
                            their_base,
                            their_base * state.ratio(&other.key, field),
                            other.whole.contains(field),
                        )
                    },
                    |(_, _, value)| *value,
                );
            // On the other side of the other tier's number than unpatched.
            (after - theirs) * (my_base - their_base) < 0.0
        })
    })
}

// Whether `key` is an item's radiant, and not its legendary.
fn is_radiant(key: &str) -> bool {
    crate::build_config::base_slug(key) != key
}

// The number of `member`'s `field` that is level with `value`: `value`
// itself where the tier counts in the same steps, and failing that the
// nearest of its own steps that is no weaker.
fn level_with(member: &Member, field: &str, value: f64, lower: bool) -> Option<f64> {
    let unpatched = *member.values.get(field)?;
    let whole = member.whole.contains(field);
    let near = fields::quantize(&member.key, field, unpatched, value, whole);
    let weaker = if lower {
        near > value + 1e-9
    } else {
        near < value - 1e-9
    };
    if !weaker {
        return Some(near);
    }
    let step = fields::step(&member.key, field, unpatched, whole);
    Some(if lower { near - step } else { near + step })
}

// Keeps a radiant no weaker than its legendary, in the numbers `changes`
// moved: the changes of one patch to one item. Every number the two tiers
// both have, a stat the item gives or a number of its passive.
//
// Where the patch leaves the legendary's number the stronger of the two,
// the radiant's comes level with it; and where it leaves the radiant's the
// weaker by moving the radiant alone, the legendary's comes level with
// that. So a buff to the legendary's Rapids is a buff to the radiant's as
// well, and a nerf to it leaves the radiant's where it is.
//
// The tier that comes level gets a change of its own for the number, or
// has the one the patch already made for it set to the level. A change of
// its own follows the other tier's (`Change::leader`), and the notes list
// it in that tier's entry.
fn level_tiers(state: &State, judged: &Judged<'_>, buff: bool, changes: &mut Vec<Change>) {
    let family = judged.family;
    let (Some(legendary), Some(radiant)) = (
        family.members.iter().find(|member| !is_radiant(&member.key)),
        family.members.iter().find(|member| is_radiant(&member.key)),
    ) else {
        return;
    };
    let shared: Vec<&String> = legendary
        .values
        .keys()
        .filter(|field| radiant.values.contains_key(*field))
        .collect();
    for field in shared {
        let Some(rule) = fields::rule_for(&legendary.key, field) else {
            continue;
        };
        // Where the patch's change to a tier's number is among `changes`.
        let moved_at = |member: &Member, changes: &[Change]| {
            changes.iter().position(|change| {
                change.field == *field && change.members.iter().any(|moved| moved.key == member.key)
            })
        };
        // What a tier's number reads: as its change leaves it, or as the
        // state has it.
        let reads = |member: &Member, changes: &[Change], index: Option<usize>| -> Option<f64> {
            if let Some(index) = index {
                return changes[index]
                    .members
                    .iter()
                    .find(|moved| moved.key == member.key)
                    .map(|moved| moved.after);
            }
            let unpatched = *member.values.get(field.as_str())?;
            Some(fields::quantize(
                &member.key,
                field,
                unpatched,
                unpatched * state.ratio(&member.key, field),
                member.whole.contains(field.as_str()),
            ))
        };
        let (low_at, high_at) = (moved_at(legendary, &changes[..]), moved_at(radiant, &changes[..]));
        if low_at.is_none() && high_at.is_none() {
            continue;
        }
        let (Some(low), Some(high)) = (
            reads(legendary, &changes[..], low_at),
            reads(radiant, &changes[..], high_at),
        ) else {
            continue;
        };
        let lower = rule.lower_is_stronger;
        let weaker = if lower {
            high > low + 1e-9
        } else {
            high < low - 1e-9
        };
        if !weaker {
            continue;
        }
        // The legendary leads where the patch moved it, and the radiant
        // where the patch moved the radiant alone.
        let (level, leader, follower, follower_at, read) = if low_at.is_some() {
            (low, legendary, radiant, high_at, high)
        } else {
            (high, radiant, legendary, low_at, low)
        };
        let (Some(after), Some(&unpatched)) = (
            level_with(follower, field, level, lower),
            follower.values.get(field.as_str()),
        ) else {
            continue;
        };
        // Not a number the player has protected, nor past the least or the
        // most they gave it: the tiers then stay as the player has them.
        if state.protected(&follower.key, field)
            || state.within(&follower.key, field, read, after) == Some(false)
        {
            continue;
        }
        let ratio = after / unpatched;
        let Some(index) = follower_at else {
            changes.push(Change {
                family: judged.id.to_string(),
                field: field.clone(),
                buff,
                ratio,
                members: vec![Moved {
                    key: follower.key.clone(),
                    before: read,
                    after,
                }],
                games: judged.tally.games,
                wins: judged.tally.wins,
                follows: false,
                manual: false,
                leader: leader.key.clone(),
            });
            continue;
        };
        // The patch moved this tier's number too: its change ends at the
        // level, and is no change where that is where the number was.
        let before = changes[index]
            .members
            .iter()
            .find(|moved| moved.key == follower.key)
            .map_or(read, |moved| moved.before);
        if (after - before).abs() < 1e-9 {
            changes.remove(index);
            continue;
        }
        changes[index].ratio = ratio;
        for moved in &mut changes[index].members {
            if moved.key == follower.key {
                moved.after = after;
            }
        }
    }
}

// The changes to one item: the steps of each tier a patch moves
// ([`tiers_to_move`]), every tier planned by itself ([`plan_for`]). So a
// patch may move the legendary and leave its radiant, or the other way
// round, and where it moves both they need not move in the same numbers:
// as it falls for the item in the patch, the tiers try their numbers in one
// order, and then mostly move the same ones, or each in an order of its
// own. What they may not do is change places ([`crosses`]), or take a stat
// under what a part of the item gives (`floors`, [`stat_floors`]); and no
// number the two share is left the weaker on the radiant ([`level_tiers`]).
//
// Every row a change brings is the item's, under its buff or its nerf: what
// makes up for a buff is a number going down in the list of buffs.
//
// And no more of those rows than the player's settings allow an item
// (`max_changes_per_item`, [`within_rows`]), counted as the notes write
// them: a number made up for another is a row of its own, the two ends of
// a range are one. The tiers share the count, so each tier's notes keep to
// it too, and still do once a tier left the weaker is brought level
// ([`level_tiers`]), which only moves a number the other tier moved. The
// upgrades kept no weaker than their parts afterwards (`level_upgrades`)
// are not held to it.
fn changes_for(
    state: &State,
    judged: &Judged<'_>,
    tallies: &HashMap<String, Tally>,
    floors: &HashMap<String, HashMap<String, f64>>,
    buff: bool,
    number: u32,
) -> Vec<Change> {
    let size = judged.size;
    let rate = RATE * (size / 2.0).clamp(RATE_SPAN.0, RATE_SPAN.1);
    let wanted = if size >= TWO_FIELDS_FROM { 2 } else { 1 };
    let family = judged.family;
    let together = Rng::seeded(number, judged.id).next() % 2 == 0;
    let cap = super::tuning::tuning().max_changes_per_item;

    // What the tiers moved so far read: tier, field, number.
    let mut landed: Vec<(usize, String, f64)> = Vec::new();
    // The rows they have taken so far, by the number that names each.
    let mut rows: Vec<String> = Vec::new();
    let mut changes = Vec::new();
    for at in tiers_to_move(family, tallies, buff) {
        let member = &family.members[at];
        let tier = Family {
            members: vec![member.clone()],
            chosen: family.chosen,
            component: family.component,
            built_into: Vec::new(),
        };
        let seed = if together {
            judged.id
        } else {
            member.key.as_str()
        };
        let plan = plan_for(
            state,
            &member.key,
            &tier,
            (number, seed),
            buff,
            rate,
            wanted,
            &|plan: &[Step], steps: &[Step]| {
                !crosses(state, family, at, steps, &landed)
                    && !undercuts(floors, &member.key, steps)
                    && within_rows(&rows, &member.key, plan, steps, cap)
            },
        );
        for step in plan {
            if let Some(moved) = step.members.first() {
                landed.push((at, step.field.clone(), moved.after));
            }
            let row = super::text::row_of(&member.key, &step.field);
            if !rows.iter().any(|taken| taken == row) {
                rows.push(row.to_string());
            }
            changes.push(Change {
                family: judged.id.to_string(),
                field: step.field,
                buff,
                ratio: step.ratio,
                members: step.members,
                games: judged.tally.games,
                wins: judged.tally.wins,
                follows: false,
                manual: false,
                leader: String::new(),
            });
        }
    }
    level_tiers(state, judged, buff, &mut changes);
    changes
}

// Whether an item's notes keep to `cap` rows with `steps` taken on top of
// `plan`, the steps of its tier `key` so far, and `rows`, what the tiers
// planned before took ([`super::text::row_of`] names a row). A number whose
// row is taken already, by this tier or another, takes none.
fn within_rows(rows: &[String], key: &str, plan: &[Step], steps: &[Step], cap: usize) -> bool {
    let mut taken: Vec<&str> = rows.iter().map(String::as_str).collect();
    for step in plan.iter().chain(steps) {
        let row = super::text::row_of(key, &step.field);
        if !taken.contains(&row) {
            taken.push(row);
        }
    }
    taken.len() <= cap
}

// The most times [`level_upgrades`] goes over the upgrades: a build path is
// four items long, and a level set at one end has to reach the other.
const LEVEL_ROUNDS: usize = 6;

// Every tier a patch can move, by its key: its family, and itself.
fn tiers_of(base: &Base) -> HashMap<&str, (&str, &Member)> {
    base.families
        .iter()
        .flat_map(|(id, family)| {
            family
                .members
                .iter()
                .map(move |member| (member.key.as_str(), (id.as_str(), member)))
        })
        .collect()
}

// The stat `field` as the game holds it for the item `key`, 0 where the
// item gives none.
fn stat_held(base: &Base, key: &str, field: &str) -> f64 {
    base.flat
        .get(key)
        .and_then(|stats| stats.get(field))
        .copied()
        .unwrap_or(0.0)
}

// The least each tier's stats may be taken down to: for every stat that a
// tier and a part it is built from both give, what the part gives as the
// state has it. A nerf is not planned under it, since the stat would only
// be put back ([`level_upgrades`]). A radiant's legendary is no such part:
// it comes down with its radiant ([`level_tiers`]), and the radiant has the
// legendary's floors.
fn stat_floors(base: &Base, state: &State) -> HashMap<String, HashMap<String, f64>> {
    let members = tiers_of(base);
    let family_of = |key: &str| members.get(key).map(|(family, _)| *family);
    let mut floors: HashMap<String, HashMap<String, f64>> = HashMap::new();
    // Twice: the second time round a radiant takes the floors its legendary
    // got in the first.
    for _ in 0..2 {
        for (part, upgrade) in &base.upgrades {
            let one_item = family_of(part).is_some() && family_of(part) == family_of(upgrade);
            for &field in fields::FLAT {
                let (part_held, upgrade_held) =
                    (stat_held(base, part, field), stat_held(base, upgrade, field));
                if part_held <= 0.0 || upgrade_held < part_held {
                    continue;
                }
                let floor = if one_item {
                    floors
                        .get(part.as_str())
                        .and_then(|stats| stats.get(field))
                        .copied()
                } else {
                    Some(stat_now(&members, base, state, &[], part, field))
                };
                let Some(floor) = floor else {
                    continue;
                };
                let known = floors
                    .entry(upgrade.clone())
                    .or_default()
                    .entry(field.to_string())
                    .or_insert(floor);
                if floor > *known {
                    *known = floor;
                }
            }
        }
    }
    floors
}

// Whether `steps` take a stat of the tier `key` down under one of its
// floors ([`stat_floors`]).
fn undercuts(floors: &HashMap<String, HashMap<String, f64>>, key: &str, steps: &[Step]) -> bool {
    let Some(floors) = floors.get(key) else {
        return false;
    };
    steps.iter().any(|step| {
        let (Some(floor), Some(moved)) = (floors.get(&step.field), step.members.first()) else {
            return false;
        };
        moved.after < moved.before && moved.after < *floor - 1e-9
    })
}

// What the stat `field` of the item `key` reads with `changes` in: a tier's
// as its change leaves it or as the state has it, and what the game holds
// for an item, or a stat of one, that no patch moves.
fn stat_now(
    members: &HashMap<&str, (&str, &Member)>,
    base: &Base,
    state: &State,
    changes: &[Change],
    key: &str,
    field: &str,
) -> f64 {
    let moved = changes
        .iter()
        .filter(|change| change.field == field)
        .flat_map(|change| change.members.iter())
        .find(|moved| moved.key == key);
    if let Some(moved) = moved {
        return moved.after;
    }
    let patched = members.get(key).and_then(|(_, member)| {
        let unpatched = *member.values.get(field)?;
        Some(fields::quantize(
            &member.key,
            field,
            unpatched,
            unpatched * state.ratio(&member.key, field),
            member.whole.contains(field),
        ))
    });
    patched.unwrap_or_else(|| stat_held(base, key, field))
}

// Sets the stat `field` of the tier `key` level with `level`: the nearest
// of its own steps that is no less where `up`, and no more where not. A
// tier `changes` has already moved in the stat has that change end at the
// level, and one it has not gets a change of its own: one that follows
// `leader`, the item the level is read off, where that is another tier of
// the same item. Whether anything was changed.
#[allow(clippy::too_many_arguments)]
fn set_level(
    members: &HashMap<&str, (&str, &Member)>,
    state: &State,
    changes: &mut Vec<Change>,
    key: &str,
    field: &str,
    level: f64,
    up: bool,
    leader: &str,
) -> bool {
    let Some(&(family, member)) = members.get(key) else {
        return false;
    };
    let Some(&unpatched) = member.values.get(field) else {
        return false;
    };
    let whole = member.whole.contains(field);
    let step = fields::step(&member.key, field, unpatched, whole);
    let near = fields::quantize(&member.key, field, unpatched, level, whole);
    let after = if up && near < level - 1e-9 {
        near + step
    } else if !up && near > level + 1e-9 {
        near - step
    } else {
        near
    };
    let ratio = after / unpatched;
    let at = changes.iter().position(|change| {
        change.field == field && change.members.iter().any(|moved| moved.key == key)
    });
    let Some(at) = at else {
        let before = fields::quantize(
            &member.key,
            field,
            unpatched,
            unpatched * state.ratio(&member.key, field),
            whole,
        );
        if (after - before).abs() < 1e-9 {
            return false;
        }
        // Not past the least or the most the player gave the number.
        if state.within(key, field, before, after) == Some(false) {
            return false;
        }
        let sister = members
            .get(leader)
            .is_some_and(|(other, _)| *other == family);
        changes.push(Change {
            family: family.to_string(),
            field: field.to_string(),
            buff: up,
            ratio,
            members: vec![Moved {
                key: key.to_string(),
                before,
                after,
            }],
            games: 0,
            wins: 0,
            follows: true,
            manual: false,
            leader: if sister {
                leader.to_string()
            } else {
                String::new()
            },
        });
        return true;
    };
    let Some((before, current)) = changes[at]
        .members
        .iter()
        .find(|moved| moved.key == key)
        .map(|moved| (moved.before, moved.after))
    else {
        return false;
    };
    if (current - after).abs() < 1e-9 {
        return false;
    }
    // Back where the number was is no change at all.
    if (before - after).abs() < 1e-9 {
        changes.remove(at);
        return true;
    }
    if state.within(key, field, before, after) == Some(false) {
        return false;
    }
    changes[at].ratio = ratio;
    for moved in &mut changes[at].members {
        if moved.key == key {
            moved.after = after;
        }
    }
    true
}

// Keeps every upgrade at least what it is built from, in the stats an item
// gives: with `changes` in, no item has less of a stat than a part it is
// built from has. Every upgrade there is, a component and its legendary as
// a legendary and its radiant, and the parts no patch moves as well: a
// component's stat does not go under what the part under it gives.
//
// The upgrade comes up to its part. A buff to a component that takes it
// over one of its legendaries is a buff to that legendary too, and a nerf
// that would take an item under one of its parts stops at the part. Where
// no patch can move the upgrade's stat, the part comes down to it.
//
// Not where the upgrade starts out with less of the stat than the part, or
// with none of it: that is an item whose passive makes up for it, as
// Overlord's Bloodmail gives its attack damage by its health, and the two
// stay as they are.
fn level_upgrades(base: &Base, state: &State, changes: &mut Vec<Change>) {
    let members = tiers_of(base);
    let frozen = &super::tuning::tuning().frozen;
    // Whether a patch can move the stat of the item: not one the player's
    // settings hold still, for every item or for this one.
    let moves = |key: &str, field: &str| {
        !frozen.contains(field)
            && !state.protected(key, field)
            && members
                .get(key)
                .is_some_and(|(_, member)| member.values.contains_key(field))
    };
    for _ in 0..LEVEL_ROUNDS {
        let mut settled = true;
        for (part, upgrade) in &base.upgrades {
            for &field in fields::FLAT {
                let (part_held, upgrade_held) =
                    (stat_held(base, part, field), stat_held(base, upgrade, field));
                if part_held <= 0.0 || upgrade_held < part_held {
                    continue;
                }
                let low = stat_now(&members, base, state, &changes[..], part, field);
                let high = stat_now(&members, base, state, &changes[..], upgrade, field);
                if high >= low - 1e-9 {
                    continue;
                }
                let moved = if moves(upgrade, field) {
                    set_level(&members, state, changes, upgrade, field, low, true, part)
                } else if moves(part, field) {
                    set_level(&members, state, changes, part, field, high, false, upgrade)
                } else {
                    false
                };
                settled &= !moved;
            }
        }
        if settled {
            break;
        }
    }
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

    let tuning = super::tuning::tuning();
    // The finished items, each on its own record.
    let mut judged: Vec<Judged<'_>> = base
        .families
        .iter()
        .filter(|(_, family)| !family.component)
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
            let score = win / WIN_UNIT + PLAY_WEIGHT * play;
            Judged {
                id: id.as_str(),
                family,
                tally,
                score,
                size: score.abs(),
                rank: 1,
                tiebreak: Rng::seeded(number, id).next(),
            }
        })
        .collect();

    // A component has no record worth judging: it is held at the end of a
    // match by whoever did not finish what it builds into. It is patched
    // for the finished items it is built into, and only where they say one
    // thing: every one of them that has the games to be judged is over the
    // mark to be nerfed, or every one is under the mark to be buffed, and
    // there are two of them at least. Then the patch moves the component in
    // their place: what they share, once, and none of them. Where they do
    // not agree, the ones over the mark are patched themselves, like any
    // item, and the component is left alone.
    //
    // A component built into one finished item only shares nothing, and as
    // a rule that item is patched itself. Now and then the component is
    // patched in its place all the same ([`ALONE_ONE_IN`]), or its numbers
    // would never move: which patches, falls by the patch and the
    // component, the same whenever that patch is made.
    //
    // Its score is the furthest of theirs, which puts it ahead of them all
    // where the changes are picked, and its changes are sized by the middle
    // of them. Its record, for the notes, is theirs added up.
    let verdicts: HashMap<&str, (f64, Tally)> = judged
        .iter()
        .filter(|item| item.tally.games >= tuning.min_games)
        .map(|item| (item.id, (item.score, item.tally)))
        .collect();
    for (id, family) in base.families.iter().filter(|(_, family)| family.component) {
        let theirs: Vec<(f64, Tally)> = family
            .built_into
            .iter()
            .filter_map(|target| verdicts.get(target.as_str()).copied())
            .collect();
        let mut lots = Rng::seeded(number, id);
        let tiebreak = lots.next();
        let alone = family.built_into.len() == 1;
        let stands_in = if alone {
            theirs.len() == 1 && lots.next() % ALONE_ONE_IN == 0
        } else {
            theirs.len() >= 2
        };
        if !stands_in {
            continue;
        }
        let low = theirs.iter().map(|(score, _)| *score).fold(f64::INFINITY, f64::min);
        let high = theirs.iter().map(|(score, _)| *score).fold(f64::NEG_INFINITY, f64::max);
        let (score, rank) = if low >= THRESHOLD {
            (high, 0)
        } else if high <= -THRESHOLD {
            (low, 2)
        } else {
            continue;
        };
        let size = theirs.iter().map(|(score, _)| score.abs()).sum::<f64>() / theirs.len() as f64;
        let tally = theirs.iter().fold(Tally::default(), |sum, (_, tally)| Tally {
            games: sum.games + tally.games,
            wins: sum.wins + tally.wins,
        });
        judged.push(Judged {
            id: id.as_str(),
            family,
            tally,
            score,
            size,
            rank,
            tiebreak,
        });
    }
    judged.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.rank.cmp(&b.rank))
            .then_with(|| a.tiebreak.cmp(&b.tiebreak))
    });

    let floors = stat_floors(base, state);
    let mut changes = Vec::new();
    // The finished items a component's patch stands in for.
    let mut covered: Vec<&str> = Vec::new();
    let mut nerfs = 0;
    for item in &judged {
        if nerfs == tuning.max_nerfs || item.score < THRESHOLD {
            break;
        }
        if item.tally.games < tuning.min_games || covered.contains(&item.id) {
            continue;
        }
        nerfs += 1;
        let made = changes_for(state, item, tallies, &floors, false, number);
        // A component that had no step left to take stands in for nothing.
        if !made.is_empty() {
            covered.extend(item.family.built_into.iter().map(String::as_str));
        }
        changes.extend(made);
    }
    let mut buffs = 0;
    for item in judged.iter().rev() {
        if buffs == tuning.max_buffs || item.score > -THRESHOLD {
            break;
        }
        if item.tally.games < tuning.min_games || covered.contains(&item.id) {
            continue;
        }
        buffs += 1;
        let made = changes_for(state, item, tallies, &floors, true, number);
        if !made.is_empty() {
            covered.extend(item.family.built_into.iter().map(String::as_str));
        }
        changes.extend(made);
    }
    level_upgrades(base, state, &mut changes);
    Patch {
        number,
        version: version.to_string(),
        matches,
        changes,
        ..Patch::default()
    }
}

// What `field` of the tier `member` reads at the balance `state` has.
pub(crate) fn reads(state: &State, member: &Member, field: &str) -> Option<f64> {
    let unpatched = *member.values.get(field)?;
    Some(fields::quantize(
        &member.key,
        field,
        unpatched,
        unpatched * state.ratio(&member.key, field),
        member.whole.contains(field),
    ))
}

// The patch the player made by hand: every number waiting in
// [`State::staged`], set to what the player typed for it in the Item Patcher
// Settings window.
//
// Nothing is judged and nothing is added. A number is taken as it was
// typed, on a patch's steps or off them, inside the item's band or outside
// it, and no other number is moved to go with it: what a patch otherwise
// keeps in order (a radiant no weaker than its legendary, an upgrade no
// weaker than its parts) is left as the player has it. One that reads what
// was typed already, or that the mod no longer has, is no change.
pub(crate) fn manual(base: &Base, state: &State, number: u32, version: &str) -> Patch {
    let members = tiers_of(base);
    let mut changes = Vec::new();
    for (key, staged) in &state.staged {
        let Some(&(family, member)) = members.get(key.as_str()) else {
            continue;
        };
        for (field, &after) in staged {
            let (Some(rule), Some(&unpatched), Some(before)) = (
                fields::rule_for(key, field),
                member.values.get(field),
                reads(state, member, field),
            ) else {
                continue;
            };
            if unpatched == 0.0 || (after - before).abs() < 1e-9 {
                continue;
            }
            changes.push(Change {
                family: family.to_string(),
                field: field.clone(),
                buff: (after.abs() > before.abs()) != rule.lower_is_stronger,
                ratio: after / unpatched,
                members: vec![Moved {
                    key: key.clone(),
                    before,
                    after,
                }],
                games: 0,
                wins: 0,
                follows: false,
                manual: true,
                leader: String::new(),
            });
        }
    }
    // The version it is a hotfix of: no version's matches were judged.
    Patch {
        number,
        version: version.to_string(),
        changes,
        ..Patch::default()
    }
}
