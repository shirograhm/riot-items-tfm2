//! Adapter between the host mod's stable-ABI extensions and the bodies in
//! `super`.

use std::sync::atomic::{AtomicBool, Ordering};

// Records the item network the host's item-build detour was handed, for the 5th
// and 6th item (`super::network_pick`). It is only known once the game first
// asks for a build, and `network_pick` proves it before every use
// (`super::network_ready`).
pub fn record_item_net(agent: usize) {
    super::NETWORK_AGENT.store(agent as u64, Ordering::Relaxed);
}

// Kill switch for this half, which does what the stable API cannot:
// `own_team_only` (it needs the athlete pointer) and the 5th and 6th slots.
// `hook.rs` installs independently either way.
const RETIRED: bool = false;

// ---------------------------------------------------------------------------
// Entry points — called from the host's stable extensions in `src/lib.rs`.
// ---------------------------------------------------------------------------

// Runs the version gate and the byte patches (`tactics_init`), and records
// whether this half came up.
pub fn on_mod_init() {
    if RETIRED {
        return;
    }
    ACTIVE.store(super::tactics_init(), Ordering::Relaxed);
}

// Whether [`on_mod_init`] ran and its version gate passed. The runtime entry
// points check it too, so that on an unrecognised game build nothing gets
// installed or patched.
static ACTIVE: AtomicBool = AtomicBool::new(false);

// Retired, or the version gate did not pass: the runtime entry points do nothing.
fn inert() -> bool {
    RETIRED || !ACTIVE.load(Ordering::Relaxed)
}

// Whether [`lift_server_mod_items`] can work on this game build; where it
// cannot, the item balance patches leave the game's own items alone
// (`patches::base`).
pub(crate) fn can_lift() -> bool {
    !inert()
}

// Was `ModServerExtension::on_server_start`.
pub fn on_server_start() {
    if inert() {
        return;
    }
    super::tactics_on_server_start();
}

// Was `ModServerExtension::before_management_tick`.
pub fn before_management_tick() {
    if inert() {
        return;
    }
    super::tactics_before_management_tick();
}

// Called every client frame.
pub fn post_update(client: &mut mod_api_stable::StableClient<'_>) {
    // Also the per-frame cost: on an unrecognised build every install retried
    // from here can only fail.
    if inert() {
        return;
    }
    let in_game = client.is_in_game();
    super::tactics_post_update(client, in_game);
}

// Takes every mod's items out of the server's item settings until the result is
// dropped, for a write that would otherwise rebuild them without their items
// (`super::lift_server_mod_items`). `None` on an unknown build or when the
// settings do not match `keys`, and then no write must be made.
pub(crate) fn lift_server_mod_items(
    ctx: &mod_api_stable::StableServerCtx<'_>,
    keys: &[String],
) -> Option<super::ModItemsLift> {
    if inert() {
        return None;
    }
    // The context is a pointer to the host's `ServerCtxV1`; the host state in
    // it is what every call through the context passes back.
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

// Hands over the game's item catalog, which the mod-item registry is built
// from, from `hook::detour` (the only route to it in a stable-ABI mod). Every
// call after the first that sticks is ignored.
pub fn record_item_catalog(catalog: Vec<(String, Vec<String>)>) {
    if RETIRED {
        return;
    }
    super::record_item_catalog(catalog);
}

// Whether a catalog is already recorded, so `hook::detour` can skip building
// the argument (two `String`s per item).
pub fn item_catalog_recorded() -> bool {
    // While retired, claim the catalog is in hand, so `hook::detour` does not
    // rebuild the argument on every call.
    if RETIRED {
        return true;
    }
    super::item_catalog_recorded()
}
