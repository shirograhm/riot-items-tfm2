//! The Smart Builds rules: what the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build.
//!
//! Five of them:
//!
//! 1. **Unique items** — the same item twice is a wasted slot, because nothing
//!    in this game stacks across two copies.
//! 2. **One Grievous Wounds item** — a second heal cut does not deepen the
//!    first. Re-applying refreshes the same buff.
//! 3. **Crit chance at or under 100%** — crit caps at 100, so crit past it
//!    buys nothing. Crit from a stacking passive counts as if fully stacked.
//!    A slot that overflows the cap is replaced by an item that adds no crit
//!    at all, rather than one that merely fits: a stand-in worth having is one
//!    whose stats the champion can use.
//! 4. **Support items stay in the support role** — the Support class (bar
//!    Protoplasm Harness) is only kept by whoever plays support, whatever the
//!    champion.
//! 5. **Items the champion scales with** — an AP-only champion keeps no item
//!    whose only offence is physical (attack, attack speed or crit), and an
//!    AD-only champion no item whose only offence is magic power. Hybrid items,
//!    hybrid champions and items with no offensive stat are never touched, and
//!    neither are support items in the support role.
//!
//! The rules only ever replace what the AI picked. A slot the player pinned in
//! the editor is kept whatever it holds, and counts toward the budgets like any
//! other item, so the AI's picks around it make way for it rather than the
//! other way round.
//!
//! Rules 4 and 5 are about the champion, not the build, and come in as a
//! [`Fit`]; see [`crate::champion_traits`] for where its facts come from.
//!
//! An earlier slot always wins: the walk keeps the first heal-cut item and the
//! crit the build can still afford, and replaces what comes after. That matches
//! how the engine orders a build — slot 0 is the item it wanted most.
//!
//! # Why the rules live here and not in their callers
//!
//! Three places apply them, and each sees the catalog through a different API:
//! the stable hook has [`mod_api_stable::StableItemBuildContext`] and its flat
//! index arrays, the training-screen detour in [`crate::hook`] has the game's own
//! `Vec<Box<dyn ItemInfo>>`, and the buy detour in [`crate::tactics`] has raw
//! catalog indices it resolves by scanning. Only the *accessors* differ, so they
//! are what the callers bring; the rules are written once.
//!
//! Every fact the rules need is keyed by item key, never read off an item: the
//! detour may only call `key()` and `next_tier()` on a `dyn ItemInfo` (see the
//! warning above `hook::detour` — asking one for its category aborted the game),
//! and a key-driven table is the only shape that works everywhere.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use mod_api_stable::{ItemTagV1, StableItem};

use crate::build_config::Role;
use crate::champion_traits::{self, Scaling};

/// Flat crit chance a build may total before the rules start replacing crit
/// items. The engine caps crit chance at 100%, so a build summing to exactly 100
/// is the goal, not a violation.
const CRIT_CAP: i32 = 100;

/// What the rules need to know about one item.
#[derive(Clone, Copy, Default)]
struct ItemTraits {
    /// Crit chance from the item's own stats, plus what its passive grants at
    /// full stacks (Atma's Reckoning, Rite of Ruin, Yun Tal Wildarrows). A
    /// passive is counted as maxed because a build is meant to hold up once
    /// the stacks are there, which is when an overflow wastes crit.
    crit_chance: i32,
    /// Whether the item applies Grievous Wounds.
    cuts_healing: bool,
    /// Whether it gives attack, attack speed or crit: stats only a champion
    /// that deals physical damage uses.
    physical: bool,
    /// Whether it gives magic power.
    magic: bool,
}

impl ItemTraits {
    fn from_stats(crit_chance: i32, attack: i32, attack_speed: i32, magic_power: i32) -> Self {
        Self {
            crit_chance,
            cuts_healing: false,
            physical: attack > 0 || attack_speed > 0 || crit_chance > 0,
            magic: magic_power > 0,
        }
    }
}

/// Item traits by key, filled from the two places items are described.
#[derive(Clone, Default)]
struct Table {
    /// This mod's items, recorded as `init` registers them. Authoritative for
    /// its own keys, because the values there are the configured ones.
    mod_items: HashMap<String, ItemTraits>,
    /// The game's items, from the settings document. No vanilla item cuts
    /// healing; the stats are read rather than hardcoded because they are
    /// config-editable through `item_setting`.
    engine_items: HashMap<String, ItemTraits>,
}

impl Table {
    /// An item neither source describes has no traits, which no rule rejects.
    fn traits(&self, key: &str) -> ItemTraits {
        self.mod_items
            .get(key)
            .or_else(|| self.engine_items.get(key))
            .copied()
            .unwrap_or_default()
    }
}

static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

fn edit_table(edit: impl FnOnce(&mut Table)) {
    if let Ok(mut table) = TABLE.lock() {
        let table = table.get_or_insert_with(|| Arc::new(Table::default()));
        edit(Arc::make_mut(table));
    }
}

fn table() -> Arc<Table> {
    TABLE
        .lock()
        .ok()
        .and_then(|table| table.clone())
        .unwrap_or_default()
}

/// Records one of this mod's items as it is registered. Called from the
/// registration macros in `lib.rs`, where the item is still a concrete type —
/// `stat()` and `tags()` are safe to call there, unlike on a `dyn ItemInfo`.
pub(crate) fn note_mod_item<T: StableItem + ?Sized>(key: &str, item: &T) {
    let stat = item.stat();
    let traits = ItemTraits {
        cuts_healing: item.tags().contains(&ItemTagV1::HealReduce),
        ..ItemTraits::from_stats(
            stat.crit_chance,
            stat.attack,
            stat.attack_speed_mult,
            stat.magic_power,
        )
    };
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), traits);
    });
}

/// Adds the crit chance an item's passive grants at full stacks to what
/// [`note_mod_item`] recorded from its flat stats. Called right after it, from
/// the `passive_crit` arm of the registration macros in `lib.rs`.
pub(crate) fn note_passive_crit(key: &str, crit_chance: i32) {
    edit_table(|table| {
        let traits = table.mod_items.entry(key.to_string()).or_default();
        traits.crit_chance += crit_chance;
        traits.physical |= crit_chance > 0;
    });
}

/// Records one of the game's items, from the settings document
/// [`crate::item_stats`] already parses. An item with none of these stats is
/// recorded too: it says the item is described, which is cheaper to keep than
/// to special-case.
pub(crate) fn note_engine_item(
    key: &str,
    crit_chance: i32,
    attack: i32,
    attack_speed: i32,
    magic_power: i32,
) {
    let traits = ItemTraits::from_stats(crit_chance, attack, attack_speed, magic_power);
    edit_table(|table| {
        table.engine_items.insert(key.to_string(), traits);
    });
}

/// Why an item cannot join a build. Also what its stand-in has to fix: a crit
/// overflow is the one case that demands a stand-in adding no crit, rather than
/// any item the build can still afford.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Reason {
    Duplicate,
    Grievous,
    Crit,
    SupportOnly,
    Scaling,
}

impl Reason {
    /// Whether the offending item's own category is the wrong place to look
    /// for its stand-in. A support item's category holds nothing but support
    /// items, and an item the champion does not scale with sits among others
    /// it does not scale with, so for these two the stand-in is taken from the
    /// categories the rest of the build uses instead.
    pub(crate) fn restyles(self) -> bool {
        matches!(self, Reason::SupportOnly | Reason::Scaling)
    }
}

/// The one Support-class item any champion may build.
const SUPPORT_ITEM_EXCEPTION: &str = "protoplasm_harness";

/// What the champion a build is for may hold, whatever the build already has.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    /// Rule 4: whether support items are allowed.
    support_items: bool,
    /// Rule 5: what the champion scales with, `None` when unknown.
    scaling: Option<Scaling>,
}

/// The [`Fit`] for `champion` playing `role`.
///
/// Support items are allowed in the support role and nowhere else — not even an
/// unknown role ([`Role::Any`]), which the buy detour falls back to when no
/// lineup has placed the champion yet. A champion nothing is known about gets
/// no scaling restriction: a missing tag must never cost a build an item.
pub(crate) fn fit(champion: &str, role: Role) -> Fit {
    let traits = champion_traits::traits(champion);
    Fit {
        support_items: role == Role::Support,
        scaling: traits.and_then(|traits| traits.scaling),
    }
}

impl Fit {
    /// Whether an item with these traits gives only stats the champion cannot
    /// use.
    fn mismatches(&self, item: &ItemTraits) -> bool {
        match self.scaling {
            Some(Scaling::Ap) => item.physical && !item.magic,
            Some(Scaling::Ad) => item.magic && !item.physical,
            Some(Scaling::Hybrid) | None => false,
        }
    }

    /// Rule 5 for one item. A support item in the support role is exempt: its
    /// worth is what it does for allies — auras, heals, shields — not the
    /// holder's own damage, so an AD support (Exorcist, Plague Doctor) still
    /// takes Echoes of Helia or Zeke's Herald.
    fn mismatches_item(&self, key: &str, item: &ItemTraits) -> bool {
        !(self.support_items && is_support_item(key)) && self.mismatches(item)
    }
}

/// Whether `key` is an item only the support role may build: the editor's Support
/// class, base or radiant, less [`SUPPORT_ITEM_EXCEPTION`].
fn is_support_item(key: &str) -> bool {
    let slug = crate::build_config::base_slug(key);
    slug != SUPPORT_ITEM_EXCEPTION && crate::item_catalog::category_of(slug) == Some("Support")
}

/// What the items a build already holds have spent of the two budgets the rules
/// police, and the [`Fit`] of the champion it is for. Carries the trait table
/// with it, so a scan across the catalog is a run of hash lookups rather than a
/// run of lock acquisitions.
pub(crate) struct Budget {
    table: Arc<Table>,
    cuts_healing: bool,
    crit_chance: i32,
    fit: Fit,
}

impl Budget {
    /// An empty budget: a build with nothing in it yet.
    pub(crate) fn empty(fit: Fit) -> Self {
        Self {
            table: table(),
            cuts_healing: false,
            crit_chance: 0,
            fit,
        }
    }

    /// The budget the given items have already spent.
    pub(crate) fn spent<'a>(keys: impl IntoIterator<Item = &'a str>, fit: Fit) -> Self {
        let mut budget = Self::empty(fit);
        for key in keys {
            budget.take(key);
        }
        budget
    }

    /// Why `key` cannot join the build, or `None` when it can.
    ///
    /// Duplicates are the caller's to spot: this knows what a build has spent,
    /// not which slots spent it.
    pub(crate) fn rejects(&self, key: &str) -> Option<Reason> {
        let traits = self.table.traits(key);
        if !self.fit.support_items && is_support_item(key) {
            Some(Reason::SupportOnly)
        } else if self.fit.mismatches_item(key, &traits) {
            Some(Reason::Scaling)
        } else if self.cuts_healing && traits.cuts_healing {
            Some(Reason::Grievous)
        } else if self.crit_chance + traits.crit_chance > CRIT_CAP {
            Some(Reason::Crit)
        } else {
            None
        }
    }

    /// Whether `key` is an item this champion may hold at all — rules 4 and 5,
    /// which do not depend on what else is in the build. An item that fails is
    /// no guide to the build's style.
    pub(crate) fn suits_champion(&self, key: &str) -> bool {
        !(!self.fit.support_items && is_support_item(key))
            && !self.fit.mismatches_item(key, &self.table.traits(key))
    }

    /// Whether `key` brings any crit chance — what the stand-in for a crit
    /// overflow must not do.
    pub(crate) fn adds_crit(&self, key: &str) -> bool {
        self.table.traits(key).crit_chance > 0
    }

    /// Whether `key` may stand in for an item rejected for `reason`.
    pub(crate) fn accepts_instead(&self, key: &str, reason: Reason) -> bool {
        self.rejects(key).is_none() && (reason != Reason::Crit || !self.adds_crit(key))
    }

    /// Records `key` as part of the build.
    pub(crate) fn take(&mut self, key: &str) {
        let traits = self.table.traits(key);
        self.cuts_healing |= traits.cuts_healing;
        self.crit_chance += traits.crit_chance;
    }
}

/// Rewrites `build` in place so that it breaks none of the five rules, as far as
/// the catalog allows.
///
/// A slot that breaks one is swapped for the next item that fixes it: unused, of
/// the same category, selectable as a final, and no violation itself. The search
/// wraps the catalog from the offending index, which is the walk the unique-items
/// rule has always used. A slot with no such stand-in — an unknown category, or a
/// category with nothing left in it — is left alone and counted, because it is in
/// the build either way.
///
/// For the two rules about the champion ([`Reason::restyles`]) "the same
/// category" is the build's, not the item's: the categories of the build's other
/// items that suit the champion, earliest slot first, and only then any category
/// at all. That keeps a stand-in in the style of the build — an attack-damage
/// build that loses a support item gets another attack-damage item.
///
/// `count` is the size of the catalog the indices in `build` refer to; `category`
/// and `is_final` are the caller's view of it, and `key` is what ties an index to
/// the trait table. `fit` is [`fit`] for the champion the build is for.
///
/// `pinned` says, per slot, whether the player pinned it (a missing entry is
/// an AI slot). A pinned slot is never rewritten. Pins — and `reserved`, the
/// player's pins for slots past the end of `build` — are counted before any AI
/// slot is looked at, so an AI pick that clashes with a pin is the one that
/// goes, wherever the two sit in the build.
pub(crate) fn enforce<C, K, G, F>(
    count: usize,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    fit: Fit,
    key: K,
    category: G,
    is_final: F,
) where
    C: PartialEq,
    K: Fn(usize) -> Option<String>,
    G: Fn(usize) -> Option<C>,
    F: Fn(usize) -> bool,
{
    if count == 0 {
        return;
    }
    let mut budget = Budget::empty(fit);
    let mut seen: HashSet<usize> = HashSet::new();
    let is_pinned = |slot: usize| pinned.get(slot).copied().unwrap_or(false);

    let pins = build
        .iter()
        .enumerate()
        .filter(|&(slot, _)| is_pinned(slot))
        .map(|(_, &index)| index)
        .chain(reserved.iter().copied());
    for index in pins {
        seen.insert(index);
        if let Some(key) = key(index) {
            budget.take(&key);
        }
    }

    // The build's style: the categories of the items that suit the champion,
    // earliest slot first, each once. Taken before any slot changes, from the
    // whole build, so a support item in slot 0 still follows the IE in slot 1.
    let mut styles: Vec<C> = Vec::new();
    for &index in build.iter() {
        if !key(index).is_some_and(|key| budget.suits_champion(&key)) {
            continue;
        }
        if let Some(style) = category(index) {
            if !styles.contains(&style) {
                styles.push(style);
            }
        }
    }

    for (position, slot) in build.iter_mut().enumerate() {
        if is_pinned(position) {
            continue;
        }
        let current = key(*slot);
        let reason = if seen.contains(slot) {
            Some(Reason::Duplicate)
        } else {
            current.as_deref().and_then(|key| budget.rejects(key))
        };

        // The item this slot ends up holding. One assignment for all four
        // outcomes — kept, replaced, unclassifiable, or nothing left to swap in —
        // so the budget cannot drift from the build it is meant to describe.
        let chosen = match reason {
            None => *slot,
            Some(reason) => {
                let offender = *slot;
                let search = |matches: &dyn Fn(usize) -> bool| {
                    (1..count)
                        .map(|step| (offender + step) % count)
                        .find(|&candidate| {
                            !seen.contains(&candidate)
                                && matches(candidate)
                                && is_final(candidate)
                                && key(candidate).is_some_and(|candidate| {
                                    budget.accepts_instead(&candidate, reason)
                                })
                        })
                };
                if reason.restyles() {
                    styles
                        .iter()
                        .find_map(|style| {
                            search(&|candidate| category(candidate).as_ref() == Some(style))
                        })
                        // A build with no usable style at all — every other
                        // item broke these rules too — still loses the item.
                        .or_else(|| search(&|_| true))
                        .unwrap_or(offender)
                } else {
                    // The category must be known: matching `None` against
                    // `None` would swap the slot for any item the caller could
                    // not classify.
                    match category(offender) {
                        None => offender,
                        Some(wanted) => search(&|candidate| {
                            category(candidate).as_ref() == Some(&wanted)
                        })
                        .unwrap_or(offender),
                    }
                }
            }
        };

        *slot = chosen;
        seen.insert(chosen);
        if let Some(key) = key(chosen) {
            budget.take(&key);
        }
    }
}
