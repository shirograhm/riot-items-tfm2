//! Both teams' item builds in the in-match Check Tactics panel.
//!
//! Vanilla's Personal Tactics table lists four item columns for one team, and
//! with this mod every cell reads "Let Player Decide": the builds are made by
//! the Build Editor and the item-build hooks, which the game's own personal
//! tactics know nothing about. This draws a table of its own over it: one
//! line a lane, the blue champion's build on the left and the red one's on
//! the right, as item icons with a tooltip for the one under the cursor.
//!
//! # Where the builds come from
//!
//! Nothing hands a mod the builds of the match on screen, so they are pieced
//! together:
//!
//! * **Which match is on screen.** Two ways. The match hook is called for
//!   every simulation, and `sim_origin` says which is the client's own
//!   ([`on_match_tick`]): a watched match says so (`ClientMatchView`, seen in
//!   game), and its players give both lineups by lane and its seed. A
//!   spectated match is not simulated through the hook at all (tried twice,
//!   2026-10-08), so it is found by its athletes instead: the layout's camera
//!   buttons name every athlete by side and lane ([`camera_seats`]), the buy
//!   detour files each grown build under its athlete's id, and the newest
//!   match whose athletes are those, seat for seat, is the one on screen
//!   ([`seed_by_athletes`]).
//! * **What each athlete will buy.** The native buy detour grows every
//!   build to six in the athlete's first moments in the match and reports
//!   what it leaves ([`note_grown`]): all six, in buying order, filed under
//!   the match's seed, side and lane. That is the build a cell shows.
//! * **Until then, or where the detour is not running**, what the item-build
//!   hooks decided for the champion ([`note_decision`]): the game's four
//!   slots, found by lineup, with whatever is pinned in the last two.
//!
//! When no match can be made out nothing is drawn and the vanilla table
//! stays.
//!
//! The draft the player watched is kept as a second source for the lineup
//! ([`note_lineup`], from [`super::draft_watch`] by way of the strategy
//! screen), for a host too old to say where a simulation runs.
//!
//! Whose side the player is on only decides two details: which cells may
//! carry pin borders under `own_team_only` until the buy detour has said so
//! itself ([`Grown::pins`]), and which side's cells can be labelled with the
//! athletes' names the vanilla rows hold ([`player_side`]). A side nobody
//! could be placed on is labelled by champion.
//!
//! # Hover
//!
//! The stable UI API reports clicks and nothing about the cursor: no hover
//! event, no focus, no mouse position (see `item_stats::ui`, which settled for
//! a click for that reason). So the cursor is read from Windows
//! ([`client_cursor`]), scaled from the window's client area into the
//! 1920x1080 space `ui_node_rect` answers in ([`cursor`]), and tested against
//! the slots' own rects. The tooltip is this module's node, not the game's
//! `#item_tooltip`, which game code shows and hides on its own schedule.
//!
//! Seen working in game on a 2560x1440 window (2026-10-08). The scaling
//! assumes the layout is fitted to the window and centred; should that be
//! wrong on some other shape of window, a click on an icon shows the same
//! tooltip ([`handle_event`]). Clicks stop counting once a hover has been
//! seen.
//!
//! While [`LOG`] is on, `match-builds.log` beside the DLL says how each match
//! was identified and what the rules make of every item shown.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::ffi::c_void;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use mod_api_stable::{
    BuffV1, RecordKindV1, SettingTargetV1, SimOriginKindV1, SimOriginV1, StableClient, StableItem,
    StableSim, UiEventKindV1,
};
use serde_json::Value;

use crate::build_config::{self, Role};
use crate::strategy_ui::ICON_SHEET;

// -- paths --------------------------------------------------------------------

/// Root of the in-match layout (`ingame:ingame_ui`), 1920x1080.
const INGAME: &str = "ingame";
/// The Check Tactics panel. Authored hidden; game code shows it.
const PANEL: &str = "ingame.strategy_info";
const PERSONAL: &str = "ingame.strategy_info.personal_panel";
/// Vanilla's column headings and its rows, both hidden while the board is up.
const HEADER: &str = "ingame.strategy_info.personal_panel.header";
const ROWS_PATH: &str = "ingame.strategy_info.personal_panel.rows";
/// The team the panel is about.
const TEAM_NAME: &str = "ingame.strategy_info.team_panel.header.name";
/// Where the layout names the two teams, blue then red: the match header,
/// and the Details tab's two columns.
const SIDE_NAMES: [[&str; 2]; 2] = [
    [
        "ingame.header.blue_info.team_name",
        "ingame.header.red_info.team_name",
    ],
    [
        "ingame.center_detail.stat.center.team_info.name",
        "ingame.center_detail.stat.right.team_info.name",
    ],
];
/// Where the camera buttons are spawned, one child a lane, in the two views.
/// Each child holds a `blue_player` and a `red_player` button with a `text`.
const CAMERA_HOSTS: [&str; 2] = [
    "ingame.center_data.camera_buttons",
    "ingame.wide_data.camera_buttons",
];
const CAMERA_HALVES: [&str; 2] = ["blue_player", "red_player"];

/// This module's table, spawned into the panel, and its tooltip.
const BOARD: &str = "riot_builds";
const BOARD_PATH: &str = "ingame.strategy_info.personal_panel.riot_builds";
const TIP_NAME: &str = "riot_build_tip";
const TIP: &str = "ingame.riot_build_tip";

/// Lanes, from Top, with the icon the layout draws for each.
const LANES: usize = 5;
const LANE_ICONS: [&str; LANES] = ["top", "jungle", "mid", "bottom", "support"];
/// The two sides' cells in a line, their colour, and the heading over each.
const SIDE_NODES: [&str; 2] = ["blue", "red"];
const SIDE_COLOR: [&str; 2] = ["#5b73ffff", "#e0555cff"];
const SIDE_HEADS: [(&str, &str); 2] = [
    ("builds.side_blue", "Blue Team"),
    ("builds.side_red", "Red Team"),
];

// -- geometry -----------------------------------------------------------------

/// The board takes the place of vanilla's headings and rows inside the panel:
/// their x, the headings' y, their width.
const BOARD_X: usize = 15;
const BOARD_Y: usize = 55;
const BOARD_W: usize = 877;
/// Height of the heading line, and where the first lane's line starts.
const HEAD_H: usize = 24;
const LINES_Y: usize = 34;
/// A line is as tall as a vanilla row and as far apart.
const ROW_H: usize = 36;
const LINE_STRIDE: usize = 38;
/// Where each side's cell starts in a line, after the lane's icon, and inside
/// a cell: the side's bar, the portrait, the name, then the slots.
const CELL_X: [usize; 2] = [36, 466];
const CELL_W: usize = 376;
const PORTRAIT_X: usize = 10;
const PORTRAIT: usize = 28;
const NAME_X: usize = 46;
const NAME_W: usize = 112;
const SLOTS_X: usize = 164;
const SLOT: usize = 30;
const SLOT_Y: usize = (ROW_H - SLOT) / 2;
const SLOT_STRIDE: usize = 36;
const ICON: usize = 26;

const SLOT_FILL: &str = "#1d1f2cff";
/// Border of a slot the AI filled, and of one the player pinned: the grey and
/// the teal the panel already uses for its rules and its team name.
const PICKED_LINE: &str = "#4a4c56ff";
const PINNED_LINE: &str = "#37d5b3ff";
const HOVER_LINE: &str = "#e8e8e8ff";
/// How wide each border is. A pin's was one pixel too, at first: on a 1440p
/// screen that came out as a line a pixel and a third wide, blended down to
/// two thirds of its colour, and it was lost beside the gold frames the icons
/// have of their own (the user, 2026-10-08: "more pronounced"). The line is
/// drawn centred on the slot's edge, so at two it still clears the icon.
const PICKED_STROKE: usize = 1;
const PINNED_STROKE: usize = 2;
/// What shows of a pinned slot between its border and its icon: a dark teal
/// where a picked one has [`SLOT_FILL`], so the border reads a pixel wider
/// than it is drawn.
const PINNED_FILL: &str = "#1a6557ff";

/// Wide enough for an effect's sentence to run about fifty characters a line.
/// It was 300 at first, a little over the game's own 274, and read cramped
/// (the user, 2026-10-08).
const TIP_W: usize = 420;
const TIP_PAD: usize = 12;
/// Top of the stat lines, under the icon, name, price and rule.
const TIP_BODY_Y: usize = 64;
const LINE_H: usize = 20;
/// The description label's box while its text is measured: taller than any
/// description, so what comes back is the text's height and not the box's.
const DESC_BOX: usize = 600;
/// Characters of description taken to fill one line, for when the text cannot
/// be measured: one for every eight pixels of the text's width. On the short
/// side, so the estimate runs tall rather than leaving text hanging out of
/// the tooltip.
const DESC_LINE_CHARS: usize = (TIP_W - 2 * TIP_PAD) / 8;
const STATS_PER_LINE: usize = 4;
/// Gap kept between the tooltip and the slot, and the edge of the screen.
const TIP_GAP: f32 = 8.0;
/// Where the tooltip waits for a layout pass before it is placed. Off screen
/// rather than hidden: a hidden node may be skipped by the pass.
const PARKED_Y: i32 = -4000;
/// Frames after a tooltip's text is written at which its placeholders are
/// filled in ([`fill_placeholders`]): one, for the label to have resolved the
/// text in the game's language.
const FILL_AFTER: u32 = 1;
/// Frames after a tooltip's text is written at which its size is read back:
/// a layout pass after the last thing that can change the text.
const MEASURE_AFTER: u32 = 3;

const STAT_ICONS: &str = "asset/base/ui/banpick/champion_stat_icon";

// -- test log ------------------------------------------------------------------

/// Whether `match-builds.log` is written beside the DLL: how each match was
/// identified, a line whenever an answer changes. On while this is being
/// tried in game. Turn it off before a release.
const LOG: bool = true;

/// Lines written in one session at most.
const LOG_LINES: usize = 600;

struct TestLog {
    file: Option<std::fs::File>,
    lines: usize,
    /// What was last written under each key.
    last: HashMap<String, String>,
}

static TEST_LOG: Mutex<Option<TestLog>> = Mutex::new(None);

/// Appends `text` under `key`, unless it is what that key last said.
fn log(key: &str, text: impl FnOnce() -> String) {
    if !LOG {
        return;
    }
    let text = text();
    let Ok(mut guard) = TEST_LOG.lock() else {
        return;
    };
    let log = guard.get_or_insert_with(|| TestLog {
        file: std::fs::File::create(crate::config::mod_dir().join("match-builds.log")).ok(),
        lines: 0,
        last: HashMap::new(),
    });
    if log.lines >= LOG_LINES || log.last.get(key) == Some(&text) {
        return;
    }
    log.lines += 1;
    if let Some(file) = log.file.as_mut() {
        let _ = writeln!(file, "{key}: {text}");
    }
    log.last.insert(key.to_string(), text);
}

// -- item cards ---------------------------------------------------------------

/// What the tooltip says about one item besides its text.
#[derive(Clone)]
struct Card {
    /// Tag in [`ICON_SHEET`].
    frame: String,
    price: usize,
    /// Stat icon tag and value, in [`STAT_ROWS`] order.
    stats: Vec<(&'static str, String)>,
    /// What stands for each placeholder in the item's effect text, for the
    /// few items whose text has any: see [`FILLS`].
    fills: Vec<(&'static str, String)>,
}

/// The placeholders the game's own items carry in their effect text, the
/// settings field each is filled from, and what the field's value is divided
/// by to be shown. Read off the game's own text and settings: Thornmail's
/// `{Flat} + {Ratio}%` of armor is `flat_damage` and `defence_ratio`, the
/// health items' `{Flat} + {Ratio}%` a second is `flat_regen` and
/// `max_hp_regen_ratio`, and the Sunfire line names its four outright. A range
/// is kept in thousandths.
///
/// One placeholder can stand for two fields, on different items; an item has
/// only one of them, and the first it has is the one. This mod's own items
/// have their numbers written into their text and need none of this.
const FILLS: &[(&str, &str, f64)] = &[
    ("Flat", "flat_damage", 1.0),
    ("Flat", "flat_regen", 1.0),
    ("Ratio", "defence_ratio", 1.0),
    ("Ratio", "max_hp_regen_ratio", 1.0),
    ("RegenFlat", "flat_regen", 1.0),
    ("DmgFlat", "flat_aoe_damage", 1.0),
    ("DmgRatio", "max_hp_aoe_ratio", 1.0),
    ("Range", "aoe_range", 1000.0),
];

/// A number as an item's text shows it: whole where it is whole.
fn shown_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.2}");
        text.trim_end_matches('0').to_string()
    }
}

/// What fills each placeholder for one of the game's items, from its object
/// in the settings document.
fn fills_of(object: &serde_json::Map<String, Value>) -> Vec<(&'static str, String)> {
    let mut fills: Vec<(&'static str, String)> = Vec::new();
    for &(placeholder, field, divisor) in FILLS {
        if fills.iter().any(|(known, _)| *known == placeholder) {
            continue;
        }
        if let Some(value) = object.get(field).and_then(Value::as_f64) {
            fills.push((placeholder, shown_number(value / divisor)));
        }
    }
    fills
}

static CARDS: Mutex<Option<HashMap<String, Card>>> = Mutex::new(None);

/// The stats a tooltip lists: field (the settings document and [`BuffV1`] name
/// them alike), its tag in the stat icon sheet, and whether it is a percentage.
const STAT_ROWS: &[(&str, &str, bool)] = &[
    ("attack", "ad_0", false),
    ("attack_mult", "ad_0", true),
    ("magic_power", "ap_0", false),
    ("magic_power_mult", "ap_0", true),
    ("hp", "hp_0", false),
    ("hp_mult", "hp_0", true),
    ("hp_regen", "hp_regen_0", false),
    ("defence", "armor_0", false),
    ("defence_mult", "armor_0", true),
    ("magic_resistance", "magic resistance_0", false),
    ("magic_resistance_mult", "magic resistance_0", true),
    ("attack_speed_mult", "attack_speed_0", true),
    ("crit_chance", "crit_chance_0", true),
    ("move_speed_mult", "speed_0", true),
    ("skill_cooldown_mult", "cdr_0", false),
    ("vamp", "vamp_0", true),
    ("defence_penetration", "armor_pen_0", true),
    ("magic_resistance_penetration", "magic_pen_0", true),
    ("toughness", "tenacity_0", true),
    ("skill_damaged_reduce", "skill_damage_reduction_0", true),
];

fn stat_lines(value: impl Fn(&str) -> i64) -> Vec<(&'static str, String)> {
    STAT_ROWS
        .iter()
        .filter_map(|&(field, tag, percent)| {
            let amount = value(field);
            (amount != 0).then(|| {
                let text = if percent {
                    format!("{amount}%")
                } else {
                    amount.to_string()
                };
                (tag, text)
            })
        })
        .collect()
}

fn buff_field(stat: &BuffV1, field: &str) -> i64 {
    match field {
        "attack" => stat.attack as i64,
        "attack_mult" => stat.attack_mult as i64,
        "magic_power" => stat.magic_power as i64,
        "magic_power_mult" => stat.magic_power_mult as i64,
        "hp" => stat.hp as i64,
        "hp_mult" => stat.hp_mult as i64,
        "hp_regen" => stat.hp_regen as i64,
        "defence" => stat.defence as i64,
        "defence_mult" => stat.defence_mult as i64,
        "magic_resistance" => stat.magic_resistance as i64,
        "magic_resistance_mult" => stat.magic_resistance_mult as i64,
        "attack_speed_mult" => stat.attack_speed_mult as i64,
        "crit_chance" => stat.crit_chance as i64,
        "move_speed_mult" => stat.move_speed_mult as i64,
        "skill_cooldown_mult" => stat.skill_cooldown_mult as i64,
        "vamp" => stat.vamp as i64,
        "defence_penetration" => stat.defence_penetration as i64,
        "magic_resistance_penetration" => stat.magic_resistance_penetration as i64,
        "toughness" => stat.toughness as i64,
        "skill_damaged_reduce" => stat.skill_damaged_reduce as i64,
        _ => 0,
    }
}

/// Records one of this mod's items as it is registered, from the registration
/// macros in `lib.rs`, where the item is still a concrete type.
pub(crate) fn note_mod_item<T: StableItem + ?Sized>(key: &str, item: &T) {
    let stat = item.stat();
    let card = Card {
        frame: item.icon(),
        price: item.price(),
        stats: stat_lines(|field| buff_field(&stat, field)),
        fills: Vec::new(),
    };
    if let Ok(mut cards) = CARDS.lock() {
        cards
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), card);
    }
}

static ENGINE_CARDS: AtomicBool = AtomicBool::new(false);

/// Frames between two tries of [`prime_engine_cards`] while the settings
/// document cannot be read.
const CARDS_RETRY_FRAMES: u32 = 30;

/// Describes the game's own items, which only the settings document does.
/// Called while a build is on screen, until it succeeds. An item this mod
/// registered keeps the card it was noted with.
fn prime_engine_cards(ctx: &StableClient<'_>) {
    static FRAME: AtomicU32 = AtomicU32::new(0);
    if ENGINE_CARDS.load(Ordering::Relaxed)
        || FRAME.fetch_add(1, Ordering::Relaxed) % CARDS_RETRY_FRAMES != 0
    {
        return;
    }
    let Some(json) = ctx.setting_get_json(SettingTargetV1::ItemSetting, "") else {
        return;
    };
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&json) else {
        return;
    };
    let mut found = Vec::new();
    crate::item_stats::each_item(
        &root,
        0,
        &mut |key: &str, object: &serde_json::Map<String, Value>| {
            let stat = |field: &str| {
                object
                    .get("stat")
                    .and_then(|stat| stat.get(field))
                    .and_then(|value| {
                        value
                            .as_i64()
                            .or_else(|| value.as_f64().map(|value| value as i64))
                    })
                    .unwrap_or(0)
            };
            let card = Card {
                frame: object
                    .get("icon")
                    .and_then(Value::as_str)
                    .filter(|icon| !icon.is_empty())
                    .unwrap_or(key)
                    .to_string(),
                price: object.get("price").and_then(Value::as_u64).unwrap_or(0) as usize,
                stats: stat_lines(stat),
                fills: fills_of(object),
            };
            found.push((key.to_string(), card));
        },
    );
    // Mods' items are in the document from the moment they register, and
    // the game's own only around a match: it is read for good once one of
    // the game's is in it, and not before. See
    // `item_stats::prime_item_traits`, which was caught out by exactly that.
    let games = found
        .iter()
        .any(|(key, _)| key == crate::item_stats::A_GAME_ITEM);
    if let Ok(mut cards) = CARDS.lock() {
        let cards = cards.get_or_insert_with(HashMap::new);
        for (key, card) in found {
            cards.entry(key).or_insert(card);
        }
    }
    if games {
        ENGINE_CARDS.store(true, Ordering::Relaxed);
    }
}

fn card(key: &str) -> Option<Card> {
    CARDS.lock().ok()?.as_ref()?.get(key).cloned()
}

// -- decisions ----------------------------------------------------------------

/// One slot of a build: the item, and whether the player pinned it there.
#[derive(Clone, PartialEq)]
struct Slot {
    key: Option<String>,
    pinned: bool,
}

/// One build either item-build hook settled on, as item keys.
struct Decision {
    /// The champion's key, as the hook gave it.
    champion: String,
    /// The lane, where the hook was told it.
    lane: Option<usize>,
    /// The champion's own team, itself included, and the other one. Sorted.
    team: Vec<String>,
    enemies: Vec<String>,
    /// The game's slots, in order: what anyone playing the champion is handed.
    build: Vec<String>,
    /// What the player's own athlete is handed instead, where `own_team_only`
    /// makes that another build.
    own: Option<Vec<String>>,
}

/// Decisions kept: sixty matches' worth. The player's are read within a
/// second of being made, so this only has to outlast the fixtures decided in
/// between.
const DECISIONS_KEPT: usize = 600;

static DECISIONS: Mutex<VecDeque<Decision>> = Mutex::new(VecDeque::new());

/// A champion's name as this module compares it. The hooks give a champion's
/// key and the simulation an entity's name; they are the same word (seen in
/// game), and this keeps case and stray space from saying otherwise.
fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

fn champion_set<'a>(champions: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut set: Vec<String> = champions.map(fold).collect();
    set.sort();
    set.dedup();
    set
}

/// Records the build a champion was handed. Called by both item-build hooks
/// for every athlete of every match, on whichever thread decides it.
pub(crate) fn note_decision(
    champion: &str,
    lane: Option<usize>,
    allies: &[&str],
    enemies: &[&str],
    build: Vec<String>,
    own: Option<Vec<String>>,
) {
    let decision = Decision {
        champion: champion.to_string(),
        lane: lane.filter(|lane| *lane < LANES),
        team: champion_set(allies.iter().copied().chain([champion])),
        enemies: champion_set(enemies.iter().copied()),
        build,
        own,
    };
    let Ok(mut decisions) = DECISIONS.lock() else {
        return;
    };
    if decisions.len() >= DECISIONS_KEPT {
        decisions.pop_front();
    }
    decisions.push_back(decision);
}

/// A build as the slots a cell draws.
///
/// A slot is pinned where the pin row names the item it holds, when `pins`
/// apply to this athlete at all. A build the buy detour has grown is whole.
/// One it has not holds the game's slots, and past them only what is pinned
/// there: no hook decides those.
fn slots_of(champion: &str, lane: usize, keys: &[String], pins: bool, grown: bool) -> Vec<Slot> {
    let row = if pins {
        // Publishes the pin snapshot `pin_row` reads.
        build_config::load_cached();
        build_config::pin_row(champion, Role::from_lane_code(lane))
    } else {
        Vec::new()
    };
    let pin = |slot: usize| row.get(slot).and_then(Option::as_ref);
    let (game_slots, all) = (build_config::game_slots(), build_config::picker_slots());
    let empty = Slot {
        key: None,
        pinned: false,
    };

    let mut slots: Vec<Slot> = keys
        .iter()
        .take(if grown { all } else { game_slots })
        .enumerate()
        .map(|(slot, key)| Slot {
            pinned: pin(slot) == Some(key),
            key: Some(key.clone()),
        })
        .collect();
    if !grown {
        slots.resize(game_slots, empty.clone());
        for slot in game_slots..all {
            let key = pin(slot).cloned();
            slots.push(Slot {
                pinned: key.is_some(),
                key,
            });
        }
    }
    slots.resize(all, empty);
    slots
}

// -- grown builds -------------------------------------------------------------

/// One athlete's whole build once the buy detour has grown it past the game's
/// four slots: what it will buy, in the order it will buy it.
struct Grown {
    /// The match's seed, or 0 where the detour could not read one.
    seed: u64,
    /// 0 blue, 1 red, as the athlete has it.
    side: u64,
    lane: usize,
    /// The athlete's id, where the detour could read it.
    athlete: Option<u64>,
    champion: String,
    keys: Vec<String>,
    /// The items the growth added and how each was come by, for the test log.
    picks: Vec<(String, &'static str)>,
    /// Whether the detour gave this athlete its pins: everyone with
    /// `own_team_only` off, the player's own with it on, and both sides of a
    /// lane or 5v5 test either way. The detour's own answer, because the
    /// panel's idea of the player's side is one side at most: it left the
    /// red team of a 5v5 test without a single pin border (2026-10-08).
    pins: bool,
}

/// Grown builds kept: two hundred matches' worth. More than the decisions,
/// because a spectated match is found among these alone, and it may have been
/// simulated a good while before it is watched.
const GROWN_KEPT: usize = 2000;

static GROWN: Mutex<VecDeque<Grown>> = Mutex::new(VecDeque::new());

/// Records an athlete's build as the buy detour left it after growing it
/// (`tactics::buy_replace_ctx`). Called on the simulation's thread, once for
/// each athlete of each copy of a match that grows its own.
#[allow(clippy::too_many_arguments)]
pub(crate) fn note_grown(
    seed: u64,
    side: u64,
    lane: usize,
    athlete: Option<u64>,
    champion: &str,
    keys: Vec<String>,
    picks: Vec<(String, &'static str)>,
    pins: bool,
) {
    if lane >= LANES {
        return;
    }
    let Ok(mut grown) = GROWN.lock() else {
        return;
    };
    // A match is played in several copies and each grows the same build for
    // the same seat: one entry a seat.
    if seed != 0 {
        let known = grown
            .iter_mut()
            .rev()
            .find(|known| known.seed == seed && known.side == side && known.lane == lane);
        if let Some(known) = known {
            known.athlete = athlete;
            known.champion = champion.to_string();
            known.keys = keys;
            known.picks = picks;
            known.pins = pins;
            return;
        }
    }
    if grown.len() >= GROWN_KEPT {
        grown.pop_front();
    }
    grown.push_back(Grown {
        seed,
        side,
        lane,
        athlete,
        champion: champion.to_string(),
        keys,
        picks,
        pins,
    });
}

/// A seat's grown build, and how sure it is to be that seat's.
struct Found {
    keys: Vec<String>,
    picks: Vec<(String, &'static str)>,
    how: &'static str,
    /// Whether it was filed under this very seat of this very match.
    exact: bool,
    /// [`Grown::pins`] of the build found. This seat's own answer only where
    /// the build is `exact`.
    pins: bool,
}

/// The grown build of one seat of the match on screen, if the detour has made
/// one. Three ways, the surest first:
///
/// 1. The same seat of the same match, by seed, which is the athlete itself.
/// 2. The newest build grown for this champion in this lane that holds
///    everything the hook decided for it.
/// 3. With the lineup read off the simulation on screen, the newest build
///    grown for this champion in this lane on this side: the match on screen
///    grows its builds as it starts, so that is nearly always its own.
///
/// The last two stand in until the first is there, which the caller keeps
/// looking for.
fn grown_for(
    grown: &VecDeque<Grown>,
    game: &Match,
    side: usize,
    lane: usize,
    seat: &Seat,
) -> Option<Found> {
    let name = fold(&seat.champion);
    let here = |known: &Grown| known.lane == lane && fold(&known.champion) == name;
    let found = |known: &Grown, how: &'static str, exact: bool| Found {
        keys: known.keys.clone(),
        picks: known.picks.clone(),
        how,
        exact,
        pins: known.pins,
    };
    if let Some(seed) = game.seed.filter(|seed| *seed != 0) {
        let same = grown
            .iter()
            .rev()
            .find(|known| here(*known) && known.seed == seed && known.side == side as u64);
        if let Some(known) = same {
            return Some(found(known, "grown, this seat", true));
        }
    }
    if let Some(decided) = seat.decided_for(game.player_side == Some(side)) {
        let held = &decided[..decided.len().min(build_config::game_slots())];
        let same = grown
            .iter()
            .rev()
            .find(|known| here(*known) && held.iter().all(|key| known.keys.contains(key)));
        if let Some(known) = same {
            return Some(found(known, "grown, same four", false));
        }
    }
    if game.on_screen {
        let same = grown
            .iter()
            .rev()
            .find(|known| here(*known) && known.side == side as u64);
        if let Some(known) = same {
            return Some(found(known, "grown, newest for the champion", false));
        }
    }
    None
}

// -- the match on screen ------------------------------------------------------

/// The match on screen, as the simulation that draws it describes it.
#[derive(Clone, Debug)]
struct Screen {
    seed: u64,
    /// The replay record of this set, or [`SimOriginV1::NONE`] while it has
    /// none: a match being played for the first time is only recorded later.
    replay: u64,
    /// Champions by side (0 blue, 1 red) and by lane. A seat is empty until
    /// its champion has been alive on a tick this looked at, and for good in
    /// a match with fewer than five a side.
    sides: [[String; LANES]; 2],
    /// Seats the simulation has a player in.
    seats: usize,
}

impl Screen {
    fn named(&self) -> usize {
        self.sides
            .iter()
            .flatten()
            .filter(|name| !name.is_empty())
            .count()
    }

    /// Whether every player's champion has been read.
    fn complete(&self) -> bool {
        self.seats > 0 && self.named() == self.seats
    }
}

static SCREEN: Mutex<Option<Screen>> = Mutex::new(None);

/// Sim ticks between two looks at a simulation: twice a second.
const SCREEN_EVERY: usize = 30;

/// The kinds of simulation the match hook has been called for, a bit per
/// `SimOriginKindV1` code and the top one for a host that names none. For the
/// test log.
static KINDS_SEEN: AtomicU32 = AtomicU32::new(0);

/// Goes up whenever what a panel is resolved from changes: the match on
/// screen, or the drafted lineup. A panel resolved before that starts over.
static SOURCES: AtomicU32 = AtomicU32::new(0);

/// Reads both lineups off the simulation the client is showing. Called from
/// the mod's match hook on every tick of every simulation, so it leaves at
/// once for any tick but each [`SCREEN_EVERY`]th, and for any simulation but
/// the client's own.
///
/// Only what the host vouches for. A spectated match never gets here (see
/// the module's notes); guessing at it by how fast a simulation ticks was
/// tried for one build and matched nothing.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>) {
    if sim.tick() % SCREEN_EVERY != 0 {
        return;
    }
    let origin = sim.sim_origin();
    KINDS_SEEN.fetch_or(
        origin.map_or(1 << 31, |origin| 1u32 << origin.kind.min(30)),
        Ordering::Relaxed,
    );
    let Some(origin) = origin else {
        return;
    };
    if !matches!(
        SimOriginKindV1::from_code(origin.kind),
        Some(
            SimOriginKindV1::ClientMatchView
                | SimOriginKindV1::ClientSpectate
                | SimOriginKindV1::ClientReplay
        )
    ) {
        return;
    }

    let seed = sim.seed();
    let known = SCREEN
        .lock()
        .ok()
        .and_then(|screen| screen.clone())
        .filter(|screen| screen.seed == seed);
    if known
        .as_ref()
        .is_some_and(|screen| screen.complete() && screen.replay == origin.replay_id)
    {
        return;
    }
    // Whether there is anything new to tell the panel. A seat that never gets
    // a name must not have it start over twice a second for the whole match.
    let mut news = known
        .as_ref()
        .is_none_or(|screen| screen.replay != origin.replay_id);
    let mut screen = known.unwrap_or_else(|| Screen {
        seed,
        replay: origin.replay_id,
        sides: Default::default(),
        seats: 0,
    });
    screen.replay = origin.replay_id;
    let mut seats = 0;
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let team = player.team();
        let Some(lane) = player
            .lane()
            .map(|lane| lane.code() as usize)
            .filter(|lane| *lane < LANES)
        else {
            continue;
        };
        if team >= 2 {
            continue;
        }
        seats += 1;
        if !screen.sides[team][lane].is_empty() {
            continue;
        }
        // A live entity: a champion that is dead on this tick has no name
        // until the next one it is alive on.
        if let Some(name) = player.champion().and_then(|champion| champion.name()) {
            let name = fold(&name);
            if !name.is_empty() {
                screen.sides[team][lane] = name;
                news = true;
            }
        }
    }
    if seats != screen.seats {
        screen.seats = seats;
        news = true;
    }
    if !news {
        return;
    }
    if let Ok(mut held) = SCREEN.lock() {
        *held = Some(screen);
    }
    SOURCES.fetch_add(1, Ordering::Relaxed);
}

/// The champions each side drafted for the match about to be played, blue
/// then red, as champion ids. The second source for the lineup, see the
/// module's notes.
static LINEUP: Mutex<Option<[Vec<String>; 2]>> = Mutex::new(None);

/// Takes the draft's picks from the strategy screen as it closes, which is
/// the match starting. A screen that knew no picks leaves the last ones be:
/// the screen can be left and come back to without a draft in between.
pub(crate) fn note_lineup(sides: [Vec<String>; 2]) {
    if sides.iter().all(Vec::is_empty) {
        return;
    }
    let sides = sides.map(|side| champion_set(side.iter().map(String::as_str)));
    if let Ok(mut lineup) = LINEUP.lock() {
        *lineup = Some(sides);
    }
    SOURCES.fetch_add(1, Ordering::Relaxed);
}

/// Enemy picks that have to be known before a decision is taken for a side,
/// when the lineup is the drafted one. Fewer than five, because a pick the
/// draft grid named in a way no champion answers to is missing from it; not
/// fewer than this, because one side's five alone could be some other
/// fixture's.
const ENEMIES_NEEDED: usize = 3;

/// The athletes of both sides as the layout's camera buttons name them, by
/// side and by lane.
type Athletes = [[Option<String>; LANES]; 2];

/// The lane a camera button's node is named for. The layout names it like
/// its lane icon ("mid", seen in game); the rest are what else it might call
/// one.
fn lane_of(node: &str) -> Option<usize> {
    let node = node.to_lowercase();
    LANE_ICONS
        .iter()
        .position(|lane| *lane == node)
        .or(match node.as_str() {
            "jg" | "jungler" => Some(1),
            "middle" => Some(2),
            "bot" | "adc" | "ad" => Some(3),
            "sup" | "supporter" => Some(4),
            _ => None,
        })
}

/// The athlete's name out of a camera button's text, which reads
/// "Lv.3 Caps (F3)": the level before it and the hotkey after it come off.
fn athlete_of(text: &str) -> Option<String> {
    let text = text.trim();
    let rest = match text.split_once(' ') {
        Some((level, rest)) if level.to_lowercase().starts_with("lv") => rest,
        _ => text,
    };
    let name = match rest.rsplit_once(" (") {
        Some((name, key)) if key.ends_with(')') => name,
        _ => rest,
    };
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Reads who is playing where off the camera buttons: one a player, in either
/// view, under a node named for its lane, with a half for each side.
fn camera_seats(ctx: &StableClient<'_>) -> Athletes {
    let mut seats: Athletes = Default::default();
    let mut nodes = Vec::new();
    for host in CAMERA_HOSTS {
        for child in ctx.ui_child_names(host) {
            let lane = lane_of(&child);
            for (side, half) in CAMERA_HALVES.iter().enumerate() {
                let name = ctx
                    .ui_text(&format!("{host}.{child}.{half}.text"))
                    .and_then(|text| athlete_of(&text));
                if let (Some(lane), Some(name)) = (lane, name) {
                    seats[side][lane].get_or_insert(name);
                }
            }
            if LOG && !nodes.contains(&child) {
                nodes.push(child);
            }
        }
    }
    log("cameras", || format!("nodes={nodes:?} -> {seats:?}"));
    seats
}

/// Athlete names by id, as this module compares them. The buy detour only
/// has an athlete's id and the layout only its name, and only the client can
/// ask the host for one from the other.
static ATHLETE_NAMES: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

/// Athletes asked about in one go, newest grown builds first: enough for a
/// match that has just started, and no more of the frame than that.
const ATHLETES_A_CALL: usize = 40;

/// Asks the host for the names of the athletes behind the newest grown
/// builds that have none yet.
fn learn_athletes(ctx: &StableClient<'_>) {
    let unknown: Vec<u64> = {
        let Ok(names) = ATHLETE_NAMES.lock() else {
            return;
        };
        let Ok(grown) = GROWN.lock() else {
            return;
        };
        let mut ids = Vec::new();
        for known in grown.iter().rev() {
            let Some(id) = known.athlete else {
                continue;
            };
            if names.as_ref().is_some_and(|names| names.contains_key(&id)) || ids.contains(&id) {
                continue;
            }
            ids.push(id);
            if ids.len() >= ATHLETES_A_CALL {
                break;
            }
        }
        ids
    };
    if unknown.is_empty() {
        return;
    }
    // An athlete the host does not name is settled too, as nobody.
    let learned: Vec<(u64, String)> = unknown
        .into_iter()
        .map(|id| {
            let name = ctx.athlete_name(id as usize);
            (id, name.map(|name| fold(&name)).unwrap_or_default())
        })
        .collect();
    if let Ok(mut names) = ATHLETE_NAMES.lock() {
        names.get_or_insert_with(HashMap::new).extend(learned);
    }
}

/// The seed of the newest match whose athletes are the ones the camera
/// buttons name, seat for seat: the match on screen, when no simulation says
/// so itself.
fn seed_by_athletes(athletes: &Athletes) -> Option<u64> {
    let wanted: Vec<(u64, usize, String)> = athletes
        .iter()
        .enumerate()
        .flat_map(|(side, lanes)| {
            lanes
                .iter()
                .enumerate()
                .filter_map(move |(lane, name)| Some((side as u64, lane, fold(name.as_ref()?))))
        })
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let names = ATHLETE_NAMES.lock().ok()?;
    let names = names.as_ref()?;
    let grown = GROWN.lock().ok()?;
    // The seats each match covers, newest match first.
    let mut seeds: Vec<(u64, u32)> = Vec::new();
    for known in grown.iter().rev() {
        if known.seed == 0 {
            continue;
        }
        let Some(name) = known.athlete.and_then(|id| names.get(&id)) else {
            continue;
        };
        let seat = wanted.iter().position(|(side, lane, athlete)| {
            known.side == *side && known.lane == *lane && athlete == name
        });
        let Some(seat) = seat else {
            continue;
        };
        match seeds.iter_mut().find(|(seed, _)| *seed == known.seed) {
            Some((_, covered)) => *covered |= 1 << seat,
            None => seeds.push((known.seed, 1 << seat)),
        }
    }
    let all = (1u32 << wanted.len()) - 1;
    let found = seeds
        .iter()
        .find(|(_, covered)| *covered == all)
        .map(|(seed, _)| *seed);
    log("athletes", || {
        format!(
            "{} seats wanted, {} athletes named, {} grown builds, {} matches with one of them -> {found:?}",
            wanted.len(),
            names.len(),
            grown.len(),
            seeds.len()
        )
    });
    found
}

/// Which side of the match the panel's team is on: 0 blue, 1 red.
///
/// Three ways, the first that gives an answer:
///
/// 1. The set's replay record names both teams by id, and the host says which
///    is the player's. A replayed set has its record; a match being played
///    for the first time does not yet.
/// 2. The panel names the team it is about, and the layout names both sides
///    in two places. Tried exactly and then as one name inside the other: the
///    header says "G2 Esports #1" where the panel says "G2 Esports". This is
///    the one seen answering in a match of the player's own.
/// 3. The panel names its athletes and so do the camera buttons, by side. A
///    spectated match has no team names in its header, so this is its one.
///
/// An answer that fits both sides or neither is no answer, and then the
/// board does without: see the module's notes.
fn player_side(ctx: &StableClient<'_>, replay: Option<u64>, cameras: &Athletes) -> Option<usize> {
    let pick = |blue: bool, red: bool| match (blue, red) {
        (true, false) => Some(0),
        (false, true) => Some(1),
        _ => None,
    };

    let own_team = ctx.player_team_id();
    let recorded = replay.filter(|id| *id != SimOriginV1::NONE).map(|id| {
        let team = |field: &str| ctx.record_get_i64(RecordKindV1::MatchReplay, id as usize, field);
        (team("blue_team_id"), team("red_team_id"))
    });
    let by_record = recorded.and_then(|(blue, red)| {
        let own = own_team? as i64;
        pick(blue == Some(own), red == Some(own))
    });
    log("side.record", || {
        format!("replay={replay:?} teams={recorded:?} own_team={own_team:?} -> {by_record:?}")
    });
    if by_record.is_some() {
        return by_record;
    }

    let text = |path: &str| {
        ctx.ui_text(path)
            .map(|text| fold(&text))
            .filter(|text| !text.is_empty())
    };
    let own = [
        text(TEAM_NAME),
        own_team
            .and_then(|team| ctx.team_name(team))
            .map(|name| fold(&name))
            .filter(|name| !name.is_empty()),
    ];
    let named = SIDE_NAMES.map(|pair| pair.map(|path| text(path)));
    let mut by_name = None;
    'names: for exact in [true, false] {
        for sides in &named {
            for name in own.iter().flatten() {
                let is = |side: &Option<String>| {
                    side.as_deref().is_some_and(|side| {
                        if exact {
                            side == name.as_str()
                        } else {
                            side.contains(name.as_str()) || name.contains(side)
                        }
                    })
                };
                by_name = pick(is(&sides[0]), is(&sides[1]));
                if by_name.is_some() {
                    break 'names;
                }
            }
        }
    }
    log("side.names", || {
        format!("own={own:?} layout={named:?} -> {by_name:?}")
    });
    if by_name.is_some() {
        return by_name;
    }

    let panel: Vec<String> = (0..LANES)
        .filter_map(|row| text(&format!("{ROWS_PATH}.row{row}.name")))
        .collect();
    let hits = [0, 1].map(|side: usize| {
        cameras[side]
            .iter()
            .flatten()
            .filter(|name| panel.contains(&fold(name)))
            .count()
    });
    let by_athlete = pick(hits[0] > 0 && hits[1] == 0, hits[1] > 0 && hits[0] == 0);
    log("side.athletes", || {
        format!("panel={panel:?} named by the cameras [blue, red]={hits:?} -> {by_athlete:?}")
    });
    by_athlete
}

/// One champion of the match on screen.
struct Seat {
    /// The champion's key.
    champion: String,
    /// What its cell says beside the portrait: the athlete where the panel
    /// names one, the champion otherwise.
    name: String,
    /// What the item-build hooks decided for it, where a decision answers to
    /// this lineup: for anyone, and for the player's own athlete.
    decided: Option<Vec<String>>,
    own: Option<Vec<String>>,
}

impl Seat {
    /// The decided build as it applies to this seat, the player's or not.
    fn decided_for(&self, players: bool) -> Option<&Vec<String>> {
        if players {
            self.own.as_ref().or(self.decided.as_ref())
        } else {
            self.decided.as_ref()
        }
    }
}

/// The match on screen: who is in it, and what the hooks decided for them.
struct Match {
    /// The match's seed, where the simulation on screen was seen.
    seed: Option<u64>,
    /// Whether the seats are the simulation's own, lanes and all.
    on_screen: bool,
    /// The side the panel's team is on, where that could be worked out.
    player_side: Option<usize>,
    /// By side (0 blue, 1 red) and by lane.
    seats: [Vec<Option<Seat>>; 2],
}

/// A champion's name as a label shows it: the game's own, as a reference the
/// label resolves in the game's language, or the key with its underscores
/// out for a champion the game's text does not name.
fn champion_label(ctx: &StableClient<'_>, champion: &str) -> String {
    let reference = format!("#asset/base/text/champion?description.{champion}.name");
    match ctx.i18n(&reference) {
        Some(name) if !name.is_empty() && !name.starts_with('#') => reference,
        _ => champion.replace('_', " "),
    }
}

/// The match on screen, or nothing while no champion of it is known.
fn resolve(ctx: &StableClient<'_>) -> Option<Match> {
    let screen = SCREEN.lock().ok().and_then(|screen| screen.clone());
    let drafted = LINEUP.lock().ok().and_then(|lineup| lineup.clone());
    log("sources", || {
        format!(
            "screen={screen:?} drafted={drafted:?} sim kinds seen={:#b}",
            KINDS_SEEN.load(Ordering::Relaxed)
        )
    });
    let cameras = camera_seats(ctx);
    let player_side = player_side(ctx, screen.as_ref().map(|screen| screen.replay), &cameras);

    // Champion key, the build decided for anyone, and for the player's own.
    type Raw = (String, Option<Vec<String>>, Option<Vec<String>>);
    let mut raw: [Vec<Option<Raw>>; 2] = [vec![None; LANES], vec![None; LANES]];
    let mut seed = screen.as_ref().map(|screen| screen.seed);
    let simulated = screen.as_ref().is_some_and(|screen| screen.named() > 0);
    let mut on_screen = simulated;
    let mut how = "nothing";
    let mut matched = 0;

    if let Some(screen) = screen.as_ref().filter(|_| simulated) {
        // The simulation says who sits where. A decision is this seat's when
        // it is for the champion and names both lineups as they are.
        how = "the simulation";
        let sets = [0, 1].map(|side: usize| {
            champion_set(
                screen.sides[side]
                    .iter()
                    .filter(|name| !name.is_empty())
                    .map(String::as_str),
            )
        });
        let decisions = DECISIONS.lock().ok()?;
        for side in 0..2 {
            for lane in 0..LANES {
                let champion = &screen.sides[side][lane];
                if champion.is_empty() {
                    continue;
                }
                // Newest first: a match decided twice keeps its last answer.
                let decision = decisions.iter().rev().find(|decision| {
                    fold(&decision.champion) == *champion
                        && decision.team == sets[side]
                        && sets[1 - side]
                            .iter()
                            .all(|enemy| decision.enemies.contains(enemy))
                });
                matched += usize::from(decision.is_some());
                raw[side][lane] = Some(match decision {
                    Some(decision) => (
                        decision.champion.clone(),
                        Some(decision.build.clone()),
                        decision.own.clone(),
                    ),
                    None => (champion.clone(), None, None),
                });
            }
        }
    } else {
        // No simulation vouched for: a spectated match. Its athletes say
        // which match it is, and the builds grown for that match who plays
        // what.
        learn_athletes(ctx);
        if let Some(found) = seed_by_athletes(&cameras) {
            how = "its athletes";
            seed = Some(found);
            on_screen = true;
            let grown = GROWN.lock().ok()?;
            for known in grown.iter().filter(|known| known.seed == found) {
                if let Some(seat) = raw
                    .get_mut(known.side as usize)
                    .and_then(|lanes| lanes.get_mut(known.lane))
                {
                    *seat = Some((known.champion.clone(), None, None));
                }
            }
        } else if let Some(drafted) = drafted.as_ref() {
            // The draft says who plays, not where: the decisions do.
            how = "the draft";
            let decisions = DECISIONS.lock().ok()?;
            for side in 0..2 {
                let (team, enemies) = (&drafted[side], &drafted[1 - side]);
                if team.len() != LANES || enemies.len() < ENEMIES_NEEDED {
                    continue;
                }
                for decision in decisions.iter().rev() {
                    let Some(lane) = decision.lane else {
                        continue;
                    };
                    if raw[side][lane].is_some()
                        || decision.team != *team
                        || !enemies.iter().all(|enemy| decision.enemies.contains(enemy))
                    {
                        continue;
                    }
                    matched += 1;
                    raw[side][lane] = Some((
                        decision.champion.clone(),
                        Some(decision.build.clone()),
                        decision.own.clone(),
                    ));
                }
            }
        }
    }
    let seated = raw.iter().flatten().flatten().count();
    log("seats", || {
        format!(
            "{seated} seated by {how}, {matched} with a decision; seed={seed:?} panel side={player_side:?}"
        )
    });
    if seated == 0 {
        return None;
    }

    // A cell is labelled with its athlete: the camera buttons name both
    // sides', the vanilla rows the panel team's, and failing both the
    // champion's own name stands in.
    let panel: Vec<String> = (0..LANES)
        .map(|lane| {
            ctx.ui_text(&format!("{ROWS_PATH}.row{lane}.name"))
                .map(|name| name.trim().to_string())
                .unwrap_or_default()
        })
        .collect();
    let mut side = 0;
    let seats = raw.map(|lanes| {
        let here = side;
        side += 1;
        lanes
            .into_iter()
            .enumerate()
            .map(|(lane, seat)| {
                let (champion, decided, own) = seat?;
                let athlete = cameras[here][lane].clone().or_else(|| {
                    panel
                        .get(lane)
                        .filter(|name| player_side == Some(here) && !name.is_empty())
                        .cloned()
                });
                Some(Seat {
                    name: athlete.unwrap_or_else(|| champion_label(ctx, &champion)),
                    champion,
                    decided,
                    own,
                })
            })
            .collect::<Vec<Option<Seat>>>()
    });
    Some(Match {
        seed,
        on_screen,
        player_side,
        seats,
    })
}

// -- the board ----------------------------------------------------------------

/// One champion's cell in the table.
#[derive(Clone, PartialEq)]
struct Cell {
    champion: String,
    name: String,
    slots: Vec<Slot>,
}

/// What is drawn: by lane, the blue cell and the red one.
type Board = Vec<[Option<Cell>; 2]>;
type Rows = Arc<Board>;

/// Where a slot is: side, lane, slot.
type Spot = (usize, usize, usize);

/// One cell in the test log's words: where its build came from, what the
/// rules make of each item on this champion, and how the grown slots were
/// picked.
fn describe(
    side: usize,
    lane: usize,
    seat: &Seat,
    slots: &[Slot],
    how: &str,
    picks: &[(String, &'static str)],
) -> String {
    let role = Role::from_lane_code(lane);
    let items: Vec<String> = slots
        .iter()
        .filter_map(|slot| slot.key.as_deref())
        .map(|key| crate::smart_builds::explain(&seat.champion, role, key))
        .collect();
    format!(
        "{} {} {} scaling={} [{how}]: {} picks={picks:?}",
        SIDE_NODES[side],
        LANE_ICONS[lane],
        seat.champion,
        crate::smart_builds::explain_champion(&seat.champion, role),
        items.join(" ")
    )
}

/// The cells to draw for a match: each seat's grown build where the buy
/// detour has made one, its decided build until then, nothing where neither
/// is known. And whether every seat has the build grown for that very seat,
/// after which there is nothing more to wait for.
fn board_of(game: &Match) -> (Board, bool) {
    // Looked up under the lock and turned into slots after it: the pin row
    // has locks of its own.
    let found: Vec<[Option<Found>; 2]> = match GROWN.lock() {
        Ok(grown) => (0..LANES)
            .map(|lane| {
                [0, 1].map(|side| {
                    let seat = game.seats[side].get(lane)?.as_ref()?;
                    grown_for(&grown, game, side, lane, seat)
                })
            })
            .collect(),
        Err(_) => (0..LANES).map(|_| [None, None]).collect(),
    };

    let own_only = build_config::own_team_only_enabled();
    // A lane or 5v5 test: both of its sides are the player's.
    let test = game.seed.is_some_and(build_config::is_test_match);
    let mut exact = true;
    let mut said = Vec::new();
    let board: Board = found
        .into_iter()
        .enumerate()
        .map(|(lane, found)| {
            let mut cells: [Option<Cell>; 2] = [None, None];
            for (side, found) in found.into_iter().enumerate() {
                let Some(seat) = game.seats[side].get(lane).and_then(Option::as_ref) else {
                    continue;
                };
                let players = game.player_side == Some(side);
                // Under `own_team_only` only the player's athletes carry the
                // pins, and in a test that is both sides. A side nobody could
                // be placed on carries none. This is the panel's reckoning,
                // for a seat the detour has not answered for itself.
                let pins = !own_only || players || test;
                exact &= found.as_ref().is_some_and(|found| found.exact);
                let (slots, how, picks) = match found {
                    Some(found) => (
                        slots_of(
                            &seat.champion,
                            lane,
                            &found.keys,
                            if found.exact { found.pins } else { pins },
                            true,
                        ),
                        found.how,
                        found.picks,
                    ),
                    None => match seat.decided_for(players) {
                        Some(keys) => (
                            slots_of(&seat.champion, lane, keys, pins, false),
                            "decided",
                            Vec::new(),
                        ),
                        None => continue,
                    },
                };
                if LOG {
                    said.push(describe(side, lane, seat, &slots, how, &picks));
                }
                cells[side] = Some(Cell {
                    champion: seat.champion.clone(),
                    name: seat.name.clone(),
                    slots,
                });
            }
            cells
        })
        .collect();
    log("board", || said.join(" | "));
    (board, exact)
}

fn drawn(board: &Board) -> bool {
    board.iter().flatten().any(Option::is_some)
}

// -- panel state --------------------------------------------------------------

/// What the tooltip's size is worked out from, once its text has been laid
/// out.
#[derive(Clone, Copy)]
struct TipBody {
    /// Height of the stat lines.
    stats_h: f32,
    /// Top of the description label, and its height as estimated from the
    /// English text. No height means no description.
    desc_y: f32,
    desc_estimate: f32,
}

struct Panel {
    /// Whether the in-match layout was up on the last frame.
    up: bool,
    /// Whether the Check Tactics panel was.
    open: bool,
    /// Frames spent in this match.
    frame: u32,
    /// The frame [`resolve`] is next due on.
    resolve_at: u32,
    /// [`SOURCES`] as of what `game` was resolved against.
    sources: u32,
    /// The match on screen, and the board drawn from it.
    game: Option<Arc<Match>>,
    rows: Option<Rows>,
    /// Whether every cell is the build the buy detour grew for that seat.
    /// Until then the board is worked out again on the frame `grown_at`.
    full: bool,
    grown_at: u32,
    /// The board changed under what is drawn: draw it again.
    repaint: bool,
    /// Whether the board on screen was spawned with the game's own items
    /// described: before that, their icons are guesses.
    spawned_with_cards: Option<bool>,
    /// The slot the tooltip is up for, and how many frames it has been.
    shown: Option<Spot>,
    shown_for: u32,
    body: Option<TipBody>,
    /// A slot a click asked the tooltip for.
    sticky: Option<Spot>,
    clicked_frame: u32,
}

impl Panel {
    const fn new() -> Self {
        Self {
            up: false,
            open: false,
            frame: 0,
            resolve_at: 0,
            sources: 0,
            game: None,
            rows: None,
            full: false,
            grown_at: 0,
            repaint: false,
            spawned_with_cards: None,
            shown: None,
            shown_for: 0,
            body: None,
            sticky: None,
            clicked_frame: 0,
        }
    }
}

static STATE: Mutex<Panel> = Mutex::new(Panel::new());

/// Never held across a host call: removing a node can fire its `Remove` event
/// into [`handle_event`] before the call returns.
fn with<T>(f: impl FnOnce(&mut Panel) -> T) -> Option<T> {
    STATE.lock().ok().map(|mut panel| f(&mut panel))
}

/// Paths a handler has been registered on in this match. A handler outlives
/// the node it was registered for and dies with the layout, so each path is
/// registered once per match (the rule `item_stats::ui` keeps).
static REGISTERED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn register_once(ctx: &mut StableClient<'_>, path: &str) {
    let fresh = REGISTERED
        .lock()
        .map(|mut set| set.insert(path.to_string()))
        .unwrap_or(false);
    if !fresh {
        return;
    }
    if !ctx.ui_register_path_events(path, handle_event) {
        if let Ok(mut set) = REGISTERED.lock() {
            set.remove(path);
        }
    }
}

/// Whether the cursor has ever been found over a slot. Until it has, a click
/// stands in for hovering.
static HOVER_SEEN: AtomicBool = AtomicBool::new(false);

/// Frames between two tries of [`resolve`] while the match is not yet known,
/// and of [`board_of`] while a cell still waits for its grown build.
const RESOLVE_EVERY: u32 = 30;

// -- per frame ----------------------------------------------------------------

/// Per-frame entry point, called from the mod's one client hook. Outside a
/// match this is one failed path lookup.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let Some(open) = ctx.ui_visible(PANEL) else {
        // The match is over, and everything spawned into its layout with it.
        if with(|panel| std::mem::replace(panel, Panel::new()).up).unwrap_or(false) {
            // None of these outlives its match. A simulation still running
            // puts itself back on its next look, and athletes are asked about
            // again: another save numbers them differently.
            if let Ok(mut names) = ATHLETE_NAMES.lock() {
                *names = None;
            }
            if let Ok(mut lineup) = LINEUP.lock() {
                *lineup = None;
            }
            if let Ok(mut screen) = SCREEN.lock() {
                *screen = None;
            }
            if let Ok(mut set) = REGISTERED.lock() {
                set.clear();
            }
        }
        return;
    };

    let sources = SOURCES.load(Ordering::Relaxed);
    let (frame, opened, due, game, rows, grow_due, outdated) = with(|panel| {
        panel.up = true;
        panel.frame = panel.frame.wrapping_add(1);
        let opened = open && !panel.open;
        panel.open = open;
        // What the board was resolved from has changed, a new match on
        // screen above all: start over, taking down what was drawn.
        let outdated = panel.sources != sources && panel.spawned_with_cards.is_some();
        if panel.sources != sources {
            panel.sources = sources;
            panel.game = None;
            panel.rows = None;
            panel.full = false;
            panel.repaint = false;
            panel.resolve_at = 0;
            panel.spawned_with_cards = None;
            panel.shown = None;
            panel.body = None;
            panel.sticky = None;
        }
        (
            panel.frame,
            opened,
            panel.frame >= panel.resolve_at,
            panel.game.clone(),
            panel.rows.clone(),
            !panel.full && panel.frame >= panel.grown_at,
            outdated,
        )
    })
    .unwrap_or((0, false, false, None, None, false, false));
    if outdated {
        unpaint(ctx);
    }

    // Asked for from the start of the match rather than when the panel opens,
    // so the icons are there the first time it does.
    let rows = match (game, rows) {
        (Some(game), Some(rows)) => {
            // The buy detour grows a build in its athlete's first moments in
            // the match, which is after this was resolved: looked for until
            // every cell has its own.
            if grow_due {
                let (board, full) = board_of(&game);
                let changed = board != *rows;
                let rows = if changed { Arc::new(board) } else { rows };
                let _ = with(|panel| {
                    panel.full = full;
                    panel.grown_at = frame.wrapping_add(RESOLVE_EVERY);
                    if changed {
                        panel.rows = Some(rows.clone());
                        panel.repaint = true;
                        panel.shown = None;
                        panel.body = None;
                        panel.sticky = None;
                    }
                });
                if changed {
                    ctx.ui_set_visible(TIP, false);
                }
                rows
            } else {
                rows
            }
        }
        _ => {
            if !due && !opened {
                return;
            }
            let game = resolve(ctx).map(Arc::new);
            let found = game.as_ref().map(|game| board_of(game));
            let full = found.as_ref().is_some_and(|(_, full)| *full);
            let rows = found.map(|(board, _)| Arc::new(board));
            let _ = with(|panel| {
                panel.resolve_at = frame.wrapping_add(RESOLVE_EVERY);
                panel.grown_at = frame.wrapping_add(RESOLVE_EVERY);
                panel.game = game.clone();
                panel.rows = rows.clone();
                panel.full = full;
            });
            let Some(rows) = rows else {
                return;
            };
            rows
        }
    };

    // Nobody's build is known yet: the vanilla table stays.
    if !drawn(&rows) {
        return;
    }
    if !open {
        show_tip(ctx, &rows, None);
        return;
    }

    prime_engine_cards(ctx);
    if !paint(ctx, &rows) {
        return;
    }

    let hovered = hovered_slot(ctx, &rows);
    if hovered.is_some() {
        HOVER_SEEN.store(true, Ordering::Relaxed);
    }
    let target = with(|panel| {
        if hovered.is_some() {
            panel.sticky = None;
        }
        hovered.or(panel.sticky)
    })
    .flatten();
    show_tip(ctx, &rows, target);
}

fn line_path(lane: usize) -> String {
    format!("{BOARD_PATH}.l{lane}")
}

fn cell_path(side: usize, lane: usize) -> String {
    format!("{BOARD_PATH}.l{lane}.{}", SIDE_NODES[side])
}

fn slot_path((side, lane, slot): Spot) -> String {
    format!("{BOARD_PATH}.l{lane}.{}.s{slot}", SIDE_NODES[side])
}

/// The slot a path names.
fn spot_of(path: &str) -> Option<Spot> {
    let rest = path.strip_prefix(BOARD_PATH)?.strip_prefix(".l")?;
    let (lane, rest) = rest.split_once('.')?;
    let (side, slot) = rest.split_once('.')?;
    let side = SIDE_NODES.iter().position(|node| *node == side)?;
    Some((
        side,
        lane.parse().ok()?,
        slot.strip_prefix('s')?.parse().ok()?,
    ))
}

/// Keeps the board in the panel and the vanilla table out of it. False while
/// the board could not be spawned, and the vanilla table is left alone then.
///
/// Re-asserted every frame the panel is open: game code owns the rows and the
/// headings, and what it does to them when it refills the panel is not known.
fn paint(ctx: &mut StableClient<'_>, board: &Rows) -> bool {
    let cards = ENGINE_CARDS.load(Ordering::Relaxed);
    let redraw = with(|panel| {
        let redraw = panel.repaint
            || panel
                .spawned_with_cards
                .is_some_and(|before| before != cards);
        panel.repaint = false;
        panel.spawned_with_cards = Some(cards);
        redraw
    })
    .unwrap_or(false);

    if redraw {
        ctx.ui_remove_node(BOARD_PATH);
    }
    if !ctx.ui_exists(BOARD_PATH) {
        let heads = SIDE_HEADS.map(|(key, fallback)| {
            let reference = format!("#asset/base/text/ui?{key}");
            match ctx.i18n(&reference) {
                Some(text) if !text.is_empty() && !text.starts_with('#') => reference,
                _ => fallback.to_string(),
            }
        });
        let spawned = ctx.ui_spawn_source(PERSONAL, &board_source(board, &heads));
        log("paint", || {
            format!("board spawned={spawned} (game items described={cards})")
        });
        if !spawned {
            return false;
        }
        for (lane, cells) in board.iter().enumerate() {
            for (side, cell) in cells.iter().enumerate() {
                let Some(cell) = cell else {
                    continue;
                };
                // The game's own portrait, by the champion's key.
                let face = PORTRAIT as f32;
                let portrait = ctx.ui_set_champion_icon(
                    &format!("{}.champ.icon", cell_path(side, lane)),
                    &cell.champion,
                    face,
                    face,
                    2.0,
                );
                if !portrait {
                    log(&format!("portrait.{}", cell.champion), || {
                        "the host drew no portrait for this champion".to_string()
                    });
                }
                for (slot, held) in cell.slots.iter().enumerate() {
                    if held.key.is_some() {
                        register_once(ctx, &slot_path((side, lane, slot)));
                    }
                }
            }
        }
    }

    ctx.ui_set_visible(ROWS_PATH, false);
    ctx.ui_set_visible(HEADER, false);
    true
}

/// Gives the panel back to vanilla: the board out, the game's headings and
/// rows back in.
fn unpaint(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(TIP, false);
    ctx.ui_remove_node(BOARD_PATH);
    ctx.ui_set_visible(ROWS_PATH, true);
    ctx.ui_set_visible(HEADER, true);
}

/// Strips what would end a `.ui` string literal.
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '{' | '}' | ';'))
        .collect()
}

/// The sheet tag an item draws. One of the mod's is its own key
/// (`StableItem::icon`), so that is also the guess for an item nothing has
/// described yet.
fn frame_of(key: &str) -> String {
    card(key).map_or_else(|| key.to_string(), |card| card.frame)
}

/// The whole board, stated up front: a heading over each side, then a line
/// for every lane somebody's build is known in, the lane's icon and the two
/// cells. It is spawned again whole when a build changes, which each does
/// once, as the buy detour grows it.
fn board_source(board: &Board, heads: &[String; 2]) -> String {
    let lines = board
        .iter()
        .filter(|cells| cells.iter().any(Option::is_some))
        .count();
    let height = LINES_Y + lines * LINE_STRIDE;
    let mut source = format!(
        "{BOARD}:empty {{\n\
         x: {BOARD_X}px;\n\
         y: {BOARD_Y}px;\n\
         width: {BOARD_W}px;\n\
         height: {height}px;\n"
    );
    for side in 0..2 {
        source.push_str(&format!(
            "#head_{node}:label {{\n\
             @\"asset/base/style/main#bold_label\";\n\
             ignore_event: true;\n\
             x: {x}px;\n\
             width: {CELL_W}px;\n\
             height: {HEAD_H}px;\n\
             size: 15;\n\
             color: {color};\n\
             align_y: Center;\n\
             text: \"{text}\";\n\
             }}\n",
            node = SIDE_NODES[side],
            x = CELL_X[side] + PORTRAIT_X,
            color = SIDE_COLOR[side],
            text = escape(&heads[side]),
        ));
    }

    let mut line = 0;
    for (lane, cells) in board.iter().enumerate() {
        if cells.iter().all(Option::is_none) {
            continue;
        }
        let y = LINES_Y + line * LINE_STRIDE;
        line += 1;
        source.push_str(&format!(
            "#l{lane}:empty {{\n\
             y: {y}px;\n\
             width: {BOARD_W}px;\n\
             height: {ROW_H}px;\n\
             \n\
             #lane:image {{\n\
             ignore_event: true;\n\
             y: 4px;\n\
             width: 28px;\n\
             height: 28px;\n\
             source: \"asset/base/ui/icons/{icon}\";\n\
             }}\n",
            icon = LANE_ICONS[lane],
        ));
        for (side, cell) in cells.iter().enumerate() {
            if let Some(cell) = cell {
                source.push_str(&cell_source(side, cell));
            }
        }
        source.push_str("}\n");
    }
    source.push_str("}\n");
    source
}

/// One champion's cell: its side's bar, its portrait (filled in after the
/// spawn, by the host), its name and its slots.
///
/// A filled slot is a button so that it takes a click and draws its own hover
/// border; an empty one is a plain square.
fn cell_source(side: usize, cell: &Cell) -> String {
    let mut source = format!(
        "#{node}:empty {{\n\
         x: {x}px;\n\
         width: {CELL_W}px;\n\
         height: {ROW_H}px;\n\
         \n\
         #bar:color {{\n\
         ignore_event: true;\n\
         y: 4px;\n\
         width: 4px;\n\
         height: 28px;\n\
         color: {color};\n\
         }}\n\
         \n\
         #champ:color {{\n\
         ignore_event: true;\n\
         x: {PORTRAIT_X}px;\n\
         y: 4px;\n\
         width: {PORTRAIT}px;\n\
         height: {PORTRAIT}px;\n\
         color: {SLOT_FILL};\n\
         rounding: Uniform {{ rounding: 6; }}\n\
         \n\
         #icon:image {{\n\
         ignore_event: true;\n\
         anchor_x: 0.5;\n\
         anchor_y: 0.5;\n\
         pivot_x: 0.5;\n\
         pivot_y: 0.5;\n\
         width: {PORTRAIT}px;\n\
         height: {PORTRAIT}px;\n\
         }}\n\
         }}\n\
         \n\
         #name:label {{\n\
         @\"asset/base/style/main#bold_label\";\n\
         ignore_event: true;\n\
         x: {NAME_X}px;\n\
         width: {NAME_W}px;\n\
         height: {ROW_H}px;\n\
         size: 15;\n\
         align_y: Center;\n\
         fit_width: true;\n\
         text: \"{name}\";\n\
         }}\n",
        node = SIDE_NODES[side],
        x = CELL_X[side],
        color = SIDE_COLOR[side],
        name = escape(&cell.name),
    );
    for (index, slot) in cell.slots.iter().enumerate() {
        let x = SLOTS_X + index * SLOT_STRIDE;
        let Some(key) = &slot.key else {
            source.push_str(&format!(
                "#s{index}:color {{\n\
                 ignore_event: true;\n\
                 x: {x}px;\n\
                 y: {SLOT_Y}px;\n\
                 width: {SLOT}px;\n\
                 height: {SLOT}px;\n\
                 color: {SLOT_FILL};\n\
                 rounding: Uniform {{ rounding: 6; }}\n\
                 }}\n"
            ));
            continue;
        };
        let (line, stroke, fill) = if slot.pinned {
            (PINNED_LINE, PINNED_STROKE, PINNED_FILL)
        } else {
            (PICKED_LINE, PICKED_STROKE, SLOT_FILL)
        };
        let frame = escape(&frame_of(key));
        source.push_str(&format!(
            "#s{index}:color_icon_button {{\n\
             x: {x}px;\n\
             y: {SLOT_Y}px;\n\
             width: {SLOT}px;\n\
             height: {SLOT}px;\n\
             \n\
             btn: {{\n\
             color: {line};\n\
             back_color: {fill};\n\
             stroke: {stroke};\n\
             rounding: Uniform {{ rounding: 6; }}\n\
             }}\n\
             \n\
             hover: {{\n\
             btn: {{\n\
             color: {HOVER_LINE};\n\
             back_color: {fill};\n\
             stroke: {stroke};\n\
             rounding: Uniform {{ rounding: 6; }}\n\
             }}\n\
             }}\n\
             \n\
             #icon:image {{\n\
             ignore_event: true;\n\
             anchor_x: 0.5;\n\
             anchor_y: 0.5;\n\
             pivot_x: 0.5;\n\
             pivot_y: 0.5;\n\
             width: {ICON}px;\n\
             height: {ICON}px;\n\
             source: \"{ICON_SHEET}\";\n\
             rect_tag: \"{frame}\";\n\
             }}\n\
             }}\n"
        ));
    }
    source.push_str("}\n");
    source
}

// -- cursor -------------------------------------------------------------------

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
    fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
    fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcessId() -> u32;
}

/// The cursor inside the game's window, in client pixels, and the client
/// area's size. Nothing while another program's window is in front.
fn client_cursor() -> Option<(f32, f32, f32, f32)> {
    let mut point = Point { x: 0, y: 0 };
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // All four take the window the user is looking at, checked to be this
    // process's own, and write into the two locals above.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        if process != GetCurrentProcessId() {
            return None;
        }
        if GetCursorPos(&mut point) == 0
            || ScreenToClient(window, &mut point) == 0
            || GetClientRect(window, &mut rect) == 0
        {
            return None;
        }
    }
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    if width <= 0 || height <= 0 {
        return None;
    }
    Some((point.x as f32, point.y as f32, width as f32, height as f32))
}

/// The layout's rect in the space `ui_node_rect` answers in: its authored
/// 1920x1080 where the host will not say.
fn canvas(ctx: &StableClient<'_>) -> (f32, f32, f32, f32) {
    ctx.ui_node_rect(INGAME)
        .filter(|rect| rect.2 > 0.0 && rect.3 > 0.0)
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0))
}

/// The cursor in the space `ui_node_rect` answers in.
///
/// Taken as the layout scaled by one factor to fit the window and centred in
/// it, which on a window of the layout's own shape is plain proportion. Not
/// confirmed in game on any window.
fn cursor(ctx: &StableClient<'_>) -> Option<(f32, f32)> {
    let (x, y, client_w, client_h) = client_cursor()?;
    let (left, top, width, height) = canvas(ctx);
    let scale = (width / client_w).max(height / client_h);
    log("cursor", || {
        format!(
            "window {client_w}x{client_h} -> layout {width}x{height} at ({left}, {top}), scale {scale}; table at {:?}",
            ctx.ui_node_rect(PERSONAL)
        )
    });
    Some((
        left + width / 2.0 + (x - client_w / 2.0) * scale,
        top + height / 2.0 + (y - client_h / 2.0) * scale,
    ))
}

/// The filled slot under the cursor.
fn hovered_slot(ctx: &StableClient<'_>, board: &Rows) -> Option<Spot> {
    let (x, y) = cursor(ctx)?;
    let under = |path: &str| {
        ctx.ui_node_rect(path)
            .is_some_and(|(left, top, width, height)| {
                x >= left && x < left + width && y >= top && y < top + height
            })
    };
    // One lookup on most frames: the cursor is rarely over this table.
    if !under(PERSONAL) {
        return None;
    }
    for (lane, cells) in board.iter().enumerate() {
        if cells.iter().all(Option::is_none) || !under(&line_path(lane)) {
            continue;
        }
        for (side, cell) in cells.iter().enumerate() {
            let Some(cell) = cell else {
                continue;
            };
            if !under(&cell_path(side, lane)) {
                continue;
            }
            let found = cell
                .slots
                .iter()
                .enumerate()
                .find(|(slot, held)| held.key.is_some() && under(&slot_path((side, lane, *slot))))
                .map(|(slot, _)| (side, lane, slot));
            if found.is_some() {
                log("hover", || {
                    "the cursor has been found over a slot".to_string()
                });
            }
            return found;
        }
        return None;
    }
    None
}

/// A click on a slot, for a window the cursor cannot be placed in: it puts
/// the tooltip up for that slot, and a second one takes it down.
fn handle_event(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // `Remove` fires as the match's layout is torn down. An unreported kind
    // counts as a click, the gate `item_stats::ui` uses.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    if HOVER_SEEN.load(Ordering::Relaxed) {
        return;
    }
    let Some(target) = spot_of(&event.path) else {
        return;
    };
    let _ = with(|panel| {
        // A repeat within a frame is a second delivery, not a second click.
        if panel.clicked_frame == panel.frame {
            return;
        }
        panel.clicked_frame = panel.frame;
        panel.sticky = (panel.sticky != Some(target)).then_some(target);
    });
}

// -- tooltip ------------------------------------------------------------------

fn tip_source() -> String {
    let inner = TIP_W - 2 * TIP_PAD;
    let name_x = TIP_PAD + 46;
    let name_w = TIP_W - name_x - TIP_PAD;
    let bar_y = TIP_BODY_Y - 6;
    format!(
        "{TIP_NAME}:color {{\n\
         ignore_event: true;\n\
         y: {PARKED_Y}px;\n\
         width: {TIP_W}px;\n\
         height: 100px;\n\
         visible: false;\n\
         color: #4a4c56ff;\n\
         rounding: Uniform {{ rounding: 12; }}\n\
         \n\
         #bg:color {{\n\
         ignore_event: true;\n\
         anchor_x: 0.5;\n\
         anchor_y: 0.5;\n\
         pivot_x: 0.5;\n\
         pivot_y: 0.5;\n\
         width: {bg_w}px;\n\
         height: 98px;\n\
         color: #161721ff;\n\
         rounding: Uniform {{ rounding: 12; }}\n\
         }}\n\
         \n\
         #slot:color {{\n\
         ignore_event: true;\n\
         x: {TIP_PAD}px;\n\
         y: {TIP_PAD}px;\n\
         width: 40px;\n\
         height: 40px;\n\
         color: #4a4c56ff;\n\
         rounding: Uniform {{ rounding: 8; }}\n\
         \n\
         #icon:image {{\n\
         ignore_event: true;\n\
         anchor_x: 0.5;\n\
         anchor_y: 0.5;\n\
         pivot_x: 0.5;\n\
         pivot_y: 0.5;\n\
         width: 36px;\n\
         height: 36px;\n\
         source: \"{ICON_SHEET}\";\n\
         }}\n\
         }}\n\
         \n\
         #name:label {{\n\
         @\"asset/base/style/main#bold_label\";\n\
         ignore_event: true;\n\
         x: {name_x}px;\n\
         y: {TIP_PAD}px;\n\
         width: {name_w}px;\n\
         height: 20px;\n\
         size: 18;\n\
         align_y: Center;\n\
         fit_width: true;\n\
         text: \"\";\n\
         }}\n\
         \n\
         #gold:image {{\n\
         ignore_event: true;\n\
         x: {name_x}px;\n\
         y: 34px;\n\
         width: 16px;\n\
         height: 16px;\n\
         source: \"asset/base/ui/gold\";\n\
         }}\n\
         \n\
         #price:label {{\n\
         @\"asset/base/style/main#label\";\n\
         ignore_event: true;\n\
         x: {price_x}px;\n\
         y: 33px;\n\
         width: 120px;\n\
         height: 18px;\n\
         size: 16;\n\
         color: #fde99fff;\n\
         align_y: Center;\n\
         text: \"\";\n\
         }}\n\
         \n\
         #bar:color {{\n\
         ignore_event: true;\n\
         x: {TIP_PAD}px;\n\
         y: {bar_y}px;\n\
         width: {inner}px;\n\
         height: 1px;\n\
         color: #1d1f2cff;\n\
         }}\n\
         \n\
         #stats:label {{\n\
         @\"asset/base/style/main#label\";\n\
         ignore_event: true;\n\
         x: {TIP_PAD}px;\n\
         y: {TIP_BODY_Y}px;\n\
         width: {inner}px;\n\
         height: {LINE_H}px;\n\
         size: 16;\n\
         line_height: {LINE_H};\n\
         text: \"\";\n\
         }}\n\
         \n\
         #desc:label {{\n\
         @\"asset/base/style/main#label\";\n\
         ignore_event: true;\n\
         x: {TIP_PAD}px;\n\
         y: {TIP_BODY_Y}px;\n\
         width: {inner}px;\n\
         height: {DESC_BOX}px;\n\
         size: 16;\n\
         line_height: {LINE_H};\n\
         text: \"\";\n\
         }}\n\
         }}\n",
        bg_w = TIP_W - 2,
        price_x = name_x + 18,
    )
}

/// A text key as a reference the label resolves itself, in the game's
/// language, beside what it says in English. `ctx.i18n` answers in English
/// whatever the locale, which is what tells a key that exists from one that
/// would be drawn as a literal `#asset/...`.
fn text_ref(ctx: &StableClient<'_>, key: &str) -> Option<(String, String)> {
    let reference = format!("#asset/base/text/item?{key}");
    let english = ctx
        .i18n(&reference)
        .filter(|text| !text.is_empty() && !text.starts_with('#'))?;
    Some((reference, english))
}

/// Lines a description is taken to wrap into: its own line breaks, and one
/// more for every [`DESC_LINE_CHARS`] of text between them. Markup takes no
/// room, bar an inline icon's two characters' worth.
fn estimate_lines(text: &str) -> usize {
    text.split('\n')
        .map(|line| {
            let mut chars = 0usize;
            let mut rest = line;
            while let Some(open) = rest.find('<') {
                chars += rest[..open].chars().count();
                let tag = &rest[open..];
                if tag.starts_with("<i#") {
                    chars += 2;
                }
                match tag.find('>') {
                    Some(close) => rest = &tag[close + 1..],
                    None => {
                        rest = "";
                    }
                }
            }
            chars += rest.chars().count();
            chars.div_ceil(DESC_LINE_CHARS).max(1)
        })
        .sum()
}

/// Writes one item into the tooltip and says how its body is laid out.
fn fill_tip(ctx: &mut StableClient<'_>, key: &str) -> TipBody {
    let card = card(key);
    let frame = card
        .as_ref()
        .map_or_else(|| key.to_string(), |card| card.frame.clone());
    ctx.ui_set_properties(
        &format!("{TIP}.slot.icon"),
        &format!(
            "source: \"{ICON_SHEET}\"; rect_tag: \"{}\";",
            escape(&frame)
        ),
    );

    let name = text_ref(ctx, &format!("{key}.name"));
    ctx.ui_set_text(
        &format!("{TIP}.name"),
        name.as_ref()
            .map_or(key, |(reference, _)| reference.as_str()),
    );

    let price = card.as_ref().map_or(0, |card| card.price);
    ctx.ui_set_visible(&format!("{TIP}.gold"), price > 0);
    ctx.ui_set_visible(&format!("{TIP}.price"), price > 0);
    ctx.ui_set_text(&format!("{TIP}.price"), &price.to_string());

    // An icon and a number each, so the line reads the same in every
    // language.
    let stats: Vec<String> = card
        .iter()
        .flat_map(|card| card.stats.iter())
        .map(|(tag, value)| format!("<i#{STAT_ICONS}:{tag}> {value}"))
        .collect();
    let lines: Vec<String> = stats
        .chunks(STATS_PER_LINE)
        .map(|line| line.join("    "))
        .collect();
    let stats_h = (lines.len() * LINE_H) as f32;
    ctx.ui_set_text(&format!("{TIP}.stats"), &lines.join("\n"));
    ctx.ui_set_properties(
        &format!("{TIP}.stats"),
        &format!("height: {}px;", lines.len().max(1) * LINE_H),
    );

    // The effect text opens with a line break in every language: the game
    // joins it to the stat block it draws itself. Here that first, empty line
    // is laid over the stat lines' last gap rather than left as a hole.
    let option = text_ref(ctx, &format!("{key}.option"));
    let (desc_y, desc_estimate) = match &option {
        Some((_, english)) => {
            let lead = if english.starts_with('\n') { LINE_H } else { 0 };
            let gap = if lines.is_empty() { 0 } else { 6 };
            (
                (TIP_BODY_Y + lines.len() * LINE_H + gap) as f32 - lead as f32,
                (estimate_lines(english) * LINE_H) as f32,
            )
        }
        None => (TIP_BODY_Y as f32, 0.0),
    };
    ctx.ui_set_text(
        &format!("{TIP}.desc"),
        option
            .as_ref()
            .map_or("", |(reference, _)| reference.as_str()),
    );
    ctx.ui_set_properties(&format!("{TIP}.desc"), &format!("y: {}px;", desc_y as i32));

    TipBody {
        stats_h,
        desc_y,
        desc_estimate,
    }
}

/// The tooltip's border for the slot it is up for: the slot's own, colour
/// and width, so a pin's tooltip is edged in the pin's teal (the user,
/// 2026-10-08). The border is what shows of the tooltip's own colour around
/// its `#bg`, which is inset by this width.
fn tip_edge(pinned: bool) -> (&'static str, usize) {
    if pinned {
        (PINNED_LINE, PINNED_STROKE)
    } else {
        (PICKED_LINE, PICKED_STROKE)
    }
}

/// The slot at `spot` on the board, if it has one there.
fn slot_at(board: &Rows, (side, lane, slot): Spot) -> Option<&Slot> {
    board
        .get(lane)
        .and_then(|cells| cells[side].as_ref())
        .and_then(|cell| cell.slots.get(slot))
}

/// Sizes the tooltip to its text and puts it by its slot: above where there
/// is room, below where there is not, and inside the screen either way.
/// `edge` is the width of its border ([`tip_edge`]).
fn place_tip(ctx: &mut StableClient<'_>, body: TipBody, slot: &str, edge: usize) {
    // The description's own height where the host measures a label's text,
    // which it does for width (`item_stats::toolbox_tab`). The box handed
    // back unchanged is no measurement, and then the estimate stands.
    let measured = ctx
        .ui_contents_rect(&format!("{TIP}.desc"))
        .map(|(_, _, _, height)| height)
        .filter(|height| *height > 0.0 && *height < (DESC_BOX - 1) as f32);
    let bottom = if body.desc_estimate > 0.0 {
        body.desc_y + measured.unwrap_or(body.desc_estimate)
    } else {
        TIP_BODY_Y as f32 + body.stats_h
    };
    let height = (bottom + TIP_PAD as f32).ceil().max(TIP_BODY_Y as f32);

    let (left, top, width, canvas_h) = canvas(ctx);
    let Some((slot_x, slot_y, slot_w, slot_h)) = ctx.ui_node_rect(slot) else {
        return;
    };
    let x = (slot_x + slot_w / 2.0 - TIP_W as f32 / 2.0)
        .min(left + width - TIP_W as f32 - TIP_GAP)
        .max(left + TIP_GAP);
    let above = slot_y - height - TIP_GAP;
    let y = if above >= top + TIP_GAP {
        above
    } else {
        (slot_y + slot_h + TIP_GAP).min(top + canvas_h - height - TIP_GAP)
    };

    ctx.ui_set_properties(
        &format!("{TIP}.bg"),
        &format!("height: {}px;", height as i32 - 2 * edge as i32),
    );
    // `ui_node_rect` is absolute and a node's own position is relative to its
    // parent, here the layout's root.
    ctx.ui_set_properties(
        TIP,
        &format!(
            "x: {}px; y: {}px; height: {}px;",
            (x - left).round() as i32,
            (y - top).round() as i32,
            height as i32
        ),
    );
}

/// Fills in the placeholders in the effect text the tooltip is showing, for
/// an item whose text has them: `{Flat}` and the like, which the game's own
/// tooltip fills from the item's settings and a label leaves as written.
///
/// The text went to the label as a reference, which it resolves in the
/// game's language, and nothing tells a mod which language that is. So the
/// label is asked for what it now holds: where that is the resolved text, the
/// numbers go into it and it stays in the game's language. Where the label
/// hands the reference back, they go into the English text, which is the
/// only one a mod can ask for. Either way no placeholder is left showing.
fn fill_placeholders(ctx: &mut StableClient<'_>, key: &str) {
    let Some(card) = card(key).filter(|card| !card.fills.is_empty()) else {
        return;
    };
    let Some((_, english)) = text_ref(ctx, &format!("{key}.option")) else {
        return;
    };
    if !english.contains('{') {
        return;
    }
    let path = format!("{TIP}.desc");
    let resolved = ctx
        .ui_text(&path)
        .filter(|text| !text.starts_with('#') && text.contains('{'));
    log(&format!("fill.{key}"), || {
        format!(
            "placeholders filled with {:?} in {}",
            card.fills,
            if resolved.is_some() {
                "the label's own text"
            } else {
                "the English text"
            }
        )
    });
    let mut text = resolved.unwrap_or(english);
    for (placeholder, value) in &card.fills {
        text = text.replace(&format!("{{{placeholder}}}"), value);
    }
    ctx.ui_set_text(&path, &text);
}

/// Puts the tooltip up for `target`, or takes it down for none.
///
/// A new item's text is written with the tooltip parked off screen. A frame
/// on its placeholders are filled in, and it is only sized and placed
/// [`MEASURE_AFTER`] frames on, once a layout pass has said how tall that
/// text is.
fn show_tip(ctx: &mut StableClient<'_>, board: &Rows, target: Option<Spot>) {
    let (changed, age, body) = with(|panel| {
        let changed = panel.shown != target;
        if changed {
            panel.shown = target;
            panel.shown_for = 0;
            panel.body = None;
        } else {
            panel.shown_for = panel.shown_for.saturating_add(1);
        }
        (changed, panel.shown_for, panel.body)
    })
    .unwrap_or((false, 0, None));

    let Some(spot) = target else {
        if changed {
            ctx.ui_set_visible(TIP, false);
        }
        return;
    };
    let held = slot_at(board, spot);
    let (line, edge) = tip_edge(held.is_some_and(|held| held.pinned));

    if changed {
        let Some(key) = held.and_then(|held| held.key.clone()) else {
            return;
        };
        // Spawned last under the root, so it draws over everything in the
        // match: stacking is tree order here, nothing in this layout is
        // raised by `z`.
        if !ctx.ui_exists(TIP) && !ctx.ui_spawn_source(INGAME, &tip_source()) {
            return;
        }
        let body = fill_tip(ctx, &key);
        ctx.ui_set_properties(
            TIP,
            &format!("y: {PARKED_Y}px; visible: true; color: {line};"),
        );
        ctx.ui_set_properties(
            &format!("{TIP}.bg"),
            &format!("width: {}px;", TIP_W - 2 * edge),
        );
        let _ = with(|panel| panel.body = Some(body));
        return;
    }

    if age == FILL_AFTER {
        if let Some(key) = held.and_then(|held| held.key.clone()) {
            fill_placeholders(ctx, &key);
        }
    }
    if age == MEASURE_AFTER {
        if let Some(body) = body {
            place_tip(ctx, body, &slot_path(spot), edge);
        }
    }
}
