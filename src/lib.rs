use arrayvec::ArrayString;
use mod_api::*;

mod build_config;
mod config;
mod constants;
mod hook;
mod item_meta;
mod items;

use items::*;

pub(crate) use constants::*;
pub(crate) use item_meta::ItemMeta;

fn percent_of(value: usize, percent: f64) -> usize {
    (value as f64 * percent / 100.0).round() as usize
}

fn percent_of_i32(value: i32, percent: f64) -> i32 {
    (value as f64 * percent / 100.0).round() as i32
}

/// Converts a duration in seconds (how config expresses them) to simulation
/// ticks (what `BuffType::Time` expects).
fn ticks(seconds: f64) -> usize {
    (seconds * TICKS_PER_SECOND).round() as usize
}

/// Converts a buff name to the fixed-capacity inline string that
/// `BuffState::name` holds. Panics if `name` exceeds that capacity — buff names
/// are compile-time constants, so that is a typo to fix, not a runtime condition.
fn buff_name<const CAP: usize>(name: &str) -> ArrayString<CAP> {
    ArrayString::try_from(name).unwrap()
}

/// Whether `entity` currently carries a buff named `name`. Buffs are stored in a
/// flat per-entity table with no lookup by name, so this is a linear scan.
fn has_buff(entity: &EntityRef, name: &str) -> bool {
    (0..entity.buff_count()).any(|i| entity.buff_at(i).name.as_str() == name)
}

/// How many buffs named `name` are on `entity`. Same-name buffs stack rather
/// than refresh, so the number of copies present *is* the stack count.
fn buff_stacks(entity: &EntityRef, name: &str) -> usize {
    (0..entity.buff_count())
        .filter(|&i| entity.buff_at(i).name.as_str() == name)
        .count()
}

/// Rate-limits an on-hit effect to once per `cooldown_seconds` per target.
///
/// Returns `true` when the caller should fire its effect (the marker is stamped as
/// a side effect), `false` while the previous proc is still on cooldown or the
/// target no longer exists.
fn try_proc_on_hit(ctx: &mut GameCtx, target: usize, marker: &str, cooldown_seconds: f64) -> bool {
    let is_ready = ctx
        .get_entity(target)
        .map(|target_ref| !has_buff(&target_ref, marker))
        .unwrap_or(false);
    if !is_ready {
        return false;
    }
    ctx.add_buff(
        target,
        BuffState {
            duration: BuffType::Time {
                tick: ticks(cooldown_seconds),
            },
            name: buff_name(marker),
            ..Default::default()
        },
    );
    true
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

/// Scales a basic attack's `damage` to simulate `lethality` flat armor
/// penetration against `target` (via [`lethality_multiplier`]). Basic attacks are
/// the only damage instance a mod can modify — ability damage is dealt by the
/// game — so lethality items apply this in `on_attack`.
fn apply_lethality(ctx: &mut GameCtx, target: usize, lethality: usize, damage: &mut usize) {
    let armor = ctx
        .get_entity(target)
        .map(|t| t.stat().defence as i32)
        .unwrap_or(0);
    let mult = lethality_multiplier(armor, lethality as i32);
    *damage = (*damage as f64 * mult).round() as usize;
}

/// How long (in ticks) an enemy champion stays "marked" as recently damaged, so a
/// death within the window counts as a takedown (kill or assist). 3s at 60/s.
const TAKEDOWN_WINDOW_TICKS: usize = 180;

/// Whether `target` is a champion on the opposing team from `caster`. Both the
/// takedown marks and the "in combat with an enemy champion" timers key off this,
/// and neither should fire on minions, towers, or a friendly hit.
fn is_enemy_champion(ctx: &mut GameCtx, caster: usize, target: usize) -> bool {
    let Some(caster_ref) = ctx.get_entity(caster) else {
        return false;
    };
    let caster_team = caster_ref.team();
    ctx.get_entity(target)
        .map(|target_ref| target_ref.is_champion() && target_ref.team() != caster_team)
        .unwrap_or(false)
}

/// Marks `target` as recently damaged, if it is an enemy champion of `caster`, so
/// that a death within `TAKEDOWN_WINDOW_TICKS` counts as a takedown. Refreshes an
/// existing mark. Shared by items whose passives trigger on takedowns (which the
/// `on_kill` hook can't identify — its `entity` arg always looks like a champion).
fn mark_enemy_champion(
    marks: &mut Vec<(usize, usize)>,
    ctx: &mut GameCtx,
    caster: usize,
    target: usize,
) {
    if !is_enemy_champion(ctx, caster, target) {
        return;
    }
    if let Some(mark) = marks.iter_mut().find(|(id, _)| *id == target) {
        mark.1 = TAKEDOWN_WINDOW_TICKS;
    } else {
        marks.push((target, TAKEDOWN_WINDOW_TICKS));
    }
}

/// Ages `marks` by one tick and returns how many marked champions died this tick
/// (each a takedown). Marks are dropped on death (counted once) or when the window
/// lapses without a death. Call once per `update`.
fn count_takedowns(marks: &mut Vec<(usize, usize)>, ctx: &mut GameCtx) -> usize {
    if marks.is_empty() {
        return 0;
    }
    let mut takedowns = 0;
    let mut kept = Vec::with_capacity(marks.len());
    for (id, ticks_left) in std::mem::take(marks) {
        // A marked champion counts as a takedown once the game stops reporting it as
        // alive: either it's gone from the entity table (`get_entity` -> None, which
        // is how TFM2 removes a champion killed this round) or it's still queryable
        // but flagged not-alive. We only mark enemy champions we damaged within the
        // last few seconds, so a disappearance here is a death.
        let is_dead = ctx.get_entity(id).map(|e| !e.is_alive()).unwrap_or(true);
        if is_dead {
            takedowns += 1;
            continue;
        }
        let remaining = ticks_left.saturating_sub(1);
        if remaining > 0 {
            kept.push((id, remaining));
        }
    }
    *marks = kept;
    takedowns
}

fn apply_adaptive_force(ctx: &mut GameCtx, player: usize, adaptive_force: i32, name: &str) {
    let Some(player_ref) = ctx.get_player(player) else {
        return;
    };
    let Some(champion_ref) = player_ref.champion() else {
        return;
    };

    if !has_buff(&champion_ref, name) {
        if champion_ref.stat().magic_power > champion_ref.stat().attack {
            ctx.add_buff(
                champion_ref.id(),
                BuffState {
                    duration: BuffType::Permanent,
                    magic_power: adaptive_force,
                    name: buff_name(name),
                    ..Default::default()
                },
            )
        } else {
            ctx.add_buff(
                champion_ref.id(),
                BuffState {
                    duration: BuffType::Permanent,
                    attack: (adaptive_force as f64 * ADAPTIVE_FORCE_AD_RATIO).round() as i32,
                    name: buff_name(name),
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
    macro_rules! configured_radiant {
        ($key:literal => $T:ty) => {
            configs
                .get($key)
                .map(<$T>::radiant_with_config)
                .unwrap_or_else(<$T>::radiant)
        };
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
    reg.add_item(configured!("haunting_guise" => HauntingGuise));
    reg.add_item(configured!("last_whisper" => LastWhisper));
    reg.add_item(configured!("needlessly_large_rod" => NeedlesslyLargeRod));
    reg.add_item(configured!("noonquiver" => Noonquiver));
    reg.add_item(configured!("phage" => Phage));
    reg.add_item(configured!("scouts_slingshot" => ScoutsSlingshot));
    reg.add_item(configured!("steel_sigil" => SteelSigil));

    // Tier 4
    reg.add_item(configured!("atmas_reckoning" => AtmasReckoning));
    reg.add_item(configured!("bastionbreaker" => Bastionbreaker));
    reg.add_item(configured!("black_cleaver" => BlackCleaver));
    reg.add_item(configured!("blackfire_torch" => BlackfireTorch));
    reg.add_item(configured!("blade_of_the_ruined_king" => BladeOfTheRuinedKing));
    reg.add_item(configured!("bloodletters_curse" => BloodlettersCurse));
    reg.add_item(configured!("bloodsong" => Bloodsong));
    reg.add_item(configured!("collector" => Collector));
    reg.add_item(configured!("deathblade" => DeathBlade));
    reg.add_item(configured!("deaths_dance" => DeathsDance));
    reg.add_item(configured!("diamond_tipped_spear" => DiamondTippedSpear));
    reg.add_item(configured!("dusk_and_dawn" => DuskAndDawn));
    reg.add_item(configured!("echoes_of_helia" => EchoesOfHelia));
    reg.add_item(configured!("experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(configured!("frozen_mallet" => FrozenMallet));
    reg.add_item(configured!("guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured!("heartsteel" => Heartsteel));
    reg.add_item(configured!("hextech_gunblade" => HextechGunblade));
    reg.add_item(configured!("hubris" => Hubris));
    reg.add_item(configured!("infinity_edge" => InfinityEdge));
    reg.add_item(configured!("jaksho_the_protean" => JakshoTheProtean));
    reg.add_item(configured!("kraken_slayer" => KrakenSlayer));
    reg.add_item(configured!("liandrys_torment" => LiandrysTorment));
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
    reg.add_item(configured_radiant!("radiant_deathblade" => DeathBlade));
    reg.add_item(configured_radiant!("radiant_deaths_dance" => DeathsDance));
    reg.add_item(configured_radiant!("radiant_diamond_tipped_spear" => DiamondTippedSpear));
    reg.add_item(configured_radiant!("radiant_dusk_and_dawn" => DuskAndDawn));
    reg.add_item(configured_radiant!("radiant_echoes_of_helia" => EchoesOfHelia));
    reg.add_item(configured_radiant!("radiant_experimental_hexplate" => ExperimentalHexplate));
    reg.add_item(configured_radiant!("radiant_frozen_mallet" => FrozenMallet));
    reg.add_item(configured_radiant!("radiant_guinsoos_rageblade" => GuinsoosRageblade));
    reg.add_item(configured_radiant!("radiant_heartsteel" => Heartsteel));
    reg.add_item(configured_radiant!("radiant_hextech_gunblade" => HextechGunblade));
    reg.add_item(configured_radiant!("radiant_hubris" => Hubris));
    reg.add_item(configured_radiant!("radiant_infinity_edge" => InfinityEdge));
    reg.add_item(configured_radiant!("radiant_jaksho_the_protean" => JakshoTheProtean));
    reg.add_item(configured_radiant!("radiant_kraken_slayer" => KrakenSlayer));
    reg.add_item(configured_radiant!("radiant_liandrys_torment" => LiandrysTorment));
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
    reg.add_item(configured_radiant!("radiant_warmogs_armor" => WarmogsArmor));
    reg.add_item(configured_radiant!("radiant_wits_end" => WitsEnd));
    reg.add_item(configured_radiant!("radiant_yun_tal_wildarrows" => YunTalWildarrows));
    reg.add_item(configured_radiant!("radiant_zekes_herald" => ZekesHerald));

    reg.set_server_extension(ItemBuildHookExtension);

    reg
}

declare_mod!(init);
