//! The player's item builds in the in-match Check Tactics panel.
//!
//! Vanilla's Personal Tactics table lists four item columns per player, and
//! with this mod every cell reads "Let Player Decide": the builds are made by
//! the Build Editor and the item-build hooks, which the game's own personal
//! tactics know nothing about. This puts the build each of the player's
//! athletes was actually handed in those cells instead, as item icons, with a
//! tooltip for the one under the cursor.
//!
//! # Where the builds come from
//!
//! Nothing hands a mod "the match on screen". What there is:
//!
//! * Every build decision passes through one of the two item-build hooks
//!   (`item_build_hook::decide_build` for league matches, `hook`'s route
//!   rewrite for the rest), and each call names the champion, the lane and
//!   both lineups. [`note_decision`] keeps the last [`DECISIONS_KEPT`] of them.
//!   Background fixtures go through the same hooks, so most are not the
//!   player's.
//! * The draft the player just watched is read off its screen by
//!   [`super::draft_watch`], as two sets of champions. The strategy screen
//!   hands them over as it closes ([`note_lineup`]).
//! * The panel names the player's team, and the match header names both
//!   sides, so the two can be compared ([`player_side`]).
//!
//! A row's build is the newest decision for its lane whose team is the
//! player's five champions and whose enemies are the other side's. When any
//! link is missing (no draft was seen, a pick could not be tied to a champion,
//! the hooks were never asked) nothing is drawn and the vanilla cells stay.
//!
//! A decision holds the four slots the game allocates. The fifth and sixth
//! are bought by the native buy detour, which picks them when the athlete can
//! afford one, so they are only known here when they are pinned; otherwise
//! the slot is drawn empty.
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
//! The scaling assumes the layout is fitted to the window and centred. Should
//! that be wrong somewhere, a click on an icon shows the same tooltip
//! ([`handle_event`]); clicks stop counting once a hover has been seen.
//!
//! None of this has been seen in game yet. `own_team_log` reports how each
//! match was resolved.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use mod_api_stable::{BuffV1, SettingTargetV1, StableClient, StableItem, UiEventKindV1};
use serde_json::Value;

use crate::build_config::{self, Role};
use crate::strategy_ui::ICON_SHEET;

// -- paths --------------------------------------------------------------------

/// Root of the in-match layout (`ingame:ingame_ui`), 1920x1080.
const INGAME: &str = "ingame";
/// The Check Tactics panel. Authored hidden; game code shows it.
const PANEL: &str = "ingame.strategy_info";
const PERSONAL: &str = "ingame.strategy_info.personal_panel";
const HEADER: &str = "ingame.strategy_info.personal_panel.header";
const ROWS_PATH: &str = "ingame.strategy_info.personal_panel.rows";
/// The team the panel is about: the player's.
const TEAM_NAME: &str = "ingame.strategy_info.team_panel.header.name";
/// The match header's team names, blue then red.
const SIDE_NAMES: [&str; 2] = [
    "ingame.header.blue_info.team_name",
    "ingame.header.red_info.team_name",
];

/// This module's node in each row, and its header and tooltip.
const BUILD: &str = "riot_build";
const HEAD: &str = "riot_build_head";
const TIP_NAME: &str = "riot_build_tip";
const TIP: &str = "ingame.riot_build_tip";

/// Rows in the table, one per lane from Top, and vanilla item cells in each.
const LANES: usize = 5;
const VANILLA_CELLS: usize = 4;

// -- geometry -----------------------------------------------------------------

/// Where vanilla's first item column starts, in a row 36px tall.
const BUILD_X: usize = 300;
const ROW_H: usize = 36;
const SLOT: usize = 30;
const SLOT_Y: usize = (ROW_H - SLOT) / 2;
const SLOT_STRIDE: usize = 38;
const ICON: usize = 26;

const SLOT_FILL: &str = "#1d1f2cff";
/// Border of a slot the AI filled, and of one the player pinned: the grey and
/// the teal the panel already uses for its rules and its team name.
const PICKED_LINE: &str = "#4a4c56ff";
const PINNED_LINE: &str = "#37d5b3ff";
const HOVER_LINE: &str = "#e8e8e8ff";

const TIP_W: usize = 300;
const TIP_PAD: usize = 12;
/// Top of the stat lines, under the icon, name, price and rule.
const TIP_BODY_Y: usize = 64;
const LINE_H: usize = 20;
/// The description label's box while its text is measured: taller than any
/// description, so what comes back is the text's height and not the box's.
const DESC_BOX: usize = 600;
/// Characters of description taken to fill one line, for when the text cannot
/// be measured. On the short side, so the estimate runs tall rather than
/// leaving text hanging out of the tooltip.
const DESC_LINE_CHARS: usize = 34;
const STATS_PER_LINE: usize = 3;
/// Gap kept between the tooltip and the slot, and the edge of the screen.
const TIP_GAP: f32 = 8.0;
/// Where the tooltip waits for a layout pass before it is placed. Off screen
/// rather than hidden: a hidden node may be skipped by the pass.
const PARKED_Y: i32 = -4000;
/// Frames between writing a tooltip's text and reading its size back.
const MEASURE_AFTER: u32 = 2;

const STAT_ICONS: &str = "asset/base/ui/banpick/champion_stat_icon";

// -- item cards ---------------------------------------------------------------

/// What the tooltip says about one item besides its text.
#[derive(Clone)]
struct Card {
    /// Tag in [`ICON_SHEET`].
    frame: String,
    price: usize,
    /// Stat icon tag and value, in [`STAT_ROWS`] order.
    stats: Vec<(&'static str, String)>,
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
                    .and_then(Value::as_i64)
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
            };
            found.push((key.to_string(), card));
        },
    );
    // Nothing found means the document was not ready: try again.
    if found.is_empty() {
        return;
    }
    if let Ok(mut cards) = CARDS.lock() {
        let cards = cards.get_or_insert_with(HashMap::new);
        for (key, card) in found {
            cards.entry(key).or_insert(card);
        }
    }
    ENGINE_CARDS.store(true, Ordering::Relaxed);
}

fn card(key: &str) -> Option<Card> {
    CARDS.lock().ok()?.as_ref()?.get(key).cloned()
}

// -- decisions ----------------------------------------------------------------

/// One slot of a build: the item, and whether the player pinned it there.
#[derive(Clone)]
struct Slot {
    key: Option<String>,
    pinned: bool,
}

/// One build either item-build hook settled on.
struct Decision {
    lane: usize,
    /// The champion's own team, itself included, and the other one. Sorted.
    team: Vec<String>,
    enemies: Vec<String>,
    /// [`build_config::picker_slots`] long.
    slots: Vec<Slot>,
}

/// Decisions kept: sixty matches' worth. The player's are read within a
/// second of being made, so this only has to outlast the fixtures decided in
/// between.
const DECISIONS_KEPT: usize = 600;

static DECISIONS: Mutex<VecDeque<Decision>> = Mutex::new(VecDeque::new());

fn champion_set<'a>(champions: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut set: Vec<String> = champions.map(str::to_string).collect();
    set.sort();
    set.dedup();
    set
}

/// Records the build a champion was handed: `build` is the game's slots, in
/// order, as item keys. Called by both item-build hooks for every athlete of
/// every match, on whichever thread decides it.
///
/// The slots past the game's are read off the pin row here, since no hook
/// decides them.
pub(crate) fn note_decision(
    champion: &str,
    lane: usize,
    allies: &[&str],
    enemies: &[&str],
    build: Vec<String>,
) {
    if lane >= LANES {
        return;
    }
    // Publishes the pin snapshot `pin_row` reads.
    build_config::load_cached();
    let row = build_config::pin_row(champion, Role::from_lane_code(lane));
    let pin = |slot: usize| row.get(slot).and_then(Option::as_ref);

    let game_slots = build_config::game_slots();
    let mut slots: Vec<Slot> = build
        .into_iter()
        .take(game_slots)
        .enumerate()
        .map(|(slot, key)| Slot {
            pinned: pin(slot) == Some(&key),
            key: Some(key),
        })
        .collect();
    slots.resize(
        game_slots,
        Slot {
            key: None,
            pinned: false,
        },
    );
    for slot in game_slots..build_config::picker_slots() {
        let key = pin(slot).cloned();
        slots.push(Slot {
            pinned: key.is_some(),
            key,
        });
    }

    let decision = Decision {
        lane,
        team: champion_set(allies.iter().copied().chain([champion])),
        enemies: champion_set(enemies.iter().copied()),
        slots,
    };
    let Ok(mut decisions) = DECISIONS.lock() else {
        return;
    };
    if decisions.len() >= DECISIONS_KEPT {
        decisions.pop_front();
    }
    decisions.push_back(decision);
}

// -- the match on screen ------------------------------------------------------

/// The champions each side drafted for the match about to be played, blue
/// then red, as champion ids.
static LINEUP: Mutex<Option<[Vec<String>; 2]>> = Mutex::new(None);

/// Goes up with every lineup taken, so a panel still showing the last one's
/// builds can tell.
static LINEUP_TAKEN: AtomicU32 = AtomicU32::new(0);

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
    LINEUP_TAKEN.fetch_add(1, Ordering::Relaxed);
}

/// Enemy picks that have to be known before a decision is taken for the
/// player's match. Fewer than five, because a pick the draft grid named in a
/// way no champion answers to is missing from the lineup; not fewer than
/// this, because the player's five alone could be some other fixture's.
const ENEMIES_NEEDED: usize = 3;

type Rows = Arc<Vec<Vec<Slot>>>;

fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Which side of the match the player's team is on: 0 blue, 1 red.
///
/// By name, the panel's against the header's. Tried exactly first and then as
/// one name inside the other, in case the header shortens it; an answer that
/// fits both sides or neither is no answer.
fn player_side(ctx: &StableClient<'_>) -> Option<usize> {
    let text = |path: &str| {
        ctx.ui_text(path)
            .map(|text| fold(&text))
            .filter(|text| !text.is_empty())
    };
    let sides = SIDE_NAMES.map(|path| text(path));
    let own = [
        text(TEAM_NAME),
        ctx.player_team_id()
            .and_then(|team| ctx.team_name(team))
            .map(|name| fold(&name))
            .filter(|name| !name.is_empty()),
    ];
    for exact in [true, false] {
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
            match (is(&sides[0]), is(&sides[1])) {
                (true, false) => return Some(0),
                (false, true) => return Some(1),
                _ => {}
            }
        }
    }
    None
}

/// The build of each of the player's five lanes in the match on screen, or
/// nothing until every one is known.
fn resolve(ctx: &StableClient<'_>) -> Option<Rows> {
    let lineup = LINEUP.lock().ok()?.clone();
    let side = player_side(ctx);
    if crate::own_team_log::ENABLED {
        crate::own_team_log::on_change(
            "match_builds.lineup",
            format!("match builds: lineup={lineup:?} player_side={side:?}"),
        );
    }
    let (lineup, side) = (lineup?, side?);
    let (mine, theirs) = (&lineup[side], &lineup[1 - side]);
    if mine.len() != LANES || theirs.len() < ENEMIES_NEEDED {
        return None;
    }

    let mut rows: Vec<Option<Vec<Slot>>> = vec![None; LANES];
    {
        let decisions = DECISIONS.lock().ok()?;
        // Newest first: a match decided twice keeps its last answer.
        for decision in decisions.iter().rev() {
            if rows[decision.lane].is_some()
                || &decision.team != mine
                || !theirs
                    .iter()
                    .all(|champion| decision.enemies.contains(champion))
            {
                continue;
            }
            rows[decision.lane] = Some(decision.slots.clone());
        }
    }
    if crate::own_team_log::ENABLED {
        crate::own_team_log::on_change(
            "match_builds.rows",
            format!(
                "match builds: lanes decided={:?}",
                rows.iter().map(Option::is_some).collect::<Vec<_>>()
            ),
        );
    }
    rows.into_iter().collect::<Option<Vec<_>>>().map(Arc::new)
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
    /// [`LINEUP_TAKEN`] as of the lineup `rows` were resolved against.
    lineup: u32,
    rows: Option<Rows>,
    /// Whether the rows on screen were spawned with the game's own items
    /// described: before that, their icons are guesses.
    spawned_with_cards: Option<bool>,
    /// The slot the tooltip is up for, and how many frames it has been.
    shown: Option<(usize, usize)>,
    shown_for: u32,
    body: Option<TipBody>,
    /// A slot a click asked the tooltip for.
    sticky: Option<(usize, usize)>,
    clicked_frame: u32,
}

impl Panel {
    const fn new() -> Self {
        Self {
            up: false,
            open: false,
            frame: 0,
            resolve_at: 0,
            lineup: 0,
            rows: None,
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

/// Frames between two tries of [`resolve`] while the match is not yet known.
const RESOLVE_EVERY: u32 = 30;

// -- per frame ----------------------------------------------------------------

/// Per-frame entry point, called from the mod's one client hook. Outside a
/// match this is one failed path lookup.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let Some(open) = ctx.ui_visible(PANEL) else {
        // The match is over, and everything spawned into its layout with it.
        if with(|panel| std::mem::replace(panel, Panel::new()).up).unwrap_or(false) {
            if let Ok(mut lineup) = LINEUP.lock() {
                *lineup = None;
            }
            if let Ok(mut set) = REGISTERED.lock() {
                set.clear();
            }
        }
        return;
    };

    let taken = LINEUP_TAKEN.load(Ordering::Relaxed);
    let (frame, opened, due, rows, outdated) = with(|panel| {
        panel.up = true;
        panel.frame = panel.frame.wrapping_add(1);
        let opened = open && !panel.open;
        panel.open = open;
        // A layout that outlived its match would go on showing that match's
        // builds: a new lineup starts this one over.
        let outdated = panel.lineup != taken && panel.spawned_with_cards.is_some();
        if panel.lineup != taken {
            panel.lineup = taken;
            panel.rows = None;
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
            panel.rows.clone(),
            outdated,
        )
    })
    .unwrap_or((0, false, false, None, false));
    if outdated {
        unpaint_rows(ctx);
    }

    // Asked for from the start of the match rather than when the panel opens,
    // so the icons are there the first time it does. The panel opening is a
    // reason to ask again at once: its team name may only be filled in then.
    let rows = match rows {
        Some(rows) => rows,
        None => {
            if !due && !opened {
                return;
            }
            let rows = resolve(ctx);
            let _ = with(|panel| {
                panel.resolve_at = frame.wrapping_add(RESOLVE_EVERY);
                panel.rows = rows.clone();
            });
            let Some(rows) = rows else {
                return;
            };
            rows
        }
    };

    if !open {
        show_tip(ctx, &rows, None);
        return;
    }

    prime_engine_cards(ctx);
    if !paint_rows(ctx, &rows) {
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

fn row_path(row: usize) -> String {
    format!("{ROWS_PATH}.row{row}")
}

fn slot_path(row: usize, slot: usize) -> String {
    format!("{ROWS_PATH}.row{row}.{BUILD}.s{slot}")
}

/// Keeps the icons in the table and the vanilla cells out of it. False until
/// every row has its icons, and the vanilla table is left whole until then.
///
/// Re-asserted every frame the panel is open: game code owns these rows, and
/// what it does to them when it refills the panel is not known.
fn paint_rows(ctx: &mut StableClient<'_>, rows: &Rows) -> bool {
    let cards = ENGINE_CARDS.load(Ordering::Relaxed);
    let redraw = with(|panel| {
        let redraw = panel
            .spawned_with_cards
            .is_some_and(|before| before != cards);
        panel.spawned_with_cards = Some(cards);
        redraw
    })
    .unwrap_or(false);

    let mut complete = true;
    for (row, slots) in rows.iter().enumerate() {
        let host = row_path(row);
        let node = format!("{host}.{BUILD}");
        if redraw {
            ctx.ui_remove_node(&node);
        }
        if ctx.ui_exists(&node) {
            continue;
        }
        if !ctx.ui_spawn_source(&host, &row_source(slots)) {
            complete = false;
            continue;
        }
        for (slot, held) in slots.iter().enumerate() {
            if held.key.is_some() {
                register_once(ctx, &slot_path(row, slot));
            }
        }
    }
    if !complete {
        return false;
    }

    for row in 0..rows.len() {
        for cell in 0..VANILLA_CELLS {
            ctx.ui_set_visible(&format!("{ROWS_PATH}.row{row}.item{cell}"), false);
        }
    }
    for column in 1..=VANILLA_CELLS {
        ctx.ui_set_visible(&format!("{HEADER}.col_item{column}"), false);
    }
    if !ctx.ui_exists(&format!("{HEADER}.{HEAD}")) {
        let source = head_source(ctx);
        ctx.ui_spawn_source(HEADER, &source);
    }
    true
}

/// Gives the table back to vanilla: this module's nodes out, the game's
/// cells and headings back in.
fn unpaint_rows(ctx: &mut StableClient<'_>) {
    ctx.ui_set_visible(TIP, false);
    for row in 0..LANES {
        ctx.ui_remove_node(&format!("{}.{BUILD}", row_path(row)));
        for cell in 0..VANILLA_CELLS {
            ctx.ui_set_visible(&format!("{ROWS_PATH}.row{row}.item{cell}"), true);
        }
    }
    for column in 1..=VANILLA_CELLS {
        ctx.ui_set_visible(&format!("{HEADER}.col_item{column}"), true);
    }
    ctx.ui_remove_node(&format!("{HEADER}.{HEAD}"));
}

/// The one column heading that replaces vanilla's four. The Builds tab's own
/// name, which every language the mod ships already has.
fn head_source(ctx: &StableClient<'_>) -> String {
    let reference = "#asset/base/text/ui?builds.tab";
    let text = match ctx.i18n(reference) {
        Some(text) if !text.is_empty() && !text.starts_with('#') => reference,
        _ => "Builds",
    };
    format!(
        "{HEAD}:label {{\n\
         @\"asset/base/style/main#label\";\n\
         ignore_event: true;\n\
         x: {BUILD_X}px;\n\
         width: 300px;\n\
         height: 24px;\n\
         size: 15;\n\
         color: #a3a9b6ff;\n\
         align_y: Center;\n\
         text: \"{text}\";\n\
         }}\n"
    )
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

/// One row's slots, all of them stated up front: a build does not change
/// during its match, so nothing here is ever repainted.
///
/// A filled slot is a button so that it takes a click and draws its own hover
/// border; an empty one is a plain square.
fn row_source(slots: &[Slot]) -> String {
    let width = slots.len() * SLOT_STRIDE;
    let mut source = format!(
        "{BUILD}:empty {{\n\
         x: {BUILD_X}px;\n\
         width: {width}px;\n\
         height: {ROW_H}px;\n"
    );
    for (index, slot) in slots.iter().enumerate() {
        let x = index * SLOT_STRIDE;
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
        let line = if slot.pinned {
            PINNED_LINE
        } else {
            PICKED_LINE
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
             back_color: {SLOT_FILL};\n\
             stroke: 1;\n\
             rounding: Uniform {{ rounding: 6; }}\n\
             }}\n\
             \n\
             hover: {{\n\
             btn: {{\n\
             color: {HOVER_LINE};\n\
             back_color: {SLOT_FILL};\n\
             stroke: 1;\n\
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
    Some((
        left + width / 2.0 + (x - client_w / 2.0) * scale,
        top + height / 2.0 + (y - client_h / 2.0) * scale,
    ))
}

/// The filled slot under the cursor, as row and slot.
fn hovered_slot(ctx: &StableClient<'_>, rows: &Rows) -> Option<(usize, usize)> {
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
    for (row, slots) in rows.iter().enumerate() {
        if !under(&format!("{}.{BUILD}", row_path(row))) {
            continue;
        }
        return slots
            .iter()
            .enumerate()
            .find(|(slot, held)| held.key.is_some() && under(&slot_path(row, *slot)))
            .map(|(slot, _)| (row, slot));
    }
    None
}

/// The row and slot a slot's path names.
fn slot_of(path: &str) -> Option<(usize, usize)> {
    let rest = path.strip_prefix(ROWS_PATH)?.strip_prefix(".row")?;
    let (row, rest) = rest.split_once('.')?;
    let slot = rest.strip_prefix(BUILD)?.strip_prefix(".s")?;
    Some((row.parse().ok()?, slot.parse().ok()?))
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
    let Some(target) = slot_of(&event.path) else {
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

/// Sizes the tooltip to its text and puts it by its slot: above where there
/// is room, below where there is not, and inside the screen either way.
fn place_tip(ctx: &mut StableClient<'_>, body: TipBody, slot: &str) {
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
        &format!("height: {}px;", height as i32 - 2),
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

/// Puts the tooltip up for `target`, or takes it down for none.
///
/// A new item's text is written with the tooltip parked off screen, and it is
/// only sized and placed [`MEASURE_AFTER`] frames on, once a layout pass has
/// said how tall that text is.
fn show_tip(ctx: &mut StableClient<'_>, rows: &Rows, target: Option<(usize, usize)>) {
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

    let Some((row, slot)) = target else {
        if changed {
            ctx.ui_set_visible(TIP, false);
        }
        return;
    };

    if changed {
        let Some(key) = rows
            .get(row)
            .and_then(|slots| slots.get(slot))
            .and_then(|held| held.key.clone())
        else {
            return;
        };
        // Spawned last under the root, so it draws over everything in the
        // match: stacking is tree order here, nothing in this layout is
        // raised by `z`.
        if !ctx.ui_exists(TIP) && !ctx.ui_spawn_source(INGAME, &tip_source()) {
            return;
        }
        let body = fill_tip(ctx, &key);
        ctx.ui_set_properties(TIP, &format!("y: {PARKED_Y}px; visible: true;"));
        let _ = with(|panel| panel.body = Some(body));
        return;
    }

    if age == MEASURE_AFTER {
        if let Some(body) = body {
            place_tip(ctx, body, &slot_path(row, slot));
        }
    }
}
