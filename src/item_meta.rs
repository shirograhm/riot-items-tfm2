#[derive(Clone, Debug)]

pub struct ItemMeta {
    pub key: &'static str,
    pub tier: usize,
    previous: &'static [&'static str],
    next: &'static [&'static str],
}

impl ItemMeta {
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

    /// True when `key` is the item this one is replaced by on an upgrade.
    ///
    /// `on_upgrade` fires for every step of a build tree, so an item that hands
    /// accumulated stacks to its successor gates the handover on the step that
    /// actually shares the state — base into radiant, never a component into
    /// something with unrelated internals.
    pub fn upgrades_to(&self, key: &str) -> bool {
        self.next.iter().any(|&next| next == key)
    }

    /// True when `key` is an item this one is built from — the gate on the
    /// receiving side of [`ItemMeta::upgrades_to`].
    pub fn upgrades_from(&self, key: &str) -> bool {
        self.previous.iter().any(|&previous| previous == key)
    }
}
