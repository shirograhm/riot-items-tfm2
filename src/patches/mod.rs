//! Item balance patches: the items are rebalanced whenever the game patches
//! its champions, from how they did on the version that just ended.
//!
//! The base game patches champions on a calendar and never touches an item.
//! This is the same idea for items, built on what the mod already keeps:
//! `item_stats` counts, for every game version, how often each item was held
//! at the end of a match and how often its holder won. When the game patches
//! its champions, the version that just ended is judged ([`balance`]), the
//! items that won and were held too much are nerfed and the ones that did
//! neither are buffed, and a news article says what moved ([`text::news`]):
//! "Item Patch Notes v…", under the version the game's own notes announce.
//!
//! That the game has patched is read off the player's inbox, where its
//! patch notes arrive ([`watch_news`]), and failing that off the first match
//! counted on a newer version, which is how it was first told and still
//! catches a patch the inbox was not seen to get. The item patch lands the
//! moment that is seen, so that its notes come with the same news as the
//! game's own ([`ALERT_FRAMES`]).
//!
//! # Where everything is
//!
//! - [`fields`]: which numbers may move, which way is a buff, in what steps.
//! - [`base`]: which items are patched, and their numbers before any patch.
//! - [`state`]: what the save remembers.
//! - [`balance`]: choosing one patch's changes.
//! - [`live`]: the balance as matches, build hooks and tooltips read it.
//! - [`text`]: the patched tooltips, and the patch notes.
//! - [`steam`]: the player's Steam name, for a hotfix's byline.
//!
//! # What follows a patch
//!
//! A number is only patched where all of these can follow it. The match:
//! see [`live`]. The tooltips: the game's own are corrected where they are
//! shown, and the mod's read the patched numbers ([`text`]). And the builds:
//! an item's patches lean the choice of it ([`leaning`]), so that a nerf
//! takes an item out of some builds as well as off its win rate. Without
//! that an item would be nerfed for being popular and stay exactly as
//! popular, down to its floor.
//!
//! # Hotfixes
//!
//! A patch waits for the game's next one, which is weeks of game time. The
//! player can have one sooner: the Run Manual Item Patch row this mod adds
//! to the Options dialog (`crate::ui::hotfix_row`) queues one, and it lands
//! the moment the calendar next moves, so that its notes arrive with the
//! news of that very move. The server's management tick is what says the
//! calendar has moved: it runs only while it does ([`server_tick`]). A
//! client with no server of its own, a guest in a shared league, has the
//! hotfix when its inbox is next seen to take news ([`watch_news`]).
//!
//! It is judged on the version being played, and is a hotfix of the game's
//! current version: "Item Patch Notes v… - Hotfix 1", numbered from one for
//! each version, and signed with the player's Steam name where that can be
//! had. The queue is the save's ([`State::queued`]). A patch the game's own
//! brings in the meantime empties it: that is the patch it was waiting for.
//!
//! With `match_builds::LOG` on, `match-builds.log` has a `patch.` line for
//! everything this does.

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

// The whole feature. Off, nothing is read, patched or written, and a save
// that has a patch state keeps it untouched.
pub(crate) const ENABLED: bool = true;

// Whether patches are on: the switch above, and the one in the patch
// settings (`tuning`).
fn enabled() -> bool {
    ENABLED && tuning::tuning().enabled
}

// The key the patch state lives under, in the mod's namespace of the save.
const KEY: &str = "item_patches";

// Frames between two looks at whether a patch is due.
const CHECK_FRAMES: u32 = 30;

// Frames a due patch is looked for on every one, after each management
// tick of the server's. A tick means the calendar is moving, and what a
// patch waits for comes with that move: the game's own patch notes, or a
// queued hotfix's turn. A patch left to the next look in [`CHECK_FRAMES`]
// can find the player on the way into a match, and its notes then come a
// day late.
const ALERT_FRAMES: u32 = 120;

// What the build hooks add to an item's score for its patches, at the most
// they lean ([`leaning`]). On the scale of `item_build_hook`'s bonuses,
// which nothing documents: a mod final gets 0.5 there for being one, so at
// full lean an item is worth that much again less a fifth, or more.
pub(crate) const LEAN_SCORE: f32 = 0.2;

// The command the client sends its own server extension to have it post a
// patch's article at once.
//
// Without it the article waits for the next management tick, and the server
// only ticks while the calendar runs: a patch that lands on an idle
// management screen would post nothing until the player moves on.
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
            },
            dirty: false,
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

// Whether a hotfix is queued in the save that is loaded. In an atomic of
// its own, beside [`State::queued`], for the Options row's click handler
// and its painting, which have no business holding the session.
static HOTFIX_QUEUED: AtomicBool = AtomicBool::new(false);
// A click has changed the queue and the save has not been told yet.
static QUEUE_CHANGED: AtomicBool = AtomicBool::new(false);
// Management ticks the server has run ([`server_tick`]): it has one only
// while the calendar moves. Counted for the client, which looks for a due
// patch on every frame for a while after each, and what the count was when
// the hotfix was queued: the hotfix is due at the next.
static CALENDAR_TICKS: AtomicU32 = AtomicU32::new(0);
static QUEUED_AT_TICK: AtomicU32 = AtomicU32::new(0);
// Times the inbox has been seen to take new articles ([`watch_news`]), and
// what that count was when the hotfix was queued: what sets a hotfix off
// where no server of this client's ticks.
static NEWS_CYCLES: AtomicU32 = AtomicU32::new(0);
static QUEUED_AT_CYCLE: AtomicU32 = AtomicU32::new(0);

// Whether a hotfix can be queued at all: patches are on.
pub(crate) fn hotfixes_offered() -> bool {
    enabled()
}

pub(crate) fn hotfix_queued() -> bool {
    HOTFIX_QUEUED.load(Ordering::Relaxed)
}

// Queues a hotfix for the calendar's next move. From the Options row's
// click handler; [`sync`] puts it in the save. One that is queued already
// stays as it is: it is not taken back, and there is nothing to queue twice.
pub(crate) fn queue_hotfix() {
    if HOTFIX_QUEUED.load(Ordering::Relaxed) {
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

// The queue as `state` has it, for a save just read or left. A queued
// hotfix waits for the calendar to move from here on: what came before is
// not counted.
fn take_queue(state: &State) {
    QUEUED_AT_TICK.store(CALENDAR_TICKS.load(Ordering::Relaxed), Ordering::Relaxed);
    HOTFIX_QUEUED.store(state.queued, Ordering::Relaxed);
    QUEUED_AT_CYCLE.store(NEWS_CYCLES.load(Ordering::Relaxed), Ordering::Relaxed);
    QUEUE_CHANGED.store(false, Ordering::Relaxed);
}

// The state `text` holds, in the shape this build keeps.
//
// A state from before the tiers of an item were patched one by one has an
// item's ratios once, under its legendary, for all of its tiers. They are
// written out for each tier here, which leaves every number where it was.
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

// A version as its numbers, to tell the newer of two: `1.10` is after
// `1.9`. Two that have the same numbers are the same version however they
// are written (`1.2` and `1.2.0`, or with a letter in front): a version is
// read from two places now, the matches and the patch notes, and a patch
// must not be set off by the two spelling one version differently. Only
// versions with no number in them are told apart by their text.
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

// What the player's inbox has said of the game's own patches.
struct Watch {
    // The team whose news was read.
    team: Option<usize>,
    // How many articles it had then.
    seen: usize,
    // The last of them, by its date and title.
    tail: Option<String>,
    // The newest version a champion patch note among them announces.
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

// Keeps [`Watch::newest`]: the version of the newest champion patch notes
// in the player's inbox. Every two seconds on the management screens, and
// on every frame for a while after the calendar has moved.
//
// The game says nowhere what version it is on (no event for a patch, no
// field for the version: only the matches played carry one). Its patch
// notes do, and they are an article in the player's team's news the day
// the patch lands: type `PatchNote`, with a `version`, in the team record's
// `news` list.
//
// A look is one small read, of the article after the last one seen, which
// is not there while nothing has come. When something has, the list is read
// whole: where in it a new article goes is not known, and a patch note is
// looked for by what it is and not by where. It is read whole every
// [`WATCH_REREAD`]th look regardless, in case the list is ever shortened
// from the front as it grows.
//
// Each reading that finds news come in since the one before is counted
// ([`NEWS_CYCLES`]): a queued hotfix goes out with the next news.
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
    let notes: Vec<&Value> = articles
        .iter()
        .filter_map(|article| article.pointer("/ty/PatchNote"))
        .collect();
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
            "team {team}: {} champion patch note(s), newest v{newest:?}{}",
            notes.len(),
            odd.map(|odd| format!("; one reads {odd}")).unwrap_or_default()
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

// The screens a patch lands on: the management ones, and the lineup that
// comes before a match, which is where a move of the calendar leaves the
// player on a match day. Not from the stadium entrance on: the numbers the
// player's match reads must not change under it.
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
    // The version the game is on: the newest its patch notes announce, or
    // the newest a counted match was played on, whichever is further along.
    let played = crate::item_stats::patches()
        .into_iter()
        .max_by(|a, b| version_order(a, b));
    let Some(current) = [played.clone(), session.watch.newest.clone()]
        .into_iter()
        .flatten()
        .max_by(|a, b| version_order(a, b))
    else {
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
            (settled, current, false)
        }
        // Queued by the player, and the calendar has moved: a hotfix of the
        // version the game is on, judged on the version being played as far
        // as it has got.
        Some(settled) if hotfix_due => {
            (played.unwrap_or_else(|| settled.clone()), settled, true)
        }
        Some(_) => return,
    };

    // A patch lands at once, the game's own or a queued hotfix, and does
    // not wait for the simulations to go quiet. Its notes go out with the
    // news of the calendar's move, and that move is the other fixtures
    // being played: by the time they are quiet the player is in their own
    // match, and the notes would be a day late.
    if !can_land(scene) {
        return;
    }
    // Whichever patch this is, it is the one a queued hotfix was waiting
    // for.
    session.state.queued = false;
    take_queue(&session.state);

    let window = crate::item_stats::snapshot(Some(judged.as_str()), None);
    let number = session.state.number + 1;
    let tallies: HashMap<String, balance::Tally> = window
        .rows
        .iter()
        .map(|(key, totals)| {
            (
                key.clone(),
                balance::Tally {
                    games: totals.games,
                    wins: totals.wins,
                },
            )
        })
        .collect();
    // Whether there is enough to judge an item on is asked item by item
    // (`tuning`'s `min_games`), of a hotfix as of any other. A version
    // of few matches has few such items, or none, and then no patch.
    let mut patch = balance::decide(
        base::base(),
        &session.state,
        &tallies,
        window.matches,
        number,
        &judged,
    );
    log("patch.landed", || {
        format!(
            "patch {number} as v{announced} (hotfix={hotfix}), judged on v{judged} ({} matches): {} change(s){}",
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
    // The save's namespace reads empty on some frames while it is still
    // loading. `item_stats` waits that out for its own table, and its word
    // that the save has answered is taken for this state too: both are read
    // from the same place, on the same frame.
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
    // The Options row has been clicked since the last frame: the save is
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

// Every server management tick, which the server only has while the
// calendar moves: the tick is counted for the client ([`CALENDAR_TICKS`]),
// and whatever state and article are waiting are seen to. An atomic add, a
// lock and an atomic read while there is neither.
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

// Puts the state the client handed over into the save from the server's
// side, ahead of the article of the patch that made it.
//
// The client writes it too ([`sync`]), and that is the copy a save the
// player makes has. A save the game made by itself was seen to have a
// hotfix's notes in its inbox and the state from before the hotfix: loaded,
// the hotfix landed a second time. Written here, the state goes into the
// save in the same call as the notes.
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

// Posts the articles the client has written. One atomic read while there
// is none.
//
// Not one the inbox has already: a patch's notes are named for its version
// and its number among that version's hotfixes, and a patch lands a second
// time when a save is loaded that kept its notes and lost its state. The
// patch is then made again, from the same matches, and says nothing new.
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

// Files the article just posted under the inbox's Patch tab. Says what
// happened, for the test log. Whatever goes wrong, the article is left as it
// was posted, under General.
//
// An article has no section of its own: the inbox works one out from the
// article's type (0.6.3 exe, the function at RVA 0x16f36a0, a jump table
// over the 53 news types). Only the game's champion patch notes are Patch
// by type, and those hold champion keys, not text. A plain article, which
// is all `news_push` makes, is filed by its content binds (RVA 0x16f3090):
// one named `Scope` decides, `patch` for Patch (`transfer` Transfer;
// `match`, `pre_match` Match; `scout`, `rating`, `season`, `meta` Report;
// `fan`, `finance`, `team`, `player`, `merch`, `staff` Club), and an
// article with no such bind is General.
//
// `news_push` takes no binds, so the bind is written into the article
// where the server keeps it, the team record's `news` list (`ty` is the
// type, by serde's name for it, `Simple`, with `content` and
// `content_bind`). A bind is a pair of strings in the exe; how the record's
// JSON writes one is not known from there, so it is copied from any bind
// the list already has, and written as a `[name, value]` pair where there
// is none to copy.
//
// Not yet seen in game when written: that the write is taken, and that the
// client's inbox has the bind without a save and a load in between.
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
