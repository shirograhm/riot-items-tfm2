//! Item balance patches: the items are rebalanced whenever the game patches
//! its champions, from how they did on the version that just ended.
//!
//! The base game patches champions on a calendar and never touches an item.
//! This is the same idea for items, built on what the mod already keeps:
//! `item_stats` counts, for every game version, how often each item was held
//! at the end of a match and how often its holder won. When the first match
//! of a new version is counted, the version before it is judged
//! ([`balance`]), the items that won and were held too much are nerfed and
//! the ones that did neither are buffed, and a news article says what moved
//! ([`text::news`]).
//!
//! # Where everything is
//!
//! - [`fields`]: which numbers may move, which way is a buff, in what steps.
//! - [`base`]: which items are patched, and their numbers before any patch.
//! - [`state`]: what the save remembers.
//! - [`balance`]: choosing one patch's changes.
//! - [`live`]: the balance as matches, build hooks and tooltips read it.
//! - [`text`]: the patched tooltips, and the patch notes.
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
//! # Testing it
//!
//! A patch waits for the game's next one, which is weeks of game time. An
//! empty file named [`FORCE_FILE`] beside the DLL makes one land within two
//! seconds, judged on the version being played, and is deleted as it is
//! taken. With `match_builds::LOG` on, `match-builds.log` has a `patch.`
//! line for everything this does.

pub(crate) mod balance;
pub(crate) mod base;
pub(crate) mod fields;
pub(crate) mod live;
pub(crate) mod state;
pub(crate) mod text;

use std::cmp::Ordering as Order;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use mod_api_stable::*;

use crate::config::ItemConfig;
use state::{Patch, State};

/// The whole feature. Off, nothing is read, patched or written, and a save
/// that has a patch state keeps it untouched.
pub(crate) const ENABLED: bool = true;

/// The key the patch state lives under, in the mod's namespace of the save.
const KEY: &str = "item_patches";

/// Frames between two looks at whether a patch is due.
const CHECK_FRAMES: u32 = 30;

/// Matches a version needs to have had counted for its items to be judged.
/// A version with fewer passes without a patch.
const MIN_MATCHES: u32 = 8;

/// See the module docs.
const FORCE_FILE: &str = "item-patch.now";

/// A patch lands on the management screens, and not while a simulation is
/// ticking: the numbers a match reads must not change under it. This is how
/// long none has to have ticked for.
const QUIET_MILLIS: u64 = 400;
/// Looks after which a due patch lands on the management screens however
/// busy the simulations are: the calendar can be left running, and
/// background fixtures with it.
const QUIET_PATIENCE: u32 = 240;

/// What the build hooks add to an item's score for its patches, at the most
/// they lean ([`leaning`]). On the scale of `item_build_hook`'s bonuses,
/// which nothing documents: a mod final gets 0.5 there for being one, so at
/// full lean an item is worth that much again less a fifth, or more.
pub(crate) const LEAN_SCORE: f32 = 0.2;

/// The command the client sends its own server extension to have it post a
/// patch's article at once.
///
/// That used to wait for the next management tick, and the server only ticks
/// while the calendar runs. The first in-game run (2026-10-10) had a patch
/// land on an idle management screen and then nothing, three minutes on with
/// the game open. With the command the article was posted as the patch
/// landed (seen in the test log the same night).
const SERVER_COMMAND: &str = "item_patches";

pub(crate) fn log(key: &str, text: impl FnOnce() -> String) {
    crate::match_builds::log(key, text);
}

// -- registration ---------------------------------------------------------------

/// Notes one of this mod's items as it registers, with the constructor that
/// makes it from a config: what a patch needs to know what the item's stats
/// would be with other numbers.
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

/// Notes something that keeps a copy of `key`'s numbers outside the item:
/// `refresh` is called with the item's config whenever the patches change.
pub(crate) fn note_refresh(
    key: &'static str,
    refresh: impl Fn(&ItemConfig) + Send + Sync + 'static,
) {
    base::note_refresh(key, Box::new(refresh));
}

/// How `key`'s patches lean a build choice, -1 (nerfed) to 1 (buffed).
pub(crate) fn leaning(key: &str) -> f32 {
    live::leaning(key)
}

// -- the save's state -----------------------------------------------------------

struct Session {
    /// The save's state has been read.
    loaded: bool,
    state: State,
    /// The state has changed since it was last written to the save.
    dirty: bool,
    frame: u32,
    /// Looks a due patch has waited for the simulations to go quiet.
    waited: u32,
}

impl Session {
    const fn new() -> Self {
        Self {
            loaded: false,
            state: State {
                format: 0,
                version: None,
                number: 0,
                ratios: std::collections::BTreeMap::new(),
                history: Vec::new(),
            },
            dirty: false,
            frame: 0,
            waited: 0,
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

/// Articles the client has written for the server to post: only the server
/// can.
static ARTICLES: Mutex<Vec<Article>> = Mutex::new(Vec::new());
static ARTICLE_WAITING: AtomicBool = AtomicBool::new(false);

/// A version as its numbers, to tell the newer of two: `1.10` is after
/// `1.9`. Versions that read the same that way fall back to their text.
fn version_order(a: &str, b: &str) -> Order {
    let numbers = |text: &str| -> Vec<u64> {
        text.split(|letter: char| !letter.is_ascii_digit())
            .filter_map(|run| run.parse().ok())
            .collect()
    };
    numbers(a).cmp(&numbers(b)).then_with(|| a.cmp(b))
}

/// Whether [`FORCE_FILE`] is there, taking it away if so.
fn forced() -> bool {
    let path = crate::config::mod_dir().join(FORCE_FILE);
    path.exists() && std::fs::remove_file(&path).is_ok()
}

/// Lands a patch if one is due. See the module docs for when that is.
fn consider(ctx: &mut StableClient<'_>, session: &mut Session) {
    let forced = session.frame % (CHECK_FRAMES * 4) == 0 && forced();
    let Some(newest) = crate::item_stats::patches()
        .into_iter()
        .max_by(|a, b| version_order(a, b))
    else {
        // No match has been counted in this save yet.
        return;
    };
    let judged = match session.state.version.clone() {
        // The first version this save is seen on: nothing came before it to
        // judge.
        None => {
            session.state.version = Some(newest);
            session.dirty = true;
            return;
        }
        Some(settled) if version_order(&newest, &settled) == Order::Greater => settled,
        // Forced: the version being played is judged as far as it has got.
        Some(_) if forced => newest.clone(),
        Some(_) => return,
    };

    if !forced {
        let managing = ctx.client_scene_kind() == Some(ClientSceneKindV1::Main);
        let quiet = live::quiet_millis() >= QUIET_MILLIS;
        if !managing || (!quiet && session.waited < QUIET_PATIENCE) {
            session.waited = session.waited.saturating_add(u32::from(managing));
            return;
        }
    }
    session.waited = 0;

    let window = crate::item_stats::snapshot(Some(judged.as_str()), None);
    let number = session.state.number + 1;
    let patch = if window.matches >= MIN_MATCHES || forced {
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
        balance::decide(
            base::base(),
            &session.state,
            &tallies,
            window.matches,
            number,
            &judged,
        )
    } else {
        Patch {
            number,
            version: judged.clone(),
            matches: window.matches,
            changes: Vec::new(),
        }
    };
    log("patch.landed", || {
        format!(
            "patch {number} for v{judged} ({} matches, forced={forced}): {} change(s){}",
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

    session.state.version = Some(newest);
    session.dirty = true;
    // A version that changed nothing is not a patch: no number, no article.
    if patch.changes.is_empty() {
        return;
    }
    let (title, body, author) = text::news(&patch);
    session.state.adopt(patch);
    live::rebuild(base::base(), &session.state);
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

/// Every client frame: the tooltips, the save's state, and whether a patch
/// is due.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    // Whatever plain article is open scrolls as far as its text goes. Ahead
    // of the switch below: the layout this goes with is in use either way.
    text::sync_article_scroll(ctx);
    if !ENABLED {
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
            .and_then(|text| State::parse(&text))
            .unwrap_or_default();
        session.loaded = true;
        live::rebuild(base::base(), &session.state);
        log("patch.loaded", || {
            format!(
                "patch {} on v{:?}, {} item(s) patched",
                session.state.number,
                session.state.version,
                session.state.ratios.len()
            )
        });
    }

    session.frame = session.frame.wrapping_add(1);
    if session.frame % CHECK_FRAMES == 0 {
        consider(ctx, session);
    }
    if session.dirty {
        // A state the save holds that is further along than this one means
        // this one was read too early. It is taken, not written over.
        let theirs = ctx.save_get_string(KEY).and_then(|text| State::parse(&text));
        if let Some(theirs) = theirs.filter(|theirs| theirs.number > session.state.number) {
            session.state = theirs;
            session.dirty = false;
            live::rebuild(base::base(), &session.state);
            return;
        }
        if ctx.save_set_string(KEY, &session.state.to_json()) {
            session.dirty = false;
        }
    }
}

/// The server's answer to [`SERVER_COMMAND`]: what its management tick does
/// for a patch, without waiting for one. `false` for any other command.
pub(crate) fn handle_command(ctx: &mut StableServerCtx<'_>, command: &StableCommand<'_>) -> bool {
    if !ENABLED || command.command != SERVER_COMMAND {
        return false;
    }
    server_tick(ctx);
    true
}

/// Every server management tick, and at once on [`SERVER_COMMAND`]: posts the
/// articles the client has written. One atomic read while there is none.
pub(crate) fn server_tick(ctx: &mut StableServerCtx<'_>) {
    if !ARTICLE_WAITING.swap(false, Ordering::Relaxed) {
        return;
    }
    let articles = ARTICLES
        .lock()
        .map(|mut articles| std::mem::take(&mut *articles))
        .unwrap_or_default();
    for article in articles {
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

/// The content bind the game files a plain article by, and the value of it
/// that files one under the inbox's Patch tab.
const SCOPE_KEY: &str = "Scope";
const SCOPE_PATCH: &str = "patch";

/// Files the article just posted under the inbox's Patch tab (the user,
/// 2026-10-10: "it should be a patch news article. right now it shows up in
/// the General section"; and, if that cannot be had, "its fine to leave it
/// in general"). Says what happened, for the test log. Whatever goes wrong,
/// the article is left as it was posted, under General.
///
/// An article has no section of its own: the inbox works one out from the
/// article's type (0.6.3 exe, the function at RVA 0x16f36a0, a jump table
/// over the 53 news types). Only the game's champion patch notes are Patch
/// by type, and those hold champion keys, not text. A plain article, which
/// is all `news_push` makes, is filed by its content binds (RVA 0x16f3090):
/// one named `Scope` decides, `patch` for Patch (`transfer` Transfer;
/// `match`, `pre_match` Match; `scout`, `rating`, `season`, `meta` Report;
/// `fan`, `finance`, `team`, `player`, `merch`, `staff` Club), and an
/// article with no such bind is General.
///
/// `news_push` takes no binds, so the bind is written into the article
/// where the server keeps it, the team record's `news` list (`ty` is the
/// type, by serde's name for it, `Simple`, with `content` and
/// `content_bind`). A bind is a pair of strings in the exe; how the record's
/// JSON writes one is not known from there, so it is copied from any bind
/// the list already has, and written as a `[name, value]` pair where there
/// is none to copy.
///
/// Not yet seen in game when written: that the write is taken, and that the
/// client's inbox has the bind without a save and a load in between.
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

/// Every tick of every simulation, from the match hook.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>) {
    if ENABLED {
        live::on_match_tick(sim);
    }
}
