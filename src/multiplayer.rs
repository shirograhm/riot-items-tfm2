//! Multiplayer-safe mode: what the mod stops doing while the player is in a
//! multiplayer session, so that every player's game plays a match the same
//! way.
//!
//! The game lets a guest into a room only when its mods match the host's, but
//! that is as far as it looks. Three things this mod does come off one
//! player's own disk and change what happens in a match, and two players who
//! differ in any of them would each be watching a different game:
//!
//! - **Pinned builds** (`item-builds.json`): nobody's pins apply.
//! - **My team only** (`mod-settings.json`): off. "My team" is a different
//!   team on every machine, which is the one answer two games cannot share.
//! - **Smart Builds** (`mod-settings.json`): on, its default, whatever the
//!   toggle says.
//!
//! All three are switched where they are read ([`crate::build_config`]), so
//! the stable item-build hook and the native detours follow without knowing.
//!
//! Not covered: item numbers changed in `config.json`. Those are fixed when
//! the mod loads, long before a session is known to be multiplayer, so every
//! player has to bring the same file (or none). Turning the mode on says so
//! in the log when this install has any.
//!
//! # Knowing a session is multiplayer
//!
//! Both kinds, a head-to-head room and a shared league, begin on the room
//! screen (`Scene::Room`): players gather there to pick sides or teams, and a
//! saved multiplayer league is hosted from one. So a session is multiplayer
//! from the first frame the room is up until the title screen is back, which
//! is the only way out of one. Host and guests see the same two screens, so
//! they agree without a message between them.
//!
//! This is read off the 0.6.2 executable's scene and packet names. It has not
//! been watched happen: nothing here was tried with a second player when it
//! was written (2026-10-05).

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api_stable::{SceneKindV1, StableClient};

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the player is in a multiplayer session. One atomic load: the buy
/// detour asks on every buy decision.
pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Follows the session in and out of multiplayer. From the client frame loop,
/// ahead of everything else the mod does that frame.
pub(crate) fn watch(ctx: &StableClient<'_>) {
    match ctx.scene_kind() {
        Some(SceneKindV1::Room) => set(true),
        Some(SceneKindV1::Title) => set(false),
        // Every other screen belongs to whichever session it was reached from.
        _ => {}
    }
}

fn set(on: bool) {
    if ACTIVE.swap(on, Ordering::Relaxed) == on {
        return;
    }
    // The pins have just appeared or gone for everything that remembers them.
    crate::build_config::mode_changed();
    eprintln!(
        "riot_items_tfm2: multiplayer_mode {}",
        if on { "on" } else { "off" }
    );
    if on {
        let overridden = crate::config::load().len();
        if overridden > 0 {
            eprintln!(
                "riot_items_tfm2: multiplayer_mode config.json overrides {overridden} item(s); \
                 every player needs the same config.json, or matches will not agree"
            );
        }
    }
}
