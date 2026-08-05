//! Adapter between the host mod's stable-ABI extensions and the classic-ABI
//! bodies in `super`.
//!
//! The classic API used to *hand* `tfm2_item_tactics` three things this mod
//! cannot ask for over the stable boundary:
//!
//! | classic | why the stable ABI can't provide it | substitute here |
//! |---|---|---|
//! | `ServerModContext::database` | the boundary is JSON/path based; no object pointers cross it | [`db`] — derived from the item network address, which `src/hook.rs` receives as `&LogisticSGDAgent` |
//! | `&mut GameUI` (`ui.root`) | `StableClient` exposes UI by *path*, not as a node tree | [`ui_root`] — the root node pointer `super::TIP_ROOT`, captured from the UI mega-function detour |
//! | `Scene::InGame { .. }` | no scene payload crosses the boundary | `StableClient::is_in_game()`, threaded in as a `bool` |
//!
//! Everything else in `super` is raw pointers and kernel32 and needed no change.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// `Node` is `mod_api`'s (the live UI tree); `Database` is `game_core`'s, the
// same crate `src/hook.rs` already links — `mod_api` names the type in
// `ServerModContext::database` but does not re-export it.
use game_core::Database;
use mod_api::Node;

/// Offset of the item recommendation network inside the `Database` object
/// (`GameData`). Established by `tfm2_item_tactics` on 0.5.1 and unchanged
/// since — see `probe_db`, which prefers this offset and keeps a window scan as
/// a fallback. Used here in reverse: network address minus this is the
/// `Database` base.
const ITEM_NET_DB_OFFSET: usize = 0x1558;

static DB_ADDR: AtomicUsize = AtomicUsize::new(0);
/// Set once `probe_db` has run against a `DB_ADDR` it accepted.
static DB_PROBED: AtomicBool = AtomicBool::new(false);

/// Records the item recommendation network the host's item-build detour was
/// handed, and derives the `Database` base from it.
///
/// This is the *only* route to the `Database` in a stable-ABI mod, which is why
/// it runs from `hook::detour` rather than from mod init: the agent is an
/// argument of the detoured function, so nothing is known until the game first
/// asks for an item build.
///
/// Both the network header (`16384 / 16384 / 1`) and its weight pointer are
/// validated before anything is stored, for the same reason `probe_db` does:
/// a lookalike passing only the header has a dangling weight pointer, and
/// dereferencing it inside `forward` is an access violation no `catch_unwind`
/// can catch.
pub fn record_item_net(agent: usize) {
    if DB_ADDR.load(Ordering::Relaxed) != 0 || agent < ITEM_NET_DB_OFFSET {
        return;
    }
    if !unsafe { super::itemnet_header_ok(agent) } {
        // Not the network `probe_db` is looking for. Leave `DB_ADDR` unset so a
        // later call can still settle it — guessing a base would send the
        // mod-item scan walking 0x60000 bytes of unrelated memory.
        return;
    }
    super::ITEM_NET_ADDR.store(agent as u64, Ordering::Relaxed);
    DB_ADDR.store(agent - ITEM_NET_DB_OFFSET, Ordering::Relaxed);
}

/// The `Database` base, or 0 until the host's item-build detour has fired once.
pub fn db_addr() -> usize {
    DB_ADDR.load(Ordering::Relaxed)
}

/// The game's `Database`, once [`record_item_net`] has settled its address.
///
/// # Safety
/// The `Database` outlives every caller here (it is owned by the running game),
/// and the layout is `repr(Rust)` fixed by the pinned compiler — the same
/// contract `src/hook.rs` already relies on.
pub unsafe fn db() -> Option<&'static Database> {
    let addr = DB_ADDR.load(Ordering::Relaxed);
    (addr != 0).then(|| &*(addr as *const Database))
}

/// The live UI root node, or `None` before the UI mega-function detour has run.
///
/// # Safety
/// Valid only while the game is inside a UI update — which is exactly when the
/// host's `post_update` runs, and the only place this is called.
pub unsafe fn ui_root() -> Option<&'static mut Node> {
    let addr = super::TIP_ROOT.load(Ordering::Relaxed);
    (addr > 0x10000).then(|| &mut *(addr as *mut Node))
}

// ---------------------------------------------------------------------------
// Entry points — called from the host's stable extensions in `src/lib.rs`.
// ---------------------------------------------------------------------------

/// Was `init()` + `declare_mod!`. Runs the version gate and, in 4-slot mode, the
/// byte patches. Returns whether the tactics half is active at all.
pub fn on_mod_init() -> bool {
    super::tactics_init()
}

/// Was `ModServerExtension::on_server_start`.
pub fn on_server_start() {
    super::tactics_on_server_start();
}

/// Was `ModServerExtension::before_management_tick`.
pub fn before_management_tick() {
    super::tactics_before_management_tick();
}

/// Was `ModExtension::post_update`. The client answers what the `Scene::InGame`
/// payload used to; the UI root is fetched from `TIP_ROOT` rather than passed
/// in.
pub fn post_update(client: &mod_api_stable::StableClient<'_>) {
    let in_game = client.is_in_game();
    super::tactics_post_update(client, in_game);
}

/// Hands over the game's item catalog so the mod-item registry can be built
/// from it rather than by scanning the `Database`.
///
/// Called from `hook::detour`, which receives the catalog as an argument. Like
/// [`record_item_net`], this is the only route to that data in a stable-ABI mod;
/// unlike it, the data arrives typed and needs no base address, so it is the
/// more trustworthy of the two.
///
/// Idempotent — every call after the first that sticks is ignored.
pub fn record_item_catalog(catalog: Vec<(String, Vec<String>)>) {
    super::record_item_catalog(catalog);
}

/// Whether this half is applying `item-builds.json` per athlete, behind
/// `is_my_athlete`.
///
/// `hook::detour` asks before applying those builds itself: exactly one of the
/// two must, or a champion gets its pinned item twice over.
pub fn injects_builds() -> bool {
    super::injects_builds()
}

/// Item slots this half is configured for: 4, or 3 when `4items.cfg` says so.
/// Only meaningful once [`on_mod_init`] has returned `true`.
pub fn slot_count() -> usize {
    super::slot_count()
}

/// Whether `probe_db` has completed against an accepted `Database` base.
pub fn db_probed() -> bool {
    DB_PROBED.load(Ordering::Relaxed)
}

pub(super) fn mark_db_probed() {
    DB_PROBED.store(true, Ordering::Relaxed);
}
