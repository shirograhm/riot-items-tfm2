//! Adapter between the host mod's stable-ABI extensions and the bodies in
//! `super`, which began life as a classic-ABI mod.
//!
//! The classic API handed that mod its `Database`, the UI root node and the
//! scene. Nothing here needs the first two any more: the `Database` scan and
//! the node-tree UI code are gone. The scene's one remaining use, "is a match
//! on screen", is `StableClient::is_in_game()`, threaded in as a `bool`.

use std::sync::atomic::{AtomicBool, Ordering};

/// Records the item recommendation network the host's item-build detour was
/// handed, for the 5th and 6th item (`super::network_pick`).
///
/// The agent is an argument of the detoured function, so nothing is known
/// until the game first asks for an item build, which is why this runs from
/// `hook::detour` rather than from mod init. It is stored exactly as it came;
/// `network_pick` proves it before every use (`super::network_ready`).
pub fn record_item_net(agent: usize) {
    super::NETWORK_AGENT.store(agent as u64, Ordering::Relaxed);
}

/// Kill switch for this half. `false` since 2026-09-16.
///
/// The half existed for the fourth item slot, which game 0.6.0 ships itself,
/// so it was retired on 2026-09-15. It came back for what the stable API
/// cannot do: `own_team_only`, since restricting configured builds to the
/// player's own athletes needs the athlete pointer the native buy detour is
/// handed (`StableItemBuildContext` offers only a 0/1 lineup index), and, from
/// 2026-09-18, the 5th and 6th item slots. What served only the old fourth
/// slot has been removed.
///
/// Live, and re-derived every game update (`tools/verify_rvas.py`): the buy,
/// spawn, launcher and seed-ctor detours, `RVA_REALLOC`, the item network's
/// scorer and the four byte patches in `tactics_init`.
///
/// `src/hooks/hook.rs` is unaffected either way -- it installs from
/// `lib.rs::on_server_start` independently, and is still the only route to
/// training/comp-test builds and the 60-champion roster.
const RETIRED: bool = false;

// ---------------------------------------------------------------------------
// Entry points — called from the host's stable extensions in `src/lib.rs`.
// ---------------------------------------------------------------------------

/// Was `init()` + `declare_mod!`. Ran the version gate and, in 4-slot mode, the
/// byte patches, and recorded whether this half came up at all.
///
/// `ACTIVE` is all that record is now, and only this file's entry points read it
/// (see [`inert`]). The slot count moved to `build_config::picker_slots` when this
/// half was retired, because it is a property of the game rather than of this mod.
pub fn on_mod_init() {
    if RETIRED {
        return;
    }
    ACTIVE.store(super::tactics_init(), Ordering::Relaxed);
}

/// Whether [`on_mod_init`] ran and its version gate passed.
///
/// The runtime entry points below check it too. The gate used to only decide what
/// `tactics_init` did, while `on_server_start` and `post_update` still went on to
/// install the buy, launcher, seed-ctor and game-view detours on any build. Each
/// of those checks its own prologue, but the gate is the promise in
/// `tactics_init`'s docs: on an unrecognised game build, nothing gets patched.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Retired, or the version gate did not pass: the runtime entry points do nothing.
fn inert() -> bool {
    RETIRED || !ACTIVE.load(Ordering::Relaxed)
}

/// Was `ModServerExtension::on_server_start`.
pub fn on_server_start() {
    if inert() {
        return;
    }
    super::tactics_on_server_start();
}

/// Was `ModServerExtension::before_management_tick`.
pub fn before_management_tick() {
    if inert() {
        return;
    }
    super::tactics_before_management_tick();
}

/// Was `ModExtension::post_update`. The client answers what the `Scene::InGame`
/// payload used to.
pub fn post_update(client: &mut mod_api_stable::StableClient<'_>) {
    // Also the per-frame cost: the detour installs below are retried from
    // here, and on an unrecognised game build each can only fail.
    if inert() {
        return;
    }
    let in_game = client.is_in_game();
    super::tactics_post_update(client, in_game);
}

/// Takes every mod's items out of the server's item settings, for a write
/// to those settings that would otherwise rebuild them without their items
/// (`super::lift_server_mod_items`). They go back in when the result is
/// dropped. `keys` is the list as the server reports it.
///
/// `None` on a game build this half does not know, where the offsets it
/// reads the settings through mean nothing, and wherever the settings do not
/// check out against `keys`. No write must be made then.
pub(crate) fn lift_server_mod_items(
    ctx: &mod_api_stable::StableServerCtx<'_>,
    keys: &[String],
) -> Option<super::ModItemsLift> {
    if inert() {
        return None;
    }
    // The context is a pointer to the host's `ServerCtxV1` and a marker, and
    // keeps the pointer to itself. The host state in it is what every call
    // through the context passes back to the host.
    const _: () = assert!(
        core::mem::size_of::<mod_api_stable::StableServerCtx<'static>>()
            == core::mem::size_of::<usize>()
    );
    let raw = unsafe {
        *(ctx as *const mod_api_stable::StableServerCtx<'_>
            as *const *const mod_api_stable::ServerCtxV1)
    };
    if raw.is_null() {
        return None;
    }
    let state = unsafe { (*raw).state } as usize;
    unsafe { super::lift_server_mod_items(state, keys) }
}

/// Hands over the game's item catalog, which the mod-item registry is built
/// from.
///
/// Called from `hook::detour`, which receives the catalog as an argument. Like
/// [`record_item_net`], this is the only route to that data in a stable-ABI mod.
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
