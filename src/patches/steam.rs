//! The player's Steam name, for the byline of a hotfix.
//!
//! The stable API has no word of who is playing: the manager's name is the
//! save's, not the player's. The game runs under Steam and has its library
//! loaded, so the name is asked of that, through the library's flat C
//! interface: the same two calls any Steam game makes to show a name.
//! Nothing is sent anywhere, and nothing but the name is read.

use std::ffi::{c_char, c_void, CStr};

/// The Steam library as the 64-bit game loads it.
const LIBRARY: &[u8] = b"steam_api64.dll\0";
/// The friends interface, by the names the library exports it under. The
/// number is the interface's version and goes up with the Steam SDK the game
/// ships: 17 in the 0.6.3 game's library, the rest in case that moves.
const FRIENDS: [&[u8]; 4] = [
    b"SteamAPI_SteamFriends_v017\0",
    b"SteamAPI_SteamFriends_v018\0",
    b"SteamAPI_SteamFriends_v019\0",
    b"SteamAPI_SteamFriends_v016\0",
];
const PERSONA_NAME: &[u8] = b"SteamAPI_ISteamFriends_GetPersonaName\0";

/// The name the player goes by on Steam. `None` where the game is not
/// running under Steam, or its library does not have these calls.
pub(crate) fn persona_name() -> Option<String> {
    extern "system" {
        fn GetModuleHandleA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    // SAFETY: the library is looked up among the modules the game has
    // already loaded and is never loaded from here; each call is made only
    // through an export that was found, with the signature the Steam SDK
    // gives it; and the name is a NUL-ended string the library owns, copied
    // before anything else is asked of it.
    unsafe {
        let library = GetModuleHandleA(LIBRARY.as_ptr());
        if library.is_null() {
            return None;
        }
        let friends = FRIENDS
            .iter()
            .map(|name| GetProcAddress(library, name.as_ptr()))
            .find(|export| !export.is_null())?;
        let name_of = GetProcAddress(library, PERSONA_NAME.as_ptr());
        if name_of.is_null() {
            return None;
        }
        let friends: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(friends);
        let name_of: unsafe extern "C" fn(*mut c_void) -> *const c_char =
            std::mem::transmute(name_of);
        // Null until the game has started Steam, and where it could not.
        let interface = friends();
        if interface.is_null() {
            return None;
        }
        let name = name_of(interface);
        if name.is_null() {
            return None;
        }
        // A label takes a leading `#` for a text reference.
        let name = CStr::from_ptr(name).to_string_lossy();
        let name = name.trim().trim_start_matches('#').trim();
        (!name.is_empty()).then(|| name.to_string())
    }
}
