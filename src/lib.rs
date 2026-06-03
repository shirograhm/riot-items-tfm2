mod bf_sword;
mod blackfire_torch;
mod blade_of_the_ruined_king;
mod deathblade;
mod executioners_calling;
mod experimental_hexplate;
mod frozen_mallet;
mod infinity_edge;
mod jaksho_the_protean;
mod mortal_reminder;
mod nashors_tooth;
mod needlessly_large_rod;
mod protectors_vow;
mod rabadons_deathcap;
mod riftmaker;
mod terminus;

use bf_sword::*;
use blackfire_torch::*;
use blade_of_the_ruined_king::*;
use deathblade::*;
use executioners_calling::*;
use experimental_hexplate::*;
use frozen_mallet::*;
use infinity_edge::*;
use jaksho_the_protean::*;
use mortal_reminder::*;
use nashors_tooth::*;
use needlessly_large_rod::*;
use protectors_vow::*;
use rabadons_deathcap::*;
use riftmaker::*;
use terminus::*;

use mod_api::*;

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn init(_ctx: &GameCtx) -> ModRegistration {
    let mut reg = ModRegistration::new("riot_items_tfm2");
    reg.add_item(BFSword::default());
    reg.add_item(DeathBlade::default());
    reg.add_item(RadiantDeathBlade::default());
    reg.add_item(InfinityEdge::default());
    reg.add_item(RadiantInfinityEdge::default());
    reg.add_item(NeedlesslyLargeRod::default());
    reg.add_item(Riftmaker::default());
    reg.add_item(RadiantRiftmaker::default());
    reg.add_item(RabadonsDeathcap::default());
    reg.add_item(RadiantRabadonsDeathcap::default());
    reg.add_item(ProtectorsVow::default());
    reg.add_item(RadiantProtectorsVow::default());
    reg.add_item(FrozenMallet::default());
    reg.add_item(RadiantFrozenMallet::default());
    reg.add_item(BladeOfTheRuinedKing::default());
    reg.add_item(RadiantBladeOfTheRuinedKing::default());
    reg.add_item(BlackfireTorch::default());
    reg.add_item(RadiantBlackfireTorch::default());
    reg.add_item(ExperimentalHexplate::default());
    reg.add_item(RadiantExperimentalHexplate::default());
    reg.add_item(ExecutionersCalling::default());
    reg.add_item(MortalReminder::default());
    reg.add_item(RadiantMortalReminder::default());
    reg.add_item(NashorsTooth::default());
    reg.add_item(RadiantNashorsTooth::default());
    reg.add_item(JakshoTheProtean::default());
    reg.add_item(RadiantJakshoTheProtean::default());
    reg.add_item(Terminus::default());
    reg.add_item(RadiantTerminus::default());
    reg
}

declare_mod!(init);
