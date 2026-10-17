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

// Whether a tier's number may be where `moved` puts it: inside its band, or one
// step out for a number whose first step already leaves it (10 Ability Haste in
// fives). A number with the player's own least or most is held to those
// ([`State::within`]).
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

// `field` of `family` made stronger or weaker by the smallest move of its
// number, searched from `from` to `to` (shares of the number, a hundredth at a
// time); `None` where no step is near enough or the number is at its bounds.
// Measured on the lead tier ([`lead`]), preferring the ratio that moves the
// most tiers.
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

// The move of `field` that best makes up for a step of `got` where `rate` was
// wanted, going `stronger`: the one leaving the pair nearest `rate`, within
// [`NET_SPAN`], and never a big one ([`Step::big`]).
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

// The item's total: each number's distance from unpatched, as a share counted
// as strength, summed, with `plan`'s fields as planned. Of the one tier being
// planned.
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

// The steps a patch takes on one tier: one or two numbers picked at random, a
// range's other end with them, and a counter-move where a smallest step
// overshoots twice what was wanted ([`counter`]; only then may a step be big).
// Numbers with no counter are passed over, and if nothing else fits, any step
// that is not big is taken.
//
// `tier` holds that one tier and `id` is its key, `seed` orders the tries, and
// `allowed` vets every step against the plan so far.
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

// The tiers of an item a patch moves, by place. A tier moves where its own
// record (with the same imaginary games) sits on the item's side of even by
// half the touch threshold; failing that, the most-held tier moves.
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

// Whether `steps` on the tier at `at` would reverse two tiers' order in a
// number they do not share unpatched (a radiant left weaker than its
// legendary). Other tiers are read as this patch left them (`landed`) or as the
// state has them.
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
            let landed_at = landed
                .iter()
                .rev()
                .find(|(tier, moved, _)| *tier == other_at && moved == field)
                .map(|(_, _, value)| *value);
            let Some(theirs) = landed_at.or_else(|| reads(state, other, field)) else {
                return false;
            };
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

// Keeps a radiant no weaker than its legendary in the numbers `changes` moved:
// where the legendary ends stronger, the radiant comes level; where the radiant
// alone was made weaker, the legendary comes down with it. The levelled tier
// gets a change following the other's (`Change::leader`), listed in that tier's
// notes.
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
        let reading = |member: &Member, changes: &[Change], index: Option<usize>| -> Option<f64> {
            if let Some(index) = index {
                return changes[index]
                    .members
                    .iter()
                    .find(|moved| moved.key == member.key)
                    .map(|moved| moved.after);
            }
            reads(state, member, field)
        };
        let (low_at, high_at) = (moved_at(legendary, &changes[..]), moved_at(radiant, &changes[..]));
        if low_at.is_none() && high_at.is_none() {
            continue;
        }
        let (Some(low), Some(high)) = (
            reading(legendary, &changes[..], low_at),
            reading(radiant, &changes[..], high_at),
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

// The changes to one item: each tier a patch moves ([`tiers_to_move`]) planned
// by itself ([`plan_for`]), so tiers may move differently or not at all. They
// may not swap order ([`crosses`]), drop a stat under a part's (`floors`,
// [`stat_floors`]), or leave the radiant weaker in a shared number
// ([`level_tiers`]).
//
// The rows, counted as the notes write them, stay within `max_changes_per_item`
// across the tiers ([`within_rows`]); `level_upgrades` afterwards is not held
// to it.
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

// Whether an item's notes stay within `cap` rows with `steps` on top of `plan`
// and `rows` (what earlier tiers took, named by [`super::text::row_of`]); a
// number whose row is taken already adds none.
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

// The least each tier's stats may be lowered to: what a part it is built from
// gives, as the state has it, since [`level_upgrades`] would put it back. A
// radiant takes its legendary's floors rather than counting it as a part
// ([`level_tiers`]).
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
    let patched = members
        .get(key)
        .and_then(|(_, member)| reads(state, member, field));
    patched.unwrap_or_else(|| stat_held(base, key, field))
}

// Sets `field` of tier `key` level with `level`, at its nearest own step no
// less (`up`) or no more. A change already made ends at the level; otherwise a
// new one follows `leader` when that is another tier of the same item. Whether
// anything changed.
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
        let Some(before) = reads(state, member, field) else {
            return false;
        };
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

// Passive numbers a part keeps at or under the same number of what it builds
// into, as (part, upgrade, field): where the part ends higher, it comes down
// to the upgrade's, or where it cannot move, the upgrade comes up. Madred's
// Razors' Maim heal tracks Feral Flare's (the user's, 2026-10-17).
const TRACKED_PASSIVES: [(&str, &str, &str); 1] =
    [("madreds_razors", "feral_flare", "effect_bonus_flat_heal")];

// Keeps every upgrade at least as strong as its parts in each stat both give,
// components and unmoved parts included: the upgrade comes up to the part, or
// where it cannot move, the part comes down. Not where the upgrade starts with
// less or none of the stat, as when a passive makes up for it (Overlord's
// Bloodmail). And the [`TRACKED_PASSIVES`] the other way round.
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
        for (part, upgrade, field) in TRACKED_PASSIVES {
            let low = stat_now(&members, base, state, &changes[..], part, field);
            let high = stat_now(&members, base, state, &changes[..], upgrade, field);
            if low <= high + 1e-9 {
                continue;
            }
            let moved = if moves(part, field) {
                set_level(&members, state, changes, part, field, high, false, upgrade)
            } else if moves(upgrade, field) {
                set_level(&members, state, changes, upgrade, field, low, true, part)
            } else {
                false
            };
            settled &= !moved;
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

    // A component is patched for the finished items it builds into, only when
    // at least two of them with enough games all point the same way; it then
    // moves in their place, and otherwise they are patched themselves. A
    // component of a single item is patched in its place one time in
    // [`ALONE_ONE_IN`], decided per patch and component.
    //
    // Its score is the furthest of theirs (so it is picked first), sized by
    // their middle, and its record for the notes is theirs added up.
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

// What `field` of the tier `member` reads at the balance `state` has: its
// unpatched number at the field's ratio, in the steps a patch moves it by.
// Nothing for a tier that has no such number.
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

// The patch the player made by hand: every number in [`State::staged`], set
// exactly as typed. Nothing is judged, banded or levelled; a number already at
// its value, or no longer the mod's, is no change.
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
