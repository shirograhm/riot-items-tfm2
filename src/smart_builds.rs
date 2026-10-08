//! The Smart Builds rules: what the editor's footer toggle
//! ([`crate::build_config::smart_builds_enabled`]) enforces on a build.
//!
//! Seventeen of them:
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
//!    champion. Zeke's Convergence was a second exception until 2026-10-08,
//!    when the user had it made a support item like the rest.
//! 5. **Items the champion scales with** — an AP-only champion keeps no item
//!    that gives a physical stat (attack, attack speed or crit), and an
//!    AD-only champion none that gives magic power. That takes a hybrid item,
//!    one that gives both, off either: it is for a hybrid champion (the user,
//!    2026-10-05; until then hybrid items were never touched). Hextech
//!    Gunblade is the only one: the AP items that give attack speed or crit
//!    count as AP all the same ([`AP_ITEMS`]), and Guinsoo's Rageblade and
//!    Statikk Shiv are rule 14's. Hybrid champions and items with no
//!    offensive stat are never touched.
//!    Support items get no pass in the support role: an AD support keeps the
//!    tank ones, not the AP ones. Rule 12's item is the exception, and Sword
//!    of Blossoming Dawn is rule 14's.
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
//! 8. **Jungle items stay in the jungle, one to a build** — Feral Flare,
//!    Grez's Spectral Lantern and Philosopher's Stone ([`JUNGLE_ITEMS`]) grow
//!    on monster kills, which only the jungle role gets, so only whoever plays
//!    jungle keeps them. A rule like the others: it is the toggle's to
//!    switch off, and it never touches a pin, so a jungle item the player
//!    pinned stays wherever its champion is played. For a day (2026-10-07)
//!    it held with the toggle off and blanked such a pin; the user had both
//!    taken back out (2026-10-08). And a jungler is suggested one, the one
//!    for its champion: Philosopher's Stone on a champion tagged `Tank`, else Feral
//!    Flare or Grez's by damage type (rule 5). The AI's other jungle items
//!    make way, all of them when the player pinned one: a pin is that build's
//!    jungle item, and pins may hold as many as the player likes. Numbered
//!    after 7 only so the rules above keep the numbers the rest of the mod
//!    cites them by.
//! 9. **Role items first** — a jungler's build holds a jungle item, the one
//!    rule 8 names, bought before the AI's other picks. Only one is
//!    guaranteed: the rest of the build is still whatever the AI chose. When
//!    neither a pin nor an AI pick is one, the AI's last pick makes way for
//!    it. A support is guaranteed nothing here: its World Atlas item (rule
//!    12) is its one dedicated support item (the user, 2026-10-04). Past
//!    that, a support *prefers* Support-class items (the exception to rule
//!    4 included) without being held to one: the item-build hook pushes
//!    them harder than the mod's other items, the AI's picks among them are
//!    bought right after the Atlas item, and they are the first place a
//!    stand-in is looked for when one of its AI picks does not suit the
//!    champion (rules 5, 10 and 13). Rule 5 still matches
//!    them to the champion: an AD support prefers the tank ones, never the
//!    AP ones. Everything else follows the champion's class, as the engine
//!    picks it. A mage (rule 12) prefers none: past its World Atlas item it
//!    builds AP items like any other mage.
//! 10. **Imperial Mandate for supports that immobilize** — a support whose
//!    kit has a stun, root, knock-up, knockback, pull, taunt, fear or charm
//!    makes Mandate its support item, since its passive pays off on exactly
//!    those: the AI's first support item gives way to it. A build the AI gave
//!    no support item gets none: this only picks which one (rule 9). A slow does not
//!    count, nor a silence or a disarm, so the game's `CC` tag only decides for
//!    a champion whose kit is not known (`ChampionTraits::can_immobilize`). A
//!    support item the player pinned is their choice and stays, a build that
//!    already holds Mandate is left alone, and the other rules still judge it
//!    (Mandate is an AP item, so rule 5 keeps it off an AD support). The other
//!    way round, a support known not to immobilize never keeps an AI's
//!    Mandate: it makes way for another support item, and is never a stand-in
//!    in that build. A champion nothing is known about is left alone either
//!    way.
//! 11. **Items for the champion's reach** — a ranged champion keeps no item
//!    whose passive wants its carrier in melee ([`MELEE_ITEMS`]: the Hydras'
//!    Cleave and Hullbreaker's Skipper weaken from past 35 range, and
//!    Heartsteel, Abyssal Mask and the Immolate auras need the enemy closer
//!    than a ranged champion stands), and a melee champion none that wants
//!    its carrier at range ([`RANGED_ITEMS`]: Runaan's Hurricane, Diamond
//!    Tipped Spear).
//!    Melee is a basic attack that reaches 35 or less, the same line those
//!    items draw. The stand-in comes from the item's own category, like a
//!    duplicate's. A champion whose reach nothing states is left alone.
//! 12. **A World Atlas item for every support** — World Atlas is the support
//!    role's starting item, and what it grows into ([`ATLAS_ITEMS`]) is to a
//!    support what a jungle item is to a jungler: its build holds one, the one
//!    for its champion, bought before everything else, so the match starts on
//!    World Atlas and its gold. Which one, in the order the user set out
//!    (2026-10-08): Solstice Sleigh on a champion tagged `Tank` whose kit
//!    immobilizes for a second or more in all, ultimate included, which is
//!    what Going Sledding pays off on; else Celestial Opposition on any
//!    other champion tagged `Tank`, the one the enemy hits, which is what
//!    Blessing of the Mountain answers to; else Dream Maker on a champion
//!    whose kit heals, shields or buffs allies, whose casts on them are what
//!    blow its bubbles (rule 13's test, so not a mage: rules 9 and 13 pass a
//!    mage over, and a Lux that shields still casts to deal damage); else
//!    Zaz'Zak's Realmspike on a mage (the game's `Magician` class), whose
//!    abilities are there to hurt; else Zaz'Zak's Realmspike on any other AP
//!    champion, for the same reason; else Bloodsong, whose Spellblade any
//!    cast readies. The AI's
//!    other Atlas items make way, all of them when the player pinned one. A
//!    build with none, pinned or picked, has the AI's
//!    last pick swapped for it, one that is not a support item rule 9 prefers
//!    where there is a choice. It is for the role, not the damage type: rule 5
//!    does not judge it, and rules 9, 10 and 13 look past it, so the support
//!    items they prefer and swap are the others. Off the support role it is a
//!    support item like any other (rule 4).
//! 13. **Heal, shield and buff items for the champions that do that** — four
//!    support items only answer to their carrier healing, shielding or
//!    buffing an ally ([`ALLY_AID_ITEMS`]: Ardent Censer, Echoes of Helia,
//!    Moonstone Renewer, Staff of Flowing Water). A support whose kit does
//!    that with a basic ability makes one its support item: with none in the
//!    build, and no support item the player pinned, the AI's first support
//!    item that is neither Mandate nor the Atlas one makes way for it. Like
//!    rule 10 it only picks which support item, so a build with none to spare
//!    gets none (until 2026-10-04 the AI's last pick made way instead, which
//!    made it a dedicated support item, and only the Atlas one is now). A
//!    support known not to never keeps one
//!    the AI picked, and neither does a mage (rule 12), whatever its kit
//!    holds. What a kit does is read off the kit where it is known
//!    (`ChampionTraits::aids_allies`), not off the `Heal` and `Shield` tags,
//!    which a Vampire that heals only itself carries and a Bard that only
//!    buffs does not. These items are for what the champion casts, not what
//!    it scales with, so rule 5 does not judge them.
//! 14. **Guinsoo's Rageblade and Statikk Shiv for marksmen** — both give
//!    attack damage and ability power beside their attack speed, which made
//!    them hybrid items. The user took them out of those and gave them to
//!    marksmen alone (2026-10-05): only a champion of the game's `Range`
//!    class keeps one ([`MARKSMAN_ONLY_ITEMS`]), whatever it scales with, so
//!    rule 5 does not judge them. The stand-in follows the build's style,
//!    like rule 5's. A champion whose class nothing states is left alone.
//!    Sword of Blossoming Dawn joined them the next day: it heals an ally for
//!    every basic attack, so the user gave it to the supports that attack,
//!    "support marksmen/attackspeed champs". It is a support item, so rule 4
//!    keeps it in the support role, and a marksman there keeps it whether it
//!    is AD or AP. No tag or class says "attack speed champion", so the
//!    `Range` class is all of that test for now.
//! 15. **One Spellblade item** — Trinity Force, Dusk and Dawn, Lich Bane,
//!    Essence Reaver, Iceborn Gauntlet and Bloodsong ([`SPELLBLADE_ITEMS`])
//!    share one cooldown, so a build holds one of them (the user,
//!    2026-10-06). Sheen is not counted: it is the component they are built
//!    from, not a finished item. The AI's second makes way for an item of its
//!    own category, like a duplicate. A support whose World Atlas item is
//!    Bloodsong (rule 12) has its one already: it keeps no other the AI
//!    picked, and that Bloodsong is never the one that goes.
//!
//! 16. **Tank and support items for tank supports** — a support with the
//!    game's `Tank` tag builds items of the editor's Tank and Support classes
//!    and no others, whatever its damage type (the user, 2026-10-08, after a
//!    Shield Bearer support was seen holding damage items). Boots are rule
//!    7's. The game's own six finals have a class through the names the
//!    mod gives them (`base_slug`: Luden's Tempest is a Mage item, Thornmail
//!    a Tank one). An offender makes way the way rule 5's does: for an item
//!    of the kind the rest of the build is made of.
//! 17. **Tank and support items for heal, shield and buff supports** — a
//!    support whose World Atlas item is Dream Maker (rule 12) is held to the
//!    same two classes (the user, 2026-10-08: "heal/shield/buff supports
//!    (dream maker builders)"), and among them looks to the items that
//!    answer to healing, shielding and buffing first, bar Moonstone
//!    Renewer, which only answers to healing ([`PREFERRED_AID_ITEMS`]):
//!    they are where a stand-in for any of its picks is looked for before
//!    anywhere else, the AI buys them right after the Atlas item, the
//!    item-build hook pushes them hardest, and the automatic 5th and 6th
//!    items come from them while one is left. A preference, not a quota: a
//!    tank or support item the AI picked itself stays.
//!
//! The rules only ever replace what the AI picked. A slot the player pinned in
//! the editor is kept whatever it holds, and counts toward the budgets like any
//! other item, so the AI's picks around it make way for it rather than the
//! other way round.
//!
//! Rules 4, 5, 8 to 14, 16 and 17 are about the champion, not the build, and come in
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
use crate::champion_traits::{self, Class, Scaling};

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

    /// Whether these say the item gives no offensive stat of any kind.
    fn blank(&self) -> bool {
        !self.physical && !self.magic && self.crit_chance == 0
    }
}

/// Item traits by key, filled from the two places items are described.
#[derive(Clone, Default)]
struct Table {
    /// This mod's items, recorded as `init` registers them. Authoritative for
    /// its own keys, because the values there are the configured ones.
    mod_items: HashMap<String, ItemTraits>,
    /// The game's items, from the mod's settings file at start-up
    /// (`item_stats::prime_game_items`), and other mods' from the client's
    /// settings document. No vanilla item cuts healing; the stats are read
    /// rather than hardcoded because they are config-editable through
    /// `item_setting`.
    engine_items: HashMap<String, ItemTraits>,
}

/// The game's own items that give attack, attack speed, crit or ability
/// power, at the values this mod ships for them (`setting/item_setting`): key,
/// crit chance, attack, attack speed, magic power. The other fifteen (the
/// armor, magic resistance and health lines) give none of the four, which
/// are all the rules read off an item's stats, so a row for one would be
/// four zeros and decide nothing: no row is what says so.
///
/// The last resort. The rules learn all thirty at start-up from the mod's
/// settings file itself (`item_stats::prime_game_items`), with whatever the
/// player's config made of them, and this only answers when that file could
/// not be read, or when a record of one of these comes out blank
/// ([`Table::traits`]). Without it the rules once took Luden's Tempest for an
/// item with no ability power, and handed it to AD champions as a 6th item
/// (2026-10-08). Kept in step with the file by hand: it is fifteen numbers a
/// balance pass rarely moves across zero, which is all that matters here
/// outside the crit of the attack speed line.
const GAME_ITEMS: [(&str, i32, i32, i32, i32); 15] = [
    ("ironsword", 0, 10, 0, 0),
    ("soldiers_longsword", 0, 20, 0, 0),
    ("ruinous_blade", 0, 25, 0, 0),
    ("conquerors_greatsword", 0, 40, 0, 0),
    ("warlords_final_judgement", 0, 50, 0, 0),
    ("dagger", 0, 0, 10, 0),
    ("wind_dagger", 0, 0, 20, 0),
    ("twin_stormblade", 10, 0, 30, 0),
    ("thunderclaw", 20, 0, 45, 0),
    ("storm_sovereign", 25, 0, 60, 0),
    ("arcane_crystal", 0, 0, 0, 15),
    ("spirit_crystal", 0, 0, 0, 30),
    ("staff_of_rapture", 0, 0, 0, 50),
    ("angels_fang", 0, 0, 0, 75),
    ("prophet_of_the_abyss", 0, 0, 0, 100),
];

/// What [`GAME_ITEMS`] says of `key`, where it is one of them.
fn game_item_traits(key: &str) -> Option<ItemTraits> {
    let &(_, crit, attack, speed, power) = GAME_ITEMS.iter().find(|(item, ..)| *item == key)?;
    Some(ItemTraits::from_stats(crit, attack, speed, power))
}

impl Table {
    /// This mod's own record of the item, then the settings document's, then
    /// [`GAME_ITEMS`]. An item none of them describes has no traits, which no
    /// rule rejects.
    ///
    /// A record that finds no offensive stat on one of the game's damage
    /// items is taken for a failed reading, not for an item that lost them,
    /// and [`GAME_ITEMS`] answers instead. The fallback alone was not enough:
    /// with it in, the rules still held blank records of Radiant Bloodthirster
    /// and Radiant Phantom Dancer (the test log, 2026-10-08), and rule 5 let
    /// the first be an AP champion's 6th item. How a blank got recorded is
    /// not known yet; `item_stats::prime_item_traits` now says what it reads.
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
    if is_ap_item(key) {
        traits.physical = false;
    }
    // Rule 12's item is every support's, whatever the champion scales with:
    // its stats are not what it is bought for, so rule 5 has no say over it.
    // Rule 13's items neither: they are for what the champion casts, and an
    // AD support that shields an ally wants Ardent Censer whatever its ability
    // power is worth to it. Rule 14's are judged by the champion's class.
    if is_atlas_item(key) || is_ally_aid_item(key) || is_marksman_only_item(key) {
        traits.physical = false;
        traits.magic = false;
    }
    edit_table(|table| {
        table.mod_items.insert(key.to_string(), traits);
    });
}

/// Rule 5: items counted as AP only, whatever physical stat they also carry.
/// By base slug, so the radiant tier follows. Sword of Blossoming Dawn was
/// the first one here (2026-09-26) and is a marksman's item now (rule 14,
/// [`MARKSMAN_ONLY_ITEMS`]); Bloodsong was here with it until it lost its
/// ability power and became the World Atlas item (rule 12). Dusk and Dawn,
/// Lich Bane and Nashor's Tooth give attack speed beside their ability power
/// and are no hybrid items: the user said so the day hybrid items became
/// hybrid champions' only (2026-10-05), and
/// that of the Mage items only Hextech Gunblade is one, which gives attack
/// damage. Rite of Ruin's crit is spent by its own passive, a shield rolled
/// on landing an ability, so it is a mage's item as well (the same day; put
/// here on my reading, the user did not name it).
const AP_ITEMS: [&str; 4] = [
    "dusk_and_dawn",
    "lich_bane",
    "nashors_tooth",
    "rite_of_ruin",
];

/// Whether `key` is one of [`AP_ITEMS`], base or radiant.
fn is_ap_item(key: &str) -> bool {
    AP_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Adds the crit chance an item's passive grants at full stacks to what
/// [`note_mod_item`] recorded from its flat stats. Called right after it, from
/// the `passive_crit` arm of the registration macros in `lib.rs`.
pub(crate) fn note_passive_crit(key: &str, crit_chance: i32) {
    edit_table(|table| {
        let traits = table.mod_items.entry(key.to_string()).or_default();
        traits.crit_chance += crit_chance;
        traits.physical |= crit_chance > 0 && !is_ap_item(key);
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
        // The document is walked in its own order, and a second object under
        // the same key that carries no stats must not undo the first.
        let known = table.engine_items.entry(key.to_string()).or_insert(traits);
        if !traits.blank() {
            *known = traits;
        }
    });
}

/// What the rules make of `key` on `champion` in `role`, for the Check Tactics
/// test log: the stats the item is taken to give (`P` physical, `M` magic) and
/// why an empty build would turn it away, if it would.
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

/// What `champion` is taken to scale with, for the same log.
pub(crate) fn explain_champion(champion: &str, role: Role) -> String {
    format!("{:?}", fit(champion, role).scaling)
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
    /// A jungler holding a jungle item that is not its champion's.
    JungleMismatch,
    /// A jungle item in a build that already holds one.
    SecondJungle,
    /// A support holding a World Atlas item that is not its champion's.
    AtlasMismatch,
    /// A World Atlas item in a build that already holds one.
    SecondAtlas,
    /// A Spellblade item in a build that already holds one (rule 15).
    SecondSpellblade,
    MandateWithoutCc,
    /// An item that answers to healing, shielding or buffing an ally, on a
    /// support whose kit does none of that.
    AllyAidWithoutAid,
    /// An item only a marksman keeps (rule 14) on a champion that is none.
    MarksmanOnly,
    /// An item that is neither a tank's nor a support's, on a support held
    /// to those two classes: one with the `Tank` tag (rule 16), or one whose
    /// World Atlas item is Dream Maker (rule 17).
    SupportClasses,
    Reach,
}

impl Reason {
    /// Whether the offending item's own category is the wrong place to look
    /// for its stand-in. A support item's category holds nothing but support
    /// items, and an item the champion does not scale with sits among others
    /// it does not scale with, so for these two the stand-in is taken from the
    /// categories the rest of the build uses instead. Imperial Mandate on a
    /// support that cannot immobilize goes the same way: it is a support item,
    /// so another support item stands in first, and an item that answers to
    /// aiding allies on a support that aids none goes with it. So does a
    /// jungler's wrong
    /// jungle item, for the same reason: its champion's own stands in first.
    /// A jungle item's category is otherwise an ordinary one (Grez's is a Mage
    /// item), so off the jungle, or as a build's second, its stand-in comes
    /// from there, like a duplicate's. So is a melee or ranged item's: a ranged
    /// champion that loses Titanic Hydra still wanted an item of that kind.
    /// A support's wrong World Atlas item is a jungler's wrong jungle item over
    /// again, and a second one is a support item, whose category holds only
    /// more of those. A marksman's item off a marksman goes with rule 5's:
    /// its category holds little but attack items, none of which a mage
    /// that loses Guinsoo's Rageblade could take.
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

/// The Support-class items any champion may build, by base slug.
const SUPPORT_ITEM_EXCEPTIONS: [&str; 1] = ["protoplasm_harness"];

/// What the champion a build is for may hold, whatever the build already has.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    /// Rule 4: whether support items are allowed.
    support_items: bool,
    /// Rule 5: what the champion scales with, `None` when unknown.
    scaling: Option<Scaling>,
    /// Rule 14: whether the champion is a marksman, the game's `Range` class,
    /// `None` when its class is unknown.
    marksman: Option<bool>,
    /// Rule 8: whether jungle items are allowed.
    jungle_items: bool,
    /// Rule 8: whether this jungler's one jungle item is Philosopher's Stone
    /// rather than a damage one: a champion tagged `Tank`. Not a ranged one,
    /// which rule 11 would take the Stone back off; it builds its damage item
    /// like any other jungler.
    stone_jungler: bool,
    /// Rule 10: whether this is a support whose kit immobilizes, whose support
    /// item is Imperial Mandate. A mage too: the user banned Mandate from mages
    /// and lifted the ban the same day (2026-10-04).
    mandate: bool,
    /// Rule 10's other half: whether this is a support known not to
    /// immobilize, who never keeps Mandate. `false` when the champion is
    /// unknown.
    no_mandate: bool,
    /// Rule 12: whether this is a support whose World Atlas item is Solstice
    /// Sleigh: a champion tagged `Tank` whose kit immobilizes for
    /// [`SLEIGH_IMMOBILIZE_TICKS`] or more in all. The first test of the five.
    sleigh: bool,
    /// Rule 12: whether this is a support of the game's `Magician` class,
    /// whose World Atlas item is Zaz'Zak's Realmspike ahead of every other
    /// test. A mage that roots or shields still casts to deal damage: Brand
    /// was handed Solstice Sleigh and Lux Dream Maker, and the user wanted
    /// Zaz'Zak's on both (2026-10-04). Rule 13 then passes mages over as
    /// well, at the user's word the same day: whatever its kit holds, a mage
    /// is treated as one that does not aid allies. And rule 9 prefers no
    /// support items for a mage, which builds AP items like any other.
    mage: bool,
    /// Rule 13: whether this is a support whose kit heals, shields or buffs
    /// allies, who holds an item that answers to it. Never a mage. Rule 12
    /// too: its World Atlas item is Dream Maker.
    ally_aid: bool,
    /// Rule 13's other half: whether this is a support known not to aid
    /// allies, or a mage, who never keeps such an item. `false` when the
    /// champion is unknown.
    no_ally_aid: bool,
    /// Rule 12: whether this is a support whose World Atlas item is Celestial
    /// Opposition: a champion tagged `Tank`, the one the enemy hits, that
    /// `sleigh` did not take. The second test of the five.
    celestial: bool,
    /// Rule 16: whether this is a support with the `Tank` tag, which builds
    /// tank and support items and no others.
    tank_support: bool,
    /// Rule 12: whether this is a support whose World Atlas item is Zaz'Zak's
    /// Realmspike without being a mage: an AP champion none of the tests
    /// before it took, so its abilities are there to deal damage.
    realmspike: bool,
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
/// item. In the jungle it is no tank either, so it builds a damage item.
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
        realmspike: role == Role::Support
            && traits.is_some_and(|traits| traits.scaling == Some(Scaling::Ap)),
        ranged: traits.and_then(|traits| traits.ranged),
    }
}

impl Fit {
    /// Rules 16 and 17 for one item: whether `key` is of a class this
    /// support does not build. Boots are rule 7's, and an item no class is
    /// written down for is left alone, like any item the rules cannot place.
    /// The game's six finals are not among those: `base_slug` gives them the
    /// name the mod draws them under, and that has a class.
    fn off_support_classes(&self, key: &str) -> bool {
        (self.tank_support || self.aid_support())
            && !matches!(
                crate::item_catalog::category_of(crate::build_config::base_slug(key)),
                None | Some("Tank" | "Support" | "Boots")
            )
    }

    /// Rule 17: whether this is a heal, shield and buff support, which is one
    /// whose World Atlas item is Dream Maker: the user's own way of naming
    /// them. Not a tank whose kit aids allies, whose Atlas item is another
    /// (Shield Bearer, Monk, Chef): rule 16 holds it to the two classes,
    /// without the preference.
    fn aid_support(&self) -> bool {
        self.support_items && self.atlas_item() == DREAM_MAKER
    }

    /// Rule 17: whether this support has items it looks to before the other
    /// tank and support items.
    pub(crate) fn prefers_aid_items(&self) -> bool {
        self.aid_support()
    }

    /// Rule 17: whether `key` is one of those items ([`PREFERRED_AID_ITEMS`])
    /// and this a support that looks to them first.
    pub(crate) fn is_preferred_aid_item(&self, key: &str) -> bool {
        self.aid_support() && PREFERRED_AID_ITEMS.contains(&crate::build_config::base_slug(key))
    }

    /// Rule 5 for one item: whether an item with these traits gives a stat the
    /// champion cannot use. A hybrid item gives both kinds, so only a hybrid
    /// champion keeps one.
    ///
    /// Support items in the support role used to be exempt, on the grounds that
    /// their worth is what they do for allies. That let an AD support keep an
    /// AP-only one (Dual Blader on Staff of Flowing Water, 2026-09-26), and the
    /// user asked for it gone. The support items an AD champion can use pass
    /// anyway: the tank ones carry no offensive stat. Sword of Blossoming Dawn
    /// is not judged here at all: it is a marksman's item (rule 14).
    fn mismatches(&self, item: &ItemTraits) -> bool {
        match self.scaling {
            Some(Scaling::Ap) => item.physical,
            Some(Scaling::Ad) => item.magic,
            Some(Scaling::Hybrid) | None => false,
        }
    }

    /// Rule 14: whether `key` is an item only a marksman keeps, on a champion
    /// known not to be one.
    fn wants_marksman(&self, key: &str) -> bool {
        self.marksman == Some(false) && is_marksman_only_item(key)
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

    /// Rule 8: whether `key` is a jungle item and this champion is not
    /// jungling. The one test even a last-resort pick is held to, while the
    /// toggle is on.
    pub(crate) fn off_role_jungle_item(&self, key: &str) -> bool {
        !self.jungle_items && is_jungle_item(key)
    }

    /// Rule 8, for a jungler: whether `key` is a jungle item other than the
    /// one its champion builds — a damage item on a tank, Philosopher's Stone
    /// on anyone else. Which of the two damage items is rule 5's to say.
    fn other_jungle_item(&self, key: &str) -> bool {
        self.jungle_items && is_jungle_item(key) && is_philosophers_stone(key) != self.stone_jungler
    }

    /// Rule 9: whether the champion's build is guaranteed an item of its role:
    /// a jungler's jungle item. A support's one dedicated item is its World
    /// Atlas item (rule 12); past that it only prefers support items
    /// ([`Fit::prefers_support_items`]).
    fn guarantees_role_item(&self) -> bool {
        self.jungle_items
    }

    /// Rule 9: whether this is a support that prefers Support-class items past
    /// its World Atlas item — any support but a mage, which builds AP items
    /// like any other mage (the user, 2026-10-04).
    fn prefers_support_items(&self) -> bool {
        self.support_items && !self.mage
    }

    /// Rule 9: whether `key` is a Support-class item this champion prefers
    /// ([`Fit::prefers_support_items`]), rule 12's World Atlas item aside.
    /// What the item-build hook pushes harder than the mod's other items.
    pub(crate) fn is_preferred_support_item(&self, key: &str) -> bool {
        self.prefers_support_items() && is_other_support_class(key)
    }

    /// Rule 9: whether `key` is an item of the role this champion plays — a
    /// support item it prefers ([`Fit::is_preferred_support_item`]), a jungle
    /// item for a jungler, nothing in any other role. What the AI buys right
    /// after the World Atlas item, and looks to first for a stand-in.
    fn is_role_item(&self, key: &str) -> bool {
        self.is_preferred_support_item(key) || (self.jungle_items && is_jungle_item(key))
    }

    /// Rule 12: whether `key` is a World Atlas item held by a support — one of
    /// [`ATLAS_ITEMS`], this champion's or not.
    fn is_atlas_pick(&self, key: &str) -> bool {
        self.support_items && is_atlas_item(key)
    }

    /// Rule 12, for a support: the World Atlas item its champion builds, in
    /// the user's order — Solstice Sleigh for a tank that immobilizes for a
    /// second or more, else Celestial Opposition for any other tank, else
    /// Dream Maker when its kit aids allies, else Zaz'Zak's Realmspike for a
    /// mage, else Zaz'Zak's Realmspike for an AP champion, Bloodsong for the
    /// rest and for a champion nothing is known about. No champion is named
    /// an item ahead of that: Chef and Monk were, for a day (Dream Maker,
    /// 2026-10-08), and the user took the exception back out.
    fn atlas_item(&self) -> &'static str {
        if self.sleigh {
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

    /// Rule 12, for a support: whether `key` is a World Atlas item other than
    /// the one its champion builds ([`Fit::atlas_item`]).
    fn other_atlas_item(&self, key: &str) -> bool {
        self.is_atlas_pick(key) && crate::build_config::base_slug(key) != self.atlas_item()
    }

    /// Rule 15: whether this is a support whose World Atlas item is Bloodsong,
    /// which rule 12 puts in its build and which is then its one Spellblade
    /// item.
    fn holds_bloodsong(&self) -> bool {
        self.support_items && self.atlas_item() == BLOODSONG
    }
}

/// Rule 6: items worth more the longer they are owned, because their passive
/// builds permanent stacks over the match. By base slug, so the radiant tier
/// follows.
const EARLY_ITEMS: [&str; 7] = [
    "heartsteel",             // permanent bonus health per charged hit on a champion
    "yun_tal_wildarrows",     // permanent crit chance per basic attack
    "hubris",                 // permanent stack per takedown
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
    "collector",              // bonus gold per kill, worth more the earlier it comes
    "rod_of_ages",            // health, ability power and haste for every 30 seconds held
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

/// The order rules 6, 9, 12 and 17 buy AI picks in: a support's World Atlas
/// item (rule 12) before anything, then the heal, shield and buff items of a
/// support that looks to them first (rule 17), then the role's own items
/// (rule 9), then by [`timing`]. Sorted on, so smaller is sooner. The order is
/// also what decides which pick drops off the end when rule 7 puts the boots
/// in, so it is never one of the first two kinds while another is left.
fn buy_order(key: Option<&str>, fit: Fit) -> (bool, bool, bool, Timing) {
    match key {
        Some(key) => (
            !fit.is_atlas_pick(key),
            !fit.is_preferred_aid_item(key),
            !fit.is_role_item(key),
            timing(key),
        ),
        None => (true, true, true, Timing::Any),
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

/// Rule 14: the items only a marksman keeps. By base slug, so the radiant
/// tier follows.
const MARKSMAN_ONLY_ITEMS: [&str; 3] = [
    "guinsoos_rageblade",
    "statikk_shiv",
    "sword_of_blossoming_dawn", // a support item, so a marksman playing support
];

/// Whether `key` is one of [`MARKSMAN_ONLY_ITEMS`], base or radiant.
fn is_marksman_only_item(key: &str) -> bool {
    MARKSMAN_ONLY_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Rule 10: whether `key` is Imperial Mandate, base or radiant.
fn is_mandate(key: &str) -> bool {
    crate::build_config::base_slug(key) == "imperial_mandate"
}

/// Rule 13: the items whose passive only answers to the carrier healing,
/// shielding or buffing an ally. By base slug, so the radiant tier follows.
/// Dream Maker is one too, and rule 12 hands that out.
const ALLY_AID_ITEMS: [&str; 4] = [
    "ardent_censer",          // Sanctify, on healing, shielding or buffing an ally
    "echoes_of_helia",        // Soul Siphon spends its charges on the same
    "moonstone_renewer",      // Starlit Grace chains a heal given to an ally
    "staff_of_flowing_water", // Rapids, on healing, shielding or buffing an ally
];

/// Whether `key` is one of [`ALLY_AID_ITEMS`], base or radiant.
fn is_ally_aid_item(key: &str) -> bool {
    ALLY_AID_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Rule 17: the ones of [`ALLY_AID_ITEMS`] a heal, shield and buff support
/// looks to first. Moonstone Renewer is not one: Starlit Grace only chains a
/// heal, so it does nothing for a kit that shields or buffs, and nothing
/// here tells such a kit from one that heals (the user, 2026-10-08: "it only
/// increases healing"). It is an ally-aid item in every other way: rule 13
/// still goes by it, and such a support may hold it like any Support item.
const PREFERRED_AID_ITEMS: [&str; 3] =
    ["ardent_censer", "echoes_of_helia", "staff_of_flowing_water"];

/// Rule 12: what World Atlas grows into, the support role's own item line. By
/// base slug, so the radiant tier follows. Which of them a support builds is
/// [`Fit::atlas_item`]'s to say: a new one needs a place there.
const ATLAS_ITEMS: [&str; 5] = [
    BLOODSONG,            // Spellblade, for a support none of the others is for
    CELESTIAL_OPPOSITION, // Blessing of the Mountain, on being hit by an enemy champion
    DREAM_MAKER,          // a Dream Bubble, on healing, shielding or buffing an ally
    SOLSTICE_SLEIGH,      // Going Sledding, on immobilizing an enemy champion
    ZAZZAKS_REALMSPIKE,   // Void Explosion, on ability damage to an enemy champion
];

/// Rule 12: the World Atlas item of a support none of the others is for.
const BLOODSONG: &str = "bloodsong";

/// Rule 12: the World Atlas item of a tank.
const CELESTIAL_OPPOSITION: &str = "celestial_opposition";

/// Rule 12: the World Atlas item of a support that heals or shields.
const DREAM_MAKER: &str = "dream_maker";

/// Rule 12: the World Atlas item of a tank that immobilizes.
const SOLSTICE_SLEIGH: &str = "solstice_sleigh";

/// Rule 12: how long a tank's kit has to immobilize for in all to be handed
/// Solstice Sleigh: a second, in ticks (the user, 2026-10-08).
const SLEIGH_IMMOBILIZE_TICKS: usize = 60;

/// Rule 12: the World Atlas item of a mage, and of any other AP support that
/// only deals damage.
const ZAZZAKS_REALMSPIKE: &str = "zazzaks_realmspike";

/// Whether `key` is one of [`ATLAS_ITEMS`], base or radiant.
fn is_atlas_item(key: &str) -> bool {
    ATLAS_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// [`is_support_item`], less rule 12's World Atlas item: what rules 9 and 10
/// mean by a support item, since every support holds the Atlas one besides.
fn is_other_support_item(key: &str) -> bool {
    is_support_item(key) && !is_atlas_item(key)
}

/// [`is_support_class`], less rule 12's World Atlas item.
fn is_other_support_class(key: &str) -> bool {
    is_support_class(key) && !is_atlas_item(key)
}

/// Rule 8: items only the jungle role may build. By base slug, so the radiant
/// tier follows.
const JUNGLE_ITEMS: [&str; 3] = [
    "feral_flare",            // a stack per takedown and monster killed
    "grezs_spectral_lantern", // ability power per takedown and monster killed
    PHILOSOPHERS_STONE,       // maximum health per takedown and monster killed
];

/// Rule 8: the jungle item of a champion tagged `Tank`. The other two are for
/// champions that are not, by damage type.
const PHILOSOPHERS_STONE: &str = "philosophers_stone";

/// Whether `key` is one of [`JUNGLE_ITEMS`], base or radiant.
pub(crate) fn is_jungle_item(key: &str) -> bool {
    JUNGLE_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// Whether `key` is Philosopher's Stone, base or radiant.
fn is_philosophers_stone(key: &str) -> bool {
    crate::build_config::base_slug(key) == PHILOSOPHERS_STONE
}

/// Rule 11: items whose passive wants the carrier in melee, which a ranged
/// champion does not keep. By base slug, so the radiant tier follows. The
/// distances are the defaults; all of them are config-editable.
const MELEE_ITEMS: [&str; 9] = [
    "ravenous_hydra",     // Cleave at half strength from past 35 range
    "titanic_hydra",      // Cleave at half strength from past 35 range
    "hullbreaker",        // Skipper at 70% strength from past 35 range
    "heartsteel",         // Ironheart charges on enemies that stay within 50 range
    "abyssal_mask",       // Unmake curses enemy champions within 50 range
    "hollow_radiance",    // Immolate burns enemies within 30 range
    "philosophers_stone", // Immolate again
    "sunfire_cape",       // Immolate again; this slug is the radiant cape's
    // Sunfire Cape itself, whose key the base game never lost: only the
    // radiant reskins have an alias for `base_slug` to undo.
    "hourglass_of_eternity",
];

/// Rule 11: items whose passive wants the carrier at range, which a melee
/// champion does not keep. By base slug, so the radiant tier follows.
const RANGED_ITEMS: [&str; 2] = [
    "runaans_hurricane", // bolts at the enemies around the carrier; ranged only in League
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

/// Rule 15: the finished Spellblade items, of which a build holds one. By base
/// slug, so the radiant tier follows. Sheen is left out: it is their
/// component, not a finished item.
const SPELLBLADE_ITEMS: [&str; 6] = [
    "trinity_force",
    "dusk_and_dawn",
    "lich_bane",
    "essence_reaver",
    "iceborn_gauntlet",
    BLOODSONG,
];

/// Whether `key` is one of [`SPELLBLADE_ITEMS`], base or radiant.
fn is_spellblade_item(key: &str) -> bool {
    SPELLBLADE_ITEMS.contains(&crate::build_config::base_slug(key))
}

/// What the items a build already holds have spent of the budgets the rules
/// police, and the [`Fit`] of the champion it is for. Carries the trait table
/// with it, so a scan across the catalog is a run of hash lookups rather than a
/// run of lock acquisitions.
pub(crate) struct Budget {
    table: Arc<Table>,
    cuts_healing: bool,
    crit_chance: i32,
    /// Rule 8: whether the build already holds a jungle item.
    jungle_item: bool,
    /// Rule 12: whether the build already holds a World Atlas item.
    atlas_item: bool,
    /// Rule 15: whether the build already holds a Spellblade item.
    spellblade_item: bool,
    fit: Fit,
}

impl Budget {
    /// An empty budget: a build with nothing in it yet.
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
            // After rule 5: an item that fails both (Runaan's on a melee mage)
            // is replaced the way rule 5 does it, in the build's style, not by
            // another item of a kind the champion cannot use.
            Some(Reason::Reach)
        } else if self.jungle_item && is_jungle_item(key) {
            // After the rules about the champion: what is left here is a
            // jungle item this jungler could hold, were it the build's first.
            Some(Reason::SecondJungle)
        } else if self.atlas_item && self.fit.is_atlas_pick(key) {
            // The same for a support's World Atlas item.
            Some(Reason::SecondAtlas)
        } else if is_spellblade_item(key)
            && !self.fit.is_atlas_pick(key)
            && (self.spellblade_item || (self.fit.holds_bloodsong() && !self.atlas_item))
        {
            // Rule 15. A support's Bloodsong is rule 12's to place, so it is
            // never the one turned away, and a build still waiting for it
            // counts as holding it. One with another Atlas item pinned is not
            // getting a Bloodsong, and is judged like anyone's.
            Some(Reason::SecondSpellblade)
        } else if self.cuts_healing && traits.cuts_healing {
            Some(Reason::Grievous)
        } else if self.crit_chance + traits.crit_chance > CRIT_CAP {
            Some(Reason::Crit)
        } else {
            None
        }
    }

    /// Whether `key` is an item this champion may hold at all — rules 4, 5, 8,
    /// 10, 11, 13, 14, 16 and 17, which do not depend on what else is in the build (rule 8's
    /// one-to-a-build half does, and is left out). An item that fails is no
    /// guide to the build's style.
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
        self.jungle_item |= is_jungle_item(key);
        self.atlas_item |= is_atlas_item(key);
        self.spellblade_item |= is_spellblade_item(key);
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
/// the build either way. A jungle item off the jungle is the exception (rule
/// 8): with nothing in its own category it makes way for the build's style,
/// then for anything the champion may hold.
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
/// Then rule 12: a support whose build holds no World Atlas item has the AI's
/// last pick swapped for its champion's.
///
/// Then rule 9: a jungler whose build holds no jungle item, pinned or picked,
/// has its last AI slot swapped for one. The walk is the one above, from the
/// item being replaced, and the stand-in is held to every other rule against
/// the rest of the build. Nothing that fits, nothing changes. A support gets
/// nothing here: rule 12's item is its one dedicated support item.
///
/// Then rule 10: a support whose kit immobilizes and whose build has no Imperial
/// Mandate, and no support item the player pinned, has the AI's first support
/// item (preferring one only supports may build) swapped for Mandate, held to
/// the other rules the same way. No AI support item, no Mandate.
///
/// Then rule 13: a support whose kit aids allies and whose build has no item
/// that answers to it, and no support item the player pinned, has one swapped
/// in the same way, over the AI's first support item that is neither Mandate
/// nor the Atlas one. None to spare, none swapped in.
///
/// Rule 17 is in the walk itself: for a support that looks to the heal,
/// shield and buff items first, one of those is the stand-in for whatever
/// pick is turned away, while one is left that the build can hold.
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
    // A mage support's support items are no style of its own (rule 9): what it
    // loses to rules 5 and 13 makes way for an item of its AP categories.
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
                // Rule 17: a heal, shield and buff support's stand-in is one
                // of the items that answer to that, whatever the pick was
                // turned away for. A wrong World Atlas item is the exception:
                // it makes way for the right one, below.
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
                    // An item of the champion's role that fails rule 5 (an
                    // AP-only support item on an AD support) or rule 8 (Feral
                    // Flare on a tank jungler) makes way for one of the role
                    // it can use first: rule 9 wants the role to keep one (a
                    // jungler) or prefers it (a support), and the build's
                    // style would not look there. A support's
                    // World Atlas item makes way for another of those first,
                    // for the same reason (rule 12). And any of a support's
                    // picks does, bar a mage's: it prefers support items
                    // (rule 9).
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
                        // A build with no usable style at all — every other
                        // item broke these rules too — still loses the item.
                        .or_else(|| search(&|_| true))
                        .unwrap_or(offender)
                } else {
                    // The category must be known: matching `None` against
                    // `None` would swap the slot for any item the caller could
                    // not classify.
                    let own_category = category(offender).and_then(|wanted| {
                        search(&|candidate| category(candidate).as_ref() == Some(&wanted))
                    });
                    // A jungle item off the jungle goes even when its category
                    // has nothing for the champion, which is every time it
                    // also fails rule 5: all the attack speed finals give
                    // attack speed, so Feral Flare on a mage in lane found no
                    // stand-in and stayed (another mod's Ryze, top,
                    // 2026-10-07), and Grez's on an AD laner the same among
                    // the magic ones. It then follows the build's style, like
                    // rule 5's, and only then any category.
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

    // Rule 12. A support's build holds a World Atlas item, as a jungler's
    // holds its jungle item: its champion's, which is the only one the budget
    // lets through. The AI's last pick makes way for it, and one that is not
    // an item rule 9 prefers where there is a choice. Ahead of rules 9, 10
    // and 13, which look past this item.
    let needs_atlas_item = fit.support_items
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

    // Rule 9, for a jungler: a support's one guaranteed item is rule 12's. The
    // AI's last pick is the one the engine wanted least, so it is the one that
    // makes way. The stand-in is judged against the rest of the build, which
    // is why the budget is rebuilt without that slot.
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

    // Rule 10. This only decides which support item, among those the AI
    // picked: a build with none gets none, since its one dedicated support
    // item is rule 12's. The player's pinned support item is their choice, so
    // a pin rules Mandate out. Rule 12's World Atlas item is not that support
    // item, pinned or picked.
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

    // Rule 13. After rule 10, and like it only deciding which support item:
    // the AI's first that is neither Mandate nor the Atlas one makes way. A
    // build with none to spare (none at all, or rule 10 took the only one for
    // Mandate) gets none: its last pick used to make way, which made this a
    // second dedicated support item. A pinned support item rules it out, as
    // it does Mandate.
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

    // Rule 6, with rule 12's World Atlas item and rule 9's role items ahead
    // of it. A stable sort, so items of the same timing keep the engine's
    // order.
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
