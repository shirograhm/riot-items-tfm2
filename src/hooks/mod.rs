//! Where the mod plugs into the game's item builds: the native tap on the
//! item-build route (`hook`) and the stable item-build hook
//! (`item_build_hook`). The match hook lives in `vfx::sunfire`.

pub(crate) mod hook;
pub(crate) mod item_build_hook;
