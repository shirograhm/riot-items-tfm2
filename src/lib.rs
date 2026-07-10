use mod_api::*;

mod aegis_of_the_legion;
mod bandleglass_mirror;
mod bf_sword;
mod black_cleaver;
mod blackfire_torch;
mod blade_of_the_ruined_king;
mod blighting_jewel;
mod bloodletters_curse;
mod build_config;
mod collector;
mod config;
mod constants;
mod deathblade;
mod deaths_dance;
mod diamond_tipped_spear;
mod echoes_of_helia;
mod executioners_calling;
mod experimental_hexplate;
mod frozen_mallet;
mod guinsoos_rageblade;
mod haunting_guise;
mod heartsteel;
mod hextech_gunblade;
mod hook;
mod infinity_edge;
mod jaksho_the_protean;
mod liandrys_torment;
mod mirage_blade;
mod morellonomicon;
mod mortal_reminder;
mod nashors_tooth;
mod needlessly_large_rod;
mod night_harvester;
mod noonquiver;
mod oblivion_orb;
mod overlords_bloodmail;
mod phage;
mod protectors_vow;
mod protoplasm_harness;
mod rabadons_deathcap;
mod riftmaker;
mod rylais_crystal_scepter;
mod scouts_slingshot;
mod shadowflame;
mod spear_of_shojin;
mod spirit_visage;
mod steel_sigil;
mod stormrazor;
mod sundered_sky;
mod terminus;
mod unending_despair;
mod void_staff;
mod warmogs_armor;
mod yun_tal_wildarrows;
mod zekes_herald;

use aegis_of_the_legion::*;
use bandleglass_mirror::*;
use bf_sword::*;
use black_cleaver::*;
use blackfire_torch::*;
use blade_of_the_ruined_king::*;
use blighting_jewel::*;
use bloodletters_curse::*;
use collector::*;
use deathblade::*;
use deaths_dance::*;
use diamond_tipped_spear::*;
use echoes_of_helia::*;
use executioners_calling::*;
use experimental_hexplate::*;
use frozen_mallet::*;
use guinsoos_rageblade::*;
use haunting_guise::*;
use heartsteel::*;
use hextech_gunblade::*;
use infinity_edge::*;
use jaksho_the_protean::*;
use liandrys_torment::*;
use mirage_blade::*;
use morellonomicon::*;
use mortal_reminder::*;
use nashors_tooth::*;
use needlessly_large_rod::*;
use night_harvester::*;
use noonquiver::*;
use oblivion_orb::*;
use overlords_bloodmail::*;
use phage::*;
use protectors_vow::*;
use protoplasm_harness::*;
use rabadons_deathcap::*;
use riftmaker::*;
use rylais_crystal_scepter::*;
use scouts_slingshot::*;
use shadowflame::*;
use spear_of_shojin::*;
use spirit_visage::*;
use steel_sigil::*;
use stormrazor::*;
use sundered_sky::*;
use terminus::*;
use unending_despair::*;
use void_staff::*;
use warmogs_armor::*;
use yun_tal_wildarrows::*;
use zekes_herald::*;

pub(crate) use constants::*;

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn percent_of_i32(value: i32, percent: f64) -> i32 {
    (value as f64 * percent / 100.0).round() as i32
}

fn apply_adaptive_force(ctx: &mut GameCtx, player: usize, adaptive_force: i32, buff_name: &str) {
    let Some(player_ref) = ctx.get_player(player) else {
        return;
    };
    let Some(entity_ref) = player_ref.champion() else {
        return;
    };

    let is_buff_applied =
        (0..entity_ref.buff_count()).any(|i| entity_ref.buff_at(i).name.as_str() == buff_name);

    if !is_buff_applied {
        if entity_ref.stat().magic_power > entity_ref.stat().attack {
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Permanent,
                    magic_power: adaptive_force,
                    name: buff_name.try_into().unwrap(),
                    ..Default::default()
                },
            )
        } else {
            ctx.add_buff(
                entity_ref.id(),
                BuffState {
                    duration: BuffType::Permanent,
                    attack: (adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32,
                    name: buff_name.try_into().unwrap(),
                    ..Default::default()
                },
            )
        }
    }
}

// Installs the experimental item-build route hook when the server starts. The
// hook is fail-closed (see `hook.rs`): on any mismatch it records a refusal and
// leaves the game function untouched.
struct ItemBuildHookExtension;

impl ModServerExtension for ItemBuildHookExtension {
    fn on_server_start(&self, _ctx: &mut ServerModContext<'_>) {
        match hook::install_hook() {
            Ok(address) => {
                let message = format!("hook_installed address=0x{address:x}");
                eprintln!("riot_items_tfm2: {message}");
            }
            Err(error) if error == "hook already installed" => {}
            Err(error) => {
                let message = format!("hook_refused error={error}");
                eprintln!("riot_items_tfm2: {message}");
            }
        }
    }
}

fn init(_ctx: &GameCtx) -> ModRegistration {
    let mut reg = ModRegistration::new("riot_items_tfm2");
    let configs = config::load();

    macro_rules! configured {
        ($key:literal => $T:ty) => {
            configs.get($key).map(<$T>::with_config).unwrap_or_default()
        };
    }

    // Tier 2
    reg.add_item(configured!("executioners_calling" => ExecutionersCalling));
    reg.add_item(configured!("oblivion_orb" => OblivionOrb));

    // Tier 3
    reg.add_item(configured!("aegis_of_the_legion" => AegisOfTheLegion));
    reg.add_item(configured!("bandleglass_mirror" => BandleglassMirror));
    reg.add_item(configured!("bf_sword" => BFSword));
    reg.add_item(configured!("blighting_jewel" => BlightingJewel));
    reg.add_item(configured!("haunting_guise" => HauntingGuise));
    reg.add_item(configured!("needlessly_large_rod" => NeedlesslyLargeRod));
    reg.add_item(configured!("noonquiver" => Noonquiver));
    reg.add_item(configured!("phage" => Phage));
    reg.add_item(configured!("scouts_slingshot" => ScoutsSlingshot));
    reg.add_item(configured!("steel_sigil" => SteelSigil));

    // Tier 4
    reg.add_item(configured!("black_cleaver" => BlackCleaver));
    reg.add_item(configured!("blackfire_torch" => BlackfireTorch));
    reg.add_item(configured!("blade_of_the_ruined_king" => BladeOfTheRuinedKing));
    reg.add_item(configured!("bloodletters_curse" => BloodlettersCurse));
    reg.add_item(configured!("collector" => Collector));
    reg.add_item(configured!("deathblade" => DeathBlade));
    reg.add_item(configured!("deaths_dance" => DeathsDance));
    reg.add_item(configured!("diamond_tipped_spear" => DiamondTippedSpear));
    reg.add_item(configured!("echoes_of_helia" => EchoesOfHelia));
    reg.add_item(configured!("experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(configured!("frozen_mallet" => FrozenMallet));
    reg.add_item(configured!("guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured!("heartsteel" => Heartsteel));
    reg.add_item(configured!("hextech_gunblade" => HextechGunblade));
    reg.add_item(configured!("infinity_edge" => InfinityEdge));
    reg.add_item(configured!("jaksho_the_protean" => JakshoTheProtean));
    reg.add_item(configured!("liandrys_torment" => LiandrysTorment));
    reg.add_item(configured!("mirage_blade" => MirageBlade));
    reg.add_item(configured!("morellonomicon" => Morellonomicon));
    reg.add_item(configured!("mortal_reminder" => MortalReminder));
    reg.add_item(configured!("nashors_tooth" => NashorsTooth));
    reg.add_item(configured!("night_harvester" => NightHarvester));
    reg.add_item(configured!("overlords_bloodmail" => OverlordsBloodmail));
    reg.add_item(configured!("protectors_vow" => ProtectorsVow));
    reg.add_item(configured!("protoplasm_harness" => ProtoplasmHarness));
    reg.add_item(configured!("rabadons_deathcap" => RabadonsDeathcap));
    reg.add_item(configured!("riftmaker" => Riftmaker));
    reg.add_item(configured!("rylais_crystal_scepter" => RylaisCrystalScepter));
    reg.add_item(configured!("shadowflame" => Shadowflame));
    reg.add_item(configured!("spear_of_shojin" => SpearOfShojin));
    reg.add_item(configured!("spirit_visage" => SpiritVisage));
    reg.add_item(configured!("stormrazor" => Stormrazor));
    reg.add_item(configured!("sundered_sky" => SunderedSky));
    reg.add_item(configured!("terminus" => Terminus));
    reg.add_item(configured!("unending_despair" => UnendingDespair));
    reg.add_item(configured!("void_staff" => VoidStaff));
    reg.add_item(configured!("warmogs_armor" => WarmogsArmor));
    reg.add_item(configured!("yun_tal_wildarrows" => YunTalWildarrows));
    reg.add_item(configured!("zekes_herald" => ZekesHerald));

    // Tier 5
    reg.add_item(configured!("radiant_black_cleaver" => RadiantBlackCleaver));
    reg.add_item(configured!("radiant_blackfire_torch" => RadiantBlackfireTorch));
    reg.add_item(configured!("radiant_blade_of_the_ruined_king" => RadiantBladeOfTheRuinedKing));
    reg.add_item(configured!("radiant_bloodletters_curse" => RadiantBloodlettersCurse));
    reg.add_item(configured!("radiant_collector" => RadiantCollector));
    reg.add_item(configured!("radiant_deathblade" => RadiantDeathBlade));
    reg.add_item(configured!("radiant_deaths_dance" => RadiantDeathsDance));
    reg.add_item(configured!("radiant_diamond_tipped_spear" => RadiantDiamondTippedSpear));
    reg.add_item(configured!("radiant_echoes_of_helia" => RadiantEchoesOfHelia));
    reg.add_item(configured!("radiant_experimental_hexplate" => RadiantExperimentalHexplate));
    reg.add_item(configured!("radiant_frozen_mallet" => RadiantFrozenMallet));
    reg.add_item(configured!("radiant_guinsoos_rageblade" => RadiantGuinsoosRageblade));
    reg.add_item(configured!("radiant_heartsteel" => RadiantHeartsteel));
    reg.add_item(configured!("radiant_hextech_gunblade" => RadiantHextechGunblade));
    reg.add_item(configured!("radiant_infinity_edge" => RadiantInfinityEdge));
    reg.add_item(configured!("radiant_jaksho_the_protean" => RadiantJakshoTheProtean));
    reg.add_item(configured!("radiant_liandrys_torment" => RadiantLiandrysTorment));
    reg.add_item(configured!("radiant_mirage_blade" => RadiantMirageBlade));
    reg.add_item(configured!("radiant_morellonomicon" => RadiantMorellonomicon));
    reg.add_item(configured!("radiant_mortal_reminder" => RadiantMortalReminder));
    reg.add_item(configured!("radiant_nashors_tooth" => RadiantNashorsTooth));
    reg.add_item(configured!("radiant_night_harvester" => RadiantNightHarvester));
    reg.add_item(configured!("radiant_overlords_bloodmail" => RadiantOverlordsBloodmail));
    reg.add_item(configured!("radiant_protectors_vow" => RadiantProtectorsVow));
    reg.add_item(configured!("radiant_protoplasm_harness" => RadiantProtoplasmHarness));
    reg.add_item(configured!("radiant_rabadons_deathcap" => RadiantRabadonsDeathcap));
    reg.add_item(configured!("radiant_riftmaker" => RadiantRiftmaker));
    reg.add_item(configured!("radiant_rylais_crystal_scepter" => RadiantRylaisCrystalScepter));
    reg.add_item(configured!("radiant_shadowflame" => RadiantShadowflame));
    reg.add_item(configured!("radiant_spear_of_shojin" => RadiantSpearOfShojin));
    reg.add_item(configured!("radiant_spirit_visage" => RadiantSpiritVisage));
    reg.add_item(configured!("radiant_stormrazor" => RadiantStormrazor));
    reg.add_item(configured!("radiant_sundered_sky" => RadiantSunderedSky));
    reg.add_item(configured!("radiant_terminus" => RadiantTerminus));
    reg.add_item(configured!("radiant_unending_despair" => RadiantUnendingDespair));
    reg.add_item(configured!("radiant_void_staff" => RadiantVoidStaff));
    reg.add_item(configured!("radiant_warmogs_armor" => RadiantWarmogsArmor));
    reg.add_item(configured!("radiant_yun_tal_wildarrows" => RadiantYunTalWildarrows));
    reg.add_item(configured!("radiant_zekes_herald" => RadiantZekesHerald));

    reg.set_server_extension(ItemBuildHookExtension);

    reg
}

declare_mod!(init);
