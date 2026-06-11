use mod_api::*;

mod bf_sword;
mod blackfire_torch;
mod blade_of_the_ruined_king;
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

    // Tier 2
    reg.add_item(ExecutionersCalling::default());

    // Tier 3
    reg.add_item(BFSword::default());
    reg.add_item(NeedlesslyLargeRod::default());

    // Tier 4
    reg.add_item(BlackfireTorch::default());
    reg.add_item(BladeOfTheRuinedKing::default());
    reg.add_item(DeathBlade::default());
    reg.add_item(ExperimentalHexplate::default());
    reg.add_item(FrozenMallet::default());
    reg.add_item(GuinsoosRageblade::default());
    reg.add_item(InfinityEdge::default());
    reg.add_item(JakshoTheProtean::default());
    reg.add_item(MirageBlade::default());
    reg.add_item(MortalReminder::default());
    reg.add_item(NashorsTooth::default());
    reg.add_item(ProtectorsVow::default());
    reg.add_item(ProtoplasmHarness::default());
    reg.add_item(RabadonsDeathcap::default());
    reg.add_item(Riftmaker::default());
    reg.add_item(SpiritVisage::default());
    reg.add_item(Terminus::default());
    reg.add_item(UnendingDespair::default());

    // Tier 5
    reg.add_item(RadiantBlackfireTorch::default());
    reg.add_item(RadiantBladeOfTheRuinedKing::default());
    reg.add_item(RadiantDeathBlade::default());
    reg.add_item(RadiantExperimentalHexplate::default());
    reg.add_item(RadiantFrozenMallet::default());
    reg.add_item(RadiantGuinsoosRageblade::default());
    reg.add_item(RadiantInfinityEdge::default());
    reg.add_item(RadiantJakshoTheProtean::default());
    reg.add_item(RadiantMirageBlade::default());
    reg.add_item(RadiantMortalReminder::default());
    reg.add_item(RadiantNashorsTooth::default());
    reg.add_item(RadiantProtectorsVow::default());
    reg.add_item(RadiantProtoplasmHarness::default());
    reg.add_item(RadiantRabadonsDeathcap::default());
    reg.add_item(RadiantRiftmaker::default());
    reg.add_item(RadiantSpiritVisage::default());
    reg.add_item(RadiantTerminus::default());
    reg.add_item(RadiantUnendingDespair::default());

    reg
}

declare_mod!(init);
