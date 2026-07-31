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
}
