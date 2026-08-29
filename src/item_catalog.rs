pub const CATEGORY_ORDER: [&str; 6] =
    ["Assassin", "Fighter", "Marksman", "Mage", "Tank", "Support"];

const CATEGORY_OF: &[(&str, &str)] = &[
    ("ardent_censer", "Support"),
    ("atmas_reckoning", "Tank"),
    ("axiom_arc", "Assassin"),
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
    ("eclipse", "Fighter"),
    ("experimental_hexplate", "Fighter"),
    ("feral_flare", "Fighter"),
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
    ("randuins_omen", "Tank"),
    ("riftmaker", "Mage"),
    ("rite_of_ruin", "Mage"),
    ("rylais_crystal_scepter", "Mage"),
    ("serpents_fang", "Assassin"),
    ("shadowflame", "Mage"),
    ("spear_of_shojin", "Fighter"),
    ("spirit_visage", "Tank"),
    ("steraks_gage", "Fighter"),
    ("stormrazor", "Marksman"),
    ("sundered_sky", "Fighter"),
    ("sunfire_cape", "Tank"),
    ("sword_of_blossoming_dawn", "Support"),
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

const RESKIN_ICON: &[(&str, &str)] = &[
    ("bloodthirster", "t4_0"),
    ("dragons_claw", "t4_3"),
    ("ludens_tempest", "t4_4"),
    ("phantom_dancer", "t4_1"),
    ("sunfire_cape", "t4_5"),
    ("thornmail", "t4_2"),
];

pub fn icon_frame(slug: &str, is_mod_item: bool) -> Option<&str> {
    if let Ok(index) = RESKIN_ICON.binary_search_by_key(&slug, |(key, _)| key) {
        return Some(RESKIN_ICON[index].1);
    }
    is_mod_item.then_some(slug)
}

/// `None` for a slug with no entry, which the editor hides rather than grouping
/// under a catch-all header. Not `unwrap`: this runs on every key the game hands
/// the picker, mod and vanilla alike, so an unmapped one has to be an ordinary
/// answer and not a panic inside a UI callback.
pub fn category_of(slug: &str) -> Option<&'static str> {
    CATEGORY_OF
        .binary_search_by_key(&slug, |(key, _)| key)
        .ok()
        .map(|index| CATEGORY_OF[index].1)
}

pub fn category_rank(category: &str) -> usize {
    CATEGORY_ORDER
        .iter()
        .position(|name| *name == category)
        .unwrap_or(CATEGORY_ORDER.len())
}
