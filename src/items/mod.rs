//! Item module index.
//!
//! Every item lives in its own file under a category directory that mirrors the
//! roles in `item_catalog.rs` (`components` holds the tier-2 parts, which the
//! catalog leaves uncategorised). The `items!` macro declares each module and
//! re-exports its public types, so registering a new item is a one-line
//! addition to that category's `mod.rs` (plus the file itself and its
//! `add_item` call in `lib.rs`).
//!
//! The macro is defined here, before the `mod` declarations below, which puts
//! it in textual scope for every category module — they call `items!` without
//! importing anything.

macro_rules! items {
    ($($module:ident),* $(,)?) => {
        $(
            mod $module;
            pub use $module::*;
        )*
    };
}

items! {
    assassin,
    components,
    fighter,
    mage,
    marksman,
    support,
    tank,
}
