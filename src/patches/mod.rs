//! Item balance patches: whenever the game patches its champions, the items are
//! rebalanced from how they did on the version that just ended (`item_stats`
//! counts per version). Winners held too much are nerfed, items that neither
//! won nor were held are buffed ([`balance`]), and a news article lists what
//! moved ([`text::news`]).
//!
//! The game's patch is noticed in the player's inbox, where its patch notes
//! arrive ([`watch_news`]); only notes the game signed count, so another mod's
//! patches (toxicsnek's Auto Balance Patcher hotfixes) bring none of the items'
//! ([`by_the_game`]). Where the inbox cannot be read, the first match on a newer
//! version stands in. The item patch lands at once so its notes come with the
//! game's ([`ALERT_FRAMES`]). A number is only patched where the match ([`live`]), the
//! tooltips ([`text`]) and the builds can follow it: build choices lean with an
//! item's patches ([`leaning`]), or a nerfed popular item would stay just as
//! popular.
//!
//! Modules: [`fields`] (what may move, in what steps), [`base`] (patched items
//! and their unpatched numbers), [`state`] (what the save keeps), [`balance`]
//! (choosing changes), [`live`] (what matches, hooks and tooltips read),
//! [`text`] (tooltips and notes), [`steam`] (a hotfix's byline).
//!
//! A hotfix is the player's own patch: numbers set in the Item Patcher Settings
//! window (`crate::ui::patch_settings`) wait in the save ([`State::staged`])
//! until Queue Manual Patch queues one ([`balance::manual`]). It lands when the
//! calendar next moves ([`server_tick`], or [`watch_news`] for a guest) as
//! "Item Patch Notes v… - Hotfix N", signed with the player's Steam name; with
//! nothing staged there is no hotfix.
//!
//! The game's own patch leaves a queued hotfix to land right behind it, and
//! with `automatic` off ([`tuning`]) hotfixes are the only patches. Hand-set
//! numbers read exactly as typed (`State::exact`), and the window also holds
//! the automatic patches' limits ([`State::limits`]); with `match_builds::LOG`
//! on, `match-builds.log` has a `patch.` line for everything.

pub(crate) mod balance;
pub(crate) mod base;
pub(crate) mod fields;
pub(crate) mod live;
pub(crate) mod state;
pub(crate) mod steam;
pub(crate) mod text;
pub(crate) mod tuning;

use std::cmp::Ordering as Order;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use mod_api_stable::*;

use crate::config::ItemConfig;
use state::State;

// Whether patches are on: the `enabled` switch of the patch settings
// (`tuning`). Off, nothing is read, patched or written, and a save that has
// a patch state keeps it untouched.
fn enabled() -> bool {
    tuning::tuning().enabled
}

// The key the patch state lives under, in the mod's namespace of the save.
const KEY: &str = "item_patches";

// Frames between two looks at whether a patch is due.
const CHECK_FRAMES: u32 = 30;

// Frames a due patch is looked for on every frame after each server management
// tick, since the calendar's move brings what a patch waits for; left to
// [`CHECK_FRAMES`], its notes could come a day late.
const ALERT_FRAMES: u32 = 120;

// What the build hooks add to an item's score at full lean ([`leaning`]), on
// the scale of `item_build_hook`'s bonuses (a mod final gets 0.5).
pub(crate) const LEAN_SCORE: f32 = 0.2;

// The command the client sends its server extension to post a patch's article
// at once. Otherwise it waits for a management tick, which an idle management
// screen never has.
const SERVER_COMMAND: &str = "item_patches";

pub(crate) fn log(key: &str, text: impl FnOnce() -> String) {
    crate::match_builds::log(key, text);
}

// -- registration ---------------------------------------------------------------

// Notes one of this mod's items as it registers, with the constructor that
// makes it from a config: what a patch needs to know what the item's stats
// would be with other numbers.
pub(crate) fn note_mod_item<T: StableItem>(
    key: &'static str,
    item: &T,
    build: fn(&ItemConfig) -> T,
) {
    base::note_mod_item(
        key,
        base::ModItem {
            tier: item.tier(),
            next: item.next_tier(),
            previous: item.previous_tier(),
            registered: item.stat(),
            stat_of: Box::new(move |config| build(config).stat()),
        },
    );
}

// Notes something that keeps a copy of `key`'s numbers outside the item:
// `refresh` is called with the item's config whenever the patches change.
pub(crate) fn note_refresh(
    key: &'static str,
    refresh: impl Fn(&ItemConfig) + Send + Sync + 'static,
) {
    base::note_refresh(key, Box::new(refresh));
}

// How `key`'s patches lean a build choice, -1 (nerfed) to 1 (buffed).
pub(crate) fn leaning(key: &str) -> f32 {
    live::leaning(key)
}

// -- the save's state -----------------------------------------------------------

struct Session {
    // The save's state has been read.
    loaded: bool,
    state: State,
    // The state has changed since it was last written to the save.
    dirty: bool,
    // Frames until a change made in the Item Patcher Settings window is
    // written ([`EDIT_SETTLE_FRAMES`]), 0 while none waits.
    edited: u32,
    frame: u32,
    // The server's management ticks as last counted ([`CALENDAR_TICKS`]),
    // and the frames left of looking on every one ([`ALERT_FRAMES`]).
    ticks: u32,
    alert: u32,
    watch: Watch,
}

impl Session {
    const fn new() -> Self {
        Self {
            loaded: false,
            state: State {
                format: 0,
                version: None,
                number: 0,
                hotfixes: 0,
                queued: false,
                ratios: std::collections::BTreeMap::new(),
                history: Vec::new(),
                exact: std::collections::BTreeMap::new(),
                staged: std::collections::BTreeMap::new(),
                limits: std::collections::BTreeMap::new(),
            },
            dirty: false,
            edited: 0,
            frame: 0,
            ticks: 0,
            alert: 0,
            watch: Watch::new(),
        }
    }
}

static SESSION: Mutex<Session> = Mutex::new(Session::new());

struct Article {
    team: usize,
    title: String,
    body: String,
    author: String,
}

// Articles the client has written for the server to post: only the server
// can.
static ARTICLES: Mutex<Vec<Article>> = Mutex::new(Vec::new());
static ARTICLE_WAITING: AtomicBool = AtomicBool::new(false);

// The state as the client last had it, for the server to put in the save
// from its side ([`keep_state`]). Taken as it is written.
static SERVER_STATE: Mutex<Option<String>> = Mutex::new(None);

// Hands `state` to the server's half of the mod.
fn hand_to_server(state: &State) {
    if let Ok(mut held) = SERVER_STATE.lock() {
        *held = Some(state.to_json());
    }
}

// Whether a hotfix is queued in the loaded save, mirrored from
// [`State::queued`] for the window's queue button.
static HOTFIX_QUEUED: AtomicBool = AtomicBool::new(false);
// A click has changed the queue and the save has not been told yet.
static QUEUE_CHANGED: AtomicBool = AtomicBool::new(false);
// Server management ticks run ([`server_tick`]), which happen only while the
// calendar moves. The client watches closely after each, and a queued hotfix is
// due at the next.
static CALENDAR_TICKS: AtomicU32 = AtomicU32::new(0);
static QUEUED_AT_TICK: AtomicU32 = AtomicU32::new(0);
// Times the inbox has been seen to take new articles ([`watch_news`]), and
// what that count was when the hotfix was queued: what sets a hotfix off
// where no server of this client's ticks.
static NEWS_CYCLES: AtomicU32 = AtomicU32::new(0);
static QUEUED_AT_CYCLE: AtomicU32 = AtomicU32::new(0);

// Whether the loaded save has hand-set numbers waiting ([`State::staged`]),
// without which no hotfix can be queued; an atomic like the queue.
static STAGED_ANY: AtomicBool = AtomicBool::new(false);

// Takes whether `state` has numbers waiting for a hotfix.
fn note_staged(state: &State) {
    STAGED_ANY.store(!state.staged.is_empty(), Ordering::Relaxed);
}

// Whether a hotfix can be queued at all: patches are on.
pub(crate) fn hotfixes_offered() -> bool {
    enabled()
}

pub(crate) fn hotfix_queued() -> bool {
    HOTFIX_QUEUED.load(Ordering::Relaxed)
}

// Whether there is anything for a hotfix to release: a number the player
// has set by hand in the Item Patcher Settings window.
pub(crate) fn hotfix_ready() -> bool {
    STAGED_ANY.load(Ordering::Relaxed)
}

// Queues a hotfix for the calendar's next move, from the window's queue button;
// [`sync`] writes it to the save. A queued one stays as it is, and nothing is
// queued with nothing to release.
pub(crate) fn queue_hotfix() {
    if HOTFIX_QUEUED.load(Ordering::Relaxed) || !hotfix_ready() {
        return;
    }
    QUEUED_AT_CYCLE.store(NEWS_CYCLES.load(Ordering::Relaxed), Ordering::Relaxed);
    QUEUED_AT_TICK.store(CALENDAR_TICKS.load(Ordering::Relaxed), Ordering::Relaxed);
    HOTFIX_QUEUED.store(true, Ordering::Relaxed);
    QUEUE_CHANGED.store(true, Ordering::Relaxed);
    // Numbered: the test log leaves out a line that says what the last one
    // of its kind said, and every queueing has to show.
    static QUEUED: AtomicU32 = AtomicU32::new(0);
    let number = QUEUED.fetch_add(1, Ordering::Relaxed) + 1;
    log("patch.queue", || format!("hotfix queued, the {number}. time this session"));
}

// -- the Item Patcher Settings window -------------------------------------------

// Frames a change made in the Item Patcher Settings window waits before it
// is written to the save: a number is typed a digit at a time, and each
// digit is a change.
const EDIT_SETTLE_FRAMES: u32 = 30;

// The save's patch state as the Item Patcher Settings window
// (`crate::ui::patch_settings`) reads it. Nothing while patches are off or
// no save's state has been read.
pub(crate) fn settings<T>(read: impl FnOnce(&State) -> T) -> Option<T> {
    let session = SESSION.lock().ok()?;
    (enabled() && session.loaded).then(|| read(&session.state))
}

// Changes the save's patch state from that window: staged numbers
// ([`State::staged`]) and limits ([`State::limits`]), neither of which a match
// reads. The save is told once the window has been left alone a moment
// ([`sync`]); returns whether there was a state.
pub(crate) fn edit_settings(edit: impl FnOnce(&mut State)) -> bool {
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if !enabled() || !session.loaded {
        return false;
    }
    edit(&mut session.state);
    note_staged(&session.state);
    session.edited = EDIT_SETTLE_FRAMES;
    true
}

// The queue as `state` has it, for a save just read or left. A queued
// hotfix waits for the calendar to move from here on: what came before is
// not counted.
fn take_queue(state: &State) {
    note_staged(state);
    QUEUED_AT_TICK.store(CALENDAR_TICKS.load(Ordering::Relaxed), Ordering::Relaxed);
    HOTFIX_QUEUED.store(state.queued, Ordering::Relaxed);
    QUEUED_AT_CYCLE.store(NEWS_CYCLES.load(Ordering::Relaxed), Ordering::Relaxed);
    QUEUE_CHANGED.store(false, Ordering::Relaxed);
}

// The state `text` holds, upgraded from the old shape (an item's ratios once,
// under its legendary) to one entry per tier.
fn read_state(text: &str) -> Option<State> {
    let mut state = State::parse(text)?;
    if state.format < state::FORMAT {
        for (id, family) in &base::base().families {
            let Some(ratios) = state.ratios.get(id).cloned() else {
                continue;
            };
            for member in &family.members {
                state
                    .ratios
                    .entry(member.key.clone())
                    .or_insert_with(|| ratios.clone());
            }
        }
        state.format = state::FORMAT;
    }
    Some(state)
}

// Orders two versions by their numbers (`1.10` after `1.9`), so the matches and
// the patch notes cannot spell one version two ways (`1.2`, `1.2.0`, a letter
// in front). Versions with no number are compared as text.
fn version_order(a: &str, b: &str) -> Order {
    let numbers = |text: &str| -> Vec<u64> {
        let mut numbers: Vec<u64> = text
            .split(|letter: char| !letter.is_ascii_digit())
            .filter_map(|run| run.parse().ok())
            .collect();
        while numbers.len() > 1 && numbers.last() == Some(&0) {
            numbers.pop();
        }
        numbers
    };
    let (a_numbers, b_numbers) = (numbers(a), numbers(b));
    if a_numbers.is_empty() && b_numbers.is_empty() {
        return a.cmp(b);
    }
    a_numbers.cmp(&b_numbers)
}

// Looks at the inbox between two full readings of it.
const WATCH_REREAD: u32 = 30;

// The text the game signs its own patch notes with (`patch_note.author` in
// `asset/base/text/news`), written into each note in the language the game was
// in then. The current language's is asked of the game ([`GAME_AUTHOR_KEY`]);
// these four, every language's as of 0.6.5, keep notes from before a language
// change counting.
const GAME_PATCH_AUTHORS: [&str; 4] = [
    "Team Samoyed",
    "Ban Phân Tích Bản Cập Nhật",
    "Zespół balansujący",
    "Hui Samoyed",
];
const GAME_AUTHOR_KEY: &str = "#asset/base/text/news?patch_note.author";

// Whether a patch note is the game's own, by its author. Another mod's patch
// notes announce no patch of the game's (toxicsnek's Auto Balance Patcher,
// `sanity_protection`, signs the hotfixes it deploys "toxicsnek"), and the items
// patch only with the game. A note with no author is taken as the game's.
fn by_the_game(author: &str, game_author: Option<&str>) -> bool {
    author.is_empty() || Some(author) == game_author || GAME_PATCH_AUTHORS.contains(&author)
}

// What the player's inbox has said of the game's own patches.
struct Watch {
    // The team whose news was read.
    team: Option<usize>,
    // How many articles it had then.
    seen: usize,
    // The last of them, by its date and title.
    tail: Option<String>,
    // The newest version a champion patch note among them announces, of the
    // game's own notes ([`by_the_game`]).
    newest: Option<String>,
    looks: u32,
}

impl Watch {
    const fn new() -> Self {
        Self {
            team: None,
            seen: 0,
            tail: None,
            newest: None,
            looks: 0,
        }
    }
}

// Keeps [`Watch::newest`], the version of the newest champion patch notes in
// the player's inbox (a `PatchNote` with a `version` in the team record's
// `news`), since the game states its version nowhere else. Only the game's own
// notes count ([`by_the_game`]). Every two seconds on the management screens,
// and every frame for a while after the calendar moves.
//
// A look reads only the article after the last one seen; when one has come, or
// every [`WATCH_REREAD`]th look, the list is read whole. Each reading with news
// counts toward [`NEWS_CYCLES`], and a queued hotfix goes out with the next.
fn watch_news(ctx: &StableClient<'_>, watch: &mut Watch, counted: bool) {
    use serde_json::Value;

    let Some(team) = ctx.player_team_id() else {
        return;
    };
    let read = |path: &str| {
        ctx.record_get_json(RecordKindV1::Team, team, path)
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            .filter(|value| !value.is_null())
    };
    let first = watch.team != Some(team);
    // The whole reading comes round among the two-second looks only: with
    // the looks of every frame counted it would be one every half second.
    let reread = first || (counted && watch.looks % WATCH_REREAD == 0);
    if counted {
        watch.looks = watch.looks.wrapping_add(1);
    }
    if !reread && read(&format!("news.{}.date", watch.seen)).is_none() {
        return;
    }
    let started = std::time::Instant::now();
    let Some(Value::Array(articles)) = read("news") else {
        log("patch.watch", || format!("team {team}'s news could not be read"));
        return;
    };
    let game_author = ctx.i18n(GAME_AUTHOR_KEY);
    let mut notes: Vec<&Value> = Vec::new();
    // Patch notes another author signed, by author.
    let mut others: HashMap<String, usize> = HashMap::new();
    for article in &articles {
        let Some(note) = article.pointer("/ty/PatchNote") else {
            continue;
        };
        let author = article.get("author").and_then(Value::as_str).unwrap_or("");
        if by_the_game(author, game_author.as_deref()) {
            notes.push(note);
        } else {
            *others.entry(author.to_string()).or_default() += 1;
        }
    }
    let newest = notes
        .iter()
        .filter_map(|note| note.get("version")?.as_str().map(str::to_string))
        .max_by(|a, b| version_order(a, b));
    if first {
        // What a whole reading costs, once: it is the dear one.
        log("patch.watch.cost", || {
            format!(
                "{} articles read and parsed in {:?}",
                articles.len(),
                started.elapsed()
            )
        });
    }
    log("patch.watch", || {
        // A note whose version is not text would be why there is no newest.
        let odd = notes
            .iter()
            .find(|note| note.get("version").and_then(Value::as_str).is_none())
            .map(|note| note.to_string().chars().take(160).collect::<String>());
        format!(
            "team {team}: {} champion patch note(s) by the game ({game_author:?}), newest v{newest:?}{}{}",
            notes.len(),
            odd.map(|odd| format!("; one reads {odd}")).unwrap_or_default(),
            if others.is_empty() {
                String::new()
            } else {
                format!("; ignored, by other authors: {others:?}")
            }
        )
    });
    // The newest article, by what stays as it was posted: reading one marks
    // it read.
    let tail = articles.last().map(|article| {
        let part = |name: &str| article.get(name).map(Value::to_string).unwrap_or_default();
        format!("{} {}", part("date"), part("title"))
    });
    // News has come in since the last reading: the list is longer, or it is
    // as long and ends on another article, as a list kept to a length would.
    let grown = articles.len() > watch.seen
        || (articles.len() == watch.seen && tail != watch.tail);
    if !first && grown {
        NEWS_CYCLES.fetch_add(1, Ordering::Relaxed);
    }
    watch.team = Some(team);
    watch.seen = articles.len();
    watch.tail = tail;
    watch.newest = newest;
}

// How every item did between two of the game's patches: the matches of each
// version counted from `judged`, the one the items were settled on, up to
// `announced`, the new one; and how many matches that is. Usually one version.
// A mod that moves the game's version between the game's own patches splits
// that time into several, and it is judged whole.
fn window_tallies(judged: &str, announced: &str) -> (HashMap<String, balance::Tally>, u32) {
    let mut tallies: HashMap<String, balance::Tally> = HashMap::new();
    let mut matches = 0;
    for version in crate::item_stats::patches() {
        if version_order(&version, judged) == Order::Less
            || version_order(&version, announced) != Order::Less
        {
            continue;
        }
        let window = crate::item_stats::snapshot(Some(version.as_str()), None);
        matches += window.matches;
        for (key, totals) in &window.rows {
            let tally = tallies.entry(key.clone()).or_default();
            tally.games += totals.games;
            tally.wins += totals.wins;
        }
    }
    (tallies, matches)
}

// The screens a patch lands on: the management ones and the pre-match lineup,
// but nothing from the stadium entrance on, so a match's numbers never change
// under it.
fn can_land(scene: Option<ClientSceneKindV1>) -> bool {
    matches!(
        scene,
        Some(ClientSceneKindV1::Main | ClientSceneKindV1::Lineup)
    )
}

// Lands a patch if one is due. See the module docs for when that is.
// `alert`: the calendar has just moved, and this is one of the looks of
// every frame that follow.
fn consider(ctx: &mut StableClient<'_>, session: &mut Session, alert: bool) {
    let scene = ctx.client_scene_kind();
    let slow_look = session.frame % (CHECK_FRAMES * 4) == 0;
    if (slow_look || alert) && can_land(scene) {
        watch_news(ctx, &mut session.watch, slow_look);
    }
    // A queued hotfix is due once the calendar has moved since it was
    // queued, or news has come in.
    let hotfix_due = HOTFIX_QUEUED.load(Ordering::Relaxed)
        && (CALENDAR_TICKS.load(Ordering::Relaxed) != QUEUED_AT_TICK.load(Ordering::Relaxed)
            || NEWS_CYCLES.load(Ordering::Relaxed) != QUEUED_AT_CYCLE.load(Ordering::Relaxed));
    // The version the game is on: the newest its own patch notes announce.
    // The items patch only when the game releases a patch, so a version that
    // only matches carry (another mod can move it between the game's patches)
    // starts no patch. A match's version stands in where the inbox could not be
    // read, and for the version this save starts on before any patch note.
    let played = || {
        crate::item_stats::patches()
            .into_iter()
            .max_by(|a, b| version_order(a, b))
    };
    let current = if session.watch.team.is_some() {
        match session.watch.newest.clone() {
            Some(newest) => Some(newest),
            None if session.state.version.is_none() => played(),
            None => None,
        }
    } else {
        played()
    };
    let Some(current) = current else {
        // No match has been counted in this save yet, and no patch note read.
        return;
    };
    // The version whose matches are judged, the version the patch is
    // announced as, and whether it is a hotfix.
    let (judged, announced, hotfix) = match session.state.version.clone() {
        // The first version this save is seen on: nothing came before it to
        // judge.
        None => {
            session.state.version = Some(current);
            session.dirty = true;
            return;
        }
        // The game has patched its champions since the items were settled:
        // the version that ended is judged, and the patch is the new one's.
        Some(settled) if version_order(&current, &settled) == Order::Greater => {
            if !tuning::tuning().automatic {
                // The game's patches bring none of the items' (`automatic`
                // is off). The new version is still the one the items are
                // on, for a queued hotfix to be a hotfix of.
                session.state.version = Some(current);
                session.state.hotfixes = 0;
                session.dirty = true;
                return;
            }
            (settled, current, false)
        }
        // Queued by the player, and the calendar has moved: a hotfix of the
        // version the game is on. It judges no version's matches.
        Some(settled) if hotfix_due => (settled.clone(), settled, true),
        Some(_) => return,
    };

    // A patch lands at once rather than waiting for the simulations to go
    // quiet: they are the other fixtures of the calendar's move, and by then
    // the player is in their match and the notes a day late.
    if !can_land(scene) {
        return;
    }
    // The game's own patch does not release what a queued hotfix is there
    // to release: that hotfix stays queued, and lands right behind this
    // patch. A hotfix itself empties the queue, whatever comes of it.
    let still_queued = !hotfix
        && HOTFIX_QUEUED.load(Ordering::Relaxed)
        && !session.state.staged.is_empty();
    if !still_queued {
        session.state.queued = false;
        take_queue(&session.state);
    }

    let number = session.state.number + 1;
    let mut patch = if hotfix {
        // A hotfix is only the numbers the player set by hand; nothing is
        // judged for it, and with none waiting there is no patch and no notes.
        if session.state.staged.is_empty() {
            session.dirty = true;
            log("patch.landed", || {
                "a hotfix came due with no number set by hand: none made".to_string()
            });
            return;
        }
        let patch = balance::manual(base::base(), &session.state, number, &judged);
        // Released, or found to change nothing: either way no longer waiting.
        session.state.staged.clear();
        note_staged(&session.state);
        patch
    } else {
        let (tallies, matches) = window_tallies(&judged, &announced);
        // Whether there is enough to judge an item on is asked item by item
        // (`tuning`'s `min_games`). A version of few matches has few such
        // items, or none, and then no patch.
        balance::decide(
            base::base(),
            &session.state,
            &tallies,
            matches,
            number,
            &judged,
        )
    };
    log("patch.landed", || {
        format!(
            "patch {number} as v{announced} (hotfix={hotfix}), on v{judged} ({} matches): {} change(s){}",
            patch.matches,
            patch.changes.len(),
            patch
                .changes
                .iter()
                .map(|change| format!(
                    " | {} {} {} x{:.3} {}/{}",
                    if change.buff { "buff" } else { "nerf" },
                    change.family,
                    change.field,
                    change.ratio,
                    change.wins,
                    change.games
                ))
                .collect::<String>()
        )
    });

    if !hotfix {
        // The game's version has moved on, and its hotfixes are counted
        // from one again.
        session.state.version = Some(announced.clone());
        session.state.hotfixes = 0;
    }
    session.dirty = true;
    // A version that changed nothing is not a patch: no number, no article.
    if patch.changes.is_empty() {
        return;
    }
    if hotfix {
        session.state.hotfixes += 1;
        patch.hotfix = session.state.hotfixes;
    }
    patch.announced = announced;
    let (title, body, author) = text::news(&patch, &session.state);
    session.state.adopt(patch);
    live::rebuild(base::base(), &session.state);
    // Before the article: the server writes the one and posts the other in
    // the same call.
    hand_to_server(&session.state);
    match ctx.player_team_id() {
        Some(team) => {
            if let Ok(mut articles) = ARTICLES.lock() {
                articles.push(Article {
                    team,
                    title,
                    body,
                    author,
                });
                ARTICLE_WAITING.store(true, Ordering::Relaxed);
            }
        }
        None => log("patch.news", || {
            "no article: the client does not know the player's team".to_string()
        }),
    }
    // Posted now ([`SERVER_COMMAND`]), or at the next management tick by a
    // host that does not carry the command.
    let sent = ctx.send_command(SERVER_COMMAND, &[]);
    log("patch.wake", || format!("server asked to post now, sent={sent}"));
}

// Every client frame: the tooltips, the save's state, and whether a patch
// is due.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    // Whatever plain article is open scrolls as far as its text goes. Ahead
    // of the switch below: the layout this goes with is in use either way.
    text::sync_article_scroll(ctx);
    if !enabled() {
        // No patches, but a tooltip still has to show the numbers of the
        // player's `config.json`, which the text the game holds does not.
        static SHOWN: std::sync::Once = std::sync::Once::new();
        SHOWN.call_once(|| live::rebuild(base::base(), &State::default()));
        text::sync_tooltips(ctx);
        return;
    }
    text::sync_tooltips(ctx);

    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let session = &mut *session;
    if !ctx.save_can_write() {
        // Back at the menu, or between saves: the next save loads its own
        // state, and until it has, matches run unpatched.
        if session.loaded {
            *session = Session::new();
            live::rebuild(base::base(), &State::default());
            take_queue(&session.state);
        }
        return;
    }
    // The save's namespace reads empty on some frames while loading;
    // `item_stats`' word that the save has answered is taken for this state
    // too.
    if !crate::item_stats::loaded() {
        return;
    }
    if !session.loaded {
        session.state = ctx
            .save_get_string(KEY)
            .and_then(|text| read_state(&text))
            .unwrap_or_default();
        session.loaded = true;
        live::rebuild(base::base(), &session.state);
        take_queue(&session.state);
        log("patch.loaded", || {
            format!(
                "patch {} on v{:?}, {} item(s) patched, hotfix queued={}",
                session.state.number,
                session.state.version,
                session.state.ratios.len(),
                session.state.queued
            )
        });
    }
    // The queue button has been clicked since the last frame: the save is
    // told.
    if QUEUE_CHANGED.swap(false, Ordering::Relaxed) {
        session.state.queued = HOTFIX_QUEUED.load(Ordering::Relaxed);
        session.dirty = true;
    }

    session.frame = session.frame.wrapping_add(1);
    // A look every [`CHECK_FRAMES`]th frame, and on every frame for a while
    // after the server has ticked ([`ALERT_FRAMES`]).
    let ticks = CALENDAR_TICKS.load(Ordering::Relaxed);
    if ticks != session.ticks {
        session.ticks = ticks;
        session.alert = ALERT_FRAMES;
    }
    let alert = session.alert > 0;
    session.alert = session.alert.saturating_sub(1);
    if alert || session.frame % CHECK_FRAMES == 0 {
        consider(ctx, session, alert);
    }
    // What the player changed in the Item Patcher Settings window is written
    // once they have left it alone for a moment.
    if session.edited > 0 {
        session.edited -= 1;
        session.dirty |= session.edited == 0;
    }
    if session.dirty {
        // A state the save holds that is further along than this one means
        // this one was read too early. It is taken, not written over.
        let theirs = ctx.save_get_string(KEY).and_then(|text| read_state(&text));
        if let Some(theirs) = theirs.filter(|theirs| theirs.number > session.state.number) {
            session.state = theirs;
            session.dirty = false;
            live::rebuild(base::base(), &session.state);
            take_queue(&session.state);
            return;
        }
        if ctx.save_set_string(KEY, &session.state.to_json()) {
            session.dirty = false;
            // And into the save from the server's side: see [`keep_state`].
            hand_to_server(&session.state);
            ctx.send_command(SERVER_COMMAND, &[]);
        }
    }
}

// The server's answer to [`SERVER_COMMAND`]: the state and the article of a
// patch that has just landed, without waiting for a management tick.
// `false` for any other command.
pub(crate) fn handle_command(ctx: &mut StableServerCtx<'_>, command: &StableCommand<'_>) -> bool {
    if !enabled() || command.command != SERVER_COMMAND {
        return false;
    }
    keep_state(ctx);
    post_articles(ctx);
    true
}

// Every server management tick (only while the calendar moves): counts the tick
// for the client ([`CALENDAR_TICKS`]) and posts whatever state and article
// wait.
pub(crate) fn server_tick(ctx: &mut StableServerCtx<'_>) {
    CALENDAR_TICKS.fetch_add(1, Ordering::Relaxed);
    keep_state(ctx);
    post_articles(ctx);
}

// When the server starts on a save: what its side of the save holds of the
// patches, for the test log to set beside what the client reads
// (`patch.loaded`).
pub(crate) fn server_started(ctx: &mut StableServerCtx<'_>) {
    log("patch.server.loaded", || {
        match ctx.save_get_string(KEY).and_then(|text| State::parse(&text)) {
            Some(state) => format!(
                "the server's side of the save has patch {} on v{:?}, hotfix queued={}",
                state.number, state.version, state.queued
            ),
            None => "the server's side of the save has no patch state".to_string(),
        }
    });
}

// Puts the state the client handed over into the save from the server's side,
// in the same call as the patch's article. A self-made save once had a hotfix's
// notes but the earlier state, and landed the hotfix twice on load.
fn keep_state(ctx: &mut StableServerCtx<'_>) {
    let Some(state) = SERVER_STATE.lock().ok().and_then(|mut held| held.take()) else {
        return;
    };
    let written = ctx.save_set_string(KEY, &state);
    // Numbered, for the test log to show every one.
    static WRITES: AtomicU32 = AtomicU32::new(0);
    let number = WRITES.fetch_add(1, Ordering::Relaxed) + 1;
    log("patch.server.state", || {
        let summary = State::parse(&state).map(|state| (state.number, state.queued));
        format!("write {number}: (patch, hotfix queued)={summary:?}, taken={written}")
    });
}

// Whether `team`'s inbox has an article called `title`.
fn in_inbox(ctx: &StableServerCtx<'_>, team: usize, title: &str) -> bool {
    use serde_json::Value;

    ctx.record_get_json(RecordKindV1::Team, team, "news")
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
        .is_some_and(|news| {
            news.as_array().is_some_and(|articles| {
                articles
                    .iter()
                    .any(|article| article.get("title").and_then(Value::as_str) == Some(title))
            })
        })
}

// Posts the articles the client has written, skipping any the inbox already has
// (a patch made again after a save kept its notes but lost its state).
fn post_articles(ctx: &mut StableServerCtx<'_>) {
    if !ARTICLE_WAITING.swap(false, Ordering::Relaxed) {
        return;
    }
    let articles = ARTICLES
        .lock()
        .map(|mut articles| std::mem::take(&mut *articles))
        .unwrap_or_default();
    for article in articles {
        if in_inbox(ctx, article.team, &article.title) {
            log("patch.news", || {
                format!(
                    "\"{}\" is in team {}'s inbox already: not posted again",
                    article.title, article.team
                )
            });
            continue;
        }
        let posted = ctx.news_push(article.team, &article.title, &article.body, &article.author);
        let filed = if posted {
            file_under_patch(ctx, article.team, &article.title)
        } else {
            String::new()
        };
        log("patch.news", || {
            format!(
                "\"{}\" posted={posted} to team {}; {filed}",
                article.title, article.team
            )
        });
    }
}

// The content bind the game files a plain article by, and the value of it
// that files one under the inbox's Patch tab.
const SCOPE_KEY: &str = "Scope";
const SCOPE_PATCH: &str = "patch";

// Files the just-posted article under the inbox's Patch tab, else leaves it
// under General; returns what happened, for the test log. The inbox files a
// plain article by its `Scope` content bind (`patch` for Patch; 0.6.3 exe, RVA
// 0x16f3090), which `news_push` cannot set, so it is written into the team
// record's `news` list, copying the shape of any bind already there.
//
// Not yet seen in game.
fn file_under_patch(ctx: &mut StableServerCtx<'_>, team: usize, title: &str) -> String {
    use serde_json::Value;

    let Some(news) = ctx
        .record_get_json(RecordKindV1::Team, team, "news")
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
    else {
        return "the team's news could not be read, left under General".to_string();
    };
    let list = news.as_array().map(Vec::as_slice).unwrap_or_default();
    // The newest of that title: a push goes on the end.
    let found = list.iter().enumerate().rev().find(|(_, article)| {
        article.get("title").and_then(Value::as_str) == Some(title)
            && article.pointer("/ty/Simple").is_some()
    });
    let Some((index, article)) = found else {
        let last: String = list
            .last()
            .map(|last| last.to_string().chars().take(300).collect())
            .unwrap_or_default();
        return format!(
            "not found among the team's {} articles, left under General; the last is {last}",
            list.len()
        );
    };
    // A bind as this record writes them, from any the list has.
    let sample = list.iter().find_map(|article| {
        [article.pointer("/ty/Simple/content_bind"), article.get("title_bind")]
            .into_iter()
            .flatten()
            .find_map(|binds| binds.as_array()?.first().cloned())
    });
    let bind = match &sample {
        Some(Value::Object(fields)) => Value::Object(
            fields
                .keys()
                .map(|field| {
                    let part = if field.to_ascii_lowercase().contains("val") {
                        SCOPE_PATCH
                    } else {
                        SCOPE_KEY
                    };
                    (field.clone(), Value::from(part))
                })
                .collect(),
        ),
        _ => serde_json::json!([SCOPE_KEY, SCOPE_PATCH]),
    };
    let binds = match article.pointer("/ty/Simple/content_bind") {
        // Written as a map of name to value.
        Some(Value::Object(binds)) => {
            let mut binds = binds.clone();
            binds.insert(SCOPE_KEY.to_string(), Value::from(SCOPE_PATCH));
            Value::Object(binds)
        }
        Some(Value::Array(binds)) => {
            let mut binds = binds.clone();
            binds.push(bind);
            Value::Array(binds)
        }
        _ => Value::Array(vec![bind]),
    };
    let path = format!("news.{index}.ty.Simple.content_bind");
    let set = ctx.record_set_json(RecordKindV1::Team, team, &path, &binds.to_string());
    let now = ctx.record_get_json(RecordKindV1::Team, team, &path);
    format!("{path} set to {binds}: {set}; it reads back {now:?} (a bind copied from {sample:?})")
}

// Every tick of every simulation, from the match hook.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>) {
    if enabled() {
        live::on_match_tick(sim);
    }
}
