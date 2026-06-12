use mod_api::*;

mod bf_sword;
mod blackfire_torch;
mod blade_of_the_ruined_king;
mod config;
mod deathblade;
mod executioners_calling;
mod experimental_hexplate;
mod frozen_mallet;
mod guinsoos_rageblade;
mod infinity_edge;
mod jaksho_the_protean;
mod mirage_blade;
mod mortal_reminder;
mod nashors_tooth;
mod needlessly_large_rod;
mod protectors_vow;
mod protoplasm_harness;
mod rabadons_deathcap;
mod riftmaker;
mod spirit_visage;
mod terminus;
mod unending_despair;

use bf_sword::*;
use blackfire_torch::*;
use blade_of_the_ruined_king::*;
use deathblade::*;
use executioners_calling::*;
use experimental_hexplate::*;
use frozen_mallet::*;
use guinsoos_rageblade::*;
use infinity_edge::*;
use jaksho_the_protean::*;
use mirage_blade::*;
use mortal_reminder::*;
use nashors_tooth::*;
use needlessly_large_rod::*;
use protectors_vow::*;
use protoplasm_harness::*;
use rabadons_deathcap::*;
use riftmaker::*;
use spirit_visage::*;
use terminus::*;
use unending_despair::*;

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn percent_of_i32(value: i32, percent: f64) -> i32 {
    (value as f64 * percent / 100.0).round() as i32
}

fn force_to_ap(force: i32) -> i32 {
    force
}

fn force_to_ad(force: i32) -> i32 {
    (force as f64 * 0.6).round() as i32
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

    // Tier 3
    reg.add_item(configured!("bf_sword" => BFSword));
    reg.add_item(configured!("needlessly_large_rod" => NeedlesslyLargeRod));

    // Tier 4
    reg.add_item(configured!("blackfire_torch" => BlackfireTorch));
    reg.add_item(configured!("blade_of_the_ruined_king" => BladeOfTheRuinedKing));
    reg.add_item(configured!("deathblade" => DeathBlade));
    reg.add_item(configured!("experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(FrozenMallet::default());
    reg.add_item(configured!("guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured!("infinity_edge" => InfinityEdge));
    reg.add_item(JakshoTheProtean::default());
    reg.add_item(configured!("mirage_blade" => MirageBlade));
    reg.add_item(MortalReminder::default());
    reg.add_item(NashorsTooth::default());
    reg.add_item(ProtectorsVow::default());
    reg.add_item(ProtoplasmHarness::default());
    reg.add_item(configured!("rabadons_deathcap" => RabadonsDeathcap));
    reg.add_item(Riftmaker::default());
    reg.add_item(SpiritVisage::default());
    reg.add_item(Terminus::default());
    reg.add_item(UnendingDespair::default());

    // Tier 5
    reg.add_item(configured!("radiant_blackfire_torch" => RadiantBlackfireTorch));
    reg.add_item(configured!("radiant_blade_of_the_ruined_king" => RadiantBladeOfTheRuinedKing));
    reg.add_item(configured!("radiant_deathblade" => RadiantDeathBlade));
    reg.add_item(configured!("radiant_experimental_hexplate" => RadiantExperimentalHexplate));
    reg.add_item(RadiantFrozenMallet::default());
    reg.add_item(configured!("radiant_guinsoos_rageblade" => RadiantGuinsoosRageblade));
    reg.add_item(configured!("radiant_infinity_edge" => RadiantInfinityEdge));
    reg.add_item(RadiantJakshoTheProtean::default());
    reg.add_item(configured!("radiant_mirage_blade" => RadiantMirageBlade));
    reg.add_item(RadiantMortalReminder::default());
    reg.add_item(RadiantNashorsTooth::default());
    reg.add_item(RadiantProtectorsVow::default());
    reg.add_item(RadiantProtoplasmHarness::default());
    reg.add_item(configured!("radiant_rabadons_deathcap" => RadiantRabadonsDeathcap));
    reg.add_item(RadiantRiftmaker::default());
    reg.add_item(RadiantSpiritVisage::default());
    reg.add_item(RadiantTerminus::default());
    reg.add_item(RadiantUnendingDespair::default());

    reg
}

declare_mod!(init);
