//! What a balance patch has to work with: which items it may touch, which of
//! their numbers, and what those numbers are before any patch.
//!
//! An item is patched as a family, the legendary and its radiant together,
//! and only finished items are: a component is held at the end of a match by
//! whoever did not get to finish it, so its win rate says nothing about it.
//!
//! A number is patched only where the player can see it afterwards. For one
//! of this mod's items that is a flat stat with a line in the tooltip, or a
//! number written into the effect text that `item-templates.json` can write
//! again ([`super::text`]). For one of the game's own thirty it is a flat
//! stat with a line in the tooltip.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use mod_api_stable::*;
use serde_json::Value;

use super::{fields, live, text};
use crate::config::ItemConfig;

/// One of this mod's items as it registered, noted from the registration
/// macros in `lib.rs`.
pub(crate) struct ModItem {
    pub tier: usize,
    /// The stats the game took a copy of, and holds for the item ever after.
    pub registered: BuffV1,
    /// The stats the item would have with another config.
    pub stat_of: Box<dyn Fn(&ItemConfig) -> BuffV1 + Send + Sync>,
}

type Refresh = Box<dyn Fn(&ItemConfig) + Send + Sync>;

/// What registration has noted so far: the items in the order they
/// registered, and whatever keeps a copy of an item's numbers outside the
/// item and has to be told when they change.
struct Noted {
    items: Vec<(&'static str, ModItem)>,
    refreshes: Vec<(&'static str, Refresh)>,
}

static NOTED: Mutex<Noted> = Mutex::new(Noted {
    items: Vec::new(),
    refreshes: Vec::new(),
});

pub(crate) fn note_mod_item(key: &'static str, item: ModItem) {
    if let Ok(mut noted) = NOTED.lock() {
        noted.items.push((key, item));
    }
}

pub(crate) fn note_refresh(key: &'static str, refresh: Refresh) {
    if let Ok(mut noted) = NOTED.lock() {
        noted.refreshes.push((key, refresh));
    }
}

/// One tier of an item.
pub(crate) struct Member {
    pub key: String,
    /// One of the game's own thirty items.
    pub game: bool,
    /// Patchable field -> its unpatched number.
    pub values: BTreeMap<String, f64>,
    /// The fields among those that are whole numbers.
    pub whole: HashSet<String>,
}

/// An item in every tier it comes in: the legendary first, the radiant last.
pub(crate) struct Family {
    pub members: Vec<Member>,
    /// Whether builds choose it. Boots, the jungle items and the World Atlas
    /// line are handed out by Smart Builds' rules, so how often they are held
    /// says nothing about them and is not held against them.
    pub chosen: bool,
}

pub(crate) struct Base {
    pub families: BTreeMap<String, Family>,
    /// Config entry -> field -> number: `config.json` over
    /// `config-default.json`, which is what `apply_config.ps1` writes the
    /// item text from.
    config: HashMap<String, HashMap<String, f64>>,
    /// `config.json` as the player wrote it, by item.
    raw: HashMap<String, Value>,
    pub mod_items: HashMap<&'static str, ModItem>,
    pub refreshes: Vec<(&'static str, Refresh)>,
}

/// When an item is at its strongest, as a patch note's reason speaks of it.
/// The user's rule (2026-10-10): a stacking item is an early-game item and a
/// scaling one a late-game item.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tempo {
    /// It stacks: some number of it is counted in stacks.
    Early,
    /// It scales: a share of its holder's or its target's stats, a range
    /// that grows with level, or growth over the match.
    Late,
    Neither,
}

/// What in a config field's name says the number scales. A stat's own
/// percentage (`attack_speed_mult`) is not among them: `attack_mult` and its
/// like multiply a total.
const SCALING: &[&str] = &[
    "_percent_",
    "effect_min_",
    "growth",
    "attack_mult",
    "magic_power_mult",
    "magic_resistance_mult",
    "defence_mult",
    "hp_mult",
];

/// Numbers an effect registered with the game at start-up carries its own
/// copy of (`reg.add_native_effect` in `lib.rs`): the projectile would keep
/// dealing the unpatched number whatever the item said.
const CAPTURED: &[(&str, &[&str])] = &[
    ("runaans_hurricane", &["effect_ad_percent_damage"]),
    (
        "statikk_shiv",
        &["effect_bonus_magic_damage", "effect_minion_percent"],
    ),
    (
        "sword_of_blossoming_dawn",
        &[
            "effect_min_heal",
            "effect_max_heal",
            "effect_ad_percent_heal",
            "effect_ap_percent_heal",
        ],
    ),
];

fn captured(family: &str, field: &str) -> bool {
    CAPTURED
        .iter()
        .any(|&(item, fields)| item == family && fields.contains(&field))
}

fn read_object(name: &str) -> serde_json::Map<String, Value> {
    let path = crate::config::mod_dir().join(name);
    match std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
    {
        Some(Value::Object(root)) => root,
        _ => serde_json::Map::new(),
    }
}

/// Everything above, built on first use. Not before the mod has finished
/// registering: it takes what registration noted.
pub(crate) fn base() -> &'static Base {
    static BASE: OnceLock<Base> = OnceLock::new();
    BASE.get_or_init(Base::build)
}

impl Base {
    fn build() -> Self {
        let texts = text::texts();
        let (items, refreshes) = NOTED
            .lock()
            .map(|mut noted| {
                (
                    std::mem::take(&mut noted.items),
                    std::mem::take(&mut noted.refreshes),
                )
            })
            .unwrap_or_default();

        let defaults = read_object("config-default.json");
        let overrides = read_object("config.json");
        let mut config: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for (item, numbers) in defaults.iter().chain(overrides.iter()) {
            let Some(numbers) = numbers.as_object() else {
                continue;
            };
            let entry = config.entry(item.clone()).or_default();
            for (field, value) in numbers {
                if let Some(number) = value.as_f64() {
                    entry.insert(field.clone(), number);
                }
            }
        }
        let raw: HashMap<String, Value> = overrides.into_iter().collect();

        let value = |item: &str, field: &str| {
            config
                .get(item)
                .and_then(|numbers| numbers.get(field))
                .copied()
                .unwrap_or(0.0)
        };
        let unverified = texts.unverified(&value);
        if !unverified.is_empty() {
            let mut listed: Vec<&String> = unverified.iter().collect();
            listed.sort();
            super::log("patch.texts", || {
                format!(
                    "{} item text(s) are not what their template makes of the config, so no number shown in them is patched: {listed:?}",
                    listed.len()
                )
            });
        }

        let mut families: BTreeMap<String, Family> = BTreeMap::new();
        // Asked of the config type once a field, not once an item.
        let mut whole_fields: HashMap<String, bool> = HashMap::new();
        let mut config_fields: HashMap<String, bool> = HashMap::new();

        for (key, item) in &items {
            let finished = item.tier >= 3
                || (crate::strategy_ui::is_mod_final_item(key) && crate::smart_builds::is_boots(key));
            if !finished {
                continue;
            }
            let Some(numbers) = config.get(*key) else {
                continue;
            };
            let id = crate::build_config::base_slug(key).to_string();
            let mut member = Member {
                key: key.to_string(),
                game: false,
                values: BTreeMap::new(),
                whole: HashSet::new(),
            };
            for (field, &number) in numbers {
                let Some(rule) = fields::rule(field) else {
                    continue;
                };
                let known = *config_fields
                    .entry(field.clone())
                    .or_insert_with(|| fields::is_config_field(field));
                if number == 0.0 || !known || captured(&id, field) {
                    continue;
                }
                let shown = texts.shown(key, field);
                if shown.is_some_and(|shown| shown.texts.iter().any(|text| unverified.contains(text))) {
                    continue;
                }
                if rule.flat {
                    // It has to be the number the game holds for the item:
                    // the tooltip's stat line is found by it, and the patch
                    // reaches the match as the difference from it.
                    let held = live::stat_number(&item.registered, field);
                    if number.fract() != 0.0 || held != Some(number as i64) {
                        continue;
                    }
                    if shown.is_none() && !texts.has_line(field) {
                        continue;
                    }
                } else if shown.is_none() {
                    continue;
                }
                let whole = rule.flat
                    || shown.is_some_and(|shown| shown.whole)
                    || *whole_fields
                        .entry(field.clone())
                        .or_insert_with(|| fields::is_whole(field));
                member.values.insert(field.clone(), number);
                if whole {
                    member.whole.insert(field.clone());
                }
            }
            if member.values.is_empty() {
                continue;
            }
            let chosen = !(crate::smart_builds::is_boots(key)
                || crate::smart_builds::is_jungle_item(key)
                || crate::smart_builds::is_atlas_item(key));
            families
                .entry(id)
                .or_insert_with(|| Family {
                    members: Vec::new(),
                    chosen,
                })
                .members
                .push(member);
        }

        // The game's own finals: its six lines' last two tiers, which the mod
        // draws as a legendary and its radiant. A patch is the difference
        // from the settings file's numbers, so those have to be the numbers
        // the game runs with, which they are where the native half can write
        // them into the server (`item_stats::sync_server_items`). Where it
        // cannot, these items are left out.
        if crate::tactics::driver::can_lift() {
            let file = crate::item_stats::game_item_file();
            let key_of = |name: &String, object: &serde_json::Map<String, Value>| {
                object
                    .get("key")
                    .and_then(Value::as_str)
                    .filter(|key| !key.is_empty())
                    .unwrap_or(name)
                    .to_string()
            };
            let tier_of = |object: &serde_json::Map<String, Value>| object.get("tier").and_then(Value::as_u64);
            let member_of = |key: String, object: &serde_json::Map<String, Value>| {
                let mut member = Member {
                    key,
                    game: true,
                    values: BTreeMap::new(),
                    whole: HashSet::new(),
                };
                let stats = object.get("stat").and_then(Value::as_object);
                for (field, number) in stats.into_iter().flatten() {
                    let Some(number) = number.as_f64().filter(|number| *number != 0.0) else {
                        continue;
                    };
                    let flat = fields::rule(field).is_some_and(|rule| rule.flat);
                    // A stat the tooltip has a line for, and that the item's
                    // own text does not also spell out (Plating's number is
                    // written into Thornmail's effect text by the generator).
                    if !flat || !texts.has_line(field) || texts.quotes(&member.key, field) {
                        continue;
                    }
                    member.values.insert(field.clone(), number);
                    member.whole.insert(field.clone());
                }
                member
            };
            for (name, object) in file {
                let Some(object) = object.as_object() else {
                    continue;
                };
                if tier_of(object) != Some(4) {
                    continue;
                }
                let radiant = key_of(name, object);
                let legendary = file.iter().find_map(|(name, below)| {
                    let below = below.as_object()?;
                    let leads = below
                        .get("next_tier")
                        .and_then(Value::as_array)
                        .is_some_and(|next| next.iter().any(|next| next.as_str() == Some(radiant.as_str())));
                    (leads && tier_of(below) == Some(3)).then(|| (key_of(name, below), below))
                });
                let mut members = Vec::new();
                let id = match legendary {
                    Some((key, below)) => {
                        members.push(member_of(key.clone(), below));
                        key
                    }
                    None => radiant.clone(),
                };
                members.push(member_of(radiant, object));
                members.retain(|member| !member.values.is_empty());
                if !members.is_empty() {
                    families.insert(
                        id,
                        Family {
                            members,
                            chosen: true,
                        },
                    );
                }
            }
        }

        super::log("patch.base", || {
            let numbers: usize = families
                .values()
                .flat_map(|family| family.members.iter())
                .map(|member| member.values.len())
                .sum();
            format!(
                "{} item families can be patched, in {numbers} numbers; {} mod items registered",
                families.len(),
                items.len()
            )
        });

        Self {
            families,
            config,
            raw,
            mod_items: items.into_iter().collect(),
            refreshes,
        }
    }

    /// The unpatched number of a config entry's field, 0 where it has none,
    /// which is what the generator makes of a missing one too.
    pub(crate) fn value(&self, item: &str, field: &str) -> f64 {
        self.config
            .get(item)
            .and_then(|numbers| numbers.get(field))
            .copied()
            .unwrap_or(0.0)
    }

    /// [`Tempo`] of the item `key`, from the names of the numbers it is
    /// configured with. An item that both stacks and scales is taken for a
    /// stacking one; one of the game's own, which has no config entry of
    /// this kind, for neither.
    pub(crate) fn tempo(&self, key: &str) -> Tempo {
        let Some(numbers) = self.config.get(key) else {
            return Tempo::Neither;
        };
        let any = |named: &dyn Fn(&str) -> bool| {
            numbers
                .iter()
                .any(|(field, number)| *number != 0.0 && named(field.as_str()))
        };
        if any(&|field: &str| field.contains("stack")) {
            Tempo::Early
        } else if any(&|field: &str| SCALING.iter().any(|mark| field.contains(mark))) {
            Tempo::Late
        } else {
            Tempo::Neither
        }
    }

    /// The player's own `config.json` entry for `key`, to lay a patch over.
    pub(crate) fn raw_config(&self, key: &str) -> serde_json::Map<String, Value> {
        match self.raw.get(key) {
            Some(Value::Object(entry)) => entry.clone(),
            _ => serde_json::Map::new(),
        }
    }
}
