//! Multiplayer: each player's Build Editor builds go to the team that player
//! manages, and every machine plays a match with the same builds.
//!
//! A match is simulated on every machine that shows it: the host's server runs
//! it first, then each player's client plays it live. The native detours that
//! plant pins (`crate::tactics`) run in all of them, so every machine has to
//! know every player's builds and athletes, not only its own. Each client sends
//! its builds and its team's athletes to the server ([`COMMAND`]); the server,
//! which runs in the host's game, sends everyone's back to every client
//! ([`EVENT`]) with the host's Smart Builds and Item Rules and its mod-item
//! registry. A detour then reads an athlete's pins from the player who has it
//! ([`owner_of`], [`pins_of`]).
//!
//! While a session is multiplayer:
//! - Builds go to their owner's athletes only, as under "Apply To Your Players
//!   Only"; the editor greys out "Apply To All Players".
//! - A guest plays by the host's Smart Builds and Item Rules
//!   ([`host_smart_builds`], [`host_item_rules`]).
//! - The automatic 5th and 6th items come from the rules alone: the game's item
//!   network is the host server's and keeps training, so a guest's game could
//!   not ask it the same question.
//! - Pins are planted where they stand on every machine: the pin-aware builds
//!   the stable hook records for the spawn hook exist on the host's only.
//!
//! A session is multiplayer from the room screen (`Room` or `Lobby`), or from
//! the first word of another player's builds, until the title screen is back.
//!
//! Not tried with two players yet (2026-10-24): this is built from the game's
//! scene names and the SDK's notes on commands and events.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use mod_api_stable::{
    SceneKindV1, StableClient, StableCommand, StableEventTarget, StableServerCtx,
};
use serde::{Deserialize, Serialize};

use crate::build_config::PinSnapshot;

// What a client sends the server, and what the server sends every client.
const COMMAND: &str = "riot_items.builds";
const EVENT: &str = "riot_items.builds";

// Frames between two looks at whether this player's builds have changed, and
// between two sends of them when they have not: a server started since
// (another save loaded) has nobody's.
const CHECK_FRAMES: u64 = 30;
const RESEND_FRAMES: u64 = 600;

// Test log: `multiplayer.log` beside the DLL, started over each session. A line
// only when something changes: the session's kind, a send, what was heard.
const LOG: bool = true;
static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

static ACTIVE: AtomicBool = AtomicBool::new(false);
// This game's server has heard from a client since the title screen: it is the
// host's (or a single-player game), whose own settings are the ones in force.
static SERVES: AtomicBool = AtomicBool::new(false);
// The editor's toggles are owed a repaint ([`take_repaint`]).
static REPAINT: AtomicBool = AtomicBool::new(false);

// The host's Smart Builds (0 off, 1 on) and Item Rules (`Rule::code`) as last
// heard; [`UNKNOWN`] before.
const UNKNOWN: u8 = u8::MAX;
static HOST_SMART_BUILDS: AtomicU8 = AtomicU8::new(UNKNOWN);
static HOST_ITEM_RULES: AtomicU8 = AtomicU8::new(UNKNOWN);

// Whether any other player's row has a component focus.
static REMOTE_FOCUS: AtomicBool = AtomicBool::new(false);

// The generation a server's next change goes out as. One counter for every
// server this game runs, so a client never mistakes a new server's state for
// one it heard.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

// One player's builds, as a client sends them and the server passes them on.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Player {
    // Which game sent them ([`own_id`]).
    id: u64,
    // The team the player manages, and its athletes the builds go to.
    team: u64,
    athletes: Vec<u64>,
    // `item-builds.json` as the player has it
    // (`build_config::own_builds_json`).
    builds: serde_json::Value,
}

// What a client sends ([`COMMAND`]).
#[derive(Serialize, Deserialize)]
struct Sent {
    player: Player,
    // The [`Shared::generation`] this client last heard, so a server with
    // nothing new still answers a client that missed its last word.
    heard: u64,
}

// What the server sends ([`EVENT`]): every player's builds, and the host's
// settings and mod-item registry (`tactics::item_registry`).
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
struct Shared {
    generation: u64,
    smart_builds: bool,
    item_rules: u8,
    registry: Vec<String>,
    finals: Vec<u64>,
    players: Vec<Player>,
}

// Another player's builds, as this client last heard them.
struct Remote {
    team: u64,
    athletes: HashSet<u64>,
    pins: Arc<PinSnapshot>,
}

static REMOTES: RwLock<Vec<Remote>> = RwLock::new(Vec::new());

// The client's side of the exchange.
#[derive(Default)]
struct Client {
    frame: u64,
    // What the last send was made from ([`fingerprint`]), and on which frame.
    last: Option<u64>,
    sent_at: u64,
    // The last [`Shared`] heard: its generation, and the other players in it.
    heard: u64,
    others: Vec<Player>,
}

static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

// The server's side: each player's latest builds, and what was last sent out.
#[derive(Default)]
struct Server {
    players: Vec<Player>,
    shared: Shared,
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn log(text: &str) {
    use std::io::Write;
    if !LOG {
        return;
    }
    let mut file = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner());
    if file.is_none() {
        *file = std::fs::File::create(crate::config::mod_dir().join("multiplayer.log")).ok();
    }
    if let Some(file) = file.as_mut() {
        let _ = writeln!(file, "{text}");
    }
}

// This game's mark on the builds it sends, so it knows its own when the server
// sends them back: the team id cannot, since it can change under a save (see
// `tactics::my_team_id`).
fn own_id() -> u64 {
    static ID: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *ID.get_or_init(|| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::process::id().hash(&mut hasher);
        std::time::SystemTime::now().hash(&mut hasher);
        hasher.finish()
    })
}

// Whether the session is multiplayer. One atomic load: the buy detour asks on
// every buy decision.
pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

// The host's Smart Builds toggle, in force on a guest's game; `None` where this
// game's own is (single player, the host, or nothing heard yet).
pub(crate) fn host_smart_builds() -> Option<bool> {
    if !active() || SERVES.load(Ordering::Relaxed) {
        return None;
    }
    match HOST_SMART_BUILDS.load(Ordering::Relaxed) {
        UNKNOWN => None,
        on => Some(on == 1),
    }
}

// The host's Item Rules (`item_tiers::Rule::code`), the same way.
pub(crate) fn host_item_rules() -> Option<u8> {
    if !active() || SERVES.load(Ordering::Relaxed) {
        return None;
    }
    match HOST_ITEM_RULES.load(Ordering::Relaxed) {
        UNKNOWN => None,
        code => Some(code),
    }
}

// Whether another player's builds have a row with a component focus.
pub(crate) fn any_focus() -> bool {
    REMOTE_FOCUS.load(Ordering::Relaxed)
}

// The team of the other player whose athletes include `athlete_id`; the lower
// team id where two have it, which every machine agrees on.
pub(crate) fn owner_of(athlete_id: u64) -> Option<u64> {
    REMOTES
        .read()
        .ok()?
        .iter()
        .filter(|remote| remote.athletes.contains(&athlete_id))
        .map(|remote| remote.team)
        .min()
}

// The builds of the other player managing `team`.
pub(crate) fn pins_of(team: u64) -> Option<Arc<PinSnapshot>> {
    REMOTES
        .read()
        .ok()?
        .iter()
        .find(|remote| remote.team == team)
        .map(|remote| remote.pins.clone())
}

// Whether the editor's toggles should be painted again: the session turned
// multiplayer or back, or the host's settings changed.
pub(crate) fn take_repaint() -> bool {
    REPAINT.swap(false, Ordering::Relaxed)
}

// Every client frame, after `tactics` has published the player's athletes:
// the session's kind, the other players' builds in, and this player's out.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    match ctx.scene_kind() {
        Some(SceneKindV1::Room | SceneKindV1::Lobby) => set_active("the room screen"),
        Some(SceneKindV1::Title) => leave(),
        // Every other screen belongs to whichever session it was reached from.
        _ => {}
    }
    // This mod's only events, so all of them are this module's.
    for event in ctx.take_events() {
        if event.event == EVENT {
            hear(&event.payload);
        }
    }
    send(ctx);
}

fn set_active(why: &str) {
    if !ACTIVE.swap(true, Ordering::Relaxed) {
        REPAINT.store(true, Ordering::Relaxed);
        log(&format!("on ({why})"));
    }
}

// Back on the title screen: whatever the last session was, it is over, and the
// next one's server is sent the builds at once.
fn leave() {
    SERVES.store(false, Ordering::Relaxed);
    HOST_SMART_BUILDS.store(UNKNOWN, Ordering::Relaxed);
    HOST_ITEM_RULES.store(UNKNOWN, Ordering::Relaxed);
    *CLIENT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if !ACTIVE.swap(false, Ordering::Relaxed) {
        return;
    }
    if let Ok(mut remotes) = REMOTES.write() {
        remotes.clear();
    }
    REMOTE_FOCUS.store(false, Ordering::Relaxed);
    crate::build_config::pins_changed();
    REPAINT.store(true, Ordering::Relaxed);
    log("off");
}

// Takes in what the server sent ([`Shared`]).
fn hear(payload: &[u8]) {
    let shared = match serde_json::from_slice::<Shared>(payload) {
        Ok(shared) => shared,
        Err(error) => {
            log(&format!("unreadable builds from the server ({error})"));
            return;
        }
    };
    let others: Vec<Player> = shared
        .players
        .iter()
        .filter(|player| player.id != own_id())
        .cloned()
        .collect();
    {
        let mut guard = CLIENT.lock().unwrap_or_else(|e| e.into_inner());
        let client = guard.get_or_insert_with(Client::default);
        client.heard = shared.generation;
        if client.others != others {
            let remotes: Vec<Remote> = others
                .iter()
                .filter_map(|player| {
                    let pins = crate::build_config::pin_snapshot(&player.builds);
                    if pins.is_none() {
                        log(&format!("team {}'s builds do not read", player.team));
                    }
                    Some(Remote {
                        team: player.team,
                        athletes: player.athletes.iter().copied().collect(),
                        pins: pins?,
                    })
                })
                .collect();
            REMOTE_FOCUS.store(
                remotes.iter().any(|remote| remote.pins.any_focus()),
                Ordering::Relaxed,
            );
            log(&format!(
                "heard {} other player(s): {}",
                remotes.len(),
                remotes
                    .iter()
                    .map(|remote| format!(
                        "team {} ({} athletes, {} builds)",
                        remote.team,
                        remote.athletes.len(),
                        remote.pins.rows()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Ok(mut held) = REMOTES.write() {
                *held = remotes;
            }
            client.others = others;
            crate::build_config::pins_changed();
        }
    }
    let smart = shared.smart_builds as u8;
    let smart_was = HOST_SMART_BUILDS.swap(smart, Ordering::Relaxed);
    let rules_was = HOST_ITEM_RULES.swap(shared.item_rules, Ordering::Relaxed);
    if (smart_was, rules_was) != (smart, shared.item_rules) {
        REPAINT.store(true, Ordering::Relaxed);
    }
    crate::tactics::adopt_item_registry(shared.registry, shared.finals);
    if REMOTES.read().is_ok_and(|remotes| !remotes.is_empty()) {
        set_active("another player's builds");
    }
}

// Sends this player's builds when they have changed, or every
// [`RESEND_FRAMES`]. Only in a game, once the athletes they go to are known.
fn send(ctx: &mut StableClient<'_>) {
    if !ctx.is_in_game() {
        return;
    }
    let mut guard = CLIENT.lock().unwrap_or_else(|e| e.into_inner());
    let client = guard.get_or_insert_with(Client::default);
    client.frame += 1;
    if client.frame % CHECK_FRAMES != 0 {
        return;
    }
    let Some(team) = crate::tactics::my_team_id() else {
        return;
    };
    let athletes = crate::tactics::my_athletes();
    if athletes.is_empty() {
        return;
    }
    // Reads `item-builds.json` if nothing has yet, which a guest's game,
    // running no server, may not have.
    crate::build_config::load_cached();
    let key = fingerprint(team, &athletes);
    if client.last == Some(key) && client.frame - client.sent_at < RESEND_FRAMES {
        return;
    }
    let sent = Sent {
        player: Player {
            id: own_id(),
            team,
            athletes,
            builds: crate::build_config::own_builds_json(),
        },
        heard: client.heard,
    };
    let Ok(bytes) = serde_json::to_vec(&sent) else {
        return;
    };
    let delivered = ctx.send_command(COMMAND, &bytes);
    if client.last != Some(key) {
        log(&format!(
            "sent team {team}'s builds: {} athletes, {} bytes, delivered={delivered}",
            sent.player.athletes.len(),
            bytes.len()
        ));
    }
    if delivered {
        client.last = Some(key);
        client.sent_at = client.frame;
    }
}

// What a send is made from: the team, its athletes, the builds (by the pins'
// generation) and this game's own settings, whose change the host's server
// sends on.
fn fingerprint(team: u64, athletes: &[u64]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    team.hash(&mut hasher);
    athletes.hash(&mut hasher);
    crate::build_config::pins_generation().hash(&mut hasher);
    crate::build_config::own_smart_builds().hash(&mut hasher);
    crate::item_tiers::own_rule_code().hash(&mut hasher);
    hasher.finish()
}

// The server's side of [`COMMAND`]: a player's builds in, and everyone's out
// to every client when anything changed, else to the sender if it missed the
// last word. `false` for any other command.
pub(crate) fn handle_command(ctx: &mut StableServerCtx<'_>, command: &StableCommand<'_>) -> bool {
    if command.command != COMMAND {
        return false;
    }
    SERVES.store(true, Ordering::Relaxed);
    let sent = match serde_json::from_slice::<Sent>(command.payload) {
        Ok(sent) => sent,
        Err(error) => {
            log(&format!("unreadable builds from a client ({error})"));
            return true;
        }
    };
    let (target, payload) = {
        let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let server = guard.get_or_insert_with(Server::default);
        // The sender's last word goes, and so does anyone else's for its team,
        // which one player manages: a game restarted to rejoin sends under a
        // new id.
        server
            .players
            .retain(|player| player.id != sent.player.id && player.team != sent.player.team);
        server.players.push(sent.player);
        let (registry, finals) = crate::tactics::item_registry().unwrap_or_default();
        let mut shared = Shared {
            generation: server.shared.generation,
            smart_builds: crate::build_config::own_smart_builds(),
            item_rules: crate::item_tiers::own_rule_code(),
            registry,
            finals,
            players: server.players.clone(),
        };
        let target = if shared != server.shared {
            shared.generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
            log(&format!(
                "sending {} player(s)' builds to everyone (generation {})",
                shared.players.len(),
                shared.generation
            ));
            server.shared = shared;
            StableEventTarget::Broadcast
        } else if sent.heard != shared.generation {
            command.reply_target()
        } else {
            return true;
        };
        let Ok(payload) = serde_json::to_vec(&server.shared) else {
            return true;
        };
        (target, payload)
    };
    let delivered = ctx.emit_event(target, EVENT, &payload);
    if !delivered {
        log(&format!("could not send the builds to {target:?}"));
    }
    true
}

// A server has started: the last save's players are not this one's.
pub(crate) fn server_started() {
    *SERVER.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
