//! The Smart Builds rules: what the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build.
//!
//! Three of them:
//!
//! 1. **Unique items** — the same item twice is a wasted slot, because nothing
//!    in this game stacks across two copies.
//! 2. **One Grievous Wounds item** — a second heal cut does not deepen the
//!    first. Re-applying refreshes the same buff.
//! 3. **Crit chance at or under 100%** — crit caps at 100, so flat crit past it
//!    buys nothing. A slot that overflows the cap is replaced by an item that
//!    adds no crit at all, rather than one that merely fits: a stand-in worth
//!    having is one whose stats the champion can use.
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

/// Flat crit chance a build may total before the rules start replacing crit
/// items. The engine caps crit chance at 100%, so a build summing to exactly 100
/// is the goal, not a violation.
const CRIT_CAP: i32 = 100;

/// What the rules need to know about one item.
#[derive(Clone, Copy, Default)]
struct ItemTraits {
    /// Flat crit chance from the item's own stats. Conditional crit — Atma's
    /// Reckoning scaling off health, Rite of Ruin stacking in combat — is not
    /// here and cannot be: it does not exist at draft time.
    crit_chance: i32,
    /// Whether the item applies Grievous Wounds.
    cuts_healing: bool,
}

/// Item traits by key, filled from the two places items are described.
#[derive(Default)]
struct Table {
    /// This mod's items, recorded as `init` registers them. Authoritative for
    /// its own keys, because the values there are the configured ones.
    mod_items: HashMap<String, ItemTraits>,
    /// The game's items, from the settings document. Crit only — no vanilla item
    /// cuts healing — and read rather than hardcoded because the three that have
    /// crit (Zeal, Phantom Dancer, Radiant Phantom Dancer) are config-editable
    /// through `item_setting`.
    engine_crit: HashMap<String, i32>,
}

impl Table {
    fn traits(&self, key: &str) -> ItemTraits {
        if let Some(traits) = self.mod_items.get(key) {
            return *traits;
        }
        ItemTraits {
            crit_chance: self.engine_crit.get(key).copied().unwrap_or(0),
            cuts_healing: false,
        }
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
    let traits = ItemTraits {
        crit_chance: item.stat().crit_chance,
        cuts_healing: item.tags().contains(&ItemTagV1::HealReduce),
    };
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), traits);
    });
}

/// Records one of the game's items, from the settings document
/// [`crate::item_stats`] already parses. Zero is recorded too: it says the item
/// is described, which is cheaper to keep than to special-case.
pub(crate) fn note_engine_crit(key: &str, crit_chance: i32) {
    edit_table(|table| {
        table.engine_crit.insert(key.to_string(), crit_chance);
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
}

/// What the items a build already holds have spent of the two budgets the rules
/// police. Carries the trait table with it, so a scan across the catalog is a run
/// of hash lookups rather than a run of lock acquisitions.
pub(crate) struct Budget {
    table: Arc<Table>,
    cuts_healing: bool,
    crit_chance: i32,
}

impl Budget {
    /// An empty budget: a build with nothing in it yet.
    pub(crate) fn empty() -> Self {
        Self {
            table: table(),
            cuts_healing: false,
            crit_chance: 0,
        }
    }

    /// The budget the given items have already spent.
    pub(crate) fn spent<'a>(keys: impl IntoIterator<Item = &'a str>) -> Self {
        let mut budget = Self::empty();
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
        if self.cuts_healing && traits.cuts_healing {
            Some(Reason::Grievous)
        } else if self.crit_chance + traits.crit_chance > CRIT_CAP {
            Some(Reason::Crit)
        } else {
            None
        }
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

/// Rewrites `build` in place so that it breaks none of the three rules, as far as
/// the catalog allows.
///
/// A slot that breaks one is swapped for the next item that fixes it: unused, of
/// the same category, selectable as a final, and no violation itself. The search
/// wraps the catalog from the offending index, which is the walk the unique-items
/// rule has always used. A slot with no such stand-in — an unknown category, or a
/// category with nothing left in it — is left alone and counted, because it is in
/// the build either way.
///
/// `count` is the size of the catalog the indices in `build` refer to; `category`
/// and `is_final` are the caller's view of it, and `key` is what ties an index to
/// the trait table.
pub(crate) fn enforce<C, K, G, F>(
    count: usize,
    build: &mut [usize],
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
    let mut budget = Budget::empty();
    let mut seen: HashSet<usize> = HashSet::new();

    for slot in build.iter_mut() {
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
            // Must be known: matching `None` against `None` would swap the slot
            // for any item the caller could not classify.
            Some(reason) => match category(*slot) {
                None => *slot,
                Some(wanted) => {
                    let offender = *slot;
                    (1..count)
                        .map(|step| (offender + step) % count)
                        .find(|candidate| {
                            !seen.contains(candidate)
                                && category(*candidate) == Some(wanted)
                                && is_final(*candidate)
                                && key(*candidate).is_some_and(|candidate| {
                                    budget.accepts_instead(&candidate, reason)
                                })
                        })
                        .unwrap_or(offender)
                }
            },
        };

        *slot = chosen;
        seen.insert(chosen);
        if let Some(key) = key(chosen) {
            budget.take(&key);
        }
    }
}
