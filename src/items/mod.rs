//! Item module index.
//!
//! Every item lives in its own file in this directory. The `items!` macro
//! declares each module and re-exports its public types, so registering a new
//! item is a one-line addition here (plus the file itself and its `add_item`
//! call in `lib.rs`).

macro_rules! items {
    ($($module:ident),* $(,)?) => {
        $(
            mod $module;
            pub use $module::*;
        )*
    };
}

items! {
    aegis_of_the_legion,
    bandleglass_mirror,
    bf_sword,
    black_cleaver,
    blackfire_torch,
    blade_of_the_ruined_king,
    blighting_jewel,
    bloodletters_curse,
    bloodsong,
    collector,
    deathblade,
    deaths_dance,
    diamond_tipped_spear,
    dusk_and_dawn,
    echoes_of_helia,
    executioners_calling,
    experimental_hexplate,
    frozen_mallet,
    guinsoos_rageblade,
    haunting_guise,
    heartsteel,
    hextech_gunblade,
    infinity_edge,
    jaksho_the_protean,
    liandrys_torment,
    mirage_blade,
    morellonomicon,
    mortal_reminder,
    nashors_tooth,
    needlessly_large_rod,
    night_harvester,
    noonquiver,
    oblivion_orb,
    overlords_bloodmail,
    phage,
    protectors_vow,
    protoplasm_harness,
    rabadons_deathcap,
    riftmaker,
    rylais_crystal_scepter,
    scouts_slingshot,
    shadowflame,
    sheen,
    spear_of_shojin,
    spirit_visage,
    steel_sigil,
    stormrazor,
    sundered_sky,
    terminus,
    trinity_force,
    unending_despair,
    void_staff,
    warmogs_armor,
    yun_tal_wildarrows,
    zekes_herald,
}
