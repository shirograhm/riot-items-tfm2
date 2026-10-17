//! The Smart Builds rules, which the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build:
//!
//! 1. **Unique items**: no item twice.
//! 2. **One Grievous Wounds item.**
//! 3. **Crit chance at or under 100%**, counting a stacking passive at full
//!    stacks; an overflowing slot is replaced by an item with no crit.
//! 4. **Support items stay in the support role**, bar Protoplasm Harness.
//! 5. **Items the champion scales with**: an AP-only champion keeps no attack,
//!    attack speed or crit item ([`AP_ITEMS`] count as AP) and an AD-only one
//!    no magic power item; only a hybrid keeps a hybrid item. Rules 12, 13 and
//!    14's items are not judged here.
//! 6. **Items bought when they pay off**: [`EARLY_ITEMS`] go first and only in
//!    the first [`EARLY_SLOTS`] slots, [`LATE_ITEMS`] only after them. A pick
//!    still on the wrong side makes way for an item of no timing. Supports are
//!    exempt.
//! 7. **One pair of boots**: a build without any gets [`boots_for`]'s pair in
//!    the second slot, or the first open slot after a pin or early item there.
//! 8. **Jungle items stay in the jungle, one to a build**: Philosopher's Stone
//!    on a `Tank`, else Feral Flare or Grez's by damage type.
//! 9. **Role items first**: a jungler's jungle item goes in the first slot,
//!    unless the player pinned it ([`Fit::starter_slot_pinned`]). A support
//!    other than a mage looks to Support-class items first, right after its
//!    World Atlas item.
//! 10. **Imperial Mandate for supports that immobilize**: the AI's first
//!     support item makes way for Mandate, and a support known not to
//!     immobilize keeps none. A pinned support item stays, and a build with no
//!     AI support item gets none.
//! 11. **Items for the champion's reach**: a ranged champion keeps no
//!     [`MELEE_ITEMS`] and a melee one (reach 35 or less) no [`RANGED_ITEMS`].
//! 12. **A World Atlas item for every support**: the one [`Fit::atlas_item`]
//!     picks, in the first slot, in place of the AI's last pick.
//! 13. **Heal, shield and buff items for supports that do that**: the AI's
//!     first spare support item makes way for one of [`ALLY_AID_ITEMS`]; a
//!     support known not to aid allies, or a mage, keeps none.
//! 14. **Marksman items for marksmen**: only a `Range`-class champion keeps
//!     [`MARKSMAN_ONLY_ITEMS`], whatever it scales with.
//! 15. **One Spellblade item** of [`SPELLBLADE_ITEMS`]; a support whose World
//!     Atlas item is Bloodsong keeps no other.
//! 16. **Tank and support items for supports**: a support keeps only Tank- and
//!     Support-class items, plus boots. A `Magician`-class support is held to
//!     them only when tagged `Tank` or building Dream Maker.
//! 17. **Heal, shield and buff items first for heal, shield and buff
//!     supports**: a Dream Maker support looks to [`PREFERRED_AID_ITEMS`]
//!     first, as a preference, not a quota.
//!
//! The rules only replace AI picks: a pinned slot is kept and counts toward the
//! budgets like any other item, and an earlier slot wins over a later one.
//!
//! The champion side comes in as a [`Fit`] (facts from
//! [`crate::champion_traits`]). Item facts are keyed by item key, never read
//! off an item (see the warning above `hook::detour`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use mod_api_stable::{ItemTagV1, StableItem};

use crate::build_config::Role;
use crate::champion_traits::{self, Class, Scaling};

// The engine caps crit chance at 100%, so a build totalling exactly 100 is
// fine.
const CRIT_CAP: i32 = 100;

#[derive(Clone, Copy, Default)]
struct ItemTraits {
    // Flat crit plus a passive's at full stacks (Atma's Reckoning, Rite of
    // Ruin, Yun Tal Wildarrows), since an overflow only wastes crit once the
    // stacks are there.
    crit_chance: i32,
    // Whether the item applies Grievous Wounds.
    cuts_healing: bool,
    // Attack, attack speed or crit: stats only a physical champion uses.
    physical: bool,
    // Whether it gives magic power.
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

    fn blank(&self) -> bool {
        !self.physical && !self.magic && self.crit_chance == 0
    }
}

#[derive(Clone, Default)]
struct Table {
    // This mod's items as `init` registers them; authoritative for its own
    // keys.
    mod_items: HashMap<String, ItemTraits>,
    // The game's items from the mod's settings file
    // (`item_stats::prime_game_items`), other mods' from the client's settings
    // document. Read rather than hardcoded because `item_setting` can change
    // them.
    engine_items: HashMap<String, ItemTraits>,
}

// The game's items with an offensive stat, at the values this mod ships: key,
// crit, attack, attack speed, magic power. A last resort for when the settings
// file cannot be read or a record comes out blank ([`Table::traits`]); kept in
// step with `setting/item_setting` by hand.
const GAME_ITEMS: [(&str, i32, i32, i32, i32); 15] = [
    ("ironsword", 0, 10, 0, 0),
    ("soldiers_longsword", 0, 20, 0, 0),
    ("ruinous_blade", 0, 25, 0, 0),
    ("conquerors_greatsword", 0, 40, 0, 0),
    ("warlords_final_judgement", 0, 50, 0, 0),
    ("dagger", 0, 0, 8, 0),
    ("wind_dagger", 0, 0, 15, 0),
    ("twin_stormblade", 10, 0, 25, 0),
    ("thunderclaw", 20, 0, 40, 0),
    ("storm_sovereign", 25, 0, 55, 0),
    ("arcane_crystal", 0, 0, 0, 15),
    ("spirit_crystal", 0, 0, 0, 30),
    ("staff_of_rapture", 0, 0, 0, 50),
    ("angels_fang", 0, 0, 0, 75),
    ("prophet_of_the_abyss", 0, 0, 0, 100),
];

fn game_item_traits(key: &str) -> Option<ItemTraits> {
    let &(_, crit, attack, speed, power) = GAME_ITEMS.iter().find(|(item, ..)| *item == key)?;
    Some(ItemTraits::from_stats(crit, attack, speed, power))
}

impl Table {
    // This mod's record, then the settings document's, then [`GAME_ITEMS`]; an
    // item none describes has no traits. A blank record for one of the game's
    // damage items is taken for a failed read.
    fn traits(&self, key: &str) -> ItemTraits {
        let recorded = self
            .mod_items
            .get(key)
            .or_else(|| self.engine_items.get(key))
            .copied();
        match (recorded, game_item_traits(key)) {
            (Some(recorded), Some(known)) if recorded.blank() => known,
            (Some(recorded), _) => recorded,
            (None, known) => known.unwrap_or_default(),
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

// Records one of this mod's items from the registration macros in `lib.rs`,
// where `stat()` and `tags()` are still safe to call (unlike on a `dyn
// ItemInfo`).
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
    if is_ap_item(key) {
        traits.physical = false;
    }
    // Rules 12, 13 and 14 judge their items by role, kit or class, so rule 5
    // has no say over them.
    if is_atlas_item(key) || is_ally_aid_item(key) || is_marksman_only_item(key) {
        traits.physical = false;
        traits.magic = false;
    }
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), traits);
    });
}

// Rule 5: items counted as AP only, by base slug. Their attack speed (or Rite
// of Ruin's crit, spent by its own passive) does not make them physical.
const AP_ITEMS: [&str; 4] = [
    "dusk_and_dawn",
    "lich_bane",
    "nashors_tooth",
    "rite_of_ruin",
];

fn is_ap_item(key: &str) -> bool {
    AP_ITEMS.contains(&crate::build_config::base_slug(key))
}

// Adds a passive's full-stack crit to what [`note_mod_item`] recorded, from the
// `passive_crit` arm in `lib.rs`.
pub(crate) fn note_passive_crit(key: &str, crit_chance: i32) {
    edit_table(|table| {
        let traits = table.mod_items.entry(key.to_string()).or_default();
        traits.crit_chance += crit_chance;
        traits.physical |= crit_chance > 0 && !is_ap_item(key);
    });
}

// Records one of the game's items from the settings document
// [`crate::item_stats`] parses, even one with none of these stats.
pub(crate) fn note_engine_item(
    key: &str,
    crit_chance: i32,
    attack: i32,
    attack_speed: i32,
    magic_power: i32,
) {
    let traits = ItemTraits::from_stats(crit_chance, attack, attack_speed, magic_power);
    edit_table(|table| {
        // A later object under the same key with no stats must not undo the
        // first.
        let known = table.engine_items.entry(key.to_string()).or_insert(traits);
        if !traits.blank() {
            *known = traits;
        }
    });
}

// What the rules make of `key` for the Check Tactics test log: `P` physical,
// `M` magic, and why an empty build would reject it.
pub(crate) fn explain(champion: &str, role: Role, key: &str) -> String {
    let budget = Budget::empty(fit(champion, role));
    let traits = budget.table.traits(key);
    format!(
        "{key}({}{}:{})",
        if traits.physical { "P" } else { "" },
        if traits.magic { "M" } else { "" },
        budget
            .rejects(key)
            .map_or_else(|| "ok".to_string(), |reason| format!("{reason:?}"))
    )
}

// What `champion` is taken to scale with, for the same log.
pub(crate) fn explain_champion(champion: &str, role: Role) -> String {
    format!("{:?}", fit(champion, role).scaling)
}

// Why an item cannot join a build. A crit overflow is the one reason whose
// stand-in must add no crit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Reason {
    Duplicate,
    Grievous,
    Crit,
    SupportOnly,
    Scaling,
    JungleOnly,
    // A jungler holding a jungle item that is not its champion's.
    JungleMismatch,
    // A jungle item in a build that already holds one.
    SecondJungle,
    // A support holding a World Atlas item that is not its champion's.
    AtlasMismatch,
    // A World Atlas item in a build that already holds one.
    SecondAtlas,
    // A Spellblade item in a build that already holds one (rule 15).
    SecondSpellblade,
    MandateWithoutCc,
    // An item that answers to healing, shielding or buffing an ally, on a
    // support whose kit does none of that.
    AllyAidWithoutAid,
    // An item only a marksman keeps (rule 14) on a champion that is none.
    MarksmanOnly,
    // Not a Tank or Support item, on a support held to those classes (rule
    // 16).
    SupportClasses,
    Reach,
}

impl Reason {
    // Whether the stand-in comes from the build's style rather than the
    // offender's own category. That category would only offer more of what the
    // champion cannot use, or the role's own item should stand in first.
    pub(crate) fn restyles(self) -> bool {
        matches!(
            self,
            Reason::SupportOnly
                | Reason::Scaling
                | Reason::JungleMismatch
                | Reason::AtlasMismatch
                | Reason::SecondAtlas
                | Reason::MandateWithoutCc
                | Reason::AllyAidWithoutAid
                | Reason::MarksmanOnly
                | Reason::SupportClasses
        )
    }
}

// The Support-class items any champion may build, by base slug.
const SUPPORT_ITEM_EXCEPTIONS: [&str; 1] = ["protoplasm_harness"];

// What the champion a build is for may hold, whatever the build already has.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    // Rule 4: whether support items are allowed.
    support_items: bool,
    // Rule 5: what the champion scales with, `None` when unknown.
    scaling: Option<Scaling>,
    // Rule 14: whether the champion is a marksman, the game's `Range` class,
    // `None` when its class is unknown.
    marksman: Option<bool>,
    // Rule 8: whether jungle items are allowed.
    jungle_items: bool,
    // Rule 8: a `Tank` jungler, whose jungle item is Philosopher's Stone. Not a
    // ranged one, since rule 11 would take the Stone back off.
    stone_jungler: bool,
    // Rule 10: a support whose kit immobilizes, whose support item is Imperial
    // Mandate (mages included).
    mandate: bool,
    // Rule 10: a support known not to immobilize, which never keeps Mandate.
    no_mandate: bool,
    // Rule 12: a `Tank` support that immobilizes for
    // [`SLEIGH_IMMOBILIZE_TICKS`] or more, whose World Atlas item is Solstice
    // Sleigh.
    sleigh: bool,
    // A `Magician`-class support: Zaz'Zak's Realmspike unless tagged `Tank`
    // (rule 12), treated as not aiding allies (rule 13), preferring no support
    // items (rule 9), and the one support rule 16 lets build other classes.
    mage: bool,
    // Rule 13: a support other than a mage whose kit heals, shields or buffs
    // allies; rule 12 gives it Dream Maker.
    ally_aid: bool,
    // Rule 13: a support known not to aid allies, or a mage, which never keeps
    // such an item.
    no_ally_aid: bool,
    // Rule 12: a `Tank` support that `sleigh` did not take, whose World Atlas
    // item is Celestial Opposition.
    celestial: bool,
    // Rule 16: a `Tank`-tagged support, held to tank and support items.
    tank_support: bool,
    // Rule 12: the World Atlas item named for this champion as a support
    // ([`ATLAS_BY_CHAMPION`]), whatever the tests would say.
    atlas_named: Option<&'static str>,
    // Rule 12: an AP support none of the earlier tests took, whose World Atlas
    // item is Zaz'Zak's Realmspike.
    realmspike: bool,
    // Rule 11: whether the champion attacks from range, `None` when unknown.
    ranged: Option<bool>,
    // Rules 9 and 12: the first slot is pinned, so the build goes without its
    // jungle or World Atlas item ([`Fit::starter_slot_pinned`]).
    no_starter: bool,
}

// The [`Fit`] for `champion` playing `role`. Support and jungle items are
// allowed only in their own role (not [`Role::Any`]), and a champion nothing is
// known about gets no restriction: a missing tag must never cost a build an
// item.
pub(crate) fn fit(champion: &str, role: Role) -> Fit {
    let traits = champion_traits::traits(champion);
    let mage =
        role == Role::Support && traits.is_some_and(|traits| traits.class == Some(Class::Magician));
    Fit {
        support_items: role == Role::Support,
        scaling: traits.and_then(|traits| traits.scaling),
        marksman: traits
            .and_then(|traits| traits.class)
            .map(|class| class == Class::Range),
        jungle_items: role == Role::Jungle,
        stone_jungler: role == Role::Jungle
            && traits.is_some_and(|traits| traits.tank && traits.ranged != Some(true)),
        mandate: role == Role::Support && traits.is_some_and(|traits| traits.can_immobilize()),
        no_mandate: role == Role::Support && traits.is_some_and(|traits| !traits.can_immobilize()),
        sleigh: role == Role::Support
            && traits.is_some_and(|traits| {
                traits.tank && traits.immobilizes_for(SLEIGH_IMMOBILIZE_TICKS)
            }),
        mage,
        ally_aid: role == Role::Support
            && !mage
            && traits.is_some_and(|traits| traits.aids_allies()),
        no_ally_aid: role == Role::Support
            && (mage || traits.is_some_and(|traits| !traits.aids_allies())),
        celestial: role == Role::Support && traits.is_some_and(|traits| traits.tank),
        tank_support: role == Role::Support && traits.is_some_and(|traits| traits.tank),
        atlas_named: ATLAS_BY_CHAMPION
            .iter()
            .find(|(key, _)| role == Role::Support && *key == champion)
            .map(|&(_, item)| item),
        realmspike: role == Role::Support
            && traits.is_some_and(|traits| traits.scaling == Some(Scaling::Ap)),
        ranged: traits.and_then(|traits| traits.ranged),
        no_starter: false,
    }
}

impl Fit {
    // This [`Fit`] for a build whose first slot the player pinned: it goes
    // without the jungle or World Atlas item that only goes there (rules 9 and
    // 12).
    pub(crate) fn starter_slot_pinned(self) -> Self {
        Self {
            no_starter: self.jungle_items || self.support_items,
            ..self
        }
    }

    // Whether `key` is the jungle or World Atlas item such a build goes
    // without.
    pub(crate) fn skips_starter(&self, key: &str) -> bool {
        self.no_starter && ((self.jungle_items && is_jungle_item(key)) || self.is_atlas_pick(key))
    }

    // Rule 16: whether `key` is of a class this support does not build. Every
    // support is held to tank and support items but a mage, which is only when
    // tagged `Tank` or building Dream Maker. Boots and items with no class are
    // left alone; `base_slug` gives the game's six finals a class.
    fn off_support_classes(&self, key: &str) -> bool {
        (self.tank_support || self.aid_support() || (self.support_items && !self.mage))
            && !matches!(
                crate::item_catalog::category_of(crate::build_config::base_slug(key)),
                None | Some("Tank" | "Support" | "Boots")
            )
    }

    // Rule 17: a support whose World Atlas item is Dream Maker. A tank that
    // aids allies but holds another Atlas item is held to the classes by rule
    // 16, without the preference.
    fn aid_support(&self) -> bool {
        self.support_items && self.atlas_item() == DREAM_MAKER
    }

    // Rule 17: whether this support looks to [`PREFERRED_AID_ITEMS`] first.
    pub(crate) fn prefers_aid_items(&self) -> bool {
        self.aid_support()
    }

    // Rule 17: whether `key` is one of those items ([`PREFERRED_AID_ITEMS`])
    // and this a support that looks to them first.
    pub(crate) fn is_preferred_aid_item(&self, key: &str) -> bool {
        self.aid_support() && PREFERRED_AID_ITEMS.contains(&crate::build_config::base_slug(key))
    }

    // Rule 5 for one item: whether it gives a stat the champion cannot use.
    // Support items are judged like any other; the tank ones carry no offensive
    // stat anyway.
    fn mismatches(&self, item: &ItemTraits) -> bool {
        match self.scaling {
            Some(Scaling::Ap) => item.physical,
            Some(Scaling::Ad) => item.magic,
            Some(Scaling::Hybrid) | None => false,
        }
    }

    // Rule 14: whether `key` is an item only a marksman keeps, on a champion
    // known not to be one.
    fn wants_marksman(&self, key: &str) -> bool {
        self.marksman == Some(false) && is_marksman_only_item(key)
    }

    // Rule 11: whether `key` is an item for the other reach — a melee item on
    // a ranged champion, or a ranged item on a melee one.
    fn out_of_reach(&self, key: &str) -> bool {
        match self.ranged {
            Some(true) => is_melee_item(key),
            Some(false) => is_ranged_item(key),
            None => false,
        }
    }

    // Rule 8: a jungle item on a champion that is not jungling, the one test
    // even a last-resort pick is held to.
    pub(crate) fn off_role_jungle_item(&self, key: &str) -> bool {
        !self.jungle_items && is_jungle_item(key)
    }

    // Rule 8, for a jungler: a jungle item other than its champion's (a damage
    // one on a tank, the Stone on anyone else). Which damage item is rule 5's
    // call.
    fn other_jungle_item(&self, key: &str) -> bool {
        self.jungle_items && is_jungle_item(key) && is_philosophers_stone(key) != self.stone_jungler
    }

    // Rule 9: whether the build is guaranteed a role item, which only a
    // jungler's is, unless its first slot is pinned.
    fn guarantees_role_item(&self) -> bool {
        self.jungle_items && !self.no_starter
    }

    // Rule 9: any support but a mage prefers Support-class items past its World
    // Atlas item.
    fn prefers_support_items(&self) -> bool {
        self.support_items && !self.mage
    }

    // Rule 9: a Support-class item this champion prefers, World Atlas aside.
    // The item-build hook pushes these harder than the mod's other items.
    pub(crate) fn is_preferred_support_item(&self, key: &str) -> bool {
        self.prefers_support_items() && is_other_support_class(key)
    }

    // Rule 9: a preferred support item, or a jungler's jungle item. Bought
    // right after the World Atlas item and looked to first for a stand-in.
    fn is_role_item(&self, key: &str) -> bool {
        self.is_preferred_support_item(key) || (self.jungle_items && is_jungle_item(key))
    }

    // Rule 12: whether `key` is any World Atlas item held by a support.
    fn is_atlas_pick(&self, key: &str) -> bool {
        self.support_items && is_atlas_item(key)
    }

    // Rule 12
    fn atlas_item(&self) -> &'static str {
        if let Some(named) = self.atlas_named {
            named
        } else if self.sleigh {
            SOLSTICE_SLEIGH
        } else if self.celestial {
            CELESTIAL_OPPOSITION
        } else if self.ally_aid {
            DREAM_MAKER
        } else if self.mage || self.realmspike {
            ZAZZAKS_REALMSPIKE
        } else {
            BLOODSONG
        }
    }
    // Rule 12
    fn other_atlas_item(&self, key: &str) -> bool {
        self.is_atlas_pick(key) && crate::build_config::base_slug(key) != self.atlas_item()
    }

    // Rule 15
    fn holds_bloodsong(&self) -> bool {
        self.support_items && self.atlas_item() == BLOODSONG
    }
}

// Rule 6
const EARLY_ITEMS: [&str; 7] = [
    "heartsteel",             // permanent bonus health per charged hit on a champion
    "yun_tal_wildarrows",     // permanent crit chance every third basic attack
    "hubris",                 // permanent stack per takedown
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
    "collector",              // bonus gold per kill, worth more the earlier it comes
    "rod_of_ages",            // health, ability power and haste for every 30 seconds held
];
// Rule 6
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

// When an item pays off, in the order rule 6 buys them.
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

// Rule 6: how many leading slots an early item may take; a late item only goes
// after them.
const EARLY_SLOTS: usize = 2;

impl Fit {
    // Rule 6's timing of `key` for this champion. Supports are exempt: to one,
    // every item is of no timing.
    fn timing(&self, key: &str) -> Timing {
        if self.support_items {
            Timing::Any
        } else {
            timing(key)
        }
    }

    // Rule 6: whether this champion may buy `key` in build slot `slot` (from
    // 0).
    pub(crate) fn fits_slot(&self, key: &str, slot: usize) -> bool {
        match self.timing(key) {
            Timing::Early => slot < EARLY_SLOTS,
            Timing::Late => slot >= EARLY_SLOTS,
            Timing::Any => true,
        }
    }
}

// The order AI picks are bought in, smallest first: a support's World Atlas
// item, rule 17's preferred items, the role's own items, then rule 6's timing
// ([`Fit::timing`]). Rule 7's boots push the last pick off, so it is never one
// of the first kinds while another is left.
fn buy_order(key: Option<&str>, fit: Fit) -> (bool, bool, bool, Timing) {
    match key {
        Some(key) => (
            !fit.is_atlas_pick(key),
            !fit.is_preferred_aid_item(key),
            !fit.is_role_item(key),
            fit.timing(key),
        ),
        None => (true, true, true, Timing::Any),
    }
}

// Rules 6 and 9 over the buy detour's picks for the 5th and 6th slots, which
// only exist after [`enforce`]. Stable, so equal picks keep their order.
pub(crate) fn sort_by_timing<T>(picks: &mut [T], fit: Fit, key: impl Fn(&T) -> Option<String>) {
    picks.sort_by_key(|pick| buy_order(key(pick).as_deref(), fit));
}

// The tier-1 boots every upgraded pair builds from.
const BASE_BOOTS: &str = "boots";
const BERSERKERS_GREAVES: &str = "berserkers_greaves";
const BOOTS_OF_SWIFTNESS: &str = "boots_of_swiftness";
const GLUTTONOUS_GREAVES: &str = "gluttonous_greaves";
const IONIAN_BOOTS: &str = "ionian_boots_of_lucidity";
const MERCURYS_TREADS: &str = "mercurys_treads";
const PLATED_STEELCAPS: &str = "plated_steelcaps";
const SORCERERS_SHOES: &str = "sorcerers_shoes";

const UPGRADED_BOOTS: [&str; 7] = [
    BERSERKERS_GREAVES,
    BOOTS_OF_SWIFTNESS,
    GLUTTONOUS_GREAVES,
    IONIAN_BOOTS,
    MERCURYS_TREADS,
    PLATED_STEELCAPS,
    SORCERERS_SHOES,
];

// Rule 7's slot: the second, where League players finish their boots. A pin or
// early item there pushes them to the next open slot.
const BOOTS_SLOT: usize = 1;

pub(crate) fn is_boots(key: &str) -> bool {
    key == BASE_BOOTS || UPGRADED_BOOTS.contains(&key)
}

// Rule 7's boots for `champion` in `role` against `enemies` (empty when
// unseen). A tank answers the enemy's main damage type, other supports take
// Lucidity, AP takes Sorcerer's, and the rest follow their class.
pub(crate) fn boots_for(champion: &str, role: Role, enemies: &[&str]) -> &'static str {
    let traits = champion_traits::traits(champion).unwrap_or_default();
    if traits.tank {
        let (physical, magic) =
            enemies.iter().fold(
                (0, 0),
                |(physical, magic), enemy| match champion_traits::traits(enemy)
                    .and_then(|traits| traits.scaling)
                {
                    Some(Scaling::Ad) => (physical + 1, magic),
                    Some(Scaling::Ap) => (physical, magic + 1),
                    Some(Scaling::Hybrid) => (physical + 1, magic + 1),
                    None => (physical, magic),
                },
            );
        return if magic > physical {
            MERCURYS_TREADS
        } else {
            PLATED_STEELCAPS
        };
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

// Whether `key` is in the editor's Support class, base or radiant —
// [`SUPPORT_ITEM_EXCEPTIONS`] included, unlike [`is_support_item`].
fn is_support_class(key: &str) -> bool {
    crate::item_catalog::category_of(crate::build_config::base_slug(key)) == Some("Support")
}

// Whether `key` is an item only the support role may build: the editor's Support
// class, base or radiant, less [`SUPPORT_ITEM_EXCEPTIONS`].
pub(crate) fn is_support_item(key: &str) -> bool {
    !SUPPORT_ITEM_EXCEPTIONS.contains(&crate::build_config::base_slug(key)) && is_support_class(key)
}

// Rule 14: the items only a marksman keeps, by base slug.
const MARKSMAN_ONLY_ITEMS: [&str; 3] = [
    "guinsoos_rageblade",
    "statikk_shiv",
    "sword_of_blossoming_dawn", // a support item, so a marksman playing support
];

fn is_marksman_only_item(key: &str) -> bool {
    MARKSMAN_ONLY_ITEMS.contains(&crate::build_config::base_slug(key))
}

fn is_mandate(key: &str) -> bool {
    crate::build_config::base_slug(key) == "imperial_mandate"
}

// Rule 13: items whose passive only answers to healing, shielding or buffing an
// ally, by base slug. Dream Maker is one too, handed out by rule 12.
const ALLY_AID_ITEMS: [&str; 4] = [
    "ardent_censer",          // Sanctify, on healing, shielding or buffing an ally
    "echoes_of_helia",        // Soul Siphon spends its charges on the same
    "moonstone_renewer",      // Starlit Grace chains a heal given to an ally
    "staff_of_flowing_water", // Rapids, on healing, shielding or buffing an ally
];

fn is_ally_aid_item(key: &str) -> bool {
    ALLY_AID_ITEMS.contains(&crate::build_config::base_slug(key))
}

// Rule 17: the [`ALLY_AID_ITEMS`] a heal, shield and buff support looks to
// first. Not Moonstone Renewer, which only chains heals and does nothing for a
// kit that shields or buffs.
const PREFERRED_AID_ITEMS: [&str; 3] =
    ["ardent_censer", "echoes_of_helia", "staff_of_flowing_water"];

// Rule 12: World Atlas's upgrades, the support role's own line, by base slug. A
// new one needs a place in [`Fit::atlas_item`].
const ATLAS_ITEMS: [&str; 5] = [
    BLOODSONG,            // Spellblade, for a support none of the others is for
    CELESTIAL_OPPOSITION, // Blessing of the Mountain, on being hit by an enemy champion
    DREAM_MAKER,          // a Dream Bubble, on healing, shielding or buffing an ally
    SOLSTICE_SLEIGH,      // Going Sledding, on immobilizing an enemy champion
    ZAZZAKS_REALMSPIKE,   // Void Explosion, on ability damage to an enemy champion
];

// Rule 12: the World Atlas item of a support none of the others is for.
const BLOODSONG: &str = "bloodsong";

// Rule 12: the World Atlas item of a tank.
const CELESTIAL_OPPOSITION: &str = "celestial_opposition";

// Rule 12: the World Atlas item of a support that heals or shields.
const DREAM_MAKER: &str = "dream_maker";

// Rule 12: champions whose World Atlas item is named outright. Chef and Monk
// are `Tank`s that aid allies, which the tests would hand Celestial Opposition
// or Solstice Sleigh.
const ATLAS_BY_CHAMPION: [(&str, &str); 2] = [("chef", DREAM_MAKER), ("monk", DREAM_MAKER)];

// Rule 12: the World Atlas item of a tank that immobilizes.
const SOLSTICE_SLEIGH: &str = "solstice_sleigh";

// Rule 12: how long a tank's kit must immobilize in all for Solstice Sleigh:
// one second, in ticks.
const SLEIGH_IMMOBILIZE_TICKS: usize = 60;

// Rule 12: the World Atlas item of a mage, and of any other AP support that
// only deals damage.
const ZAZZAKS_REALMSPIKE: &str = "zazzaks_realmspike";

pub(crate) fn is_atlas_item(key: &str) -> bool {
    ATLAS_ITEMS.contains(&crate::build_config::base_slug(key))
}

// [`is_support_item`] less the World Atlas item: what rules 9 and 10 mean by a
// support item.
fn is_other_support_item(key: &str) -> bool {
    is_support_item(key) && !is_atlas_item(key)
}

// [`is_support_class`], less rule 12's World Atlas item.
fn is_other_support_class(key: &str) -> bool {
    is_support_class(key) && !is_atlas_item(key)
}

// Rule 8: items only the jungle role may build, by base slug.
const JUNGLE_ITEMS: [&str; 3] = [
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
    PHILOSOPHERS_STONE,       // maximum health per takedown and monster killed
];

// Rule 8: the jungle item of a `Tank`; the other two go by damage type.
const PHILOSOPHERS_STONE: &str = "philosophers_stone";

pub(crate) fn is_jungle_item(key: &str) -> bool {
    JUNGLE_ITEMS.contains(&crate::build_config::base_slug(key))
}

fn is_philosophers_stone(key: &str) -> bool {
    crate::build_config::base_slug(key) == PHILOSOPHERS_STONE
}

// Rule 11: items whose passive wants the carrier in melee, by base slug.
const MELEE_ITEMS: [&str; 9] = [
    "ravenous_hydra",     // Cleave at half strength from past 35 range
    "titanic_hydra",      // Cleave at half strength from past 35 range
    "hullbreaker",        // Skipper at 70% strength from past 35 range
    "heartsteel",         // Ironheart charges on enemies that stay within 50 range
    "abyssal_mask",       // Unmake curses enemy champions within 50 range
    "hollow_radiance",    // Immolate burns enemies within 30 range
    "philosophers_stone", // Immolate again
    "sunfire_cape",       // Immolate again; this slug is the radiant cape's
    // The vanilla Sunfire Cape's own key, which `base_slug` does not alias.
    "hourglass_of_eternity",
];

// Rule 11: items whose passive wants the carrier at range, by base slug.
const RANGED_ITEMS: [&str; 2] = [
    "runaans_hurricane", // bolts at the enemies around the carrier; ranged only in League
    "diamond_tipped_spear", // Sweet Spot grows with the distance to the target, up to 100 range
];

fn is_melee_item(key: &str) -> bool {
    MELEE_ITEMS.contains(&crate::build_config::base_slug(key))
}

fn is_ranged_item(key: &str) -> bool {
    RANGED_ITEMS.contains(&crate::build_config::base_slug(key))
}

// Rule 15: the finished Spellblade items, by base slug (Sheen, their component,
// is not one).
const SPELLBLADE_ITEMS: [&str; 6] = [
    "trinity_force",
    "dusk_and_dawn",
    "lich_bane",
    "essence_reaver",
    "iceborn_gauntlet",
    BLOODSONG,
];

fn is_spellblade_item(key: &str) -> bool {
    SPELLBLADE_ITEMS.contains(&crate::build_config::base_slug(key))
}

// What a build's items have spent of the rules' budgets, plus its [`Fit`].
// Holds the trait table so a catalog scan is hash lookups, not lock
// acquisitions.
pub(crate) struct Budget {
    table: Arc<Table>,
    cuts_healing: bool,
    crit_chance: i32,
    // Rule 8: whether the build already holds a jungle item.
    jungle_item: bool,
    // Rule 12: whether the build already holds a World Atlas item.
    atlas_item: bool,
    // Rule 15: whether the build already holds a Spellblade item.
    spellblade_item: bool,
    fit: Fit,
}

impl Budget {
    pub(crate) fn empty(fit: Fit) -> Self {
        Self {
            table: table(),
            cuts_healing: false,
            crit_chance: 0,
            jungle_item: false,
            atlas_item: false,
            spellblade_item: false,
            fit,
        }
    }

    pub(crate) fn spent<'a>(keys: impl IntoIterator<Item = &'a str>, fit: Fit) -> Self {
        let mut budget = Self::empty(fit);
        for key in keys {
            budget.take(key);
        }
        budget
    }

    // Why `key` cannot join the build, or `None`. Duplicates are the caller's
    // to spot.
    pub(crate) fn rejects(&self, key: &str) -> Option<Reason> {
        let traits = self.table.traits(key);
        if !self.fit.support_items && is_support_item(key) {
            Some(Reason::SupportOnly)
        } else if self.fit.off_role_jungle_item(key) {
            Some(Reason::JungleOnly)
        } else if self.fit.other_jungle_item(key) {
            Some(Reason::JungleMismatch)
        } else if self.fit.other_atlas_item(key) {
            Some(Reason::AtlasMismatch)
        } else if self.fit.no_mandate && is_mandate(key) {
            Some(Reason::MandateWithoutCc)
        } else if self.fit.no_ally_aid && is_ally_aid_item(key) {
            Some(Reason::AllyAidWithoutAid)
        } else if self.fit.off_support_classes(key) {
            Some(Reason::SupportClasses)
        } else if self.fit.wants_marksman(key) {
            Some(Reason::MarksmanOnly)
        } else if self.fit.mismatches(&traits) {
            Some(Reason::Scaling)
        } else if self.fit.out_of_reach(key) {
            // After rule 5, so an item failing both (Runaan's on a melee mage)
            // is replaced in the build's style.
            Some(Reason::Reach)
        } else if (self.jungle_item || self.fit.no_starter) && is_jungle_item(key) {
            // After the champion rules: a jungle item this jungler could hold
            // only as the build's first, and none when the first slot is pinned
            // (rule 9).
            Some(Reason::SecondJungle)
        } else if (self.atlas_item || self.fit.no_starter) && self.fit.is_atlas_pick(key) {
            // The same for a support's World Atlas item (rule 12).
            Some(Reason::SecondAtlas)
        } else if is_spellblade_item(key)
            && !self.fit.is_atlas_pick(key)
            && (self.spellblade_item
                || (self.fit.holds_bloodsong() && !self.atlas_item && !self.fit.no_starter))
        {
            // Rule 15. A support's Bloodsong is rule 12's, so it is never the
            // one turned away, and a build still due one counts as holding it.
            Some(Reason::SecondSpellblade)
        } else if self.cuts_healing && traits.cuts_healing {
            Some(Reason::Grievous)
        } else if self.crit_chance + traits.crit_chance > CRIT_CAP {
            Some(Reason::Crit)
        } else {
            None
        }
    }

    // Whether this champion may hold `key` at all, by the rules that do not
    // depend on the rest of the build. An item that fails is no guide to the
    // build's style.
    pub(crate) fn suits_champion(&self, key: &str) -> bool {
        !(!self.fit.support_items && is_support_item(key))
            && !self.fit.off_role_jungle_item(key)
            && !self.fit.other_jungle_item(key)
            && !self.fit.other_atlas_item(key)
            && !(self.fit.no_mandate && is_mandate(key))
            && !(self.fit.no_ally_aid && is_ally_aid_item(key))
            && !self.fit.off_support_classes(key)
            && !self.fit.wants_marksman(key)
            && !self.fit.mismatches(&self.table.traits(key))
            && !self.fit.out_of_reach(key)
    }

    // Whether `key` brings any crit, which a crit overflow's stand-in must not.
    pub(crate) fn adds_crit(&self, key: &str) -> bool {
        self.table.traits(key).crit_chance > 0
    }

    // Whether `key` may stand in for an item rejected for `reason`.
    pub(crate) fn accepts_instead(&self, key: &str, reason: Reason) -> bool {
        self.rejects(key).is_none() && (reason != Reason::Crit || !self.adds_crit(key))
    }

    pub(crate) fn take(&mut self, key: &str) {
        let traits = self.table.traits(key);
        self.cuts_healing |= traits.cuts_healing;
        self.crit_chance += traits.crit_chance;
        self.jungle_item |= is_jungle_item(key);
        self.atlas_item |= is_atlas_item(key);
        self.spellblade_item |= is_spellblade_item(key);
    }
}

// Rewrites `build` in place so it breaks none of the rules, as far as the
// catalog allows. An offending AI slot is swapped for the next unused final
// that fixes it, wrapping from its index, from its own category or, for
// [`Reason::restyles`], the build's style; with none, it stays.
//
// `pinned` marks the player's slots, never rewritten, and `reserved` holds
// their pins past the end of `build`; both are counted first, so an AI pick
// that clashes with a pin is the one that goes. `key`, `category` and
// `is_final` describe a `count`-item catalog, `boots` is [`boots_for`]'s pair,
// and `later_open` says a 5th or 6th slot is free for it
// ([`crate::build_config::later_slot_open`]).
//
// After the walk come rules 12, 9, 10 and 13 (each swapping one AI pick), then
// rule 6's order, rule 7's boots and rule 6's slot line.
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
    let is_pinned = |slot: usize| pinned.get(slot).copied().unwrap_or(false);
    // A jungler's jungle item and a support's World Atlas item go in the
    // first slot or not at all (rules 9 and 12).
    let fit = if is_pinned(0) {
        fit.starter_slot_pinned()
    } else {
        fit
    };
    let mut budget = Budget::empty(fit);
    let mut seen: HashSet<usize> = HashSet::new();

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

    // The build's style: the categories of its items that suit the champion,
    // earliest first, taken before any slot changes. A mage support's support
    // items set no style (rule 9).
    let sets_style = |key: &str| budget.suits_champion(key) && !(fit.mage && is_support_class(key));
    let mut styles: Vec<C> = Vec::new();
    for &index in build.iter() {
        if !key(index).is_some_and(|key| sets_style(&key)) {
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

        // One assignment for every outcome, so the budget cannot drift from the
        // build.
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
                // Rule 17: a heal, shield and buff support's stand-in is a
                // preferred aid item, unless the offender is a wrong World
                // Atlas item.
                let aid_item = (fit.prefers_aid_items() && reason != Reason::AtlasMismatch)
                    .then(|| {
                        search(&|candidate| {
                            key(candidate).is_some_and(|key| fit.is_preferred_aid_item(&key))
                        })
                    })
                    .flatten();
                if let Some(aid_item) = aid_item {
                    aid_item
                } else if reason.restyles() {
                    // A role item failing rule 5 or 8, a wrong World Atlas
                    // item, or any pick of a support other than a mage makes
                    // way for a role item first (rules 9 and 12), which the
                    // build's style would not reach.
                    let atlas = current.as_deref().is_some_and(|key| fit.is_atlas_pick(key));
                    let role_first = atlas
                        || fit.prefers_support_items()
                        || current.as_deref().is_some_and(|key| fit.is_role_item(key));
                    let role_item = role_first
                        .then(|| {
                            search(&|candidate| {
                                key(candidate).is_some_and(|key| {
                                    if atlas {
                                        fit.is_atlas_pick(&key)
                                    } else {
                                        fit.is_role_item(&key)
                                    }
                                })
                            })
                        })
                        .flatten();
                    role_item
                        .or_else(|| {
                            styles.iter().find_map(|style| {
                                search(&|candidate| category(candidate).as_ref() == Some(style))
                            })
                        })
                        // With no usable style left, the item still goes.
                        .or_else(|| search(&|_| true))
                        .unwrap_or(offender)
                } else {
                    // The category must be known: `None` matching `None` would
                    // accept any unclassified item.
                    let own_category = category(offender).and_then(|wanted| {
                        search(&|candidate| category(candidate).as_ref() == Some(&wanted))
                    });
                    // A jungle item off the jungle goes even when its category
                    // holds nothing for the champion (Feral Flare on a mage):
                    // it follows the build's style, then any category.
                    let elsewhere = || {
                        (reason == Reason::JungleOnly)
                            .then(|| {
                                styles
                                    .iter()
                                    .find_map(|style| {
                                        search(&|candidate| {
                                            category(candidate).as_ref() == Some(style)
                                        })
                                    })
                                    .or_else(|| search(&|_| true))
                            })
                            .flatten()
                    };
                    own_category.or_else(elsewhere).unwrap_or(offender)
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

    // Rule 12: a support with no World Atlas item has the AI's last pick,
    // preferably not a rule 9 role item, swapped for its champion's (the only
    // one the budget allows). Ahead of rules 9, 10 and 13, which look past it.
    let needs_atlas_item = fit.support_items
        && !fit.no_starter
        && !build
            .iter()
            .chain(reserved)
            .any(|&index| key(index).is_some_and(|key| is_atlas_item(&key)));
    if needs_atlas_item {
        let last_pick_but = |spare_role_item: bool| {
            (0..build.len()).rev().find(|&slot| {
                !is_pinned(slot)
                    && !key(build[slot]).is_some_and(|key| {
                        is_boots(&key) || (spare_role_item && fit.is_role_item(&key))
                    })
            })
        };
        if let Some(slot) = last_pick_but(true).or_else(|| last_pick_but(false)) {
            let rest = budget_without(build, slot);
            let atlas_item = (0..count).find(|&candidate| {
                is_final(candidate)
                    && !build.contains(&candidate)
                    && !reserved.contains(&candidate)
                    && key(candidate).is_some_and(|candidate| {
                        is_atlas_item(&candidate) && rest.rejects(&candidate).is_none()
                    })
            });
            if let Some(atlas_item) = atlas_item {
                build[slot] = atlas_item;
            }
        }
    }

    // Rule 9: a jungler with no jungle item has its last AI pick, the one the
    // engine wanted least, swapped for one judged against the rest of the
    // build.
    let needs_role_item = fit.guarantees_role_item()
        && !build
            .iter()
            .chain(reserved)
            .any(|&index| key(index).is_some_and(|key| fit.is_role_item(&key)));
    if needs_role_item {
        let last_pick = (0..build.len()).rev().find(|&slot| {
            !is_pinned(slot)
                && !key(build[slot]).is_some_and(|key| is_boots(&key) || fit.is_atlas_pick(&key))
        });
        if let Some(slot) = last_pick {
            let offender = build[slot];
            let rest = budget_without(build, slot);
            let role_item = (1..count)
                .map(|step| (offender + step) % count)
                .find(|&candidate| {
                    !seen.contains(&candidate)
                        && !build.contains(&candidate)
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

    // Rule 10 only decides which of the AI's support items becomes Mandate: no
    // AI support item, or a pinned one, means no Mandate. The World Atlas item
    // is not that support item.
    let holds = |index: usize, want: fn(&str) -> bool| key(index).is_some_and(|key| want(&key));
    let pinned_support = (0..build.len())
        .filter(|&slot| is_pinned(slot))
        .map(|slot| build[slot])
        .chain(reserved.iter().copied())
        .any(|index| holds(index, is_other_support_class));
    let has_mandate = build
        .iter()
        .chain(reserved)
        .any(|&index| holds(index, is_mandate));
    if fit.mandate && !has_mandate && !pinned_support {
        let ai_slot_holding = |want: fn(&str) -> bool| {
            (0..build.len()).find(|&slot| !is_pinned(slot) && holds(build[slot], want))
        };
        if let Some(slot) = ai_slot_holding(is_other_support_item)
            .or_else(|| ai_slot_holding(is_other_support_class))
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

    // Rule 13, after rule 10 and likewise: the AI's first support item that is
    // neither Mandate nor the Atlas one makes way for an ally-aid item.
    let has_ally_aid = build
        .iter()
        .chain(reserved)
        .any(|&index| holds(index, is_ally_aid_item));
    if fit.ally_aid && !has_ally_aid && !pinned_support {
        let spare = (0..build.len()).find(|&slot| {
            !is_pinned(slot)
                && key(build[slot])
                    .is_some_and(|key| is_other_support_class(&key) && !is_mandate(&key))
        });
        if let Some(slot) = spare {
            let offender = build[slot];
            let rest = budget_without(build, slot);
            let aid_item = (1..count)
                .map(|step| (offender + step) % count)
                .find(|&candidate| {
                    is_final(candidate)
                        && !build.contains(&candidate)
                        && !reserved.contains(&candidate)
                        && key(candidate).is_some_and(|candidate| {
                            is_ally_aid_item(&candidate) && rest.rejects(&candidate).is_none()
                        })
                });
            if let Some(aid_item) = aid_item {
                build[slot] = aid_item;
            }
        }
    }

    // Rule 6, behind rule 12's World Atlas item and rule 9's role items; a
    // support is exempt ([`Fit::timing`]). Stable, so equal timings keep the
    // engine's order.
    let open: Vec<usize> = (0..build.len()).filter(|&slot| !is_pinned(slot)).collect();
    let mut picks: Vec<usize> = open.iter().map(|&slot| build[slot]).collect();
    picks.sort_by_key(|&index| buy_order(key(index).as_deref(), fit));
    // An early item the first two slots cannot hold goes to the back, where
    // rule 7's boots push it off or the slot line below swaps it. A role item
    // keeps its place.
    let overflows = |pick: usize, at: usize| {
        open[at] >= EARLY_SLOTS
            && key(pick).is_some_and(|key| {
                fit.timing(&key) == Timing::Early && !fit.is_role_item(&key)
            })
    };
    let (mut kept, mut overflow) = (Vec::new(), Vec::new());
    for (at, &pick) in picks.iter().enumerate() {
        if overflows(pick, at) {
            overflow.push(pick);
        } else {
            kept.push(pick);
        }
    }
    kept.append(&mut overflow);
    let mut picks = kept;

    // Rule 7.
    let has_boots = build
        .iter()
        .chain(reserved)
        .any(|&index| key(index).is_some_and(|key| is_boots(&key)));
    // Where the boots go in `picks`: the first open slot from the second on,
    // else none when a 5th or 6th is open (the buy detour places them there),
    // else the first.
    let at = match (0..open.len()).find(|&pick| open[pick] >= BOOTS_SLOT) {
        Some(at) => Some(at),
        None if later_open => None,
        None => (!open.is_empty()).then_some(0),
    };
    // An early item in one of the first two slots keeps it, and the boots go
    // after it.
    let at = at.map(|mut at| {
        while at < picks.len()
            && open[at] < EARLY_SLOTS
            && key(picks[at]).is_some_and(|key| fit.timing(&key) == Timing::Early)
        {
            at += 1;
        }
        at
    });
    if let (Some(boots), Some(at), false) = (boots, at, has_boots) {
        picks.insert(at, boots);
        picks.truncate(open.len());
    }

    for (&slot, index) in open.iter().zip(picks) {
        build[slot] = index;
    }

    // Rule 6's slot line: a pick still on the wrong side makes way for an item
    // of no timing, from its own category, then the build's style, then
    // anything. A role item stays.
    for &slot in &open {
        let offender = build[slot];
        let Some(current) = key(offender) else {
            continue;
        };
        if fit.fits_slot(&current, slot) || fit.is_role_item(&current) {
            continue;
        }
        let rest = budget_without(build, slot);
        let search = |matches: &dyn Fn(usize) -> bool| {
            (1..count)
                .map(|step| (offender + step) % count)
                .find(|&candidate| {
                    is_final(candidate)
                        && !build.contains(&candidate)
                        && !reserved.contains(&candidate)
                        && matches(candidate)
                        && key(candidate).is_some_and(|candidate| {
                            fit.timing(&candidate) == Timing::Any
                                && !is_boots(&candidate)
                                && rest.rejects(&candidate).is_none()
                        })
                })
        };
        let in_category =
            |wanted: &C| search(&|candidate| category(candidate).as_ref() == Some(wanted));
        let stand_in = category(offender)
            .and_then(|own| in_category(&own))
            .or_else(|| styles.iter().find_map(in_category))
            .or_else(|| search(&|_| true));
        if let Some(stand_in) = stand_in {
            build[slot] = stand_in;
        }
    }
}
