//! The Smart Builds rules: what the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build.
//!
//! Eleven of them:
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
//!    Protoplasm Harness and Zeke's Convergence) is only kept by whoever plays
//!    support, whatever the champion.
//! 5. **Items the champion scales with** — an AP-only champion keeps no item
//!    whose only offence is physical (attack, attack speed or crit), and an
//!    AD-only champion no item whose only offence is magic power. Hybrid items,
//!    hybrid champions and items with no offensive stat are never touched.
//!    Support items get no pass in the support role: an AD support keeps the
//!    tank ones, not the AP ones ([`AP_ITEMS`] puts Bloodsong and Sword of
//!    Blossoming Dawn with those despite their attack speed).
//! 6. **Items bought when they pay off** — an item whose value accumulates
//!    the longer it is owned ([`EARLY_ITEMS`]: Heartsteel's permanent health,
//!    Hubris's takedown stacks) is bought before the AI's other picks, and one
//!    that scales with stats the rest of the build brings ([`LATE_ITEMS`]:
//!    Riftmaker's health-to-AP, Rabadon's multiplier) after them. This rule
//!    only reorders; the other five decide what is in the build.
//! 7. **One pair of boots** — a build with no boots gets the pair
//!    [`boots_for`] picks for its champion, in the second slot, or the first
//!    open slot after it when a pin holds that one (the 5th and 6th included),
//!    else the first slot (every slot pinned, no boots). The engine
//!    never plans toward a tier-3 item, so without this no AI build holds any.
//!    Boots the player pinned anywhere, the 5th and 6th slots included, count,
//!    and no AI pick is ever swapped *for* boots by the other rules.
//! 8. **Jungle items stay in the jungle** — Feral Flare and Grez's Spectral
//!    Lantern ([`JUNGLE_ITEMS`]) grow on monster kills, which only the jungle
//!    role gets, so only whoever plays jungle keeps them. Numbered after 7
//!    only so the rules above keep the numbers the rest of the mod cites them by.
//! 9. **Role items first** — a support's build holds a Support-class item
//!    (the two exceptions to rule 4 included) and a jungler's a jungle item, bought
//!    before the AI's other picks. Only one is guaranteed: the rest of the
//!    build is still whatever the AI chose. When neither a pin nor an AI pick
//!    is one, the AI's last pick makes way for the first role item every
//!    other rule accepts. Rule 5 is what matches it to the champion: an AD
//!    support gets a tank support item, never an AP one, and a jungler Feral
//!    Flare or Grez's by damage type.
//! 10. **Imperial Mandate for supports with crowd control** — a support whose
//!    champion is tagged `CC` makes Mandate its support item, since its passive
//!    pays off on the slows and stuns that champion lands: the AI's first
//!    support item gives way to it. A support item the player pinned is their
//!    choice and stays, a build that already holds Mandate is left alone, and
//!    the other rules still judge it (Mandate is an AP item, so rule 5 keeps it
//!    off an AD support). The other way round, a support known to have no
//!    crowd control never keeps an AI's Mandate: it makes way for another
//!    support item, and rule 9 never hands one out. A champion nothing is known
//!    about is left alone either way.
//! 11. **Items for the champion's reach** — a ranged champion keeps no item
//!    whose passive wants its carrier in melee ([`MELEE_ITEMS`]: the Hydras'
//!    Cleave and Hullbreaker's Skipper weaken from past 35 range, and
//!    Heartsteel and the Immolate auras need the enemy closer than a ranged
//!    champion stands), and a melee champion none that wants its carrier at
//!    range ([`RANGED_ITEMS`]: Runaan's Hurricane, Diamond Tipped Spear).
//!    Melee is a basic attack that reaches 35 or less, the same line those
//!    items draw. The stand-in comes from the item's own category, like a
//!    duplicate's. A champion whose reach nothing states is left alone.
//!
//! The rules only ever replace what the AI picked. A slot the player pinned in
//! the editor is kept whatever it holds, and counts toward the budgets like any
//! other item, so the AI's picks around it make way for it rather than the
//! other way round.
//!
//! Rules 4, 5, 8, 9, 10 and 11 are about the champion, not the build, and come in
//! as a [`Fit`]; see [`crate::champion_traits`] for where its facts come from.
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
    let mut traits = ItemTraits {
        cuts_healing: item.tags().contains(&ItemTagV1::HealReduce),
        ..ItemTraits::from_stats(
            stat.crit_chance,
            stat.attack,
            stat.attack_speed_mult,
            stat.magic_power,
        )
    };
    if AP_ITEMS.contains(&crate::build_config::base_slug(key)) {
        traits.physical = false;
    }
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), traits);
    });
}

/// Rule 5: items counted as AP only, whatever physical stat they also carry.
/// By base slug, so the radiant tier follows. Bloodsong and Sword of Blossoming
/// Dawn give attack speed beside their ability power, which made them hybrid
/// and let AD supports keep them; the user put them with the AP support items
/// (2026-09-26).
const AP_ITEMS: [&str; 2] = ["bloodsong", "sword_of_blossoming_dawn"];

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
    JungleOnly,
    MandateWithoutCc,
    Reach,
}

impl Reason {
    /// Whether the offending item's own category is the wrong place to look
    /// for its stand-in. A support item's category holds nothing but support
    /// items, and an item the champion does not scale with sits among others
    /// it does not scale with, so for these two the stand-in is taken from the
    /// categories the rest of the build uses instead. Imperial Mandate on a
    /// support without crowd control goes the same way: it is a support item,
    /// so another support item stands in first. A jungle item's category is an
    /// ordinary one (Grez's is a Mage item), so its stand-in comes from there,
    /// like a duplicate's. So is a melee or ranged item's: a ranged champion
    /// that loses Titanic Hydra still wanted an item of that kind.
    pub(crate) fn restyles(self) -> bool {
        matches!(
            self,
            Reason::SupportOnly | Reason::Scaling | Reason::MandateWithoutCc
        )
    }
}

/// The Support-class items any champion may build, by base slug. Zeke's
/// Convergence keeps its old key, `zekes_herald`.
const SUPPORT_ITEM_EXCEPTIONS: [&str; 2] = ["protoplasm_harness", "zekes_herald"];

/// What the champion a build is for may hold, whatever the build already has.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    /// Rule 4: whether support items are allowed.
    support_items: bool,
    /// Rule 5: what the champion scales with, `None` when unknown.
    scaling: Option<Scaling>,
    /// Rule 8: whether jungle items are allowed.
    jungle_items: bool,
    /// Rule 10: whether this is a support with crowd control, whose support
    /// item is Imperial Mandate.
    mandate: bool,
    /// Rule 10's other half: whether this is a support known to have no crowd
    /// control, who never keeps Mandate. `false` when the champion is unknown.
    no_mandate: bool,
    /// Rule 11: whether the champion attacks from range, `None` when unknown.
    ranged: Option<bool>,
}

/// The [`Fit`] for `champion` playing `role`.
///
/// Support items are allowed in the support role and nowhere else — not even an
/// unknown role ([`Role::Any`]), which the buy detour falls back to when no
/// lineup has placed the champion yet. Jungle items follow the same line, for
/// the jungle role. A champion nothing is known about gets no scaling
/// restriction and none on its reach: a missing tag must never cost a build an
/// item.
pub(crate) fn fit(champion: &str, role: Role) -> Fit {
    let traits = champion_traits::traits(champion);
    Fit {
        support_items: role == Role::Support,
        scaling: traits.and_then(|traits| traits.scaling),
        jungle_items: role == Role::Jungle,
        mandate: role == Role::Support && traits.is_some_and(|traits| traits.cc),
        no_mandate: role == Role::Support && traits.is_some_and(|traits| !traits.cc),
        ranged: traits.and_then(|traits| traits.ranged),
    }
}

impl Fit {
    /// Rule 5 for one item: whether an item with these traits gives only stats
    /// the champion cannot use.
    ///
    /// Support items in the support role used to be exempt, on the grounds that
    /// their worth is what they do for allies. That let an AD support keep an
    /// AP-only one (Dual Blader on Staff of Flowing Water, 2026-09-26), and the
    /// user asked for it gone. The support items an AD champion can use pass
    /// anyway: the tank ones carry no offensive stat. Bloodsong and Sword of
    /// Blossoming Dawn would pass as hybrid too, but [`AP_ITEMS`] counts them
    /// as AP.
    fn mismatches(&self, item: &ItemTraits) -> bool {
        match self.scaling {
            Some(Scaling::Ap) => item.physical && !item.magic,
            Some(Scaling::Ad) => item.magic && !item.physical,
            Some(Scaling::Hybrid) | None => false,
        }
    }

    /// Rule 11: whether `key` is an item for the other reach — a melee item on
    /// a ranged champion, or a ranged item on a melee one.
    fn out_of_reach(&self, key: &str) -> bool {
        match self.ranged {
            Some(true) => is_melee_item(key),
            Some(false) => is_ranged_item(key),
            None => false,
        }
    }

    /// Whether the champion plays a role with items of its own (rule 9).
    fn has_role_items(&self) -> bool {
        self.support_items || self.jungle_items
    }

    /// Rule 9: whether `key` is an item of the role this champion plays — a
    /// Support-class item for a support, a jungle item for a jungler, nothing
    /// in any other role.
    fn is_role_item(&self, key: &str) -> bool {
        (self.support_items && is_support_class(key)) || (self.jungle_items && is_jungle_item(key))
    }
}

/// Rule 6: items worth more the longer they are owned, because their passive
/// builds permanent stacks over the match. By base slug, so the radiant tier
/// follows.
const EARLY_ITEMS: [&str; 6] = [
    "heartsteel",             // permanent bonus health per charged hit on a champion
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

/// The order rules 6 and 9 buy AI picks in: the role's own items (rule 9)
/// first, then by [`timing`]. Sorted on, so smaller is sooner.
fn buy_order(key: Option<&str>, fit: Fit) -> (bool, Timing) {
    match key {
        Some(key) => (!fit.is_role_item(key), timing(key)),
        None => (true, Timing::Any),
    }
}

/// Rules 6 and 9 over AI picks chosen elsewhere: the buy detour's, for the 5th
/// and 6th slots the build only grows to after [`enforce`] has run. Stable,
/// like the sort in [`enforce`], so picks of the same timing keep their order.
pub(crate) fn sort_by_timing<T>(picks: &mut [T], fit: Fit, key: impl Fn(&T) -> Option<String>) {
    picks.sort_by_key(|pick| buy_order(key(pick).as_deref(), fit));
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

/// The build slot rule 7 puts boots in: the second, which is where League
/// players finish theirs. A pin always wins the slot: when one holds it, the
/// boots take the first open slot after it (up to the 6th), then the first
/// slot, and with every slot pinned there are no boots.
const BOOTS_SLOT: usize = 1;

/// Whether `key` is a pair of boots, tier 1 or upgraded.
pub(crate) fn is_boots(key: &str) -> bool {
    key == BASE_BOOTS || UPGRADED_BOOTS.contains(&key)
}

/// The boots rule 7 gives `champion` playing `role`, against `enemies` (empty
/// when the caller cannot see them).
///
/// In order: a tank answers the enemy's main damage type (Mercury's against
/// more magic than physical, Steelcaps otherwise, which also covers not
/// knowing), whatever its role; any other support takes haste (Lucidity),
/// since it wins through its abilities' uptime rather than their damage; then
/// the champion's own damage: ability power takes magic
/// penetration, and a physical champion follows its class — attack speed for
/// the ranged, haste for assassins, omnivamp for fighters. A utility champion
/// that heals or shields takes haste (Lucidity), and one that does neither, or
/// one nothing is known about, the plain speed of Swiftness.
pub(crate) fn boots_for(champion: &str, role: Role, enemies: &[&str]) -> &'static str {
    use champion_traits::Class;
    let traits = champion_traits::traits(champion).unwrap_or_default();
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

/// Whether `key` is in the editor's Support class, base or radiant —
/// [`SUPPORT_ITEM_EXCEPTIONS`] included, unlike [`is_support_item`].
fn is_support_class(key: &str) -> bool {
    crate::item_catalog::category_of(crate::build_config::base_slug(key)) == Some("Support")
}

/// Whether `key` is an item only the support role may build: the editor's Support
/// class, base or radiant, less [`SUPPORT_ITEM_EXCEPTIONS`].
fn is_support_item(key: &str) -> bool {
    !SUPPORT_ITEM_EXCEPTIONS.contains(&crate::build_config::base_slug(key)) && is_support_class(key)
}

/// Rule 10: whether `key` is Imperial Mandate, base or radiant.
fn is_mandate(key: &str) -> bool {
    crate::build_config::base_slug(key) == "imperial_mandate"
}

/// Rule 8: items only the jungle role may build. By base slug, so the radiant
/// tier follows.
const JUNGLE_ITEMS: [&str; 2] = [
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
];

/// Whether `key` is one of [`JUNGLE_ITEMS`], base or radiant.
fn is_jungle_item(key: &str) -> bool {
    JUNGLE_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Rule 11: items whose passive wants the carrier in melee, which a ranged
/// champion does not keep. By base slug, so the radiant tier follows. The
/// distances are the defaults; all of them are config-editable.
const MELEE_ITEMS: [&str; 7] = [
    "ravenous_hydra",  // Cleave at half strength from past 35 range
    "titanic_hydra",   // Cleave at half strength from past 35 range
    "hullbreaker",     // Skipper at 70% strength from past 35 range
    "heartsteel",      // Ironheart charges on enemies that stay within 50 range
    "hollow_radiance", // Immolate burns enemies within 30 range
    "sunfire_cape",    // Immolate again; this slug is the radiant cape's
    // Sunfire Cape itself, whose key the base game never lost: only the
    // radiant reskins have an alias for `base_slug` to undo.
    "hourglass_of_eternity",
];

/// Rule 11: items whose passive wants the carrier at range, which a melee
/// champion does not keep. By base slug, so the radiant tier follows.
const RANGED_ITEMS: [&str; 2] = [
    "runaans_hurricane",    // bolts at the enemies around the carrier; ranged only in League
    "diamond_tipped_spear", // Sweet Spot grows with the distance to the target, up to 100 range
];

/// Whether `key` is one of [`MELEE_ITEMS`], base or radiant.
fn is_melee_item(key: &str) -> bool {
    MELEE_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Whether `key` is one of [`RANGED_ITEMS`], base or radiant.
fn is_ranged_item(key: &str) -> bool {
    RANGED_ITEMS.contains(&crate::build_config::base_slug(key))
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
        } else if !self.fit.jungle_items && is_jungle_item(key) {
            Some(Reason::JungleOnly)
        } else if self.fit.no_mandate && is_mandate(key) {
            Some(Reason::MandateWithoutCc)
        } else if self.fit.mismatches(&traits) {
            Some(Reason::Scaling)
        } else if self.fit.out_of_reach(key) {
            // After rule 5: an item that fails both (Runaan's on a melee mage)
            // is replaced the way rule 5 does it, in the build's style, not by
            // another item of a kind the champion cannot use.
            Some(Reason::Reach)
        } else if self.cuts_healing && traits.cuts_healing {
            Some(Reason::Grievous)
        } else if self.crit_chance + traits.crit_chance > CRIT_CAP {
            Some(Reason::Crit)
        } else {
            None
        }
    }

    /// Whether `key` is an item this champion may hold at all — rules 4, 5, 8,
    /// 10 and 11, which do not depend on what else is in the build. An item
    /// that fails is no guide to the build's style.
    pub(crate) fn suits_champion(&self, key: &str) -> bool {
        !(!self.fit.support_items && is_support_item(key))
            && !(!self.fit.jungle_items && is_jungle_item(key))
            && !(self.fit.no_mandate && is_mandate(key))
            && !self.fit.mismatches(&self.table.traits(key))
            && !self.fit.out_of_reach(key)
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

/// Rewrites `build` in place so that it breaks none of the eight rules, as far as
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
/// Then rule 9: a support or jungler whose build holds no item of its role,
/// pinned or picked, has its last AI slot swapped for one. The walk is the one
/// above, from the item being replaced, and the stand-in is held to every
/// other rule against the rest of the build. Nothing that fits, nothing
/// changes.
///
/// Then rule 10: a support with crowd control whose build has no Imperial
/// Mandate, and no support item the player pinned, has the AI's first support
/// item (preferring one only supports may build) swapped for Mandate, held to
/// the other rules the same way.
///
/// Last, rule 6 reorders the AI's slots among themselves: the role's own items
/// first (rule 9), then early items, late items last, and the engine's order
/// within each group. A pinned slot keeps its position and its item, so the
/// player's buy order is never moved. Every caller decides the build before
/// the match, when nothing is bought yet, so no owned item can end up in a
/// later slot.
///
/// Then rule 7: `boots` is the catalog index of the pair [`boots_for`] picked
/// (`None` when the catalog has none). A build with no boots in any slot or in
/// `reserved` gets them in the second slot, or the first open slot after it
/// when a pin holds that one, else the first ([`BOOTS_SLOT`]); the AI picks from there on move
/// one AI slot later and the last one drops off. Boots are never a stand-in for
/// the other rules, whatever `is_final` says about them.
///
/// `later_open`: a slot past the end of `build` — the 5th or 6th, which exist
/// only once the buy detour grows the build — that no pin holds
/// ([`crate::build_config::later_slot_open`]). When every slot of `build` from
/// the second on is taken, the boots are left to that slot
/// (`tactics::extra_slot_pick`) rather than put in the first.
pub(crate) fn enforce<C, K, G, F>(
    count: usize,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    later_open: bool,
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
                    // An item of the champion's role that fails rule 5 (an
                    // AP-only support item on an AD support) makes way for one
                    // of the role it can use first: rule 9 wants the role to
                    // keep one, and the build's style would not look there.
                    let role_item = current
                        .as_deref()
                        .is_some_and(|key| fit.is_role_item(key))
                        .then(|| {
                            search(&|candidate| {
                                key(candidate).is_some_and(|key| fit.is_role_item(&key))
                            })
                        })
                        .flatten();
                    role_item
                        .or_else(|| {
                            styles.iter().find_map(|style| {
                                search(&|candidate| category(candidate).as_ref() == Some(style))
                            })
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

    // What the rest of the build spends with `slot` left out: the budget a
    // stand-in for that slot is judged against (rules 9 and 10).
    let budget_without = |build: &[usize], slot: usize| {
        let mut rest = Budget::empty(fit);
        let others = build
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != slot)
            .map(|(_, &index)| index);
        for index in others.chain(reserved.iter().copied()) {
            if let Some(key) = key(index) {
                rest.take(&key);
            }
        }
        rest
    };

    // Rule 9. The AI's last pick is the one the engine wanted least, so it is
    // the one that makes way. The stand-in is judged against the rest of the
    // build, which is why the budget is rebuilt without that slot.
    let needs_role_item = fit.has_role_items()
        && !build
            .iter()
            .chain(reserved)
            .any(|&index| key(index).is_some_and(|key| fit.is_role_item(&key)));
    if needs_role_item {
        let last_pick = (0..build.len()).rev().find(|&slot| {
            !is_pinned(slot) && !key(build[slot]).is_some_and(|key| is_boots(&key))
        });
        if let Some(slot) = last_pick {
            let offender = build[slot];
            let rest = budget_without(build, slot);
            let role_item = (1..count)
                .map(|step| (offender + step) % count)
                .find(|&candidate| {
                    !seen.contains(&candidate)
                        && is_final(candidate)
                        && key(candidate).is_some_and(|candidate| {
                            fit.is_role_item(&candidate)
                                && !is_boots(&candidate)
                                && rest.rejects(&candidate).is_none()
                        })
                });
            if let Some(role_item) = role_item {
                build[slot] = role_item;
            }
        }
    }

    // Rule 10. After rule 9, so a support always holds a support item here
    // unless nothing fit; this only decides which one. The player's pinned
    // support item is their choice, so a pin rules Mandate out.
    let holds = |index: usize, want: fn(&str) -> bool| key(index).is_some_and(|key| want(&key));
    let pinned_support = (0..build.len())
        .filter(|&slot| is_pinned(slot))
        .map(|slot| build[slot])
        .chain(reserved.iter().copied())
        .any(|index| holds(index, is_support_class));
    let has_mandate = build
        .iter()
        .chain(reserved)
        .any(|&index| holds(index, is_mandate));
    if fit.mandate && !has_mandate && !pinned_support {
        let ai_slot_holding = |want: fn(&str) -> bool| {
            (0..build.len()).find(|&slot| !is_pinned(slot) && holds(build[slot], want))
        };
        if let Some(slot) =
            ai_slot_holding(is_support_item).or_else(|| ai_slot_holding(is_support_class))
        {
            let rest = budget_without(build, slot);
            let mandate = (0..count).find(|&candidate| {
                is_final(candidate)
                    && !build.contains(&candidate)
                    && key(candidate).is_some_and(|candidate| {
                        is_mandate(&candidate) && rest.rejects(&candidate).is_none()
                    })
            });
            if let Some(mandate) = mandate {
                build[slot] = mandate;
            }
        }
    }

    // Rule 6, with rule 9's role items ahead of it. A stable sort, so items
    // of the same timing keep the engine's order.
    let open: Vec<usize> = (0..build.len()).filter(|&slot| !is_pinned(slot)).collect();
    let mut picks: Vec<usize> = open.iter().map(|&slot| build[slot]).collect();
    picks.sort_by_key(|&index| buy_order(key(index).as_deref(), fit));

    // Rule 7.
    let has_boots = build
        .iter()
        .chain(reserved)
        .any(|&index| key(index).is_some_and(|key| is_boots(&key)));
    // Where the boots go, as a position in `picks` (the AI's slots, in
    // order): the second build slot, or the first open one after it when a pin
    // holds it — the 5th and 6th included, which the buy detour fills when it
    // grows the build, so an open one there leaves the boots to it — or the
    // first slot when every one from the second on is pinned. No open slot at
    // all, no boots.
    let at = match (0..open.len()).find(|&pick| open[pick] >= BOOTS_SLOT) {
        Some(at) => Some(at),
        None if later_open => None,
        None => (!open.is_empty()).then_some(0),
    };
    if let (Some(boots), Some(at), false) = (boots, at, has_boots) {
        picks.insert(at, boots);
        picks.truncate(open.len());
    }

    for (&slot, index) in open.iter().zip(picks) {
        build[slot] = index;
    }
}
