//! What a balance patch works with: which items it may touch, which numbers,
//! and their values before any patch.
//!
//! An item is a family (legendary and radiant), each tier planned by itself
//! (`balance`), and every item from the third tier up may be touched
//! ([`PATCHED_FROM_TIER`]), boots and third-tier components included. A
//! component's own record says little, so it is patched for the finished items
//! it builds into ([`Family::built_into`]) when they all go one way.
//!
//! A number is patched only where the player can see it: a flat stat with a
//! tooltip line, or for this mod's items a number in the effect text that
//! `item-templates.json` can rewrite ([`super::text`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use mod_api_stable::*;
use serde_json::Value;

use super::{fields, live, text};
use crate::config::ItemConfig;

// One of this mod's items as it registered, noted from the registration
// macros in `lib.rs`.
pub(crate) struct ModItem {
    pub tier: usize,
    // The items it is built into, and the ones it is built from, as the
    // item itself names them.
    pub next: Vec<String>,
    pub previous: Vec<String>,
    // The stats the game took a copy of, and holds for the item ever after.
    pub registered: BuffV1,
    // The stats the item would have with another config.
    pub stat_of: Box<dyn Fn(&ItemConfig) -> BuffV1 + Send + Sync>,
}

type Refresh = Box<dyn Fn(&ItemConfig) + Send + Sync>;

// What registration has noted so far: the items in the order they
// registered, and whatever keeps a copy of an item's numbers outside the
// item and has to be told when they change.
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

// The lowest tier a patch touches, counted from 0 as an item's own `tier`
// is: the third.
const PATCHED_FROM_TIER: usize = 2;
// The tier an item is finished at, from which builds choose it.
const FINISHED_TIER: usize = 3;

// One tier of an item.
#[derive(Clone)]
pub(crate) struct Member {
    pub key: String,
    // One of the game's own thirty items.
    pub game: bool,
    // Patchable field -> its unpatched number.
    pub values: BTreeMap<String, f64>,
    // The fields among those that are whole numbers.
    pub whole: HashSet<String>,
}

// An item in every tier it comes in: the legendary first, the radiant last.
pub(crate) struct Family {
    pub members: Vec<Member>,
    // Whether builds choose it. Boots, jungle items, the World Atlas line,
    // support-only items (all placed by Smart Builds) and components say
    // nothing by how often they are held, so they are judged on winning alone.
    pub chosen: bool,
    // An item of a tier no build ends on, boots apart: it is patched for
    // what it is built into, and not on a record of its own.
    pub component: bool,
    // For a component, the finished items it is built into that a patch can
    // touch, by family.
    pub built_into: Vec<String>,
}

pub(crate) struct Base {
    pub families: BTreeMap<String, Family>,
    // Every upgrade there is, whatever the tiers: an item's key, and the
    // key of an item it is built into. A legendary and its radiant are one.
    pub upgrades: Vec<(String, String)>,
    // The stats every item gives, as the game holds them: key -> stat ->
    // number. The items no patch moves are here too, for what is built from
    // them to be kept no weaker than they are.
    pub flat: HashMap<String, HashMap<String, f64>>,
    // Config entry -> field -> number: `config.json` over
    // `config-default.json`.
    config: HashMap<String, HashMap<String, f64>>,
    // The numbers `config.json` gives that are not the default ones, as
    // (config entry, field). An effect text that shows one is rewritten for
    // it before any patch: the text the game holds is the default one.
    custom: Vec<(String, String)>,
    // `config.json` as the player wrote it, by item.
    raw: HashMap<String, Value>,
    pub mod_items: HashMap<&'static str, ModItem>,
    pub refreshes: Vec<(&'static str, Refresh)>,
}

// When an item is at its strongest, as a patch note's reason speaks of it:
// a stacking item is an early-game item and a scaling one a late-game item.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tempo {
    // It stacks: some number of it is counted in stacks.
    Early,
    // It scales: a share of its holder's or its target's stats, a range
    // that grows with level, or growth over the match.
    Late,
    Neither,
}

// What in a config field's name says the number scales. A stat's own
// percentage (`attack_speed_mult`) is not among them: `attack_mult` and its
// like multiply a total.
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

// Numbers an effect registered with the game at start-up carries its own
// copy of (`reg.add_native_effect` in `lib.rs`): the projectile would keep
// dealing the unpatched number whatever the item said.
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

pub(crate) fn read_object(name: &str) -> serde_json::Map<String, Value> {
    let path = crate::config::mod_dir().join(name);
    match std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
    {
        Some(Value::Object(root)) => root,
        _ => serde_json::Map::new(),
    }
}

// Everything above, built on first use. Not before the mod has finished
// registering: it takes what registration noted.
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
        // The default numbers, and those with the player's over them.
        let mut shipped: HashMap<String, HashMap<String, f64>> = HashMap::new();
        let mut config: HashMap<String, HashMap<String, f64>> = HashMap::new();
        let mut custom: Vec<(String, String)> = Vec::new();
        for (players, root) in [(false, &defaults), (true, &overrides)] {
            for (item, numbers) in root {
                let Some(numbers) = numbers.as_object() else {
                    continue;
                };
                for (field, value) in numbers {
                    let Some(number) = value.as_f64() else {
                        continue;
                    };
                    if players {
                        let default = shipped.get(item).and_then(|numbers| numbers.get(field));
                        if default != Some(&number) {
                            custom.push((item.clone(), field.clone()));
                        }
                    } else {
                        shipped
                            .entry(item.clone())
                            .or_default()
                            .insert(field.clone(), number);
                    }
                    config
                        .entry(item.clone())
                        .or_default()
                        .insert(field.clone(), number);
                }
            }
        }
        let raw: HashMap<String, Value> = overrides.into_iter().collect();

        let number_in = |numbers: &HashMap<String, HashMap<String, f64>>, item: &str, field: &str| {
            numbers
                .get(item)
                .and_then(|numbers| numbers.get(field))
                .copied()
                .unwrap_or(0.0)
        };
        let value = |item: &str, field: &str| number_in(&config, item, field);
        let default = |item: &str, field: &str| number_in(&shipped, item, field);
        let unverified = texts.unverified(&default, &value);
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
        // What each item is built into, by key: an item names what it grows
        // into, and what it is grown from.
        let mut into: HashMap<String, Vec<String>> = HashMap::new();
        for (key, item) in &items {
            for next in &item.next {
                into.entry(key.to_string()).or_default().push(next.clone());
            }
            for previous in &item.previous {
                into.entry(previous.clone()).or_default().push(key.to_string());
            }
        }
        let mut flat: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for (key, item) in &items {
            for &field in fields::FLAT {
                let held = live::stat_number(&item.registered, field).filter(|number| *number != 0);
                if let Some(number) = held {
                    flat.entry(key.to_string())
                        .or_default()
                        .insert(field.to_string(), number as f64);
                }
            }
        }
        // Asked of the config type once a field, not once an item.
        let mut whole_fields: HashMap<String, bool> = HashMap::new();
        let mut config_fields: HashMap<String, bool> = HashMap::new();

        for (key, item) in &items {
            if item.tier < PATCHED_FROM_TIER {
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
            let chosen = item.tier >= FINISHED_TIER
                && !(crate::smart_builds::is_boots(key)
                    || crate::smart_builds::is_jungle_item(key)
                    || crate::smart_builds::is_atlas_item(key)
                    || crate::smart_builds::is_support_item(key));
            let component = item.tier < FINISHED_TIER && !crate::smart_builds::is_boots(key);
            families
                .entry(id)
                .or_insert_with(|| Family {
                    members: Vec::new(),
                    chosen,
                    component,
                    built_into: Vec::new(),
                })
                .members
                .push(member);
        }

        // The game's own items of the same tiers: each line's last two, drawn
        // as legendary and radiant, and the tier before. A patch is the
        // difference from the settings file, which holds only where the native
        // half can write the server's numbers
        // (`item_stats::sync_server_items`); otherwise these are left out.
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
            // Every one of the game's items, whatever its tier: what it is
            // built into, and the stats it gives.
            for (name, object) in file {
                let Some(object) = object.as_object() else {
                    continue;
                };
                let key = key_of(name, object);
                let next = object.get("next_tier").and_then(Value::as_array);
                into.entry(key.clone()).or_default().extend(
                    next.into_iter()
                        .flatten()
                        .filter_map(|next| next.as_str().map(str::to_string)),
                );
                let stats = object.get("stat").and_then(Value::as_object);
                for (field, number) in stats.into_iter().flatten() {
                    let is_flat = fields::rule(field).is_some_and(|rule| rule.flat);
                    let held = number.as_f64().filter(|number| is_flat && *number != 0.0);
                    if let Some(number) = held {
                        flat.entry(key.clone()).or_default().insert(field.clone(), number);
                    }
                }
            }
            for (name, object) in file {
                let Some(object) = object.as_object() else {
                    continue;
                };
                if tier_of(object) == Some(PATCHED_FROM_TIER as u64) {
                    let member = member_of(key_of(name, object), object);
                    if !member.values.is_empty() {
                        let next = object.get("next_tier").and_then(Value::as_array);
                        into.entry(member.key.clone()).or_default().extend(
                            next.into_iter()
                                .flatten()
                                .filter_map(|next| next.as_str().map(str::to_string)),
                        );
                        families.insert(
                            member.key.clone(),
                            Family {
                                members: vec![member],
                                chosen: false,
                                component: true,
                                built_into: Vec::new(),
                            },
                        );
                    }
                    continue;
                }
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
                            component: false,
                            built_into: Vec::new(),
                        },
                    );
                }
            }
        }

        // What each component is patched for: the families of the finished
        // items it is built into.
        let components: Vec<(String, String)> = families
            .iter()
            .filter(|(_, family)| family.component)
            .filter_map(|(id, family)| Some((id.clone(), family.members.first()?.key.clone())))
            .collect();
        for (id, key) in &components {
            let mut built: Vec<String> = into
                .get(key)
                .into_iter()
                .flatten()
                .filter_map(|target| {
                    [crate::build_config::base_slug(target), target.as_str()]
                        .into_iter()
                        .find(|family| families.get(*family).is_some_and(|family| !family.component))
                        .map(str::to_string)
                })
                .collect();
            built.sort();
            built.dedup();
            if let Some(family) = families.get_mut(id) {
                family.built_into = built;
            }
        }

        super::log("patch.base", || {
            let numbers: usize = families
                .values()
                .flat_map(|family| family.members.iter())
                .map(|member| member.values.len())
                .sum();
            let shared = families
                .values()
                .filter(|family| family.built_into.len() >= 2)
                .count();
            format!(
                "{} item families can be patched, in {numbers} numbers; {} are components, {shared} of them built into two finished items or more; {} mod items registered",
                families.len(),
                components.len(),
                items.len()
            )
        });

        let mut upgrades: Vec<(String, String)> = into
            .iter()
            .flat_map(|(part, built)| built.iter().map(move |upgrade| (part.clone(), upgrade.clone())))
            .collect();
        upgrades.sort();
        upgrades.dedup();

        Self {
            families,
            upgrades,
            flat,
            config,
            custom,
            raw,
            mod_items: items.into_iter().collect(),
            refreshes,
        }
    }

    // The unpatched number of a config entry's field, 0 where it has none,
    // which is what the generator makes of a missing one too.
    pub(crate) fn value(&self, item: &str, field: &str) -> f64 {
        self.config
            .get(item)
            .and_then(|numbers| numbers.get(field))
            .copied()
            .unwrap_or(0.0)
    }

    // [`Tempo`] of `key`, from its config field names: stacking wins over
    // scaling, and the game's own items are neither.
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

    // The config numbers the player changed, as (config entry, field).
    pub(crate) fn custom(&self) -> &[(String, String)] {
        &self.custom
    }

    // The player's own `config.json` entry for `key`, to lay a patch over.
    pub(crate) fn raw_config(&self, key: &str) -> serde_json::Map<String, Value> {
        match self.raw.get(key) {
            Some(Value::Object(entry)) => entry.clone(),
            _ => serde_json::Map::new(),
        }
    }
}
