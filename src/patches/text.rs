//! Item text under a balance patch: what an item's tooltip has to say once
//! its numbers have moved, and putting that on screen.
//!
//! # Why the mod writes into the game's tooltips
//!
//! The game draws an item's tooltip from two things. The stat lines come from
//! the numbers it holds for the item, and for one of this mod's items those
//! are a copy taken when the mod loaded (0.5.2: `ModItemEntry::stat` copies
//! them out of the entry and never asks the item). The effect text is looked
//! up by the item's key (`ModItemEntry::option_desc`), in `text/item.i18n`,
//! which has the default numbers already in the sentences.
//! Neither follows a patch, and the stable API can change neither, so the
//! text a tooltip is showing is corrected where it is shown: the label is
//! read, the unpatched stat lines and effect text are found in it, and it is
//! written back with the patched ones in their place ([`sync_tooltips`]).
//! Anything not found is left as it is, so a tooltip in a shape this does
//! not know shows the unpatched text rather than a broken one.
//!
//! # Where the patched sentences come from
//!
//! `item-templates.json`, compiled into the DLL: every item's effect text in
//! every language, with a slot naming the config field where each number
//! goes. It is the source of those texts: `text/item.i18n` is written from it
//! (`tools/item_i18n.py`), so filled with the numbers of
//! `config-default.json` it gives that file back, letter for letter. The
//! same check is run here at start-up against the text file as the player
//! has it ([`Texts::unverified`]), and an item that fails it is not patched
//! in any number its text shows.
//!
//! The text the game holds is that default one, whatever the player's
//! `config.json` says. So a tooltip is rewritten for a number the player
//! changed just as it is for one a patch moved (`Base::custom`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use mod_api_stable::*;
use serde_json::Value;

use super::state::{Change, Patch, State};

// One config number in a slot, as the generator reads it.
pub(crate) struct Term {
    pub item: String,
    pub field: String,
    // Read as `[int]`: the sentence shows a whole number.
    pub whole: bool,
}

enum Part {
    Text(String),
    // One number, or the product of several (Haunting Guise's total is its
    // share a stack times its stacks).
    Slot(Vec<Term>),
}

// An item's effect text with its numbers still to be written in.
pub(crate) struct Template {
    parts: Vec<Part>,
}

impl Template {
    fn parse(text: &str, open: &str, close: &str) -> Option<Self> {
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find(open) {
            if start > 0 {
                parts.push(Part::Text(rest[..start].to_string()));
            }
            let inside = &rest[start + open.len()..];
            let end = inside.find(close)?;
            let mut terms = Vec::new();
            for term in inside[..end].split('*') {
                let (path, cast) = term.split_once('|')?;
                let (item, field) = path.split_once('.')?;
                terms.push(Term {
                    item: item.to_string(),
                    field: field.to_string(),
                    whole: cast == "int",
                });
            }
            parts.push(Part::Slot(terms));
            rest = &inside[end + close.len()..];
        }
        if !rest.is_empty() {
            parts.push(Part::Text(rest.to_string()));
        }
        Some(Self { parts })
    }

    fn terms(&self) -> impl Iterator<Item = &Term> {
        self.parts
            .iter()
            .filter_map(|part| match part {
                Part::Slot(terms) => Some(terms),
                Part::Text(_) => None,
            })
            .flatten()
    }

    // The text with every number in, `value` giving each by config item and
    // field. Written as the generator writes them: see [`shown`].
    pub(crate) fn fill(&self, value: &dyn Fn(&str, &str) -> f64) -> String {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Slot(terms) => out.push_str(&shown(terms, value)),
            }
        }
        out
    }

    // The name of the passive `item`'s `field` is a number of, and whether
    // the sentence puts a percent sign after the number. A passive's text
    // opens with its name and a colon, in every language. The name is empty
    // for a number in a sentence that has none: the lethality an item gives
    // is a sentence of its own, "Gain 18 Lethality."
    fn passive_of(&self, item: &str, field: &str) -> Option<(String, bool)> {
        let mut text = String::new();
        let mut found = None;
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                Part::Text(plain) => text.push_str(plain),
                Part::Slot(terms) => {
                    let wanted = terms.len() == 1 && terms[0].item == item && terms[0].field == field;
                    if wanted && found.is_none() {
                        let percent = matches!(
                            self.parts.get(index + 1),
                            Some(Part::Text(next)) if next.starts_with('%')
                        );
                        found = Some((text.len(), percent));
                    }
                    text.push('0');
                }
            }
        }
        let (at, percent) = found?;
        // The paragraph the number is in, and no further back: a sentence
        // with no name is not part of the passive above it.
        let start = text[..at].rfind("\n\n").map_or(0, |gap| gap + 2);
        let before = plain(&text[start..at]);
        let name = before
            .split_once([':', '\u{ff1a}'])
            .map(|(name, _)| name.trim())
            .filter(|name| name.chars().count() <= 40)
            .unwrap_or_default();
        Some((name.to_string(), percent))
    }

    // How the text draws `item`'s `field`: the colour of the span the
    // number is in, and the icon the text has for what it is a number of.
    // That icon is the one of the number's colour, in a span of that colour
    // or bare and followed by one ("of your <icon> <#60e84dff>maximum
    // health<>"): the nearest after the number in its paragraph, or failing
    // one the nearest before it. Neither for a number in no colour span.
    fn tint_of(&self, item: &str, field: &str) -> Tint {
        // The paragraph's icons: where each comes among its numbers and
        // icons, the icon, and its colour.
        let mut icons: Vec<(usize, &str, Option<String>)> = Vec::new();
        // Where the number comes, and its colour.
        let mut slot: Option<(usize, Option<String>)> = None;
        let mut order = 0usize;
        let mut colours: Vec<String> = Vec::new();
        // The last icon is in no colour span, and nothing but blanks has
        // come since: a span that opens now is its colour.
        let mut bare = false;
        'parts: for part in &self.parts {
            let text = match part {
                Part::Slot(terms) => {
                    let wanted = terms.len() == 1 && terms[0].item == item && terms[0].field == field;
                    if wanted && slot.is_none() {
                        slot = Some((order, colours.last().cloned()));
                    }
                    order += 1;
                    bare = false;
                    continue;
                }
                Part::Text(text) => text,
            };
            let mut rest = text.as_str();
            while let Some(at) = rest.find(['<', '\n']) {
                if !rest[..at].trim().is_empty() {
                    bare = false;
                }
                let tail = &rest[at..];
                if tail.starts_with("\n\n") {
                    if slot.is_some() {
                        break 'parts;
                    }
                    icons.clear();
                    colours.clear();
                    bare = false;
                    rest = &tail[2..];
                    continue;
                }
                if tail.starts_with('\n') {
                    rest = &tail[1..];
                    continue;
                }
                let Some(end) = tail.find('>') else {
                    break;
                };
                let tag = &tail[1..end];
                if tag.is_empty() {
                    colours.pop();
                    bare = false;
                } else if let Some(icon) = tag.strip_prefix("i#") {
                    icons.push((order, icon, colours.last().cloned()));
                    order += 1;
                    bare = true;
                } else if let Some(colour) = tag.strip_prefix('#') {
                    let colour = colour.to_lowercase();
                    if bare {
                        if let Some(last) = icons.last_mut().filter(|icon| icon.2.is_none()) {
                            last.2 = Some(colour.clone());
                        }
                    }
                    colours.push(colour);
                    bare = false;
                }
                rest = &tail[end + 1..];
            }
            if !rest.trim().is_empty() {
                bare = false;
            }
        }
        let Some((at, Some(colour))) = slot else {
            return Tint::default();
        };
        let same = |icon: &&(usize, &str, Option<String>)| icon.2.as_deref() == Some(colour.as_str());
        let icon = icons
            .iter()
            .filter(same)
            .find(|icon| icon.0 > at)
            .or_else(|| icons.iter().rev().filter(same).find(|icon| icon.0 < at))
            .map(|icon| icon.1.to_string());
        Tint {
            colour: Some(colour),
            icon,
        }
    }

    // How the text writes the range of `item` that runs from `low` to
    // `high`: what stands between the two numbers (" - "), and the bracket
    // that follows the second where it holds words and no number
    // (" (based on level)"), with the colours and the icon the text has in
    // it. Nothing where the two are not written one after the other.
    fn range_of(&self, item: &str, low: &str, high: &str) -> Option<(String, String)> {
        let is = |part: &Part, field: &str| {
            matches!(
                part,
                Part::Slot(terms)
                    if terms.len() == 1 && terms[0].item == item && terms[0].field == field
            )
        };
        let at = self.parts.iter().position(|part| is(part, low))?;
        let Some(Part::Text(between)) = self.parts.get(at + 1) else {
            return None;
        };
        if !self.parts.get(at + 2).is_some_and(|part| is(part, high)) {
            return None;
        }
        let between = plain(between);
        if between.contains('\n') || between.trim().chars().count() > 3 {
            return None;
        }
        let tail = match self.parts.get(at + 3) {
            Some(Part::Text(after)) => {
                // The span the second number is in closes first.
                let after = after.strip_prefix("<>").unwrap_or(after);
                let words = after.trim_start();
                let bracket = words
                    .starts_with(['(', '\u{ff08}'])
                    .then(|| words.find([')', '\u{ff09}']))
                    .flatten()
                    .map(|end| {
                        let close = words[end..].chars().next().map_or(1, char::len_utf8);
                        let bracket = &words[..end + close];
                        // With its markup where every colour span in it
                        // is whole, and without any of it otherwise.
                        if bracket.matches("<#").count() == bracket.matches("<>").count() {
                            bracket.to_string()
                        } else {
                            plain(bracket)
                        }
                    });
                match bracket {
                    Some(bracket) if after.starts_with(char::is_whitespace) => format!(" {bracket}"),
                    Some(bracket) => bracket,
                    None => String::new(),
                }
            }
            _ => String::new(),
        };
        Some((between, tail))
    }
}

// How an item's text draws one of its numbers ([`Template::tint_of`]),
// which is how a patch note draws a passive's number that has a row format.
#[derive(Clone, Default)]
struct Tint {
    // The colour, as its tag has it less the `#`.
    colour: Option<String>,
    // The icon, as `sheet:tag`.
    icon: Option<String>,
}

impl Tint {
    // `phrase` in the colour, behind the icon where `icon` asks for it:
    // "<icon> <colour>10 health<>".
    fn draw(&self, phrase: &str, icon: bool) -> String {
        let mark = match &self.icon {
            Some(mark) if icon => format!("<i#{mark}> "),
            _ => String::new(),
        };
        match &self.colour {
            Some(colour) => format!("{mark}<#{colour}>{phrase}<>"),
            None => format!("{mark}{phrase}"),
        }
    }
}

// Numbers a patch note draws with no icon, whatever the text has by them,
// as (base item, field): the flat part of an amount whose share of ability
// power has the icon, "25 (+10% <icon> AP)".
const NO_ICON: [(&str, &str); 2] = [
    ("terminus", "effect_bonus_flat_damage"),
    ("zazzaks_realmspike", "effect_bonus_flat_damage"),
];

// `text` without its markup: colour spans and inline icons are `<...>`.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for letter in text.chars() {
        match letter {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(letter),
            _ => {}
        }
    }
    out
}

// A slot's number as it is written into a sentence, in `text/item.i18n` as
// here (`tools/item_i18n.py` writes that file the same way): an `int` slot
// rounds a half to the even number, and a `double` is written to fifteen
// significant digits.
fn shown(terms: &[Term], value: &dyn Fn(&str, &str) -> f64) -> String {
    let read = |term: &Term| {
        let number = value(&term.item, &term.field);
        if term.whole {
            number.round_ties_even()
        } else {
            number
        }
    };
    let product: f64 = terms.iter().map(read).product();
    if terms.iter().all(|term| term.whole) {
        format!("{}", product as i64)
    } else {
        number_text(product)
    }
}

// A number to fifteen significant digits, with no trailing zeros.
pub(crate) fn number_text(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return "0".to_string();
    }
    let magnitude = value.abs().log10().floor() as i32;
    let decimals = (14 - magnitude).clamp(0, 17) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

// Where one config number is written.
#[derive(Default)]
pub(crate) struct Shown {
    // The items whose effect text has it, by text key. Usually the item
    // itself; a few sentences quote another item's number.
    pub texts: Vec<String>,
    // Some sentence writes it as a whole number, so it has to stay one.
    pub whole: bool,
}

// Everything read from the mod's text files, once.
pub(crate) struct Texts {
    // Language -> item key -> effect text, as the game was given it.
    options: HashMap<String, HashMap<String, String>>,
    // Language -> item key -> name.
    names: HashMap<String, HashMap<String, String>>,
    // Language -> stat field -> its tooltip line, `{Value}` in it. The
    // mod's languages, then the base game's own for the rest.
    spec: Vec<(String, HashMap<String, String>)>,
    // Language -> item key -> template.
    templates: HashMap<String, HashMap<String, Template>>,
    // An item's name in any language -> its key.
    name_keys: HashMap<String, String>,
    shown: HashMap<(String, String), Shown>,
    // Language -> the mod's own patch-note strings (`text/ui.i18n`,
    // `item_patch`).
    ui: HashMap<String, HashMap<String, String>>,
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

// Compiled in rather than read from the mod folder, so the templates are
// always the ones this DLL was built with.
const TEMPLATES: &str = include_str!("../../item-templates.json");

pub(crate) fn texts() -> &'static Texts {
    static TEXTS: OnceLock<Texts> = OnceLock::new();
    TEXTS.get_or_init(Texts::load)
}

impl Texts {
    fn load() -> Self {
        let dir = crate::config::mod_dir();
        let mut texts = Self {
            options: HashMap::new(),
            names: HashMap::new(),
            spec: Vec::new(),
            templates: HashMap::new(),
            name_keys: HashMap::new(),
            shown: HashMap::new(),
            ui: HashMap::new(),
        };

        if let Some(Value::Object(langs)) = read_json(&dir.join("text").join("item.i18n")) {
            for (lang, entries) in &langs {
                let Some(entries) = entries.as_object() else {
                    continue;
                };
                let mut names = HashMap::new();
                let mut options = HashMap::new();
                for (key, entry) in entries {
                    if key == "spec" {
                        texts.spec.push((lang.clone(), lines_of(entry)));
                        continue;
                    }
                    let Some(name) = entry.get("name").and_then(Value::as_str) else {
                        continue;
                    };
                    texts
                        .name_keys
                        .entry(name.to_string())
                        .or_insert_with(|| key.clone());
                    names.insert(key.clone(), name.to_string());
                    if let Some(option) = entry.get("option").and_then(Value::as_str) {
                        options.insert(key.clone(), option.to_string());
                    }
                }
                texts.names.insert(lang.clone(), names);
                texts.options.insert(lang.clone(), options);
            }
        }

        if let Ok(file) = serde_json::from_str::<Value>(TEMPLATES) {
            let open = file.get("open").and_then(Value::as_str).unwrap_or("[[~");
            let close = file.get("close").and_then(Value::as_str).unwrap_or("~]]");
            if let Some(Value::Object(langs)) = file.get("t") {
                for (lang, entries) in langs {
                    let Some(entries) = entries.as_object() else {
                        continue;
                    };
                    let mut templates = HashMap::new();
                    for (key, text) in entries {
                        let Some(template) = text
                            .as_str()
                            .and_then(|text| Template::parse(text, open, close))
                        else {
                            continue;
                        };
                        for term in template.terms() {
                            let place = texts
                                .shown
                                .entry((term.item.clone(), term.field.clone()))
                                .or_default();
                            if !place.texts.contains(key) {
                                place.texts.push(key.clone());
                            }
                            place.whole |= term.whole;
                        }
                        templates.insert(key.clone(), template);
                    }
                    texts.templates.insert(lang.clone(), templates);
                }
            }
            if let Some(Value::Object(langs)) = file.get("spec") {
                for (lang, lines) in langs {
                    if !texts.spec.iter().any(|(known, _)| known == lang) {
                        texts.spec.push((lang.clone(), lines_of(lines)));
                    }
                }
            }
        }

        if let Some(Value::Object(langs)) = read_json(&dir.join("text").join("ui.i18n")) {
            for (lang, entries) in &langs {
                if let Some(strings) = entries.get("item_patch") {
                    texts.ui.insert(lang.clone(), lines_of(strings));
                }
            }
        }
        texts
    }

    // Where `item`'s `field` is written, if any effect text has it.
    pub(crate) fn shown(&self, item: &str, field: &str) -> Option<&Shown> {
        self.shown.get(&(item.to_string(), field.to_string()))
    }

    // Whether the effect text of `key` spells out a number of `field`,
    // whichever config entry it takes it from.
    pub(crate) fn quotes(&self, key: &str, field: &str) -> bool {
        self.templates.values().any(|templates| {
            templates
                .get(key)
                .is_some_and(|template| template.terms().any(|term| term.field == field))
        })
    }

    // Whether a tooltip has a stat line for `field`.
    pub(crate) fn has_line(&self, field: &str) -> bool {
        self.spec
            .iter()
            .any(|(_, lines)| lines.get(field).is_some_and(|line| line.contains("{Value}")))
    }

    // The items whose effect text, in some language, is not what its
    // template makes of the default numbers (`shipped`): a text file from
    // another version than the templates. What a patch would write into such
    // a tooltip cannot be trusted to be the sentence that is in it.
    //
    // A text made of the player's own numbers (`value`) passes too: a folder
    // whose text file was written from the player's config shows the right
    // numbers already.
    pub(crate) fn unverified(
        &self,
        shipped: &dyn Fn(&str, &str) -> f64,
        value: &dyn Fn(&str, &str) -> f64,
    ) -> HashSet<String> {
        let mut out = HashSet::new();
        for (lang, templates) in &self.templates {
            let options = self.options.get(lang);
            for (key, template) in templates {
                let have = options
                    .and_then(|options| options.get(key))
                    .map(String::as_str);
                let made_of = |numbers: &dyn Fn(&str, &str) -> f64| {
                    have == Some(template.fill(numbers).as_str())
                };
                if !made_of(shipped) && !made_of(value) {
                    out.insert(key.clone());
                }
            }
        }
        out
    }

    // The effect text of `key` in every language it has a template in: the
    // one the game holds, and the one `value`'s numbers make.
    pub(crate) fn options_of(
        &self,
        key: &str,
        value: &dyn Fn(&str, &str) -> f64,
    ) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for (lang, templates) in &self.templates {
            let Some(template) = templates.get(key) else {
                continue;
            };
            let Some(base) = self.options.get(lang).and_then(|options| options.get(key)) else {
                continue;
            };
            let patched = template.fill(value);
            if &patched != base {
                out.push((lang.clone(), base.clone(), patched));
            }
        }
        out
    }

    // The key of the item a tooltip's name label is showing: the name in
    // whatever language, or the reference a label was handed to resolve.
    pub(crate) fn key_of_name(&self, label: &str) -> Option<&str> {
        if let Some(key) = label
            .strip_prefix("#asset/base/text/item?")
            .and_then(|path| path.strip_suffix(".name"))
        {
            return self.name_keys.values().find(|known| known.as_str() == key).map(String::as_str);
        }
        self.name_keys.get(label.trim()).map(String::as_str)
    }

    // An item's name in `lang`, in English where that language has none.
    fn name(&self, lang: &str, key: &str) -> String {
        [lang, "en"]
            .iter()
            .find_map(|lang| self.names.get(*lang).and_then(|names| names.get(key)))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    fn ui(&self, lang: &str, key: &str, fallback: &str) -> String {
        [lang, "en"]
            .iter()
            .find_map(|lang| self.ui.get(*lang).and_then(|strings| strings.get(key)))
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    // The stat line of `field` in `lang` (English failing that), as a label
    // for a patch note: its words, and whether it reads as a percentage.
    fn line_label(&self, lang: &str, field: &str) -> Option<(String, bool)> {
        let line = [lang, "en"].iter().find_map(|lang| {
            self.spec
                .iter()
                .find(|(known, _)| known == lang)
                .and_then(|(_, lines)| lines.get(field))
                .filter(|line| line.contains("{Value}"))
        })?;
        let percent = line.contains("{Value}%");
        let words = plain(line).replace("{Value}%", "").replace("{Value}", "");
        let words = words.trim().trim_start_matches('+').trim();
        (!words.is_empty()).then(|| (words.to_string(), percent))
    }
}

impl Texts {
    // The icon a stat's tooltip line opens with, as (sheet, tag): the line
    // is `<i#sheet:tag> {Value} words`.
    fn line_icon(&self, field: &str) -> Option<(String, String)> {
        let line = self
            .spec
            .iter()
            .find_map(|(_, lines)| lines.get(field).filter(|line| line.contains("<i#")))?;
        let inside = line.split_once("<i#")?.1.split_once('>')?.0;
        let (sheet, tag) = inside.rsplit_once(':')?;
        Some((sheet.to_string(), tag.to_string()))
    }

    // How `key`'s effect text writes the range from `low` to `high`
    // ([`Template::range_of`]), in `lang` or failing that in English: what
    // stands between the two numbers, and what follows them. A dash and
    // nothing where the text has no such range.
    fn range_words(&self, lang: &str, key: &str, low: &str, high: &str) -> (String, String) {
        [lang, "en"]
            .iter()
            .find_map(|lang| self.templates.get(*lang)?.get(key))
            .and_then(|template| template.range_of(key, low, high))
            .unwrap_or_else(|| (" - ".to_string(), String::new()))
    }

    // How `key`'s effect text draws its `field` ([`Template::tint_of`]). Read
    // off the English text whatever the language: the colours and icons are
    // the same in every one, and a translation moves the words around them.
    // Less the icon for a number of [`NO_ICON`].
    fn tint(&self, key: &str, field: &str) -> Tint {
        let mut tint = self
            .templates
            .get("en")
            .and_then(|templates| templates.get(key))
            .map_or_else(Tint::default, |template| template.tint_of(key, field));
        let slug = crate::build_config::base_slug(key);
        if NO_ICON.iter().any(|&(item, other)| item == slug && other == field) {
            tint.icon = None;
        }
        tint
    }
}

fn lines_of(value: &Value) -> HashMap<String, String> {
    value
        .as_object()
        .map(|lines| {
            lines
                .iter()
                .filter_map(|(key, line)| Some((key.clone(), line.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

// The game's language, from its own settings file (`config/game/base.json`
// beside the executable, `lang`). The stable API does not say, and answers
// every text lookup in English. Read again every few seconds: the player
// can change it in the options.
pub(crate) fn game_language() -> String {
    static LANG: Mutex<Option<(Instant, String)>> = Mutex::new(None);
    let Ok(mut cached) = LANG.lock() else {
        return "en".to_string();
    };
    if let Some((read, lang)) = cached.as_ref() {
        if read.elapsed().as_secs() < 5 {
            return lang.clone();
        }
    }
    let lang = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("config").join("game").join("base.json")))
        .and_then(|path| read_json(&path))
        .and_then(|file| file.get("lang").and_then(Value::as_str).map(str::to_string))
        .filter(|lang| !lang.is_empty())
        .unwrap_or_else(|| "en".to_string());
    *cached = Some((Instant::now(), lang.clone()));
    lang
}

// -- what a patched item shows -------------------------------------------------

// What a tooltip of one item has to say that the game will not: built by
// [`super::live`] whenever the patch state changes.
#[derive(Default)]
pub(crate) struct Display {
    // (language, the effect text the game holds, the patched one).
    pub options: Vec<(String, String, String)>,
    // (stat field, the number the game's stat line shows, the patched one).
    // Empty for the game's own items, whose stat lines follow their data.
    pub flats: Vec<(String, i64, i64)>,
}

impl Display {
    // `desc` with this item's patched numbers in, or nothing where it holds
    // none of the text they would replace.
    fn rewritten(&self, texts: &Texts, desc: &str) -> Option<String> {
        let mut text = desc.to_string();
        let mut changed = false;
        for (_, base, patched) in &self.options {
            if !base.is_empty() && text.contains(base.as_str()) {
                text = text.replacen(base.as_str(), patched, 1);
                changed = true;
                break;
            }
        }
        for (field, before, after) in &self.flats {
            for (_, lines) in &texts.spec {
                let Some(line) = lines.get(field).filter(|line| line.contains("{Value}")) else {
                    continue;
                };
                let old = line.replace("{Value}", &before.to_string());
                if text.contains(&old) {
                    text = text.replacen(&old, &line.replace("{Value}", &after.to_string()), 1);
                    changed = true;
                    break;
                }
            }
        }
        changed.then_some(text)
    }
}

// The patched effect text of `key` in the game's language, for a tooltip
// the mod draws itself. Nothing where the patch leaves that text alone.
pub(crate) fn option_now(key: &str) -> Option<String> {
    let live = super::live::current()?;
    let display = live.display.get(key)?;
    let lang = game_language();
    [lang.as_str(), "en"].iter().find_map(|lang| {
        display
            .options
            .iter()
            .find(|(known, _, _)| known == lang)
            .map(|(_, _, patched)| patched.clone())
    })
}

// -- the game's own tooltips ---------------------------------------------------

// One place the game shows an item's description.
struct Site {
    // The node game code shows and hides.
    root: String,
    name: String,
    desc: String,
    // What was last written to `desc`: while the label still says it,
    // there is nothing to do.
    written: Option<String>,
}

struct Tooltips {
    sites: Vec<Site>,
    frame: u32,
}

static TOOLTIPS: Mutex<Tooltips> = Mutex::new(Tooltips {
    sites: Vec::new(),
    frame: 0,
});

// Frames between two looks for tooltips not known yet.
const DISCOVER_FRAMES: u32 = 45;

// The hover tooltip of an item slot, which three screens have under their
// own root with the same nodes inside: in a match, on the match result
// (whose layout's root is `main` too), and on the solo rank page.
const TOOLTIP: &str = "item_tooltip";
// The description panel of the Item Info page (`item_info.ui`), under Game
// Info and in the strategy screen's Item Info popup. Inside the page's
// `#data`: the page is `main.top.right.item_info`, and the panel is not
// straight under it.
const DETAIL: &str = "data.item_detail";

fn site_paths(root: String, inner: &str) -> (String, String, String) {
    let name = format!("{root}.{inner}name");
    let desc = format!("{root}.{inner}desc");
    (root, name, desc)
}

// Finds the description labels now on screen. The two roots a layout puts
// its tooltip under are known; where a screen is mounted inside the
// management UI is game code's doing, so those are looked for one level
// down from the places screens are put.
fn discover(ctx: &StableClient<'_>, sites: &mut Vec<Site>) {
    let mut wanted = vec![
        site_paths(format!("ingame.{TOOLTIP}"), "data."),
        site_paths(format!("main.{TOOLTIP}"), "data."),
    ];
    for parent in ["main", "main.top.right", "main.contents"] {
        for child in ctx.ui_child_names(parent) {
            let base = format!("{parent}.{child}");
            wanted.push(site_paths(format!("{base}.{TOOLTIP}"), "data."));
            wanted.push(site_paths(format!("{base}.{DETAIL}"), ""));
            if child == "item_info_popup" {
                let content = format!("{base}.popup.content");
                for inner in ctx.ui_child_names(&content) {
                    wanted.push(site_paths(format!("{content}.{inner}.{DETAIL}"), ""));
                }
            }
        }
    }
    // Where the management screens are mounted, for the test log: a line
    // whenever the screen changes. It is what says where to look when a
    // description is on screen and none of the paths above found it.
    super::log("patch.screen", || {
        format!(
            "tab {:?}: main.top.right holds {:?}, main.contents holds {:?}",
            ctx.client_main_tab(),
            ctx.ui_child_names("main.top.right"),
            ctx.ui_child_names("main.contents")
        )
    });
    sites.retain(|site| ctx.ui_exists(&site.desc));
    for (root, name, desc) in wanted {
        if sites.iter().any(|site| site.desc == desc) || !ctx.ui_exists(&desc) {
            continue;
        }
        super::log("patch.site", || format!("found {desc}"));
        sites.push(Site {
            root,
            name,
            desc,
            written: None,
        });
    }
}

// Puts the patched numbers into whatever item description the game is
// showing. Every client frame; one atomic read while no patch shows
// anything.
//
// The label is written again whenever it stops saying what was written,
// which covers both ways game code may fill it: once when the hover
// starts, or every frame.
pub(crate) fn sync_tooltips(ctx: &mut StableClient<'_>) {
    let Some(live) = super::live::current().filter(|live| !live.display.is_empty()) else {
        return;
    };
    let Ok(mut tooltips) = TOOLTIPS.lock() else {
        return;
    };
    let tooltips = &mut *tooltips;
    if tooltips.frame % DISCOVER_FRAMES == 0 {
        discover(ctx, &mut tooltips.sites);
    }
    tooltips.frame = tooltips.frame.wrapping_add(1);

    let texts = texts();
    for site in &mut tooltips.sites {
        if ctx.ui_visible(&site.root) != Some(true) {
            site.written = None;
            continue;
        }
        let Some(desc) = ctx.ui_text(&site.desc) else {
            continue;
        };
        if site.written.as_deref() == Some(desc.as_str()) {
            continue;
        }
        let name = ctx.ui_text(&site.name);
        let Some(key) = name
            .as_deref()
            .and_then(|name| texts.key_of_name(name).map(str::to_string))
        else {
            // A description is up and its name is no item of the mod's text
            // file: another mod's item, or a label that does not hold what
            // this takes it to. Said in the test log, once a name.
            super::log("patch.name", || {
                format!("{} names no item: {name:?}", site.name)
            });
            continue;
        };
        let Some(display) = live.display.get(&key) else {
            continue;
        };
        match display.rewritten(texts, &desc) {
            Some(text) => {
                ctx.ui_set_text(&site.desc, &text);
                super::log(&format!("patch.tip.{key}"), || {
                    format!("rewritten at {}", site.desc)
                });
                site.written = Some(text);
            }
            // The label holds neither the unpatched text nor what was
            // written: a shape of tooltip this does not know. Said once an
            // item, in the test log.
            None => super::log(&format!("patch.tip.{key}"), || {
                let shown: String = desc.chars().take(240).collect();
                format!("NOT rewritten at {}: the label reads {shown:?}", site.desc)
            }),
        }
    }
}

// -- patch notes ---------------------------------------------------------------

// A number as a note shows it.
fn note_number(value: f64, percent: bool) -> String {
    format!("{}{}", number_text(value), if percent { "%" } else { "" })
}

// What a note calls the number a change moved, and whether it reads as a
// percentage: a flat stat by its tooltip line, a passive's number by the
// passive's name. Empty where neither is found, and for a passive's number
// in a sentence with no name.
fn label_of(texts: &Texts, lang: &str, change: &Change) -> (String, bool) {
    let Some(first) = change.members.first() else {
        return (String::new(), false);
    };
    let label = if super::fields::FLAT.contains(&change.field.as_str()) {
        texts.line_label(lang, &change.field)
    } else {
        None
    }
    .or_else(|| {
        [lang, "en"].iter().find_map(|lang| {
            texts
                .templates
                .get(*lang)
                .and_then(|templates| templates.get(&first.key))
                .and_then(|template| template.passive_of(&first.key, &change.field))
        })
    });
    label.unwrap_or_else(|| (String::new(), false))
}

// The sheet the item icons are in (the mod's own, by `mod.override_info`).
const ITEM_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";
// The arrow the game's own patch notes put between two numbers, in the
// font they take it from (`text/news`, `patch.stat`).
const ARROW: &str = "<f#asset/base/font/set/symbol>\u{2192}<f>";
const BOLD: &str = "asset/base/font/set/bold";
// The green of the game's "Buffs" mark (`ui/icons/up_patch`), a red for
// "Nerfs", an amber for the "Adjustments" between the two, and the grey its
// notes give a reason in (`patch_reason_row`).
const BUFF_COLOR: &str = "#4ed5bdff";
const NERF_COLOR: &str = "#ff6b6bff";
const ADJUST_COLOR: &str = "#f0c05aff";
const REASON_COLOR: &str = "#8b8d9aff";

// One changed number of an entry: the icon the row opens with, which a
// flat stat has where its tooltip line has one and a passive's number never
// has, and the line. A passive's number that has a row format has its
// colour and icon in the line itself ([`Tint`]).
struct NoteRow {
    icon: Option<(String, String)>,
    text: String,
    // Where it comes among its entry's rows: a flat stat by its place in
    // `fields::FLAT`, and a passive's number after every one of those.
    rank: usize,
}

// One item of a patch's notes. A legendary and its radiant are two items,
// each with an entry of its own, under its own name and icon.
struct NoteEntry {
    // The item: one tier.
    key: String,
    // Why the item was touched, as a designer would put it: see
    // [`reason_pool`]. Empty where the text file has no such sentence.
    reason: String,
    rows: Vec<NoteRow>,
    // The other tier's numbers brought level with this one's
    // ([`Change::leader`]), under that tier's key: the same change carried
    // over, and not one the patch made of the other tier by itself.
    followers: Vec<(String, Vec<NoteRow>)>,
    // Whether any of its numbers got stronger, and whether any got weaker.
    // The section it is listed under goes by these, and not by which way
    // the patch meant the item to go: a nerf made up for in another number
    // has both.
    stronger: bool,
    weaker: bool,
}

impl NoteEntry {
    // Adds `row`, of the tier `key`: to the entry's own rows where that is
    // this tier, and to that tier's among the followers where not.
    fn add(&mut self, key: &str, row: NoteRow) {
        if key == self.key {
            self.rows.push(row);
            return;
        }
        match self.followers.iter_mut().find(|(known, _)| known == key) {
            Some((_, rows)) => rows.push(row),
            None => self.followers.push((key.to_string(), vec![row])),
        }
    }
}

// The other tier of the item whose entry lists the row of the tier `key`
// that writes `fields`, and that tier's change of them: where every change
// `patch` makes of `key` in them follows that tier ([`Change::leader`]), and
// that tier has a change of them in the patch as well. Nothing where the
// row is `key`'s own.
fn followed<'a>(patch: &'a Patch, key: &str, fields: &[&str]) -> Option<(&'a str, &'a Change)> {
    let has = |change: &Change, key: &str| {
        fields.contains(&change.field.as_str())
            && change.members.iter().any(|moved| moved.key == key)
    };
    let mut leader: Option<&'a str> = None;
    for change in patch.changes.iter().filter(|change| has(change, key)) {
        if change.leader.is_empty() || leader.is_some_and(|known| known != change.leader.as_str()) {
            return None;
        }
        leader = Some(change.leader.as_str());
    }
    let leader = leader?;
    patch
        .changes
        .iter()
        .find(|change| has(change, leader))
        .map(|change| (leader, change))
}

// The tag an item draws in the item sheet: one of the mod's is its own key,
// one of the game's has it in the settings file.
fn frame_of(key: &str) -> String {
    crate::item_stats::game_item_file()
        .values()
        .filter_map(Value::as_object)
        .find(|object| object.get("key").and_then(Value::as_str) == Some(key))
        .and_then(|object| object.get("icon").and_then(Value::as_str))
        .filter(|icon| !icon.is_empty())
        .unwrap_or(key)
        .to_string()
}

// A holder's win share from which an item counts as winning, and under
// which as losing; and holders a match from which it counts as popular.
const WINNING: f64 = 0.6;
const LOSING: f64 = 0.45;
const OFTEN_HELD: f64 = 0.8;

// The reasons that fit one item's change, as keys of the `item_patch`
// strings in `text/ui.i18n` (`why_nerf_early_1`, ...).
//
// A reason is a designer's sentence, in the voice of the game's own
// champion notes and of League's patch notes, and not the item's record
// read out. An item that stacks is talked of as an early-game item and one
// that scales as a late-game one, and no sentence calls an item oppressive.
// What picks the sentences is
// still the record: an item nobody held gets one about being left on the
// shelf, one that won and was held a lot gets one about the meta, and so
// on; and the item's kind adds the early- or late-game ones.
//
// An item builds do not choose (`Family::chosen`: boots, the jungle items,
// what only a support builds) is in every build it can be in, and is judged
// on how it wins alone. Nothing is said of how often it is built: it is too
// strong, or it is too weak. And a component is patched for the items it is
// built into, which is all there is to say of it.
fn reason_pool(
    buff: bool,
    chosen: bool,
    component: bool,
    tempo: super::base::Tempo,
    change: &Change,
    matches: u32,
) -> Vec<&'static str> {
    use super::base::Tempo;
    if component {
        return if buff {
            vec!["why_buff_component_1", "why_buff_component_2"]
        } else {
            vec!["why_nerf_component_1", "why_nerf_component_2"]
        };
    }
    let games = f64::from(change.games);
    let share = if change.games > 0 {
        f64::from(change.wins) / games
    } else {
        0.5
    };
    let held = if matches > 0 {
        games / f64::from(matches)
    } else {
        0.0
    };
    let mut pool: Vec<&'static str> = Vec::new();
    if buff {
        // An item nobody held. No patch touches one as it stands (an item
        // needs `tuning`'s `min_games` to be judged); the sentences are kept
        // for the day one does.
        if change.games == 0 {
            pool.extend(["why_buff_unbuilt_1", "why_buff_unbuilt_2", "why_buff_unbuilt_3"]);
        }
        match tempo {
            Tempo::Early => pool.extend(["why_buff_early_1", "why_buff_early_2"]),
            Tempo::Late => pool.extend(["why_buff_late_1", "why_buff_late_2"]),
            Tempo::Neither => {}
        }
        if change.games > 0 {
            if share <= LOSING || !chosen {
                pool.extend(["why_buff_weak_1", "why_buff_weak_2"]);
            } else {
                pool.extend(["why_buff_niche_1", "why_buff_niche_2"]);
            }
        }
    } else {
        match tempo {
            Tempo::Early => pool.extend(["why_nerf_early_1", "why_nerf_early_2"]),
            Tempo::Late => pool.extend(["why_nerf_late_1", "why_nerf_late_2"]),
            Tempo::Neither => {}
        }
        pool.extend(match (share >= WINNING, chosen && held >= OFTEN_HELD) {
            (true, true) => ["why_nerf_meta_1", "why_nerf_meta_2"],
            (false, true) => ["why_nerf_popular_1", "why_nerf_popular_2"],
            _ => ["why_nerf_strong_1", "why_nerf_strong_2"],
        });
    }
    pool
}

// A number from a patch and an item, to start a pick at.
fn seed_of(number: u32, text: &str) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(number);
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash >> 16) as usize
}

// How a patch note words one passive's number. Its row is called by the
// passive's name and then by what the number does, "Flux - Base Ultimate
// Haste: 10 -> 12", and the words that lead up to the number and that
// follow it, where there are any, are given too: "Sweet Spot - Bonus Damage:
// up to 25% -> up to 30%", "Stasis - Stasis Duration: 2.5 seconds -> 3
// seconds". A format with no label has the passive's name alone ahead of
// the numbers.
//
// Every one is written by hand. Nothing is read off the item's sentence but
// the passive's name and the number's percent sign
// ([`Template::passive_of`]), so a number with no format here, and none of
// [`FIELD_FORMATS`], is a bare one under its passive's name.
//
// `item` is the base key, so a radiant reads as its legendary does. The
// strings are `item_patch` ones in `text/ui.i18n`, here as their key and
// the English.
struct RowFormat {
    item: &'static str,
    field: &'static str,
    label: Option<(&'static str, &'static str)>,
    lead: Option<(&'static str, &'static str)>,
    unit: Option<(&'static str, &'static str)>,
}

// How a patch note words a passive's number of one kind, whichever item it
// is of: by config field. An amount of a stat the game has no tooltip line
// for is called by the stat, and a cooldown, a duration and a slow by what
// they are, each after the passive's name where the sentence has one:
// "Lethality: 18 -> 19", "Illusion - Adaptive Force: 30 -> 33", "Annul -
// Cooldown: 40 seconds -> 35 seconds", "Icy - Slow: 20% -> 25%". An item's
// own row format comes first ([`ROW_FORMATS`]): a cooldown that is one per
// target has one, and so has each duration of an item with two of them.
struct FieldFormat {
    fields: &'static [&'static str],
    label: (&'static str, &'static str),
    unit: Option<(&'static str, &'static str)>,
}

const FIELD_FORMATS: [FieldFormat; 5] = [
    FieldFormat {
        fields: &["effect_lethality", "effect_bonus_lethality"],
        label: ("row_lethality", "Lethality"),
        unit: None,
    },
    FieldFormat {
        fields: &["adaptive_force", "effect_adaptive_force"],
        label: ("row_adaptive_force", "Adaptive Force"),
        unit: None,
    },
    FieldFormat {
        fields: &["effect_cooldown_seconds"],
        label: ("row_cooldown", "Cooldown"),
        unit: Some(("unit_seconds", " seconds")),
    },
    FieldFormat {
        fields: &["effect_duration_seconds", "effect_shield_seconds"],
        label: ("row_effect_duration", "Effect Duration"),
        unit: Some(("unit_seconds", " seconds")),
    },
    FieldFormat {
        fields: &["effect_slow_amount"],
        label: ("row_slow", "Slow"),
        unit: None,
    },
];

const ROW_FORMATS: [RowFormat; 120] = [
    RowFormat {
        item: "deaths_dance",
        field: "effect_delayed_damage_percent",
        label: Some(("row_damage_reduction", "Damage Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "echoes_of_helia",
        field: "effect_damage_conversion",
        label: Some(("row_damage_stored", "Damage Stored")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "heartsteel",
        field: "effect_bonus_hp_percent_of_damage",
        label: Some(("row_permanent_health", "Permanent Health")),
        lead: None,
        unit: Some(("unit_of_damage_dealt", " of the damage dealt")),
    },
    RowFormat {
        item: "randuins_omen",
        field: "effect_crit_damage_percent_heal",
        label: Some(("row_crit_healing", "Critical Strike Healing")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "hubris",
        field: "effect_stack_attack",
        label: Some(("row_scaling", "Scaling")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "zhonyas_hourglass",
        field: "effect_duration_seconds",
        label: Some(("row_stasis_duration", "Stasis Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "hubris",
        field: "effect_bonus_flat_attack",
        label: Some(("row_base_ad_kill_bonus", "Base AD Kill Bonus")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "cloak_of_starry_night",
        field: "effect_skill_damaged_reduce",
        label: Some(("row_base_skill_damage_reduction", "Base Skill Damage Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "black_cleaver",
        field: "effect_percent_armor_shred",
        label: Some(("row_armor_reduction", "Armor Reduction")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "bloodletters_curse",
        field: "effect_percent_mr_shred",
        label: Some(("row_mr_reduction", "Magic Resistance Reduction")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "abyssal_mask",
        field: "effect_percent_mr_shred",
        label: Some(("row_mr_reduction", "Magic Resistance Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "frozen_heart",
        field: "effect_attack_speed_reduce",
        label: Some(("row_attack_speed_reduction", "Attack Speed Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "executioners_calling",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "mortal_reminder",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "chempunk_chainsword",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "chemtech_putrifier",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "morellonomicon",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "oblivion_orb",
        field: "effect_heal_reduce",
        label: Some(("row_healing_reduction", "Healing Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "bloodsong",
        field: "effect_damaged_amplify",
        label: Some(("row_damage_taken_bonus", "Damage Taken Bonus")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "imperial_mandate",
        field: "effect_damaged_amplify",
        label: Some(("row_damage_taken_bonus", "Damage Taken Bonus")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "celestial_opposition",
        field: "effect_damaged_reduce",
        label: Some(("row_damage_reduction", "Damage Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "spirit_visage",
        field: "effect_heal_mult",
        label: Some(("row_bonus_healing_received", "Bonus Healing Received")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "lord_dominiks_regards",
        field: "effect_max_percent_bonus",
        label: Some(("row_bonus_damage_cap", "Bonus Damage Cap")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "eclipse",
        field: "effect_cooldown_seconds",
        label: Some(("row_cooldown", "Cooldown")),
        lead: None,
        unit: Some(("unit_seconds_per_target", " seconds per target")),
    },
    RowFormat {
        item: "heartsteel",
        field: "effect_cooldown_seconds",
        label: Some(("row_cooldown", "Cooldown")),
        lead: None,
        unit: Some(("unit_seconds_per_target", " seconds per target")),
    },
    RowFormat {
        item: "night_harvester",
        field: "effect_cooldown_seconds",
        label: Some(("row_cooldown", "Cooldown")),
        lead: None,
        unit: Some(("unit_seconds_per_target", " seconds per target")),
    },
    RowFormat {
        item: "stormsurge",
        field: "effect_cooldown_seconds",
        label: Some(("row_cooldown", "Cooldown")),
        lead: None,
        unit: Some(("unit_seconds_per_target", " seconds per target")),
    },
    RowFormat {
        item: "sundered_sky",
        field: "effect_cooldown_seconds",
        label: Some(("row_cooldown", "Cooldown")),
        lead: None,
        unit: Some(("unit_seconds_per_target", " seconds per target")),
    },
    RowFormat {
        item: "celestial_opposition",
        field: "effect_duration_seconds",
        label: Some(("row_damage_reduction_duration", "Damage Reduction Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "celestial_opposition",
        field: "effect_slow_seconds",
        label: Some(("row_slow_duration", "Slow Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "eclipse",
        field: "effect_duration_seconds",
        label: Some(("row_mark_duration", "Mark Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "eclipse",
        field: "effect_shield_seconds",
        label: Some(("row_shield_duration", "Shield Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "opportunity",
        field: "effect_out_of_combat_seconds",
        label: Some(("row_out_of_combat_timer", "Out of Combat Timer")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "opportunity",
        field: "effect_duration_seconds",
        label: Some(("row_lethality_duration", "Lethality Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "rite_of_ruin",
        field: "effect_shield_seconds",
        label: Some(("row_shield_duration", "Shield Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "rite_of_ruin",
        field: "effect_duration_seconds",
        label: Some(("row_stack_duration", "Stack Duration")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "haunting_guise",
        field: "effect_out_of_combat_seconds",
        label: Some(("row_out_of_combat_timer", "Out of Combat Timer")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "warmogs_armor",
        field: "effect_duration_seconds",
        label: Some(("row_out_of_combat_timer", "Out of Combat Timer")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "stormsurge",
        field: "effect_duration_seconds",
        label: Some(("row_burst_window", "Burst Window")),
        lead: None,
        unit: Some(("unit_seconds", " seconds")),
    },
    RowFormat {
        item: "grezs_spectral_lantern",
        field: "effect_percent_bonus_damage",
        label: Some(("row_bonus_magic_damage", "Bonus Magic Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "spirit_stone",
        field: "effect_percent_bonus_damage",
        label: Some(("row_bonus_magic_damage", "Bonus Magic Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "diamond_tipped_spear",
        field: "effect_max_percent_bonus",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: Some(("lead_up_to", "up to ")),
        unit: None,
    },
    RowFormat {
        item: "fiendhunter_bolts",
        field: "effect_percent_bonus_damage",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "haunting_guise",
        field: "effect_stack_percent_damage",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "infinity_edge",
        field: "effect_crit_damage_bonus",
        label: Some(("row_bonus_crit_damage", "Bonus Crit Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "kraken_slayer",
        field: "effect_max_percent_bonus",
        label: Some(("row_missing_health_scaling", "Missing Health Scaling")),
        lead: Some(("lead_up_to", "up to ")),
        unit: None,
    },
    RowFormat {
        item: "lord_dominiks_regards",
        field: "effect_percent_bonus_damage",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "sundered_sky",
        field: "effect_percent_bonus_damage",
        label: Some(("row_first_attack_multiplier", "First Attack Multiplier")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "grezs_spectral_lantern",
        field: "effect_bonus_hp_percent_of_damage",
        label: Some(("row_healing", "Healing")),
        lead: None,
        unit: Some(("unit_of_your_damage_dealt", " of your damage dealt")),
    },
    RowFormat {
        item: "spirit_stone",
        field: "effect_bonus_hp_percent_of_damage",
        label: Some(("row_healing", "Healing")),
        lead: None,
        unit: Some(("unit_of_your_damage_dealt", " of your damage dealt")),
    },
    RowFormat {
        item: "feral_flare",
        field: "effect_bonus_flat_heal",
        label: Some(("row_healing", "Healing")),
        lead: None,
        unit: Some(("unit_health", " health")),
    },
    RowFormat {
        item: "guardian_angel",
        field: "effect_caster_hp_percent_heal",
        label: Some(("row_resurrect_health", "Resurrect Health")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "moonstone_renewer",
        field: "effect_heal_chain_percent",
        label: Some(("row_chained_healing", "Chained Healing")),
        lead: None,
        unit: Some(("unit_of_original_amount", " of the original amount")),
    },
    RowFormat {
        item: "steraks_gage",
        field: "effect_caster_hp_percent_shield",
        label: Some(("row_shielding", "Shielding")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "warmogs_armor",
        field: "effect_caster_hp_percent_heal",
        label: Some(("row_out_of_combat_healing", "Out of Combat Healing")),
        lead: None,
        unit: Some(("unit_max_hp_per_second", " max HP per second")),
    },
    RowFormat {
        item: "guinsoos_rageblade",
        field: "effect_bonus_magic_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "wits_end",
        field: "effect_bonus_magic_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "ravenous_hydra",
        field: "effect_ad_percent_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_ad", " AD")),
    },
    RowFormat {
        item: "tiamat",
        field: "effect_ad_percent_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_ad", " AD")),
    },
    RowFormat {
        item: "hextech_alternator",
        field: "effect_bonus_flat_damage",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "scouts_slingshot",
        field: "effect_bonus_flat_damage",
        label: Some(("row_bonus_damage", "Bonus Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "ardent_censer",
        field: "effect_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "ardent_censer",
        field: "effect_enemy_max_hp_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_target_max_hp", " target max HP")),
    },
    RowFormat {
        item: "blade_of_the_ruined_king",
        field: "effect_hp_percent_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_target_current_hp", " target current HP")),
    },
    RowFormat {
        item: "dead_mans_plate",
        field: "effect_max_bonus_damage",
        label: Some(("row_on_hit_damage_cap", "On-Hit Damage Cap")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "deaths_dance",
        field: "effect_burn_hp_percent_cap",
        label: Some(("row_delayed_damage_cap", "Delayed Damage Cap")),
        lead: None,
        unit: Some(("unit_max_hp_per_second", " max HP per second")),
    },
    RowFormat {
        item: "eclipse",
        field: "effect_hp_percent_damage",
        label: Some(("row_damage", "Damage")),
        lead: None,
        unit: Some(("unit_target_max_hp", " target max HP")),
    },
    RowFormat {
        item: "essence_reaver",
        field: "effect_ad_percent_damage",
        label: Some(("row_on_hit_ad_ratio", "On-Hit Damage AD Ratio")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "essence_reaver",
        field: "effect_crit_percent_damage",
        label: Some(("row_on_hit_crit_ratio", "On-Hit Damage Crit Ratio")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "fiendhunter_bolts",
        field: "effect_damage_conversion",
        label: Some(("row_true_damage_ratio", "True Damage Ratio")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "hamstringer",
        field: "effect_crit_percent_damage",
        label: Some(("row_bleed_crit_ratio", "Bleed Crit Ratio")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "kraken_slayer",
        field: "effect_bonus_flat_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "liandrys_torment",
        field: "effect_hp_percent_damage",
        label: Some(("row_burn_damage", "Burn Damage")),
        lead: None,
        unit: Some(("unit_target_max_hp", " target max HP")),
    },
    RowFormat {
        item: "night_harvester",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "stormrazor",
        field: "effect_bonus_flat_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "stormrazor",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "titanic_hydra",
        field: "effect_caster_hp_percent_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "titanic_hydra",
        field: "effect_splash_caster_hp_percent",
        label: Some(("row_aoe_damage", "AoE Damage")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "voltaic_cyclosword",
        field: "effect_hp_percent_damage",
        label: Some(("row_on_hit_damage", "On-Hit Damage")),
        lead: None,
        unit: Some(("unit_target_current_hp", " target current HP")),
    },
    RowFormat {
        item: "zekes_herald",
        field: "effect_bonus_magic_damage",
        label: Some(("row_storm_damage", "Storm Damage")),
        lead: None,
        unit: Some(("unit_per_second", " per second")),
    },
    RowFormat {
        item: "atmas_reckoning",
        field: "effect_stack_crit_chance",
        label: Some(("row_crit_chance", "Crit Chance")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "bandlepipes",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "bandlepipes",
        field: "effect_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "blackfire_torch",
        field: "effect_stack_magic_power",
        label: Some(("row_ability_power", "Ability Power")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "cosmic_drive",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "dead_mans_plate",
        field: "effect_move_speed_per_stack",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: Some(("unit_per_momentum_stack", " per <#92dc7bff>Momentum<> stack")),
    },
    RowFormat {
        item: "dream_maker",
        field: "effect_skill_damaged_reduce",
        label: Some(("row_skill_damage_reduction", "Skill Damage Reduction")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "endless_hunger",
        field: "effect_vamp",
        label: Some(("row_omnivamp", "Omnivamp")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "experimental_hexplate",
        field: "effect_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "experimental_hexplate",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "fiendhunter_bolts",
        field: "effect_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "grezs_spectral_lantern",
        field: "effect_stack_magic_power",
        label: Some(("row_ability_power", "Ability Power")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "guinsoos_rageblade",
        field: "effect_stack_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "hullbreaker",
        field: "effect_bonus_defence",
        label: Some(("row_armor", "Armor")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "hullbreaker",
        field: "effect_bonus_magic_resistance",
        label: Some(("row_magic_resistance", "Magic Resistance")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "jaksho_the_protean",
        field: "effect_stack_defence_mult",
        label: Some(("row_armor", "Armor")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "jaksho_the_protean",
        field: "effect_stack_magic_resistance_mult",
        label: Some(("row_magic_resistance", "Magic Resistance")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "locket_of_the_iron_solari",
        field: "effect_bonus_defence",
        label: Some(("row_armor", "Armor")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "locket_of_the_iron_solari",
        field: "effect_bonus_magic_resistance",
        label: Some(("row_magic_resistance", "Magic Resistance")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "locket_of_the_iron_solari",
        field: "effect_bonus_hp_regen",
        label: Some(("row_health_regeneration", "Health Regeneration")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "mirage_blade",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "overlords_bloodmail",
        field: "effect_caster_hp_percent_attack",
        label: Some(("row_attack_damage", "Attack Damage")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "phage",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "philosophers_stone",
        field: "effect_stack_hp_mult",
        label: Some(("row_bonus_health", "Bonus Health")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "riftmaker",
        field: "effect_vamp",
        label: Some(("row_omnivamp", "Omnivamp")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "riftmaker",
        field: "effect_caster_hp_percent_power",
        label: Some(("row_ability_power", "Ability Power")),
        lead: None,
        unit: Some(("unit_max_hp", " max HP")),
    },
    RowFormat {
        item: "rite_of_ruin",
        field: "effect_stack_crit_chance",
        label: Some(("row_crit_chance", "Crit Chance")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "rod_of_ages",
        field: "effect_growth_hp",
        label: Some(("row_max_health", "Max Health")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "rod_of_ages",
        field: "effect_growth_magic_power",
        label: Some(("row_ability_power", "Ability Power")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "rod_of_ages",
        field: "effect_growth_skill_cooldown_mult",
        label: Some(("row_ability_haste", "Ability Haste")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "seekers_armguard",
        field: "effect_stack_defence",
        label: Some(("row_armor", "Armor")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "solstice_sleigh",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "spear_of_shojin",
        field: "effect_stack_attack_mult",
        label: Some(("row_attack_damage", "Attack Damage")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "staff_of_flowing_water",
        field: "effect_magic_power",
        label: Some(("row_ability_power", "Ability Power")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "staff_of_flowing_water",
        field: "effect_skill_cooldown_mult",
        label: Some(("row_ability_haste", "Ability Haste")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "terminus",
        field: "effect_armor_pen_per_stack",
        label: Some(("row_armor_penetration", "Armor Penetration")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "terminus",
        field: "effect_magic_pen_per_stack",
        label: Some(("row_mr_penetration", "Magic Resistance Penetration")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
    RowFormat {
        item: "warmogs_armor",
        field: "effect_move_speed_mult",
        label: Some(("row_movement_speed", "Movement Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "yun_tal_wildarrows",
        field: "effect_flurry_attack_speed_mult",
        label: Some(("row_attack_speed", "Attack Speed")),
        lead: None,
        unit: None,
    },
    RowFormat {
        item: "yun_tal_wildarrows",
        field: "effect_stack_crit_chance",
        label: Some(("row_crit_chance", "Crit Chance")),
        lead: None,
        unit: Some(("unit_per_stack", " per stack")),
    },
];

// A row of two or three of a passive's numbers that make one amount, a
// flat part and its shares: "Defy - Healing: 45 + 15% missing HP -> 50 +
// 15% missing HP", "Shadow - On-Hit Damage: 30 (+5% AD) (+10% AP) -> ...".
// Every number of it is written whichever of them the patch moved, as the
// ends of a level range are ([`amount_rows`]).
//
// `amount` is the amount with `{A}`, `{B}` and `{C}` where the numbers go,
// in the order of `fields`, percent signs and all, and with each number's
// part of it in square brackets ([`amount_text`]): "[{A}] + [{B}% missing
// HP]". `item` is the base key, and the strings are `item_patch` ones, by
// key and the English.
struct AmountFormat {
    item: &'static str,
    fields: &'static [&'static str],
    label: (&'static str, &'static str),
    amount: (&'static str, &'static str),
}

// The placeholders of an amount's numbers, in the order of its fields.
const AMOUNT_SLOTS: [&str; 3] = ["{A}", "{B}", "{C}"];

const AMOUNT_FORMATS: [AmountFormat; 26] = [
    AmountFormat {
        item: "deaths_dance",
        fields: &["effect_bonus_flat_heal", "effect_kill_heal_missing_percent"],
        label: ("row_healing", "Healing"),
        amount: ("amount_flat_missing_hp", "[{A}] + [{B}% missing HP]"),
    },
    AmountFormat {
        item: "dusk_and_dawn",
        fields: &["effect_caster_ap_percent_heal", "effect_caster_hp_percent_heal"],
        label: ("row_healing", "Healing"),
        amount: ("amount_ap_and_max_hp", "[{A}% AP] and [{B}% max HP]"),
    },
    AmountFormat {
        item: "eclipse",
        fields: &["effect_bonus_flat_shield", "effect_ad_percent_shield"],
        label: ("row_shielding", "Shielding"),
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "sundered_sky",
        fields: &["effect_bonus_flat_heal", "effect_caster_hp_percent_heal"],
        label: ("row_healing", "Healing"),
        amount: ("amount_flat_missing_hp", "[{A}] + [{B}% missing HP]"),
    },
    AmountFormat {
        item: "unending_despair",
        fields: &["effect_bonus_flat_heal", "effect_caster_hp_percent_heal"],
        label: ("row_healing", "Healing"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "bamis_cinder",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: ("row_burn_damage", "Burn Damage"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "hollow_radiance",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: ("row_burn_damage", "Burn Damage"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "philosophers_stone",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: ("row_burn_damage", "Burn Damage"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "dusk_and_dawn",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "lich_bane",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "bastionbreaker",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "heartsteel",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "hollow_radiance",
        fields: &["effect_explosion_flat_damage", "effect_explosion_caster_hp_percent"],
        label: ("row_damage", "Damage"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "feral_flare",
        fields: &["effect_bonus_magic_damage", "effect_stack_magic_damage"],
        label: ("row_damage", "Damage"),
        amount: ("amount_flat_per_feral_stack", "[{A}] ([+{B}] per <#92dc7bff>Feral<> stack)"),
    },
    AmountFormat {
        item: "hullbreaker",
        fields: &["effect_ad_percent_damage", "effect_caster_hp_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_ad_max_hp", "[{A}% AD] ([+{B}% max HP])"),
    },
    AmountFormat {
        item: "nashors_tooth",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "night_harvester",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: ("row_damage", "Damage"),
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "serpents_fang",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: ("row_bonus_damage", "Bonus Damage"),
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "stormsurge",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: ("row_damage", "Damage"),
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "trinity_force",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "terminus",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage", "effect_ap_percent_damage"],
        label: ("row_on_hit_damage", "On-Hit Damage"),
        amount: ("amount_flat_ad_ap", "[{A}] ([+{B}% AD]) ([+{C}% AP])"),
    },
    AmountFormat {
        item: "zazzaks_realmspike",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage", "effect_enemy_max_hp_damage"],
        label: ("row_damage", "Damage"),
        amount: ("amount_flat_ap_target_max_hp", "[{A}] ([+{B}% AP]) ([+{C}% target max HP])"),
    },
    AmountFormat {
        item: "axiom_arc",
        fields: &["effect_ult_cooldown_mult", "effect_ult_cooldown_per_lethality"],
        label: ("row_ultimate_haste", "Ultimate Haste"),
        amount: ("amount_flat_per_lethality", "[{A}] ([+{B}] per <#ffdd8eff>1 <i#asset/base/ui/banpick/champion_stat_icon:armor_pen_0> Lethality<>)"),
    },
    AmountFormat {
        item: "endless_hunger",
        fields: &["effect_skill_cooldown_mult", "effect_ad_percent_haste"],
        label: ("row_ability_haste", "Ability Haste"),
        amount: ("amount_flat_ad_bracket", "[{A}] ([+{B}% AD])"),
    },
    AmountFormat {
        item: "protectors_vow",
        fields: &["effect_bonus_flat_hp", "effect_caster_defence_percent_hp"],
        label: ("row_bonus_health", "Bonus Health"),
        amount: ("amount_flat_armor", "[{A}] + [{B}% Armor]"),
    },
    AmountFormat {
        item: "protoplasm_harness",
        fields: &["effect_bonus_flat_hp", "effect_hp_percent_boost"],
        label: ("row_bonus_health", "Bonus Health"),
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
];

// The amount `pattern` makes of `numbers`: [`AMOUNT_SLOTS`] are the
// numbers, and what stands in square brackets is drawn as the item's text
// draws the number in it ([`Tint::draw`]), the rest as it is written. An
// icon two parts in a row share is put in once, ahead of the first.
fn amount_text(pattern: &str, numbers: &[String], tints: &[Tint]) -> String {
    let fill = |text: &str| {
        let mut text = text.to_string();
        for (slot, number) in AMOUNT_SLOTS.iter().zip(numbers) {
            text = text.replace(slot, number);
        }
        text
    };
    let mut out = String::new();
    let mut rest = pattern;
    // The icon of the part before.
    let mut shown: Option<&str> = None;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        out.push_str(&fill(&rest[..open]));
        let inside = &rest[open + 1..open + close];
        let tint = AMOUNT_SLOTS
            .iter()
            .zip(tints)
            .find(|part| inside.contains(*part.0))
            .map(|part| part.1);
        match tint {
            Some(tint) => {
                let icon = tint.icon.as_deref();
                out.push_str(&tint.draw(&fill(inside), icon != shown));
                shown = icon;
            }
            None => out.push_str(&fill(inside)),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(&fill(rest));
    out
}

// A row's line: the numbers under what they are called.
fn row_text(label: &str, numbers: String) -> String {
    if label.is_empty() {
        numbers
    } else {
        format!("{label}: {numbers}")
    }
}

// The rows of the numbers of `change`'s item that a note writes as one
// amount, `fields`: a row a tier, with the tier it is of, the whole amount
// as it was and as it is, whichever of its numbers the patch moved. `write`
// makes the amount of the numbers, which it is given in the order of
// `fields`. A number the patch left where it was is read at the balance the
// save has, which is `state` before the patch is taken into it.
//
// Nothing where a tier has some of the numbers and not the others, or no
// tier's amount moved: the numbers then get rows of their own.
fn amount_rows(
    state: &State,
    patch: &Patch,
    change: &Change,
    fields: &[&str],
    label: &str,
    write: &dyn Fn(&[f64]) -> String,
) -> Option<Vec<(String, NoteRow)>> {
    let family = super::base::base().families.get(&change.family)?;
    // What the patch says of `field` in the tier `key`: each tier's change
    // is its own.
    let moved = |field: &str, key: &str| {
        patch
            .changes
            .iter()
            .filter(|other| other.family == change.family && other.field == field)
            .flat_map(|other| other.members.iter())
            .find(|moved| moved.key == key)
    };
    let mut rows = Vec::new();
    for member in &family.members {
        // Each number as it was and as it is, where the tier has it.
        let numbers: Vec<Option<(f64, f64)>> = fields
            .iter()
            .map(|&field| match moved(field, &member.key) {
                Some(moved) => Some((moved.before, moved.after)),
                None => super::balance::reads(state, member, field).map(|value| (value, value)),
            })
            .collect();
        if numbers.iter().all(Option::is_none) {
            continue;
        }
        let numbers = numbers.into_iter().collect::<Option<Vec<(f64, f64)>>>()?;
        if numbers.iter().all(|(was, is)| was == is) {
            continue;
        }
        let was: Vec<f64> = numbers.iter().map(|number| number.0).collect();
        let is: Vec<f64> = numbers.iter().map(|number| number.1).collect();
        let text = format!("{} {ARROW} {}", write(&was), write(&is));
        rows.push((
            member.key.clone(),
            NoteRow {
                icon: None,
                text: row_text(label, text),
                rank: super::fields::FLAT.len(),
            },
        ));
    }
    (!rows.is_empty()).then_some(rows)
}

// The rows of a range that grows with level, `low` to `high` of `change`'s
// item, under the passive's name: "40 - 100 (based on level) -> 45 - 110
// (based on level)", whichever of its ends the patch moved
// ([`amount_rows`]). Each end is in its colour, behind the icon the text
// has for the amount: once, where the two ends have the same one.
fn range_rows(
    texts: &Texts,
    state: &State,
    patch: &Patch,
    lang: &str,
    change: &Change,
    (low, high): (&str, &str),
) -> Option<Vec<(String, NoteRow)>> {
    let family = super::base::base().families.get(&change.family)?;
    let lead = family.members.first()?;
    let (label, percent) = label_of(texts, lang, change);
    let (between, tail) = texts.range_words(lang, &lead.key, low, high);
    let (low_tint, high_tint) = (texts.tint(&lead.key, low), texts.tint(&lead.key, high));
    let again = low_tint.icon != high_tint.icon;
    amount_rows(state, patch, change, &[low, high], &label, &|ends: &[f64]| {
        format!(
            "{}{between}{}{tail}",
            low_tint.draw(&note_number(ends[0], percent), true),
            high_tint.draw(&note_number(ends[1], percent), again)
        )
    })
}

// The number that names the row of a patch's notes the number `field` of
// the item `key` is written in: the low end of a range ([`range_rows`]),
// the first number of an amount ([`AMOUNT_FORMATS`]), and any other number
// itself. So the rows an item gets are counted the way they are written,
// for the most a patch gives one (`max_changes_per_item`).
pub(crate) fn row_of<'a>(key: &str, field: &'a str) -> &'a str {
    if let Some((low, _)) = super::fields::range_of(field) {
        return low;
    }
    let slug = crate::build_config::base_slug(key);
    AMOUNT_FORMATS
        .iter()
        .find(|format| format.item == slug && format.fields.contains(&field))
        .map_or(field, |format| format.fields[0])
}

// A patch's items, an entry an item: the ones it set out to buff and then
// the ones it set out to nerf, each lot in the order the patch has them. A
// legendary and its radiant are two items here, each with the numbers of
// its own that moved, and a patch that moved one of them lists that one.
// A number of one that was only brought level with the other's
// ([`Change::leader`]) is no number of its own: it is listed in the other's
// entry, under the tier's name ([`NoteEntry::followers`]), and has no say
// in which section that entry is in.
//
// A row is one changed number: "Attack Damage: 20 -> 21". An entry's flat
// stats come first and its passives' numbers after them, each lot in a set
// order. A passive's number is worded by its row format ([`ROW_FORMATS`]),
// "Flux - Base Ultimate Haste: 10 -> 12", or by its field's
// ([`FIELD_FORMATS`]). The two ends of a range that grows with level are one
// row ([`range_rows`]), and so are the numbers that make one amount
// ([`AMOUNT_FORMATS`]).
fn note_entries(patch: &Patch, state: &State, lang: &str) -> Vec<NoteEntry> {
    let texts = texts();
    let base = super::base::base();
    let mut entries: Vec<NoteEntry> = Vec::new();
    // The ranges and the amounts written so far, by item family and first
    // number: another of their numbers that comes up is in the row the first
    // one made.
    let mut ranged: Vec<(&str, &str)> = Vec::new();
    // Reasons given so far, so that no two items share one while another
    // fits.
    let mut used: Vec<&'static str> = Vec::new();
    // Where the entry of the item `key` is among `entries`, made where it
    // has none yet. `change` is one of the patch's changes to the item, or
    // to another tier of it: what it was touched for is the same.
    let mut entry_of = |entries: &mut Vec<NoteEntry>, key: &str, change: &Change| -> usize {
        if let Some(at) = entries.iter().position(|entry| entry.key == key) {
            return at;
        }
        // One of the sentences that fit the item, a different one for
        // every item of the article as far as the pool goes, and the same
        // one whenever this patch's notes are written.
        let family = base.families.get(&change.family);
        let chosen = family.map_or(true, |family| family.chosen);
        let component = family.is_some_and(|family| family.component);
        // An item moved only to keep an upgrade no weaker than its part
        // was not judged, and says so. One the player set by hand gives no
        // reason at all: the article says whose patch it is.
        let pool = if change.manual {
            Vec::new()
        } else if change.follows {
            vec!["why_follow_1", "why_follow_2"]
        } else {
            reason_pool(
                change.buff,
                chosen,
                component,
                base.tempo(key),
                change,
                patch.matches,
            )
        };
        let from = seed_of(patch.number, key) % pool.len().max(1);
        let picked = (0..pool.len())
            .map(|step| pool[(from + step) % pool.len()])
            .find(|reason| !used.contains(reason))
            .or_else(|| pool.get(from).copied());
        let reason = picked.map_or_else(String::new, |reason| {
            used.push(reason);
            texts.ui(lang, reason, "")
        });
        entries.push(NoteEntry {
            key: key.to_string(),
            reason,
            rows: Vec::new(),
            followers: Vec::new(),
            stronger: false,
            weaker: false,
        });
        entries.len() - 1
    };
    let buffs = patch.changes.iter().filter(|change| change.buff);
    let nerfs = patch.changes.iter().filter(|change| !change.buff);
    for change in buffs.chain(nerfs) {
        let Some(first) = change.members.first() else {
            continue;
        };
        // Which way this number went, as strength: a cooldown that got
        // shorter is stronger. Told of each tier it moved, in that tier's
        // own entry.
        let lower = super::fields::rule_for(&first.key, &change.field)
            .is_some_and(|rule| rule.lower_is_stronger);
        for moved in &change.members {
            // A number brought level with the other tier's went that tier's
            // way, and is told in that tier's entry.
            if followed(patch, &moved.key, &[change.field.as_str()]).is_some() {
                continue;
            }
            let at = entry_of(&mut entries, &moved.key, change);
            if (moved.after.abs() > moved.before.abs()) != lower {
                entries[at].stronger = true;
            } else {
                entries[at].weaker = true;
            }
        }
        if let Some((low, high)) = super::fields::range_of(&change.field) {
            if ranged.contains(&(change.family.as_str(), low)) {
                continue;
            }
            if let Some(rows) = range_rows(texts, state, patch, lang, change, (low, high)) {
                ranged.push((change.family.as_str(), low));
                for (key, row) in rows {
                    let (owner, why) =
                        followed(patch, &key, &[low, high]).unwrap_or((key.as_str(), change));
                    let at = entry_of(&mut entries, owner, why);
                    entries[at].add(&key, row);
                }
                continue;
            }
        }
        let slug = crate::build_config::base_slug(&first.key);
        // The numbers that make one amount are one row as well, written
        // when the first of them comes up.
        let amount = AMOUNT_FORMATS
            .iter()
            .find(|format| format.item == slug && format.fields.contains(&change.field.as_str()));
        if let Some(format) = amount {
            if ranged.contains(&(change.family.as_str(), format.fields[0])) {
                continue;
            }
            let (name, _) = label_of(texts, lang, change);
            let what = texts.ui(lang, format.label.0, format.label.1);
            let label = if name.is_empty() {
                what
            } else {
                format!("{name} - {what}")
            };
            let pattern = texts.ui(lang, format.amount.0, format.amount.1);
            let tints: Vec<Tint> = format
                .fields
                .iter()
                .map(|field| texts.tint(&first.key, field))
                .collect();
            let write = |numbers: &[f64]| {
                let numbers: Vec<String> = numbers.iter().map(|number| number_text(*number)).collect();
                amount_text(&pattern, &numbers, &tints)
            };
            if let Some(rows) = amount_rows(state, patch, change, format.fields, &label, &write) {
                ranged.push((change.family.as_str(), format.fields[0]));
                for (key, row) in rows {
                    let (owner, why) =
                        followed(patch, &key, format.fields).unwrap_or((key.as_str(), change));
                    let at = entry_of(&mut entries, owner, why);
                    entries[at].add(&key, row);
                }
                continue;
            }
        }
        let (label, percent) = label_of(texts, lang, change);
        let flat = super::fields::FLAT
            .iter()
            .position(|field| *field == change.field);
        // A flat stat's row opens with the stat's icon. A passive's number
        // has none: its row is named for the passive.
        let icon = flat.and_then(|_| texts.line_icon(&change.field));
        // A passive's number is called by its passive's name and then by
        // what it does, with the words that lead up to it and that follow
        // it: all as the item's row format has them, or failing one as the
        // format of the number's field has. One with neither is the bare
        // number under the passive's name.
        let worded = if flat.is_some() {
            None
        } else {
            ROW_FORMATS
                .iter()
                .find(|format| format.item == slug && format.field == change.field)
                .map(|format| (format.label, format.lead, format.unit))
                .or_else(|| {
                    FIELD_FORMATS
                        .iter()
                        .find(|format| format.fields.contains(&change.field.as_str()))
                        .map(|format| (Some(format.label), None, format.unit))
                })
        };
        let said = |words: Option<(&str, &str)>| {
            words.map_or_else(String::new, |(key, english)| texts.ui(lang, key, english))
        };
        let (what, lead, unit) = worded
            .map(|(what, lead, unit)| (said(what), said(lead), said(unit)))
            .unwrap_or_default();
        let label = if what.is_empty() {
            label
        } else if label.is_empty() {
            what
        } else {
            format!("{label} - {what}")
        };
        // A passive's number that has a format is drawn as the item's text
        // draws it, in its colour and behind its icon, with the words that
        // follow it. A flat stat's number, and a passive's that has no
        // format yet, are left as they are.
        let tint = if worded.is_some() {
            texts.tint(&first.key, &change.field)
        } else {
            Tint::default()
        };
        let phrase = |number: f64| {
            let amount = format!("{}{unit}", note_number(number, percent));
            format!("{lead}{}", tint.draw(&amount, true))
        };
        for moved in &change.members {
            let numbers = format!("{} {ARROW} {}", phrase(moved.before), phrase(moved.after));
            let (owner, why) = followed(patch, &moved.key, &[change.field.as_str()])
                .unwrap_or((moved.key.as_str(), change));
            let at = entry_of(&mut entries, owner, why);
            entries[at].add(
                &moved.key,
                NoteRow {
                    icon: icon.clone(),
                    text: row_text(&label, numbers),
                    rank: flat.unwrap_or(super::fields::FLAT.len()),
                },
            );
        }
    }
    // The stats first and the passives' numbers after. The rows of one
    // rank stay as the patch has them.
    for entry in &mut entries {
        entry.rows.sort_by_key(|row| row.rank);
        for (_, rows) in &mut entry.followers {
            rows.sort_by_key(|row| row.rank);
        }
    }
    entries
}

// The size a row of a patch's notes is written in.
const ROW_SIZE: u32 = 18;
// How far in an entry's rows start, and the rows of the other tier that
// came level with them ([`NoteEntry::followers`]).
const ROW_INDENT: &str = "        ";
const FOLLOW_INDENT: &str = "                ";

// A style tag at the head of `text`: a colour, a font or a size, or the
// close of one. An inline icon is not one.
fn style_tag(text: &str) -> Option<&str> {
    let end = text.strip_prefix('<')?.find('>')? + 2;
    let tag = &text[..end];
    let style = matches!(tag, "<>" | "<f>" | "<s>")
        || tag.starts_with("<#")
        || tag.starts_with("<f#")
        || tag.starts_with("<s#");
    style.then_some(tag)
}

// `text` as a line of one size. Every run of it between two style tags is
// written out whole and by itself, `<s#size><f#font><#colour>run<><f><s>`,
// with the font and the colour that are open around it; a run is words and
// inline icons, and an icon is drawn the size of the words.
//
// One size span around the line does not do: the game draws what follows
// the close of a colour at the label's own size again, size span or not.
// Written this way nothing is drawn after a tag has closed, which is how
// the article's headings are written.
fn sized(text: &str, size: u32) -> String {
    let mut out = String::new();
    // The colour and the font tags open where the text has got to.
    let mut colours: Vec<&str> = Vec::new();
    let mut fonts: Vec<&str> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(tag) = style_tag(rest) {
            if tag == "<>" {
                colours.pop();
            } else if tag == "<f>" {
                fonts.pop();
            } else if tag.starts_with("<#") {
                colours.push(tag);
            } else if tag.starts_with("<f#") {
                fonts.push(tag);
            }
            // A size of the text's own is dropped: the line has one.
            rest = &rest[tag.len()..];
            continue;
        }
        // The run: up to the next style tag, icons whole.
        let mut end = 0;
        loop {
            let ahead = &rest[end..];
            if ahead.is_empty() || style_tag(ahead).is_some() {
                break;
            }
            end += if ahead.starts_with("<i#") {
                ahead.find('>').map_or(ahead.len(), |close| close + 1)
            } else {
                ahead.chars().next().map_or(1, char::len_utf8)
            };
        }
        out.push_str(&format!("<s#{size}>"));
        out.push_str(fonts.last().copied().unwrap_or_default());
        out.push_str(colours.last().copied().unwrap_or_default());
        out.push_str(&rest[..end]);
        if !colours.is_empty() {
            out.push_str("<>");
        }
        if !fonts.is_empty() {
            out.push_str("<f>");
        }
        out.push_str("<s>");
        rest = &rest[end..];
    }
    out
}

// The news article for one patch: title, body, author. In the game's
// language as it is when the patch lands; an article is text once written.
// `state` is the save's balance before the patch is taken into it, which
// is where a number the patch did not move is read from.
//
// The body is formatted in the markup the game's own text uses (a colour
// `<#rrggbbaa>..<>`, a font `<f#asset>..<f>`, a size `<s#n>..<s>`, an inline
// icon `<i#sheet:tag>`). The layout: the buffs, the adjustments and the
// nerfs, each under a heading in its colour, and for each item
//
// ```text
// <icon> Trinity Force
// <reason>
//     <stat icon> Attack Damage: 20 -> 21
//     Spellblade: 40 - 100 (based on level) -> 45 - 110 (based on level)
//     <icon> Radiant Trinity Force (to match):
//         <stat icon> Attack Damage: 25 -> 26
// ```
//
// A flat stat's row opens with the stat's icon. A passive's number has no
// icon there: its row opens with the passive's name. The number, with the
// words that follow it where it is written as a phrase, "10 health" or "30
// + 3% max HP", is in the colour the item's text has it in, behind the icon
// the text has for it.
//
// Every item is an entry, a radiant apart from its legendary, but for the
// numbers one tier only had brought level with the other's: those are the
// last rows of the other's entry, under a grey line with the tier's name.
// An item is a buff where every number of it got stronger, a nerf where
// every one got weaker, and an adjustment where it has both. Only where it is listed
// goes by that: its reason is still the one for what the patch set out to
// do with it.
//
// One article and one column, however long the patch. A long one is read
// by scrolling, which a plain article has by the mod's override of its
// layout (`ui/layout/news_component/simple_content.ui`) and
// [`sync_article_scroll`].
pub(crate) fn news(patch: &Patch, state: &State) -> (String, String, String) {
    let texts = texts();
    let lang = game_language();
    // Named as the game names its own, "Patch Notes v…", after the version
    // the game was on when it landed; a patch set off by hand is a numbered
    // hotfix of that version.
    let mut title = texts
        .ui(&lang, "title", "Item Patch Notes v{Version}")
        .replace("{Version}", &patch.announced);
    if patch.hotfix > 0 {
        title = texts
            .ui(&lang, "hotfix", "{Title} - Hotfix {N}")
            .replace("{Title}", &title)
            .replace("{N}", &patch.hotfix.to_string());
    }
    // A patch the player made by hand was judged on no matches, and says
    // what it is.
    let by_hand = !patch.changes.is_empty() && patch.changes.iter().all(|change| change.manual);
    let mut body = if by_hand {
        texts.ui(&lang, "intro_manual", "These items were adjusted by hand.")
    } else {
        texts
            .ui(&lang, "intro", "Items were rebalanced after {Matches} matches on v{Version}.")
            .replace("{Matches}", &patch.matches.to_string())
            .replace("{Version}", &patch.version)
    };
    let entries = note_entries(patch, state, &lang);
    for (stronger, weaker, heading, fallback, color) in [
        (true, false, "buffed", "Buffs", BUFF_COLOR),
        (true, true, "adjusted", "Adjustments", ADJUST_COLOR),
        (false, true, "nerfed", "Nerfs", NERF_COLOR),
    ] {
        let section: Vec<&NoteEntry> = entries
            .iter()
            .filter(|entry| entry.stronger == stronger && entry.weaker == weaker)
            .collect();
        if section.is_empty() {
            continue;
        }
        body.push_str(&format!(
            "\n<s#24><f#{BOLD}><{color}>{}<><f><s>",
            texts.ui(&lang, heading, fallback)
        ));
        for entry in section {
            body.push_str(&format!(
                "\n<i#{ITEM_SHEET}:{frame}> <f#{BOLD}>{name}<f>",
                frame = frame_of(&entry.key),
                name = texts.name(&lang, &entry.key),
            ));
            if !entry.reason.is_empty() {
                body.push_str(&format!(
                    "\n<s#15><{REASON_COLOR}>{}<><s>",
                    entry.reason
                ));
            }
            let row_line = |row: &NoteRow, indent: &str| {
                let mark = match &row.icon {
                    Some((sheet, tag)) => format!("<i#{sheet}:{tag}> "),
                    None => String::new(),
                };
                format!("\n{}", sized(&format!("{indent}{mark}{}", row.text), ROW_SIZE))
            };
            for row in &entry.rows {
                body.push_str(&row_line(row, ROW_INDENT));
            }
            // The other tier's numbers that came level with these: under
            // that tier's name and a step further in, as part of this entry.
            for (key, rows) in &entry.followers {
                let head = texts
                    .ui(&lang, "follow_tier", "{Item} (to match):")
                    .replace("{Item}", &texts.name(&lang, key));
                let line = format!(
                    "{ROW_INDENT}<i#{ITEM_SHEET}:{frame}> <{REASON_COLOR}>{head}<>",
                    frame = frame_of(key),
                );
                body.push_str("\n");
                body.push_str(&sized(&line, ROW_SIZE));
                for row in rows {
                    body.push_str(&row_line(row, FOLLOW_INDENT));
                }
            }
        }
    }
    if patch.changes.is_empty() {
        body.push_str("\n\n");
        body.push_str(&texts.ui(&lang, "none", "No item was changed."));
    }
    // A hotfix is the player's own, and signed so where Steam says who
    // that is.
    let author = (patch.hotfix > 0)
        .then(super::steam::persona_name)
        .flatten()
        .unwrap_or_else(|| texts.ui(&lang, "author", "shirograhm"));
    (title, body, author)
}

// -- for the Item Patcher Settings window --------------------------------------

// An item's name in the game's language.
pub(crate) fn item_name(key: &str) -> String {
    texts().name(&game_language(), key)
}

// The tag an item draws in the item sheet ([`ITEM_SHEET`]).
pub(crate) fn item_frame(key: &str) -> String {
    frame_of(key)
}

// One of the mod's `item_patch` strings in the game's language, or the
// English given.
pub(crate) fn ui_string(key: &str, english: &str) -> String {
    texts().ui(&game_language(), key, english)
}

// What the window calls the number `field` of the item `key`, and whether
// it reads as a percentage: as a patch note's row calls it, a stat by its
// tooltip line, "Attack Damage", and a passive's number by the passive's
// name and what the number does, "Flux - Ultimate Haste". The numbers that
// make one amount in the notes share those words, and so do the two ends of
// a range: the window tells them apart by the config field it shows under
// each. Empty for a number nothing names.
pub(crate) fn field_label(key: &str, field: &str) -> (String, bool) {
    let texts = texts();
    let lang = game_language();
    let flat = super::fields::FLAT.contains(&field);
    let named = if flat {
        texts.line_label(&lang, field)
    } else {
        None
    }
    .or_else(|| {
        [lang.as_str(), "en"].iter().find_map(|lang| {
            texts
                .templates
                .get(*lang)
                .and_then(|templates| templates.get(key))
                .and_then(|template| template.passive_of(key, field))
        })
    });
    let (name, percent) = named.unwrap_or_default();
    if flat {
        return (name, percent);
    }
    let slug = crate::build_config::base_slug(key);
    let what = ROW_FORMATS
        .iter()
        .find(|format| format.item == slug && format.field == field)
        .map(|format| format.label)
        .or_else(|| {
            FIELD_FORMATS
                .iter()
                .find(|format| format.fields.contains(&field))
                .map(|format| Some(format.label))
        })
        .flatten()
        .or_else(|| {
            AMOUNT_FORMATS
                .iter()
                .find(|format| format.item == slug && format.fields.contains(&field))
                .map(|format| format.label)
        })
        .map(|(key, english)| texts.ui(&lang, key, english))
        .unwrap_or_default();
    let label = match (name.is_empty(), what.is_empty()) {
        (_, true) => name,
        (true, false) => what,
        (false, false) => format!("{name} - {what}"),
    };
    (label, percent)
}

// -- a plain article's scroll ---------------------------------------------------

// The news screen, and a plain article in it
// (`news_component/simple_content`, mounted under its root's name): its
// title, author and date, and its text.
const NEWS_SCREEN: &str = "main.top.right.news";
const ARTICLE_TITLE: &str = "main.top.right.news.contents.simple_container.top.title";
const ARTICLE_AUTHOR: &str = "main.top.right.news.contents.simple_container.top.author";
const ARTICLE_TIME: &str = "main.top.right.news.contents.simple_container.top.time";
const ARTICLE_TEXT: &str = "main.top.right.news.contents.simple_container.contents.text";
// The node whose height is how far the body scrolls.
const ARTICLE_EXTENT: &str = "main.top.right.news.contents.simple_container.contents.dummy";
// The body's height on screen, which is the least its scroll is long.
const ARTICLE_VIEW_H: f32 = 774.0;
// The text as the layout authors it: how wide, how high a line, and the
// size it is written in where the text names no other.
const ARTICLE_TEXT_W: f32 = 1043.0;
const ARTICLE_LINE_H: f32 = 36.0;
const ARTICLE_TEXT_SIZE: f32 = 20.0;
// Room under the last line: a line.
const ARTICLE_FOOT: f32 = 36.0;

// A height set that the nodes then did not have, with the heights they had
// instead (the scroll's, the text box's): the same is not set a second
// time. Setting a height every frame that never takes is how a fix for the
// scroll would become what stops it.
static SCROLL_TRIED: Mutex<Option<(f32, f32, f32)>> = Mutex::new(None);

// The article whose scroll was last worked out, by its title, and what that
// came to ([`scroll_for`]). Finding an article's text can mean reading the
// whole inbox, which is done once an article and not on every frame it is
// open.
static SCROLL_FOR: Mutex<Option<(String, Option<f32>)>> = Mutex::new(None);

// Whether a character is drawn a full size wide (Chinese, Japanese, Korean
// and the full-width forms) and not about half of one.
fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x11FF
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
    )
}

// How tall an article's text is drawn: a line of the layout's height for
// every line of the text, and for a line wider than the label as many as
// it wraps to.
//
// Worked out from the text because the host does not measure it: the
// label's rect and its contents rect are both its authored box, and
// `fit_height` does not grow it here (a 35px box whatever the text). The
// widths are a guess at the font, a little wide.
// Nothing the mod writes wraps, so its own articles come out exact; the
// guess only decides whether one of the game's long articles gets a short
// scroll it did not have.
fn article_text_height(text: &str) -> f32 {
    let mut lines = 0usize;
    for line in text.split('\n') {
        let mut size = ARTICLE_TEXT_SIZE;
        let mut width = 0f32;
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            // Markup takes no room, but for an icon; a size holds until `<s>`.
            let tag_end = if c == '<' { rest.find('>') } else { None };
            if let Some(end) = tag_end {
                let tag = &rest[1..end];
                if let Some(number) = tag.strip_prefix("s#") {
                    size = number.parse().unwrap_or(ARTICLE_TEXT_SIZE);
                } else if tag == "s" {
                    size = ARTICLE_TEXT_SIZE;
                } else if tag.starts_with("i#") {
                    width += size;
                }
                rest = &rest[end + 1..];
                continue;
            }
            width += size * if is_wide(c) { 1.0 } else { 0.55 };
            rest = &rest[c.len_utf8()..];
        }
        lines += ((width / ARTICLE_TEXT_W).ceil() as usize).max(1);
    }
    lines as f32 * ARTICLE_LINE_H
}

// Makes the open article scroll as far as its text is long, and no
// further. Every client frame; off the news screen it is one failed
// lookup.
//
// The mod's override of the plain article layout makes its body a scroll
// view. Two things about one, neither of them to be had from the layouts:
//
// - It is as long as a node in it says, and does not take its length from
//   a label: with only the label in it the article does not scroll. The
//   game's own scrolling article (`news_component/tutorial_last`) has a
//   `#dummy` node for the length, whose height game code sets, and the
//   override has the same node.
// - A node whose box has left the view is not drawn, text and all: a label
//   35px high, with the text running out under it, is gone at the first
//   turn of the wheel. So the label's box is as long as the scroll, always.
//
// The layout gives both a height that is enough for the longest patch
// notes, and this sets both to what the open article's text takes
// ([`scroll_for`]): the patch notes scroll to their last line and no
// further, and a short article does not scroll at all. One whose text
// cannot be had keeps the layout's length, and scrolls past its end.
//
// For any plain article, the mod's own or not: the layout is every plain
// article's.
pub(crate) fn sync_article_scroll(ctx: &mut StableClient<'_>) {
    // A plain article in the mod's layout is up: its length node is there.
    let Some((_, _, _, length)) = ctx.ui_node_rect(ARTICLE_EXTENT) else {
        return;
    };
    probe_article(ctx);
    let Some((_, _, _, text_box)) = ctx.ui_node_rect(ARTICLE_TEXT) else {
        return;
    };
    let Some(wanted) = scroll_for(ctx) else {
        return;
    };
    let Ok(mut tried) = SCROLL_TRIED.lock() else {
        return;
    };
    if (length - wanted).abs() < 1.0 && (text_box - wanted).abs() < 1.0 {
        *tried = None;
        return;
    }
    if *tried == Some((wanted, length, text_box)) {
        super::log("patch.scroll.kept", || {
            format!(
                "set to {wanted}px, and the scroll is still {length}px long and the text's box {text_box}px high"
            )
        });
        return;
    }
    let height = format!("height: {wanted}px;");
    let set = ctx.ui_set_properties(ARTICLE_EXTENT, &height)
        & ctx.ui_set_properties(ARTICLE_TEXT, &height);
    *tried = Some((wanted, length, text_box));
    // No positions in this line: they change with every turn of the wheel,
    // and a line that changes is a line written again.
    super::log("patch.scroll", || {
        format!(
            "the scroll was {length}px long and the text's box {text_box}px high, both set to {wanted}px: {set}"
        )
    });
}

// A label's text: as the host gives it, or failing that out of the label's
// state, which has it too.
fn label_text(ctx: &StableClient<'_>, path: &str) -> Option<String> {
    ctx.ui_text(path).or_else(|| {
        let state: Value = serde_json::from_str(&ctx.ui_state_json(path)?).ok()?;
        state.get("text")?.as_str().map(str::to_string)
    })
}

// How long the scroll of the open article is to be: as long as its text is
// drawn, and never shorter than the view, which is no scroll at all.
// Nothing where the text could not be had.
//
// The text is the label's where the host gives it. For the patch notes it
// has been seen to give none, with the label there, and why is not known.
// So failing that the article is looked up in the inbox by its title
// ([`inbox_article`]). An article the inbox does not have by its title is
// one of the game's own, and has no scroll, as in the game's own layout.
fn scroll_for(ctx: &StableClient<'_>) -> Option<f32> {
    let title = label_text(ctx, ARTICLE_TITLE);
    let body = label_text(ctx, ARTICLE_TEXT);
    // What the answer was worked out from: while it is the same, so is the
    // answer.
    let asked = format!(
        "{}\n{}",
        title.as_deref().unwrap_or_default(),
        body.as_ref().map_or(0, String::len)
    );
    if let Ok(known) = SCROLL_FOR.lock() {
        if let Some((_, length)) = known.as_ref().filter(|(known, _)| *known == asked) {
            return *length;
        }
    }
    let length_of = |text: &str| {
        (article_text_height(text) + ARTICLE_FOOT)
            .max(ARTICLE_VIEW_H)
            .ceil()
    };
    let (length, from) = match (&body, &title) {
        (Some(body), _) => (Some(length_of(body)), "the label's text"),
        (None, Some(title)) => match inbox_article(ctx, title) {
            Some(Some(text)) => (Some(length_of(&text)), "the inbox's text"),
            Some(None) => (Some(ARTICLE_VIEW_H), "no article of that title in the inbox"),
            None => (None, "an inbox that could not be read"),
        },
        (None, None) => (None, "nothing: neither the title nor the text can be read"),
    };
    super::log("patch.scroll.for", || {
        format!("title {title:?}: {length:?}px, from {from}")
    });
    if let Ok(mut known) = SCROLL_FOR.lock() {
        *known = Some((asked, length));
    }
    length
}

// The text of the newest plain article in the player's inbox that goes by
// `title`. Nothing inside where the inbox has none by that title: a title
// the game makes from a template is not kept as it is shown, and one the
// mod posts is. Nothing at all where the inbox could not be read.
fn inbox_article(ctx: &StableClient<'_>, title: &str) -> Option<Option<String>> {
    let team = ctx.player_team_id()?;
    let news = ctx.record_get_json(RecordKindV1::Team, team, "news")?;
    let news: Value = serde_json::from_str(&news).ok()?;
    let articles = news.as_array()?;
    Some(articles.iter().rev().find_map(|article| {
        let named = article.get("title")?.as_str()?;
        if named.trim() != title.trim() {
            return None;
        }
        article
            .pointer("/ty/Simple/content")?
            .as_str()
            .map(str::to_string)
    }))
}

// Children of a node [`tree`] writes out.
const TREE_WIDTH: usize = 6;

// What the news screen gives of an open plain article, for the test log:
// what each of its labels answers, the screen's tree, and the newest of the
// inbox. Looked at every thirtieth frame and written when it has changed.
// With the log off it is a counter.
fn probe_article(ctx: &StableClient<'_>) {
    use std::sync::atomic::{AtomicU32, Ordering};

    static LOOKS: AtomicU32 = AtomicU32::new(0);
    if LOOKS.fetch_add(1, Ordering::Relaxed) % 30 != 0 {
        return;
    }
    let head = |text: Option<String>| {
        text.map(|text| {
            format!(
                "{} bytes {:?}",
                text.len(),
                text.chars().take(40).collect::<String>()
            )
        })
    };
    super::log("patch.scroll.labels", || {
        [ARTICLE_TITLE, ARTICLE_AUTHOR, ARTICLE_TIME, ARTICLE_TEXT, ARTICLE_EXTENT]
            .iter()
            .map(|&path| {
                format!(
                    "{}: a {:?}, visible={:?}, size={:?}, text={:?}, state={:?}",
                    path.rsplit('.').next().unwrap_or(path),
                    ctx.ui_runner_name(path),
                    ctx.ui_visible(path),
                    ctx.ui_node_rect(path).map(|(_, _, width, height)| (width, height)),
                    head(ctx.ui_text(path)),
                    head(ctx.ui_state_json(path)),
                )
            })
            .collect::<Vec<_>>()
            .join(" | ")
    });
    super::log("patch.scroll.tree", || tree(ctx, NEWS_SCREEN, 5, &mut 160));
    super::log("patch.scroll.inbox", || {
        let Some(team) = ctx.player_team_id() else {
            return "the player's team is not known".to_string();
        };
        let news = ctx
            .record_get_json(RecordKindV1::Team, team, "news")
            .and_then(|json| serde_json::from_str::<Value>(&json).ok());
        let Some(Value::Array(list)) = news else {
            return format!("team {team}'s news could not be read");
        };
        let newest: Vec<String> = list
            .iter()
            .rev()
            .take(5)
            .map(|article| {
                format!(
                    "{:?} a {:?}, read={:?}, {:?} bytes of plain text",
                    article
                        .get("title")
                        .and_then(Value::as_str)
                        .map(|title| title.chars().take(40).collect::<String>()),
                    article
                        .get("ty")
                        .and_then(Value::as_object)
                        .and_then(|ty| ty.keys().next()),
                    article.get("is_read"),
                    article
                        .pointer("/ty/Simple/content")
                        .and_then(Value::as_str)
                        .map(str::len),
                )
            })
            .collect();
        format!(
            "{} articles, the newest first: {}",
            list.len(),
            newest.join(" | ")
        )
    });
}

// The UI tree under `path` by name and kind, `depth` levels of it, the first
// [`TREE_WIDTH`] children of each node and no more than `budget` nodes in
// all: a label with the start of its text, a selectable with whether it is
// selected, and of a node with more children which of them are.
fn tree(ctx: &StableClient<'_>, path: &str, depth: usize, budget: &mut usize) -> String {
    let kind = ctx.ui_runner_name(path).unwrap_or_default();
    let mut out = format!("{}:{kind}", path.rsplit('.').next().unwrap_or(path));
    if kind == "label" {
        let text = label_text(ctx, path).map(|text| text.chars().take(24).collect::<String>());
        out.push_str(&format!("={text:?}"));
    }
    if let Some(selected) = ctx.ui_selectable_selected(path) {
        out.push_str(&format!("(selected={selected})"));
    }
    if ctx.ui_visible(path) == Some(false) {
        out.push_str("(hidden)");
    }
    let children = ctx.ui_child_names(path);
    if children.is_empty() {
        return out;
    }
    if depth == 0 {
        out.push_str(&format!("[{} children]", children.len()));
        return out;
    }
    if children.len() > TREE_WIDTH {
        let selected: Vec<&String> = children
            .iter()
            .filter(|child| ctx.ui_selectable_selected(&format!("{path}.{child}")) == Some(true))
            .collect();
        out.push_str(&format!(
            "({} children, selected {selected:?})",
            children.len()
        ));
    }
    out.push('[');
    for (at, child) in children.iter().take(TREE_WIDTH).enumerate() {
        if *budget == 0 {
            out.push_str(" ...");
            break;
        }
        *budget -= 1;
        if at > 0 {
            out.push(' ');
        }
        out.push_str(&tree(ctx, &format!("{path}.{child}"), depth - 1, budget));
    }
    out.push(']');
    out
}
