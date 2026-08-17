use mod_api_stable::*;
use std::cell::Cell;

mod build_config;
mod config;
mod constants;
mod hook;
mod item_build_hook;
mod item_catalog;
mod item_meta;
mod items;
mod solo_rank_ui;
mod strategy_ui;
mod tactics;

use items::*;

pub(crate) use constants::*;
pub(crate) use item_meta::ItemMeta;

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn percent_of_i32(value: i32, percent: f64) -> i32 {
    (value as f64 * percent / 100.0).round() as i32
}

fn ticks(seconds: f64) -> usize {
    (seconds * TICKS_PER_SECOND).round() as usize
}

fn has_buff(entity: &StableEntity<'_, '_>, name: &str) -> bool {
    (0..entity.buff_count()).any(|i| entity.buff_at(i).is_some_and(|b| b.name() == name))
}

fn buff_stacks(entity: &StableEntity<'_, '_>, name: &str) -> usize {
    (0..entity.buff_count())
        .filter(|&i| entity.buff_at(i).is_some_and(|b| b.name() == name))
        .count()
}

/// Damage multiplier that simulates `lethality` flat armor penetration against a
/// target with `armor`. The game mitigates physical damage by `100 / (100 + armor)`
/// (verified: `mitigated = floor(raw * 100 / (100 + armor))`), so scaling the
/// outgoing damage by this factor makes the post-mitigation result match what the
/// target would take at `armor - lethality` (floored at 0):
/// `(100 + armor) / (100 + max(0, armor - lethality))`. The bump is largest against
/// low-armor targets, like real lethality. Returns `1.0` when there is nothing to
/// penetrate (non-positive armor or lethality).
fn lethality_multiplier(armor: i32, lethality: i32) -> f64 {
    if armor <= 0 || lethality <= 0 {
        return 1.0;
    }
    let effective_armor = (armor - lethality).max(0);
    (100 + armor) as f64 / (100 + effective_armor) as f64
}

/// How much armor earlier lethality items already stripped during the attack
/// currently being resolved, so the next item starts where they stopped.
#[derive(Clone, Copy)]
struct Penetration {
    tick: usize,
    caster: usize,
    target: usize,
    /// Damage after the last item ran. An item continuing the same attack is
    /// handed exactly this value, which is what distinguishes "next item on this
    /// attack" from "first item on a fresh attack against the same target".
    damage: usize,
    armor: i32,
}

thread_local! {
    /// Thread-local because parallel match simulations resolve attacks
    /// concurrently. The items of a single attack always run consecutively on
    /// one thread, so this needs no lock.
    static PENETRATION: Cell<Option<Penetration>> = Cell::new(None);
}

/// Scales a basic attack's `damage` to simulate `lethality` flat armor
/// penetration against `target` (via [`lethality_multiplier`]). Basic attacks are
/// the only damage instance a mod can modify — ability damage is dealt by the
/// game — so lethality items apply this in `on_attack`.
///
/// Lethality is additive across items, but each item knows only its own value
/// and the host threads one `damage` through every item in turn. Having each
/// penetrate the target's *full* armor compounds into more penetration than the
/// sum — worst against low-armor targets, where every item independently strips
/// the armor to zero and gets paid for it. So each item instead penetrates from
/// where the previous one stopped, and the multipliers telescope into a single
/// reduction by the total:
///
/// ```text
/// (100+A)/(100+A-L1) * (100+A-L1)/(100+A-L1-L2) = (100+A)/(100+A-L1-L2)
/// ```
///
/// A lone lethality item sees `already == 0`, so its result is unchanged.
fn apply_lethality(
    ctx: &mut StableSim<'_>,
    caster: usize,
    target: usize,
    lethality: usize,
    damage: &mut usize,
) {
    let armor = ctx
        .get_entity(target)
        .map(|t| t.stat().defence as i32)
        .unwrap_or(0);
    let tick = ctx.tick();

    let already = PENETRATION.with(|cell| match cell.get() {
        Some(prev)
            if prev.tick == tick
                && prev.caster == caster
                && prev.target == target
                && prev.damage == *damage =>
        {
            prev.armor
        }
        _ => 0,
    });

    let mult = lethality_multiplier((armor - already).max(0), lethality as i32);
    *damage = (*damage as f64 * mult).round() as usize;

    PENETRATION.with(|cell| {
        cell.set(Some(Penetration {
            tick,
            caster,
            target,
            damage: *damage,
            armor: already + lethality as i32,
        }))
    });
}

fn is_enemy_champion(ctx: &mut StableSim<'_>, caster: usize, target: usize) -> bool {
    let Some(caster_team) = ctx.get_entity(caster).map(|c| c.team()) else {
        return false;
    };
    ctx.get_entity(target)
        .map(|target_ref| target_ref.is_champion() && target_ref.team() != caster_team)
        .unwrap_or(false)
}

fn apply_adaptive_force(ctx: &mut StableSim<'_>, player: usize, adaptive_force: i32, name: &str) {
    let Some((champion_id, favors_ap, already_applied)) = ctx.get_player(player).and_then(|p| {
        let champion_ref = p.champion()?;
        let stat = champion_ref.stat();
        Some((
            champion_ref.id(),
            stat.magic_power > stat.attack,
            has_buff(&champion_ref, name),
        ))
    }) else {
        return;
    };

    if already_applied {
        return;
    }

    let buff = if favors_ap {
        BuffV1 {
            magic_power: adaptive_force,
            ..BuffV1::named(name)
        }
    } else {
        BuffV1 {
            attack: (adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32,
            ..BuffV1::named(name)
        }
    };
    ctx.add_buff(champion_id, &buff);
}

// Installs the native tap on the item-build route function when the server
// starts. It is fail-closed (see `hook.rs`): on any mismatch it records a
// refusal and leaves the game function untouched. It is the one part of this
// mod that is NOT stable-ABI — it detours the game binary directly, so it needs
// the pinned toolchain in `rust-toolchain.toml` and the `game_core` rlib in
// `.cargo/config.toml`, and it must be re-verified after every game update.
//
// It no longer decides builds; `item_build_hook::ConfiguredBuilds` does, on the
// stable API. What it still supplies is the `Database` address and the item
// catalog the tactics half cannot reach any other way, plus the full champion
// roster for the editor — so a refusal costs those, not the builds.
struct NativeTapExtension;

impl StableServerExtension for NativeTapExtension {
    fn before_management_tick(&self, _ctx: &mut StableServerCtx<'_>) {
        tactics::driver::before_management_tick();
    }

    fn on_server_start(&self, _ctx: &mut StableServerCtx<'_>) {
        tactics::driver::on_server_start();

        match hook::install_hook() {
            Ok(address) => {
                let message = format!("hook_installed address=0x{address:x}");
                eprintln!("riot_items_tfm2: {message}");
            }
            Err(error) if error == "hook already installed" => {}
            Err(error) => {
                eprintln!("riot_items_tfm2: hook_refused error={error}");
                // Resolution failed diagnostics
                match hook::candidate_report() {
                    Ok(candidates) => {
                        eprintln!(
                            "riot_items_tfm2: hook_candidates count={}",
                            candidates.len()
                        );
                        for candidate in candidates {
                            eprintln!("riot_items_tfm2: hook_candidate {candidate}");
                        }
                    }
                    Err(error) => {
                        eprintln!("riot_items_tfm2: hook_candidates_failed error={error}");
                    }
                }
            }
        }
    }
}

fn init(host: &StableHost) -> StableMod {
    let mut reg = StableMod::new("riot_items_tfm2");
    let configs = config::load();

    tactics::driver::on_mod_init();

    macro_rules! configured {
        ($key:literal => $T:ty) => {
            configs.get($key).map(<$T>::with_config).unwrap_or_default()
        };
    }
    macro_rules! configured_radiant {
        ($key:literal => $T:ty) => {{
            let item = configs
                .get($key)
                .map(<$T>::radiant_with_config)
                .unwrap_or_else(<$T>::radiant);
            // The category is taken off the built item rather than written down
            // beside the key: it is the same value the engine will be told, so
            // the two cannot drift. `tactics` needs it to find a same-category
            // stand-in when unique enforcement rejects a 4th item.
            strategy_ui::note_final_item($key, StableItem::category(&item));
            item
        }};
    }

    // Tier 1
    reg.add_item(configured!("glowing_mote" => GlowingMote));

    // Tier 2
    reg.add_item(configured!("executioners_calling" => ExecutionersCalling));
    reg.add_item(configured!("oblivion_orb" => OblivionOrb));
    reg.add_item(configured!("serrated_dirk" => SerratedDirk));
    reg.add_item(configured!("sheen" => Sheen));

    // Tier 3
    reg.add_item(configured!("aegis_of_the_legion" => AegisOfTheLegion));
    reg.add_item(configured!("bandleglass_mirror" => BandleglassMirror));
    reg.add_item(configured!("bf_sword" => BFSword));
    reg.add_item(configured!("blighting_jewel" => BlightingJewel));
    reg.add_item(configured!("glacial_buckler" => GlacialBuckler));
    reg.add_item(configured!("haunting_guise" => HauntingGuise));
    reg.add_item(configured!("last_whisper" => LastWhisper));
    reg.add_item(configured!("needlessly_large_rod" => NeedlesslyLargeRod));
    reg.add_item(configured!("noonquiver" => Noonquiver));
    reg.add_item(configured!("phage" => Phage));
    reg.add_item(configured!("scouts_slingshot" => ScoutsSlingshot));
    reg.add_item(configured!("steel_sigil" => SteelSigil));
    reg.add_item(configured!("winged_moonplate" => WingedMoonplate));

    // Tier 4
    reg.add_item(configured!("atmas_reckoning" => AtmasReckoning));
    reg.add_item(configured!("bastionbreaker" => Bastionbreaker));
    reg.add_item(configured!("black_cleaver" => BlackCleaver));
    reg.add_item(configured!("blackfire_torch" => BlackfireTorch));
    reg.add_item(configured!("blade_of_the_ruined_king" => BladeOfTheRuinedKing));
    reg.add_item(configured!("bloodletters_curse" => BloodlettersCurse));
    reg.add_item(configured!("bloodsong" => Bloodsong));
    reg.add_item(configured!("collector" => Collector));
    reg.add_item(configured!("dead_mans_plate" => DeadMansPlate));
    reg.add_item(configured!("deathblade" => DeathBlade));
    reg.add_item(configured!("deaths_dance" => DeathsDance));
    reg.add_item(configured!("diamond_tipped_spear" => DiamondTippedSpear));
    reg.add_item(configured!("dusk_and_dawn" => DuskAndDawn));
    reg.add_item(configured!("echoes_of_helia" => EchoesOfHelia));
    reg.add_item(configured!("experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(configured!("frozen_heart" => FrozenHeart));
    reg.add_item(configured!("frozen_mallet" => FrozenMallet));
    reg.add_item(configured!("guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured!("heartsteel" => Heartsteel));
    reg.add_item(configured!("hextech_gunblade" => HextechGunblade));
    reg.add_item(configured!("hubris" => Hubris));
    reg.add_item(configured!("infinity_edge" => InfinityEdge));
    reg.add_item(configured!("jaksho_the_protean" => JakshoTheProtean));
    reg.add_item(configured!("kraken_slayer" => KrakenSlayer));
    reg.add_item(configured!("liandrys_torment" => LiandrysTorment));
    reg.add_item(configured!("locket_of_the_iron_solari" => LocketOfTheIronSolari));
    reg.add_item(configured!("lord_dominiks_regards" => LordDominiksRegards));
    reg.add_item(configured!("malignance" => Malignance));
    reg.add_item(configured!("mirage_blade" => MirageBlade));
    reg.add_item(configured!("morellonomicon" => Morellonomicon));
    reg.add_item(configured!("mortal_reminder" => MortalReminder));
    reg.add_item(configured!("nashors_tooth" => NashorsTooth));
    reg.add_item(configured!("night_harvester" => NightHarvester));
    reg.add_item(configured!("opportunity" => Opportunity));
    reg.add_item(configured!("overlords_bloodmail" => OverlordsBloodmail));
    reg.add_item(configured!("protectors_vow" => ProtectorsVow));
    reg.add_item(configured!("protoplasm_harness" => ProtoplasmHarness));
    reg.add_item(configured!("rabadons_deathcap" => RabadonsDeathcap));
    reg.add_item(configured!("riftmaker" => Riftmaker));
    reg.add_item(configured!("rite_of_ruin" => RiteOfRuin));
    reg.add_item(configured!("rylais_crystal_scepter" => RylaisCrystalScepter));
    reg.add_item(configured!("serpents_fang" => SerpentsFang));
    reg.add_item(configured!("shadowflame" => Shadowflame));
    reg.add_item(configured!("spear_of_shojin" => SpearOfShojin));
    reg.add_item(configured!("spirit_visage" => SpiritVisage));
    reg.add_item(configured!("stormrazor" => Stormrazor));
    reg.add_item(configured!("sundered_sky" => SunderedSky));
    reg.add_item(configured!("terminus" => Terminus));
    reg.add_item(configured!("trinity_force" => TrinityForce));
    reg.add_item(configured!("unending_despair" => UnendingDespair));
    reg.add_item(configured!("void_staff" => VoidStaff));
    reg.add_item(configured!("voltaic_cyclosword" => VoltaicCyclosword));
    reg.add_item(configured!("warmogs_armor" => WarmogsArmor));
    reg.add_item(configured!("wits_end" => WitsEnd));
    reg.add_item(configured!("yun_tal_wildarrows" => YunTalWildarrows));
    reg.add_item(configured!("zekes_herald" => ZekesHerald));

    // Tier 5
    reg.add_item(configured_radiant!("radiant_atmas_reckoning" => AtmasReckoning));
    reg.add_item(configured_radiant!("radiant_bastionbreaker" => Bastionbreaker));
    reg.add_item(configured_radiant!("radiant_black_cleaver" => BlackCleaver));
    reg.add_item(configured_radiant!("radiant_blackfire_torch" => BlackfireTorch));
    reg.add_item(configured_radiant!("radiant_blade_of_the_ruined_king" => BladeOfTheRuinedKing));
    reg.add_item(configured_radiant!("radiant_bloodletters_curse" => BloodlettersCurse));
    reg.add_item(configured_radiant!("radiant_bloodsong" => Bloodsong));
    reg.add_item(configured_radiant!("radiant_collector" => Collector));
    reg.add_item(configured_radiant!("radiant_dead_mans_plate" => DeadMansPlate));
    reg.add_item(configured_radiant!("radiant_deathblade" => DeathBlade));
    reg.add_item(configured_radiant!("radiant_deaths_dance" => DeathsDance));
    reg.add_item(configured_radiant!("radiant_diamond_tipped_spear" => DiamondTippedSpear));
    reg.add_item(configured_radiant!("radiant_dusk_and_dawn" => DuskAndDawn));
    reg.add_item(configured_radiant!("radiant_echoes_of_helia" => EchoesOfHelia));
    reg.add_item(configured_radiant!("radiant_experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(configured_radiant!("radiant_frozen_heart" => FrozenHeart));
    reg.add_item(configured_radiant!("radiant_frozen_mallet" => FrozenMallet));
    reg.add_item(configured_radiant!("radiant_guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured_radiant!("radiant_heartsteel" => Heartsteel));
    reg.add_item(configured_radiant!("radiant_hextech_gunblade" => HextechGunblade));
    reg.add_item(configured_radiant!("radiant_hubris" => Hubris));
    reg.add_item(configured_radiant!("radiant_infinity_edge" => InfinityEdge));
    reg.add_item(configured_radiant!("radiant_jaksho_the_protean" => JakshoTheProtean));
    reg.add_item(configured_radiant!("radiant_kraken_slayer" => KrakenSlayer));
    reg.add_item(configured_radiant!("radiant_liandrys_torment" => LiandrysTorment));
    reg.add_item(configured_radiant!("radiant_locket_of_the_iron_solari" => LocketOfTheIronSolari));
    reg.add_item(configured_radiant!("radiant_lord_dominiks_regards" => LordDominiksRegards));
    reg.add_item(configured_radiant!("radiant_malignance" => Malignance));
    reg.add_item(configured_radiant!("radiant_mirage_blade" => MirageBlade));
    reg.add_item(configured_radiant!("radiant_morellonomicon" => Morellonomicon));
    reg.add_item(configured_radiant!("radiant_mortal_reminder" => MortalReminder));
    reg.add_item(configured_radiant!("radiant_nashors_tooth" => NashorsTooth));
    reg.add_item(configured_radiant!("radiant_night_harvester" => NightHarvester));
    reg.add_item(configured_radiant!("radiant_opportunity" => Opportunity));
    reg.add_item(configured_radiant!("radiant_overlords_bloodmail" => OverlordsBloodmail));
    reg.add_item(configured_radiant!("radiant_protectors_vow" => ProtectorsVow));
    reg.add_item(configured_radiant!("radiant_protoplasm_harness" => ProtoplasmHarness));
    reg.add_item(configured_radiant!("radiant_rabadons_deathcap" => RabadonsDeathcap));
    reg.add_item(configured_radiant!("radiant_riftmaker" => Riftmaker));
    reg.add_item(configured_radiant!("radiant_rite_of_ruin" => RiteOfRuin));
    reg.add_item(configured_radiant!("radiant_rylais_crystal_scepter" => RylaisCrystalScepter));
    reg.add_item(configured_radiant!("radiant_serpents_fang" => SerpentsFang));
    reg.add_item(configured_radiant!("radiant_shadowflame" => Shadowflame));
    reg.add_item(configured_radiant!("radiant_spear_of_shojin" => SpearOfShojin));
    reg.add_item(configured_radiant!("radiant_spirit_visage" => SpiritVisage));
    reg.add_item(configured_radiant!("radiant_stormrazor" => Stormrazor));
    reg.add_item(configured_radiant!("radiant_sundered_sky" => SunderedSky));
    reg.add_item(configured_radiant!("radiant_terminus" => Terminus));
    reg.add_item(configured_radiant!("radiant_trinity_force" => TrinityForce));
    reg.add_item(configured_radiant!("radiant_unending_despair" => UnendingDespair));
    reg.add_item(configured_radiant!("radiant_void_staff" => VoidStaff));
    reg.add_item(configured_radiant!("radiant_voltaic_cyclosword" => VoltaicCyclosword));
    reg.add_item(configured_radiant!("radiant_warmogs_armor" => WarmogsArmor));
    reg.add_item(configured_radiant!("radiant_wits_end" => WitsEnd));
    reg.add_item(configured_radiant!("radiant_yun_tal_wildarrows" => YunTalWildarrows));
    reg.add_item(configured_radiant!("radiant_zekes_herald" => ZekesHerald));

    // New items go at the END of this list, never sorted into the blocks above
    // — see `registration_order` below for why.

    // What `item-builds.json` reaches the game through. Registered whether or
    // not a config exists: the hook keeps the engine's build when it has nothing
    // to say, so an inert install costs one call per player per match.
    reg.add_item_build_hook(item_build_hook::ConfiguredBuilds);

    reg.set_server_extension(NativeTapExtension);
    // Client-side in-game build picker on the strategy screen. Purely additive:
    // it no-ops unless the `ui/layout/strategy` asset override is in place.
    reg.set_extension(strategy_ui::StrategyPicker);

    host.log(
        LogLevel::Info,
        &format!(
            "riot_items_tfm2: registered items, config entries={}",
            configs.len()
        ),
    );

    reg
}

declare_stable_mod!(init);

/// Guards the one thing about this file that is not free to change: the order
/// `reg.add_item` is called in.
///
/// Saves do not store item keys — a 47MB save contains zero of them — so items
/// are addressed numerically, and the number a mod's item gets is decided by
/// registration order (`add_item` is a `push`). Treat the sequence below as a
/// wire format: the only edit that certainly preserves it is an append.
///
/// Two releases have inserted mid-list anyway — 0.8.3 put
/// `radiant_locket_of_the_iron_solari` at index 98, and 0.9.0 put
/// `rite_of_ruin` at 57 and `radiant_rite_of_ruin` at 114. Whether that
/// actually misreads an existing save has **not** been demonstrated; it was
/// investigated as the cause of the 2026-08-13 blank solo-rank screen and
/// ruled out (that reproduces with every mod disabled, so it is a base-game
/// bug). The rule is kept because the cost of following it is zero and the
/// cost of being wrong about it is every save on disk.
///
/// The frozen list is 0.9.0's shipped order, which is what saves in the wild
/// were written against.
#[cfg(test)]
mod registration_order {
    /// Every `add_item` key in `init`, in call order, read out of this file's
    /// own source. Parsing the source rather than running `init` keeps the check
    /// free of a `StableHost`, which only the game can supply.
    fn registered_keys() -> Vec<&'static str> {
        const SOURCE: &str = include_str!("lib.rs");
        SOURCE
            .lines()
            .filter_map(|line| {
                // Only real call sites: the macro definitions and this module's
                // own literals do not start with `reg.add_item(configured`.
                let rest = line.trim_start().strip_prefix("reg.add_item(configured")?;
                let rest = rest.strip_prefix("_radiant").unwrap_or(rest);
                let rest = rest.strip_prefix("!(\"")?;
                rest.split_once('"').map(|(key, _)| key)
            })
            .collect()
    }

    /// The 0.9.0 (shipped) registration order. Append below, never insert.
    const FROZEN: &[&str] = &[
        "glowing_mote", "executioners_calling", "oblivion_orb", "serrated_dirk", "sheen",
        "aegis_of_the_legion", "bandleglass_mirror", "bf_sword", "blighting_jewel",
        "glacial_buckler", "haunting_guise", "last_whisper", "needlessly_large_rod",
        "noonquiver", "phage", "scouts_slingshot", "steel_sigil", "winged_moonplate",
        "atmas_reckoning", "bastionbreaker", "black_cleaver", "blackfire_torch",
        "blade_of_the_ruined_king", "bloodletters_curse", "bloodsong", "collector",
        "dead_mans_plate", "deathblade", "deaths_dance", "diamond_tipped_spear",
        "dusk_and_dawn", "echoes_of_helia", "experimental_hexplate", "frozen_heart",
        "frozen_mallet", "guinsoos_rageblade", "heartsteel", "hextech_gunblade", "hubris",
        "infinity_edge", "jaksho_the_protean", "kraken_slayer", "liandrys_torment",
        "locket_of_the_iron_solari", "lord_dominiks_regards", "malignance", "mirage_blade",
        "morellonomicon", "mortal_reminder", "nashors_tooth", "night_harvester", "opportunity",
        "overlords_bloodmail", "protectors_vow", "protoplasm_harness", "rabadons_deathcap",
        "riftmaker", "rite_of_ruin", "rylais_crystal_scepter", "serpents_fang", "shadowflame",
        "spear_of_shojin", "spirit_visage", "stormrazor", "sundered_sky", "terminus",
        "trinity_force", "unending_despair", "void_staff", "voltaic_cyclosword",
        "warmogs_armor", "wits_end", "yun_tal_wildarrows", "zekes_herald",
        "radiant_atmas_reckoning", "radiant_bastionbreaker", "radiant_black_cleaver",
        "radiant_blackfire_torch", "radiant_blade_of_the_ruined_king",
        "radiant_bloodletters_curse", "radiant_bloodsong", "radiant_collector",
        "radiant_dead_mans_plate", "radiant_deathblade", "radiant_deaths_dance",
        "radiant_diamond_tipped_spear", "radiant_dusk_and_dawn", "radiant_echoes_of_helia",
        "radiant_experimental_hexplate", "radiant_frozen_heart", "radiant_frozen_mallet",
        "radiant_guinsoos_rageblade", "radiant_heartsteel", "radiant_hextech_gunblade",
        "radiant_hubris", "radiant_infinity_edge", "radiant_jaksho_the_protean",
        "radiant_kraken_slayer", "radiant_liandrys_torment",
        "radiant_locket_of_the_iron_solari", "radiant_lord_dominiks_regards",
        "radiant_malignance", "radiant_mirage_blade", "radiant_morellonomicon",
        "radiant_mortal_reminder", "radiant_nashors_tooth", "radiant_night_harvester",
        "radiant_opportunity", "radiant_overlords_bloodmail", "radiant_protectors_vow",
        "radiant_protoplasm_harness", "radiant_rabadons_deathcap", "radiant_riftmaker",
        "radiant_rite_of_ruin", "radiant_rylais_crystal_scepter", "radiant_serpents_fang",
        "radiant_shadowflame", "radiant_spear_of_shojin", "radiant_spirit_visage",
        "radiant_stormrazor", "radiant_sundered_sky", "radiant_terminus",
        "radiant_trinity_force", "radiant_unending_despair", "radiant_void_staff",
        "radiant_voltaic_cyclosword", "radiant_warmogs_armor", "radiant_wits_end",
        "radiant_yun_tal_wildarrows", "radiant_zekes_herald",
    ];

    /// The frozen order must still be a *prefix* of what `init` registers.
    /// Appending is the only safe edit; inserting, sorting or deleting is not.
    #[test]
    fn frozen_prefix_is_intact() {
        let keys = registered_keys();
        assert!(
            keys.len() >= FROZEN.len(),
            "{} items registered, fewer than the {} frozen ones - a key was deleted, which \
             shifts every item after it in existing saves",
            keys.len(),
            FROZEN.len()
        );
        for (index, (frozen, actual)) in FROZEN.iter().zip(keys.iter()).enumerate() {
            assert_eq!(
                frozen, actual,
                "registration order changed at index {index}. Saves store items by this index, \
                 so a new item must be APPENDED at the end of `init`, not sorted into place. \
                 If the reorder is deliberate and you accept breaking every existing save, \
                 update FROZEN."
            );
        }
    }

    /// A duplicate key is the other way to corrupt the mapping, and it is quiet:
    /// both entries are registered, so everything after the second one shifts.
    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for key in registered_keys() {
            assert!(seen.insert(key), "duplicate registration key {key:?}");
        }
    }

    /// Smoke test on the parser itself: if `registered_keys` ever stopped
    /// matching the call sites, every assertion above would pass on an empty
    /// list and the guard would be silently inert.
    #[test]
    fn parser_sees_the_call_sites() {
        assert_eq!(
            registered_keys().first(),
            Some(&"glowing_mote"),
            "source parsing did not find the add_item call sites - the guard is inert"
        );
    }
}
