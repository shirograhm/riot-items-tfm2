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
    // Nothing reads `db()` while this half is retired, and settling a
    // `Database` base costs a `VirtualQuery` plus a 64KB `readable` probe
    // on the weight array, on the detour's hot path.
    if RETIRED {
        return;
    }
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

/// Forgets the `Database` base so the next `record_item_net` can settle a new
/// one. Called at the session boundary (`tactics_on_server_start`).
///
/// [`record_item_net`] takes the first address that validates and then refuses
/// to look again — which is right within a session and wrong across one. The
/// `Database` does not survive a return to the main menu, so without this the
/// mod spent every session after the first holding the address of a freed
/// object. It did not crash, because `itemnet_forward` re-checks the weight
/// pointer on every call, but that check only *skips* the neural 4th-item pick
/// — so it silently fell back to the champion-hash vanilla choice for the whole
/// second session, and every session after it.
pub fn reset_session() {
    DB_ADDR.store(0, Ordering::Relaxed);
    DB_PROBED.store(false, Ordering::Relaxed);
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

/// **Partly revived, 2026-09-16, for the team gate only.**
///
/// This half existed for the fourth item slot, and 0.6.0 ships that natively
/// -- the game's own slot-count clamps already read 4 and every build row
/// carries `#item3`. So it was retired on 2026-09-15 and none of its RVAs
/// were re-derived.
///
/// One thing came back with it that is not about slots at all:
/// **`own_team_only`**. Restricting configured builds to the player's own
/// athletes needs `is_my_athlete`, and the stable API cannot express it --
/// `StableItemBuildContext` (re-checked at ABI 9) offers only a 0/1 lineup
/// index that says neither which side is the player's nor whether the player
/// is in the match at all. The native buy detour is the only thing that can,
/// because it is handed the athlete pointer.
///
/// So the three addresses that path needs were re-derived against the
/// release and this is `false` again. **Everything else stays inert**, and
/// through gates that already existed rather than new ones: `slot_count()`
/// is pinned at 3, which is what the four 3 -> 4 byte patches, the build
/// extension, the slot-3 icon and `uinj::MODE4` are all keyed on;
/// `UI_INJECT_ENABLED` and `SPAWN_INJECT_ENABLED` are off in `super`, each
/// with its reason recorded there.
///
/// Live, and therefore re-derived and covered by `tools/verify_rvas.py`:
/// `RVA_BUY_ITEM`, `SEEDCTOR_RVA`, `CL_LAUNCHER_RVA` and the athlete/provider
/// offsets (`O_ATHLETE_ID`, `ATH_STRIDE`, `O_PROVIDER_SEED` -- all three
/// unchanged from beta2, confirmed by a STRICT exe2exe match of the 286-byte
/// roster walk). Not re-derived, and not reachable: LOADER/PARSER/ALLOC,
/// GV_UPDATE, REALLOC, ITEMNET_FORWARD, SPAWN, PV_*.
///
/// `src/hook.rs` is unaffected either way -- it installs from
/// `lib.rs::on_server_start` independently, and is still the only route to
/// training/comp-test builds and the 60-champion roster.
const RETIRED: bool = false;

// ---------------------------------------------------------------------------
// Entry points — called from the host's stable extensions in `src/lib.rs`.
// ---------------------------------------------------------------------------

/// Was `init()` + `declare_mod!`. Ran the version gate and, in 4-slot mode, the
/// byte patches, and recorded whether this half came up at all.
///
/// `ACTIVE` is all that record is now. Nothing outside reads it: the slot count
/// moved to `build_config::picker_slots` when this half was retired, because it
/// is a property of the game rather than of this mod.
pub fn on_mod_init() {
    if RETIRED {
        return;
    }
    ACTIVE.store(super::tactics_init(), Ordering::Relaxed);
}

/// Whether [`on_mod_init`] ran and its version gate passed.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Was `ModServerExtension::on_server_start`.
pub fn on_server_start() {
    if RETIRED {
        return;
    }
    super::tactics_on_server_start();
}

/// Was `ModServerExtension::before_management_tick`.
pub fn before_management_tick() {
    if RETIRED {
        return;
    }
    super::tactics_before_management_tick();
}

/// Was `ModExtension::post_update`. The client answers what the `Scene::InGame`
/// payload used to; the UI root is fetched from `TIP_ROOT` rather than passed
/// in.
pub fn post_update(client: &mut mod_api_stable::StableClient<'_>) {
    // Also the per-frame cost: this retried four detour installs every
    // frame, each of which can only fail on the release image.
    if RETIRED {
        return;
    }
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
    if RETIRED {
        return;
    }
    super::record_item_catalog(catalog);
}

/// Whether a catalog has already been recorded, so `hook::detour` can skip
/// building the argument for [`record_item_catalog`] — which would otherwise be
/// two `String` allocations per item, discarded, on every call after the first.
pub fn item_catalog_recorded() -> bool {
    // While retired, claim the catalog is already in hand. Nothing consumes
    // one, and the honest `false` would make `hook::detour` rebuild the
    // argument on EVERY call — two `String` allocations per item, per call,
    // immediately discarded — instead of only on the first.
    if RETIRED {
        return true;
    }
    super::item_catalog_recorded()
}

/// Whether `probe_db` has completed against an accepted `Database` base.
pub fn db_probed() -> bool {
    DB_PROBED.load(Ordering::Relaxed)
}

pub(super) fn mark_db_probed() {
    DB_PROBED.store(true, Ordering::Relaxed);
}
