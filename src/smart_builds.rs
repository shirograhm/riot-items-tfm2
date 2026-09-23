//! The Smart Builds rules: what the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build.
//!
//! Seven of them:
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
//! 6. **Items bought when they pay off** — an item whose value accumulates
//!    the longer it is owned ([`EARLY_ITEMS`]: Heartsteel's permanent health,
//!    Hubris's takedown stacks) is bought before the AI's other picks, and one
//!    that scales with stats the rest of the build brings ([`LATE_ITEMS`]:
//!    Riftmaker's health-to-AP, Rabadon's multiplier) after them. This rule
//!    only reorders; the other five decide what is in the build.
//! 7. **One pair of boots** — a build with no boots gets the pair
//!    [`boots_for`] picks for its champion, as its second AI pick. The engine
//!    never plans toward a tier-3 item, so without this no AI build holds any.
//!    Boots the player pinned anywhere, the 5th and 6th slots included, count,
//!    and no AI pick is ever swapped *for* boots by the other rules.
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
//! how the engine orders a build — slot 0 is the item it wanted most. Rule 6
//! runs after that walk, so it is the engine's preference that decides which
//! items stay, and only then the timing that decides when each is bought.
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

/// Rule 6: items worth more the longer they are owned, because their passive
/// builds permanent stacks over the match. By base slug, so the radiant tier
/// follows.
const EARLY_ITEMS: [&str; 6] = [
    "heartsteel",             // permanent bonus health every 20 seconds
    "yun_tal_wildarrows",     // permanent crit chance per basic attack
    "hubris",                 // permanent stack per takedown
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
    "collector",              // bonus gold per kill, worth more the earlier it comes
];

/// Rule 6: items whose passive scales with a stat the rest of the build
/// provides, so they are worth most once the others are in.
const LATE_ITEMS: [&str; 9] = [
    "riftmaker",             // ability power from maximum health
    "overlords_bloodmail",   // attack damage from maximum health
    "atmas_reckoning",       // crit chance from maximum health
    "protectors_vow",        // maximum health from armor
    "cloak_of_starry_night", // multiplies magic resistance
    "rabadons_deathcap",     // multiplies ability power
    "deathblade",            // multiplies attack damage
    "infinity_edge",         // crit damage, worth nothing without crit chance
    "lord_dominiks_regards", // grows with the target's health, which grows late
];

/// When an item pays off, in the order rule 6 buys them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Timing {
    Early,
    Any,
    Late,
}

fn timing(key: &str) -> Timing {
    let slug = crate::build_config::base_slug(key);
    if EARLY_ITEMS.contains(&slug) {
        Timing::Early
    } else if LATE_ITEMS.contains(&slug) {
        Timing::Late
    } else {
        Timing::Any
    }
}

/// The tier-1 boots every upgraded pair builds from.
const BASE_BOOTS: &str = "boots";
const BERSERKERS_GREAVES: &str = "berserkers_greaves";
const BOOTS_OF_SWIFTNESS: &str = "boots_of_swiftness";
const GLUTTONOUS_GREAVES: &str = "gluttonous_greaves";
const IONIAN_BOOTS: &str = "ionian_boots_of_lucidity";
const MERCURYS_TREADS: &str = "mercurys_treads";
const PLATED_STEELCAPS: &str = "plated_steelcaps";
const SORCERERS_SHOES: &str = "sorcerers_shoes";

/// Every upgraded pair.
const UPGRADED_BOOTS: [&str; 7] = [
    BERSERKERS_GREAVES,
    BOOTS_OF_SWIFTNESS,
    GLUTTONOUS_GREAVES,
    IONIAN_BOOTS,
    MERCURYS_TREADS,
    PLATED_STEELCAPS,
    SORCERERS_SHOES,
];

/// The AI slot rule 7 puts boots in: the second, which is where League players
/// finish theirs. Counted among the AI's slots, so a pin before it pushes the
/// boots later rather than displacing the pin.
const BOOTS_SLOT: usize = 1;

/// Whether `key` is a pair of boots, tier 1 or upgraded.
pub(crate) fn is_boots(key: &str) -> bool {
    key == BASE_BOOTS || UPGRADED_BOOTS.contains(&key)
}

/// Base movement speed at and above which a champion never rolls for Boots of
/// Swiftness. The fastest base-game champion, Cavalry Knight, sits exactly here.
const SWIFTNESS_SPEED_CEILING: u32 = 1200;

/// Percent chance of Boots of Swiftness per 100 base movement speed below
/// [`SWIFTNESS_SPEED_CEILING`]: 10% at 1100, 20% at 1000, 30% at 900.
const SWIFTNESS_CHANCE_PER_100_SPEED: u32 = 10;

/// The percent chance a champion with this base speed takes Swiftness. No
/// chance when the speed is unknown.
fn swiftness_chance(move_speed: Option<u32>) -> u32 {
    move_speed.map_or(0, |speed| {
        (SWIFTNESS_SPEED_CEILING.saturating_sub(speed) * SWIFTNESS_CHANCE_PER_100_SPEED / 100)
            .min(100)
    })
}

/// A roll in `0..100` for `champion` in this lineup.
///
/// Not random, on purpose: a match is simulated in the background and again
/// when the player watches it, and both must build the same boots. So the
/// roll is a hash of the lineup — the same match always rolls the same, a new
/// draft rolls again. Each side is sorted first, so the order a caller lists
/// the champions in cannot change the answer.
fn lineup_roll(champion: &str, allies: &[&str], enemies: &[&str]) -> u32 {
    let mut allies = allies.to_vec();
    let mut enemies = enemies.to_vec();
    allies.sort_unstable();
    enemies.sort_unstable();
    // FNV-1a, 64-bit.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let parts = std::iter::once(champion).chain(allies).chain(["|"]).chain(enemies);
    for part in parts {
        for byte in part.bytes().chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    (hash % 100) as u32
}

/// The boots rule 7 gives `champion` playing `role`, beside `allies` and
/// against `enemies` (either empty when the caller cannot see them).
///
/// First, slower champions may take Boots of Swiftness: the chance is
/// [`swiftness_chance`] of the champion's base speed, and [`lineup_roll`]
/// decides it. Otherwise, in order: a tank answers the enemy's main damage type (Mercury's against
/// more magic than physical, Steelcaps otherwise, which also covers not
/// knowing), whatever its role; any other support takes haste (Lucidity),
/// since it wins through its abilities' uptime rather than their damage; then
/// the champion's own damage: ability power takes magic
/// penetration, and a physical champion follows its class — attack speed for
/// the ranged, haste for assassins, omnivamp for fighters. A utility champion
/// that heals or shields takes haste (Lucidity), and one that does neither, or
/// one nothing is known about, the plain speed of Swiftness.
pub(crate) fn boots_for(
    champion: &str,
    role: Role,
    allies: &[&str],
    enemies: &[&str],
) -> &'static str {
    use champion_traits::Class;
    let traits = champion_traits::traits(champion).unwrap_or_default();
    if lineup_roll(champion, allies, enemies) < swiftness_chance(traits.move_speed) {
        return BOOTS_OF_SWIFTNESS;
    }
    if traits.tank {
        let (physical, magic) = enemies.iter().fold((0, 0), |(physical, magic), enemy| {
            match champion_traits::traits(enemy).and_then(|traits| traits.scaling) {
                Some(Scaling::Ad) => (physical + 1, magic),
                Some(Scaling::Ap) => (physical, magic + 1),
                Some(Scaling::Hybrid) => (physical + 1, magic + 1),
                None => (physical, magic),
            }
        });
        return if magic > physical { MERCURYS_TREADS } else { PLATED_STEELCAPS };
    }
    if role == Role::Support {
        return IONIAN_BOOTS;
    }
    if traits.scaling == Some(Scaling::Ap) {
        return SORCERERS_SHOES;
    }
    match traits.class {
        Some(Class::Range) => BERSERKERS_GREAVES,
        Some(Class::Assassin) => IONIAN_BOOTS,
        Some(Class::Melee) => GLUTTONOUS_GREAVES,
        Some(Class::Magician) => SORCERERS_SHOES,
        Some(Class::Util) if traits.sustains => IONIAN_BOOTS,
        Some(Class::Util) | None => BOOTS_OF_SWIFTNESS,
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

/// Rewrites `build` in place so that it breaks none of the seven rules, as far as
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
///
/// Last, rule 6 reorders the AI's slots among themselves: early items first,
/// late items last, and the engine's order within each group. A pinned slot
/// keeps its position and its item, so the player's buy order is never moved.
/// Every caller decides the build before the match, when nothing is bought
/// yet, so no owned item can end up in a later slot.
///
/// Then rule 7: `boots` is the catalog index of the pair [`boots_for`] picked
/// (`None` when the catalog has none). A build with no boots in any slot or in
/// `reserved` gets them as its second AI pick (the only one, if it has one); the
/// AI picks from there on move one AI slot later and the last one drops off. Boots are never a
/// stand-in for the other rules, whatever `is_final` says about them.
pub(crate) fn enforce<C, K, G, F>(
    count: usize,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    fit: Fit,
    boots: Option<usize>,
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
                                    !is_boots(&candidate)
                                        && budget.accepts_instead(&candidate, reason)
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

    // Rule 6. A stable sort, so items of the same timing keep the engine's
    // order.
    let open: Vec<usize> = (0..build.len()).filter(|&slot| !is_pinned(slot)).collect();
    let mut picks: Vec<usize> = open.iter().map(|&slot| build[slot]).collect();
    picks.sort_by_key(|&index| key(index).map_or(Timing::Any, |key| timing(&key)));

    // Rule 7.
    let has_boots = build
        .iter()
        .chain(reserved)
        .any(|&index| key(index).is_some_and(|key| is_boots(&key)));
    if let Some(boots) = boots.filter(|_| !has_boots && !open.is_empty()) {
        // `open` is not empty, so neither is `picks`: a lone AI slot takes them.
        picks.insert(BOOTS_SLOT.min(picks.len() - 1), boots);
        picks.truncate(open.len());
    }

    for (&slot, index) in open.iter().zip(picks) {
        build[slot] = index;
    }
}
