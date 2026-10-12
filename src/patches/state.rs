//! What a save remembers of its item patches: how far every patched number
//! has moved, and the last few patches as they were announced.
//!
//! Kept in the mod's own namespace inside the save file, like the item
//! totals it is worked out from (`item_stats`), so it goes where the save
//! goes: loading an older save shows that save's balance, and a new game
//! starts unpatched.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// The shape this build writes, and the oldest it reads. A state in a shape
// outside the two is left where it is and the save starts over unpatched.
//
// 2: the ratios are kept a tier of an item ([`State::ratios`]). In 1 they
// were kept an item, under its legendary's key, for all of its tiers, and
// such a state is read and spread over the tiers (`read_state`).
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
    // Item -> field -> what the field is of its unpatched value after
    // every patch so far. An item here is one tier: a legendary and its
    // radiant are patched one by one, and need not move together or in the
    // same numbers.
    #[serde(rename = "r")]
    pub ratios: BTreeMap<String, BTreeMap<String, f64>>,
    // Newest last.
    #[serde(rename = "h")]
    pub history: Vec<Patch>,
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

    // Takes a patch in: its ratios, for the tiers each change moved, and
    // its place in the history.
    pub(crate) fn adopt(&mut self, patch: Patch) {
        for change in &patch.changes {
            for moved in &change.members {
                self.ratios
                    .entry(moved.key.clone())
                    .or_default()
                    .insert(change.field.clone(), change.ratio);
            }
        }
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
}
