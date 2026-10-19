//! What a save remembers of its item patches: how far every patched number has
//! moved, and the last few patches as announced. Kept in the save's mod
//! namespace beside the item totals, so an older save shows its own balance and
//! a new game starts unpatched.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// The shape this build writes, and the oldest it reads; any other is left alone
// and the save starts unpatched. Format 1 kept an item's ratios once under its
// legendary, and is read and spread over the tiers (`read_state`).
pub(crate) const FORMAT: u32 = 2;
const OLDEST_FORMAT: u32 = 1;

// Patches kept for the notes. The numbers themselves need none of them:
// [`State::ratios`] is the whole balance.
const HISTORY_KEPT: usize = 12;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub(crate) struct State {
    #[serde(rename = "v")]
    pub format: u32,
    // The game's balance version the items were last settled for. A match
    // recorded on a newer one is what sets off the next patch.
    #[serde(rename = "ver")]
    pub version: Option<String>,
    // Item patches so far.
    #[serde(rename = "n")]
    pub number: u32,
    // Hotfixes released on `version` so far: the patches the player set
    // off by hand. Counted from one again when the game's version moves on.
    #[serde(rename = "hf")]
    pub hotfixes: u32,
    // A hotfix the player has queued, which goes out with the next news.
    #[serde(rename = "q")]
    pub queued: bool,
    // Item -> field -> its share of the unpatched value after every patch so
    // far. An item is one tier: a legendary and its radiant are patched
    // separately.
    #[serde(rename = "r")]
    pub ratios: BTreeMap<String, BTreeMap<String, f64>>,
    // Newest last.
    #[serde(rename = "h")]
    pub history: Vec<Patch>,
    // Item -> field -> numbers the player set by hand, which read exactly as
    // set rather than on a patch's steps (`fields::quantize`) until a patch
    // moves them.
    #[serde(rename = "x", skip_serializing_if = "BTreeMap::is_empty")]
    pub exact: BTreeMap<String, BTreeMap<String, f64>>,
    // Item -> field -> a number the player has set in the Item Patcher
    // Settings window and that no patch has released yet: the next manual
    // patch is made of these.
    #[serde(rename = "s", skip_serializing_if = "BTreeMap::is_empty")]
    pub staged: BTreeMap<String, BTreeMap<String, f64>>,
    // Item -> field -> what the player allows a patch to do with the number.
    #[serde(rename = "l", skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, BTreeMap<String, Limit>>,
}

// What the player allows the automatic patches to do with one number of one
// item, set in the Item Patcher Settings window. A number the player sets by
// hand is the player's word and is held to none of it.
#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq)]
#[serde(default)]
pub(crate) struct Limit {
    // The least and the most a patch may make the number. One that is not
    // given is the band every number has (`tuning`).
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(rename = "z", skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    // No automatic patch moves the number.
    #[serde(rename = "p", skip_serializing_if = "std::ops::Not::not")]
    pub protected: bool,
}

impl Limit {
    // Whether it says anything: one that does not is not kept.
    pub(crate) fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none() && !self.protected
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub(crate) struct Patch {
    #[serde(rename = "n")]
    pub number: u32,
    // The game version whose matches were judged.
    #[serde(rename = "ver")]
    pub version: String,
    // Matches counted on that version.
    #[serde(rename = "m")]
    pub matches: u32,
    // The game version it was announced as: the one the game was on when
    // it landed, which for the patch a new version brings is the version
    // after the one judged. Empty in a patch from before this was kept.
    #[serde(rename = "as")]
    pub announced: String,
    // Which hotfix of that version it is, from one; zero for the patch a
    // new version brings.
    #[serde(rename = "hf")]
    pub hotfix: u32,
    #[serde(rename = "c")]
    pub changes: Vec<Change>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub(crate) struct Change {
    #[serde(rename = "k")]
    pub family: String,
    #[serde(rename = "f")]
    pub field: String,
    #[serde(rename = "b")]
    pub buff: bool,
    // [`State::ratios`] for the field, of the tiers moved, once this
    // change is in.
    #[serde(rename = "r")]
    pub ratio: f64,
    // What each tier the change moved read before and reads after: one
    // tier, since a patch plans every tier by itself.
    #[serde(rename = "t")]
    pub members: Vec<Moved>,
    // What the item was judged on: times it was held, and how many of
    // those won.
    #[serde(rename = "g")]
    pub games: u32,
    #[serde(rename = "w")]
    pub wins: u32,
    // Made to keep an upgrade no weaker than what it is built from, and
    // not for how the item did: an item no verdict was reached on.
    #[serde(rename = "u")]
    pub follows: bool,
    // Set by the player in the Item Patcher Settings window, and not made
    // by a patch: no verdict was reached on the item, and its number reads
    // exactly as set.
    #[serde(rename = "m")]
    pub manual: bool,
    // The other tier this change brings level (a radiant raised with its
    // legendary, or a legendary lowered with its radiant), whose notes are in
    // that tier's entry. Empty otherwise.
    #[serde(rename = "l")]
    pub leader: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub(crate) struct Moved {
    #[serde(rename = "k")]
    pub key: String,
    #[serde(rename = "a")]
    pub before: f64,
    #[serde(rename = "z")]
    pub after: f64,
}

impl State {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let state: Self = serde_json::from_str(text).ok()?;
        (OLDEST_FORMAT..=FORMAT)
            .contains(&state.format)
            .then_some(state)
    }

    pub(crate) fn to_json(&self) -> String {
        let mut state = self.clone();
        state.format = FORMAT;
        serde_json::to_string(&state).unwrap_or_default()
    }

    // Takes a patch in: its ratios for the tiers each change moved, and its
    // place in the history. Hand-set numbers it moved go back to the patch's
    // steps.
    pub(crate) fn adopt(&mut self, patch: Patch) {
        for change in &patch.changes {
            for moved in &change.members {
                self.ratios
                    .entry(moved.key.clone())
                    .or_default()
                    .insert(change.field.clone(), change.ratio);
                if change.manual {
                    self.exact
                        .entry(moved.key.clone())
                        .or_default()
                        .insert(change.field.clone(), moved.after);
                } else if let Some(exact) = self.exact.get_mut(&moved.key) {
                    exact.remove(&change.field);
                }
            }
        }
        self.exact.retain(|_, fields| !fields.is_empty());
        self.number = patch.number;
        self.history.push(patch);
        if self.history.len() > HISTORY_KEPT {
            let extra = self.history.len() - HISTORY_KEPT;
            self.history.drain(..extra);
        }
    }

    // What `field` of the item `key`, one tier, is of its unpatched value.
    pub(crate) fn ratio(&self, key: &str, field: &str) -> f64 {
        self.ratios
            .get(key)
            .and_then(|fields| fields.get(field))
            .copied()
            .unwrap_or(1.0)
    }

    // What the player allows a patch to do with `field` of the item `key`.
    pub(crate) fn limit(&self, key: &str, field: &str) -> Limit {
        self.limits
            .get(key)
            .and_then(|fields| fields.get(field))
            .copied()
            .unwrap_or_default()
    }

    // Whether the player has put `field` of the item `key` out of the
    // automatic patches' reach.
    pub(crate) fn protected(&self, key: &str, field: &str) -> bool {
        self.limit(key, field).protected
    }

    // Whether a patch may take `field` of `key` from `before` to `after` within
    // the player's least and most, or back towards them from outside; `None`
    // where neither is given.
    pub(crate) fn within(&self, key: &str, field: &str, before: f64, after: f64) -> Option<bool> {
        let limit = self.limit(key, field);
        if limit.min.is_none() && limit.max.is_none() {
            return None;
        }
        let low = limit.min.is_none_or(|min| after >= min - 1e-9 || after > before);
        let high = limit.max.is_none_or(|max| after <= max + 1e-9 || after < before);
        Some(low && high)
    }

    // Keeps what the player allows a patch to do with `field` of the item
    // `key`, or forgets it where `limit` says nothing.
    pub(crate) fn set_limit(&mut self, key: &str, field: &str, limit: Limit) {
        if limit.is_empty() {
            if let Some(fields) = self.limits.get_mut(key) {
                fields.remove(field);
            }
            self.limits.retain(|_, fields| !fields.is_empty());
        } else {
            self.limits
                .entry(key.to_string())
                .or_default()
                .insert(field.to_string(), limit);
        }
    }

    // Keeps `value` as the number the next manual patch sets `field` of the
    // item `key` to, or with none takes the number out of that patch.
    pub(crate) fn stage(&mut self, key: &str, field: &str, value: Option<f64>) {
        match value {
            Some(value) => {
                self.staged
                    .entry(key.to_string())
                    .or_default()
                    .insert(field.to_string(), value);
            }
            None => {
                if let Some(fields) = self.staged.get_mut(key) {
                    fields.remove(field);
                }
                self.staged.retain(|_, fields| !fields.is_empty());
            }
        }
    }

    // How many numbers wait for the next manual patch.
    pub(crate) fn staged_count(&self) -> usize {
        self.staged.values().map(BTreeMap::len).sum()
    }
}
