//! Item grouping for the in-game build editor.
//!
//! The editor groups its item dropdowns under class headers, from a hand-kept
//! `item id -> category` map. The map is maintained rather than derived because
//! there is no category field on an item in the game's settings document to
//! derive it *from*, and inferring one from stat lines would be guesswork that
//! silently reshuffles the list whenever an item is rebalanced.
//!
//! Keys are *base* slugs (`collector`, not `radiant_collector` and not the
//! internal `warlords_final_judgement`); [`crate::build_config::base_slug`]
//! normalizes a runtime item key into one. Anything unmapped falls under
//! [`OTHER_CATEGORY`] so a newly added item is never dropped from the picker.

/// Category headers, in the order they appear in the item list.
pub const CATEGORY_ORDER: [&str; 6] = [
    "Assassin",
    "Fighter",
    "Marksman",
    "Mage",
    "Tank",
    "Support",
];

/// Group for items with no entry in [`CATEGORY_OF`]; always sorts last.
pub const OTHER_CATEGORY: &str = "Other";

/// Base slug -> category. A sorted `&[(&str, &str)]` rather than a `HashMap`:
/// it is built at compile time, read a few dozen times per modal build, and
/// binary search over sixty entries is not the cost that matters here.
const CATEGORY_OF: &[(&str, &str)] = &[
    ("atmas_reckoning", "Tank"),
    ("bastionbreaker", "Assassin"),
    ("black_cleaver", "Fighter"),
    ("blackfire_torch", "Mage"),
    ("blade_of_the_ruined_king", "Marksman"),
    ("bloodletters_curse", "Mage"),
    ("bloodsong", "Support"),
    ("bloodthirster", "Fighter"),
    ("collector", "Assassin"),
    ("dead_mans_plate", "Tank"),
    ("deathblade", "Marksman"),
    ("deaths_dance", "Fighter"),
    ("diamond_tipped_spear", "Marksman"),
    ("dragons_claw", "Tank"),
    ("dusk_and_dawn", "Mage"),
    ("echoes_of_helia", "Support"),
    ("experimental_hexplate", "Fighter"),
    ("frozen_heart", "Tank"),
    ("frozen_mallet", "Fighter"),
    ("guinsoos_rageblade", "Marksman"),
    ("heartsteel", "Tank"),
    ("hextech_gunblade", "Mage"),
    ("hubris", "Assassin"),
    ("infinity_edge", "Marksman"),
    ("jaksho_the_protean", "Tank"),
    ("kraken_slayer", "Marksman"),
    ("liandrys_torment", "Mage"),
    ("locket_of_the_iron_solari", "Support"),
    ("lord_dominiks_regards", "Marksman"),
    ("ludens_tempest", "Mage"),
    ("malignance", "Mage"),
    ("mirage_blade", "Marksman"),
    ("morellonomicon", "Mage"),
    ("mortal_reminder", "Marksman"),
    ("nashors_tooth", "Mage"),
    ("night_harvester", "Mage"),
    ("opportunity", "Assassin"),
    ("overlords_bloodmail", "Fighter"),
    ("phantom_dancer", "Marksman"),
    ("protectors_vow", "Tank"),
    ("protoplasm_harness", "Support"),
    ("rabadons_deathcap", "Mage"),
    ("riftmaker", "Mage"),
    ("rylais_crystal_scepter", "Mage"),
    ("serpents_fang", "Assassin"),
    ("shadowflame", "Mage"),
    ("spear_of_shojin", "Fighter"),
    ("spirit_visage", "Tank"),
    ("stormrazor", "Marksman"),
    ("sundered_sky", "Fighter"),
    ("sunfire_cape", "Tank"),
    ("terminus", "Marksman"),
    ("thornmail", "Tank"),
    ("trinity_force", "Fighter"),
    ("unending_despair", "Tank"),
    ("void_staff", "Mage"),
    ("voltaic_cyclosword", "Assassin"),
    ("warmogs_armor", "Tank"),
    ("wits_end", "Marksman"),
    ("yun_tal_wildarrows", "Marksman"),
    ("zekes_herald", "Support"),
];

/// Sheet frames for the six vanilla finals the mod reskins. Every other mod
/// item has a frame named after its base slug, but a reskin keeps the vanilla
/// art's tier-indexed frame name, so those six need naming explicitly.
const RESKIN_ICON: &[(&str, &str)] = &[
    ("bloodthirster", "t4_0"),
    ("dragons_claw", "t4_3"),
    ("ludens_tempest", "t4_4"),
    ("phantom_dancer", "t4_1"),
    ("sunfire_cape", "t4_5"),
    ("thornmail", "t4_2"),
];

/// Sprite-sheet frame for a base slug, for an `image` node's `rect_tag`.
///
/// `is_mod_item` says whether the mod registered this item: only those have art
/// in the sheet. A vanilla final the mod leaves alone has no frame, and naming a
/// frame that does not exist is not something the engine is documented to
/// tolerate, so those get `None` and render as name-only rows.
pub fn icon_frame(slug: &str, is_mod_item: bool) -> Option<&str> {
    if let Ok(index) = RESKIN_ICON.binary_search_by_key(&slug, |(key, _)| key) {
        return Some(RESKIN_ICON[index].1);
    }
    is_mod_item.then_some(slug)
}

/// Category a base slug belongs to, or [`OTHER_CATEGORY`].
pub fn category_of(slug: &str) -> &'static str {
    CATEGORY_OF
        .binary_search_by_key(&slug, |(key, _)| key)
        .map(|index| CATEGORY_OF[index].1)
        .unwrap_or(OTHER_CATEGORY)
}

/// Sort key putting items in [`CATEGORY_ORDER`] order, with "Other" last.
pub fn category_rank(category: &str) -> usize {
    CATEGORY_ORDER
        .iter()
        .position(|name| *name == category)
        .unwrap_or(CATEGORY_ORDER.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `category_of` binary-searches, so an out-of-order entry would silently
    /// mis-group items rather than fail loudly.
    #[test]
    fn tables_are_sorted_by_key() {
        assert!(CATEGORY_OF.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(RESKIN_ICON.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    /// Every category used in the table has a header to sit under, or its items
    /// would sort into the "Other" bucket while claiming a name of their own.
    #[test]
    fn every_category_has_a_header() {
        for (slug, category) in CATEGORY_OF {
            assert!(
                CATEGORY_ORDER.contains(category),
                "{slug} is in unknown category {category}"
            );
        }
    }
}
