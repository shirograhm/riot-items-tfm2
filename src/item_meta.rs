//! Position of an item in the upgrade tree.
//!
//! Items that have a radiant upgrade are implemented as a *single* type with two
//! constructors rather than two near-identical types: the variants differ only in
//! their stat numbers, their buff names, and this metadata, never in behavior.
//! `ItemMeta` is the metadata half, so `key`/`icon`/`tier`/`previous_tier`/
//! `next_tier` become data the constructor supplies instead of five hand-written
//! trait methods per variant.
//!
//! The shape is uniform across every base/radiant pair in the mod: a tier-3 base
//! keyed `<name>` that builds from some earlier item and upgrades into the tier-4
//! `radiant_<name>`, which tops out the chain.

#[derive(Clone, Debug)]
pub struct ItemMeta {
    /// The registered item key. Also the icon name (they never differ) and the
    /// prefix each variant's buff names are namespaced under.
    pub key: &'static str,
    pub tier: usize,
    previous: &'static [&'static str],
    next: &'static [&'static str],
}

impl ItemMeta {
    /// A tier-3 base item: builds from `previous`, upgrades into `next` (its
    /// `radiant_` variant).
    pub const fn base(
        key: &'static str,
        previous: &'static [&'static str],
        next: &'static [&'static str],
    ) -> Self {
        Self {
            key,
            tier: 3,
            previous,
            next,
        }
    }

    /// A tier-4 radiant item: upgraded from `previous` (its base variant) and the
    /// end of the chain, so it has no next tier.
    pub const fn radiant(key: &'static str, previous: &'static [&'static str]) -> Self {
        Self {
            key,
            tier: 4,
            previous,
            next: &[],
        }
    }

    pub fn previous_tier(&self) -> Vec<String> {
        self.previous.iter().map(|key| key.to_string()).collect()
    }

    pub fn next_tier(&self) -> Vec<String> {
        self.next.iter().map(|key| key.to_string()).collect()
    }
}
