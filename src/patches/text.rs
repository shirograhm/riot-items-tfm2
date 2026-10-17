//! Item text under a balance patch: what an item's tooltip says once its
//! numbers have moved, and putting that on screen.
//!
//! The game draws a tooltip's stat lines from its own copy of the item's
//! numbers and its effect text from `text/item.i18n`, neither of which follows
//! a patch, so the label on screen is read and its unpatched numbers replaced
//! ([`sync_tooltips`]); anything not found is left as it is. Numbers the
//! player's `config.json` changed are rewritten the same way (`Base::custom`).
//!
//! The patched sentences come from `item-templates.json`, compiled in: every
//! effect text in every language with slots for config fields, from which
//! `text/item.i18n` is generated (`tools/item_i18n.py`). An item whose text
//! file fails that round trip at start-up ([`Texts::unverified`]) has none of
//! its quoted numbers patched.

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

    // The passive `item`'s `field` belongs to (a passive's text opens with its
    // name and a colon), and whether a percent sign follows the number. The
    // name is empty for a sentence of its own, "Gain 18 Lethality."
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

    // How the text draws `item`'s `field`: the colour of its span, and the icon
    // of that colour nearest after it in its paragraph, or before. Neither for
    // a number in no colour span.
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

    // How the text writes `item`'s range from `low` to `high`: what stands
    // between the numbers (" - "), and a wordy bracket after the second ("
    // (based on level)"), colours and icon included. `None` where the two are
    // not adjacent.
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

// A slot's number as written into a sentence, matching `tools/item_i18n.py`: an
// `int` rounds half to even, a `double` takes fifteen significant digits.
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

    // Items whose effect text in some language differs from what its template
    // makes of the default numbers (`shipped`) or the player's (`value`): a
    // text file from another version, which a patch cannot be trusted to
    // rewrite.
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

    // How `key`'s effect text writes the range `low`-`high`
    // ([`Template::range_of`]), in `lang` or else English; a dash and nothing
    // where it has none.
    fn range_words(&self, lang: &str, key: &str, low: &str, high: &str) -> (String, String) {
        [lang, "en"]
            .iter()
            .find_map(|lang| self.templates.get(*lang)?.get(key))
            .and_then(|template| template.range_of(key, low, high))
            .unwrap_or_else(|| (" - ".to_string(), String::new()))
    }

    // How `key`'s effect text draws its `field` ([`Template::tint_of`]), read
    // off the English, since colours and icons are the same in every language;
    // no icon for [`NO_ICON`].
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

// The game's language, from `config/game/base.json` beside the executable (the
// stable API answers everything in English). Re-read every few seconds, since
// the options can change it.
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
// The description panel of the Item Info page (`item_info.ui`), under Game Info
// and in the strategy screen's Item Info popup: inside the page's `#data`.
const DETAIL: &str = "data.item_detail";

fn site_paths(root: String, inner: &str) -> (String, String, String) {
    let name = format!("{root}.{inner}name");
    let desc = format!("{root}.{inner}desc");
    (root, name, desc)
}

// Finds the description labels on screen: under the two known tooltip roots,
// and one level below where management screens are mounted.
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

// Puts the patched numbers into whatever item description is showing; every
// frame, one atomic read while nothing patched shows. Rewritten whenever the
// label stops saying what was written, which covers game code filling it once
// or every frame.
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

// What a note calls a changed number, and whether it is a percentage: a flat
// stat by its tooltip line, a passive's number by the passive's name; empty
// where neither is found.
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

// One changed number of an entry: the stat icon a flat stat's row opens with (a
// passive's never has one), and the line, which carries a formatted passive's
// colour and icon ([`Tint`]).
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
    // Whether any number got stronger, and any weaker: the entry's section
    // follows these, not the patch's intent, so a nerf made up for elsewhere
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

// The other tier whose entry lists `key`'s row for `fields`, with that tier's
// change: where all of `key`'s changes to them follow that tier
// ([`Change::leader`]) and it changed them too. `None` for `key`'s own row.
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

// The reasons that fit one item's change, as `item_patch` keys in
// `text/ui.i18n` (`why_nerf_early_1`, ...): a designer's sentence in the voice
// of the game's and League's notes, never the record read out or calling an
// item oppressive.
//
// The record picks the pool (left on the shelf, the meta, ...) and the item's
// tempo adds early- or late-game ones. Items builds do not choose
// (`Family::chosen`) are only too strong or too weak, and a component is
// patched for what it builds into.
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

// How a patch note words one passive's number: the passive's name, what the
// number does, and any words around it ("Sweet Spot - Bonus Damage: up to 25%
// -> up to 30%"); a format with no label has the name alone. All written by
// hand: without one here or in [`FIELD_FORMATS`] a number is bare under its
// passive's name, and `item` is the base key.
struct RowFormat {
    item: &'static str,
    field: &'static str,
    label: Option<Words>,
    lead: Option<Words>,
    unit: Option<Words>,
}

// How a patch note words a passive's number by config field, whatever the item:
// a stat with no tooltip line by the stat, a cooldown, duration or slow by what
// it is, after the passive's name ("Annul - Cooldown: 40 seconds -> 35
// seconds"). An item's own format ([`ROW_FORMATS`]) comes first.
struct FieldFormat {
    fields: &'static [&'static str],
    label: Words,
    unit: Option<Words>,
}

// A patch note's words, as (key in `text/ui.i18n`, English), shared by
// [`FIELD_FORMATS`], [`ROW_FORMATS`] and [`AMOUNT_FORMATS`].
type Words = (&'static str, &'static str);

// What a number does: "Movement Speed".
mod label {
    use super::Words;

    pub(super) const ABILITY_HASTE: Words = ("row_ability_haste", "Ability Haste");
    pub(super) const ABILITY_POWER: Words = ("row_ability_power", "Ability Power");
    pub(super) const ADAPTIVE_FORCE: Words = ("row_adaptive_force", "Adaptive Force");
    pub(super) const AOE_DAMAGE: Words = ("row_aoe_damage", "AoE Damage");
    pub(super) const ARMOR: Words = ("row_armor", "Armor");
    pub(super) const ARMOR_PENETRATION: Words = ("row_armor_penetration", "Armor Penetration");
    pub(super) const ARMOR_REDUCTION: Words = ("row_armor_reduction", "Armor Reduction");
    pub(super) const ATTACK_DAMAGE: Words = ("row_attack_damage", "Attack Damage");
    pub(super) const ATTACK_SPEED: Words = ("row_attack_speed", "Attack Speed");
    pub(super) const ATTACK_SPEED_REDUCTION: Words = ("row_attack_speed_reduction", "Attack Speed Reduction");
    pub(super) const BASE_AD_KILL_BONUS: Words = ("row_base_ad_kill_bonus", "Base AD Kill Bonus");
    pub(super) const BASE_SKILL_DAMAGE_REDUCTION: Words = ("row_base_skill_damage_reduction", "Base Skill Damage Reduction");
    pub(super) const BLEED_CRIT_RATIO: Words = ("row_bleed_crit_ratio", "Bleed Crit Ratio");
    pub(super) const BONUS_CRIT_DAMAGE: Words = ("row_bonus_crit_damage", "Bonus Crit Damage");
    pub(super) const BONUS_DAMAGE: Words = ("row_bonus_damage", "Bonus Damage");
    pub(super) const BONUS_DAMAGE_CAP: Words = ("row_bonus_damage_cap", "Bonus Damage Cap");
    pub(super) const BONUS_HEALING_RECEIVED: Words = ("row_bonus_healing_received", "Bonus Healing Received");
    pub(super) const BONUS_HEALTH: Words = ("row_bonus_health", "Bonus Health");
    pub(super) const BONUS_MAGIC_DAMAGE: Words = ("row_bonus_magic_damage", "Bonus Magic Damage");
    pub(super) const BURN_DAMAGE: Words = ("row_burn_damage", "Burn Damage");
    pub(super) const BURST_WINDOW: Words = ("row_burst_window", "Burst Window");
    pub(super) const CHAINED_HEALING: Words = ("row_chained_healing", "Chained Healing");
    pub(super) const COOLDOWN: Words = ("row_cooldown", "Cooldown");
    pub(super) const COOLDOWN_REFUND: Words = ("row_cooldown_refund", "Cooldown Refund");
    pub(super) const CRIT_CHANCE: Words = ("row_crit_chance", "Crit Chance");
    pub(super) const CRIT_HEALING: Words = ("row_crit_healing", "Critical Strike Healing");
    pub(super) const DAMAGE: Words = ("row_damage", "Damage");
    pub(super) const DAMAGE_REDUCTION: Words = ("row_damage_reduction", "Damage Reduction");
    pub(super) const DAMAGE_REDUCTION_DURATION: Words = ("row_damage_reduction_duration", "Damage Reduction Duration");
    pub(super) const DAMAGE_STORED: Words = ("row_damage_stored", "Damage Stored");
    pub(super) const DAMAGE_TAKEN_BONUS: Words = ("row_damage_taken_bonus", "Damage Taken Bonus");
    pub(super) const DELAYED_DAMAGE_CAP: Words = ("row_delayed_damage_cap", "Delayed Damage Cap");
    pub(super) const EFFECT_DURATION: Words = ("row_effect_duration", "Effect Duration");
    pub(super) const FIRST_ATTACK_MULTIPLIER: Words = ("row_first_attack_multiplier", "First Attack Multiplier");
    pub(super) const HEALING: Words = ("row_healing", "Healing");
    pub(super) const HEALING_REDUCTION: Words = ("row_healing_reduction", "Healing Reduction");
    pub(super) const HEALTH_REGENERATION: Words = ("row_health_regeneration", "Health Regeneration");
    pub(super) const LETHALITY: Words = ("row_lethality", "Lethality");
    pub(super) const LETHALITY_DURATION: Words = ("row_lethality_duration", "Lethality Duration");
    pub(super) const MAGIC_RESISTANCE: Words = ("row_magic_resistance", "Magic Resistance");
    pub(super) const MARK_DURATION: Words = ("row_mark_duration", "Mark Duration");
    pub(super) const MAX_HEALTH: Words = ("row_max_health", "Max Health");
    pub(super) const MISSING_HEALTH_SCALING: Words = ("row_missing_health_scaling", "Missing Health Scaling");
    pub(super) const MOVEMENT_SPEED: Words = ("row_movement_speed", "Movement Speed");
    pub(super) const MR_PENETRATION: Words = ("row_mr_penetration", "Magic Resistance Penetration");
    pub(super) const MR_REDUCTION: Words = ("row_mr_reduction", "Magic Resistance Reduction");
    pub(super) const OMNIVAMP: Words = ("row_omnivamp", "Omnivamp");
    pub(super) const ON_HIT_AD_RATIO: Words = ("row_on_hit_ad_ratio", "On-Hit Damage AD Ratio");
    pub(super) const ON_HIT_CRIT_RATIO: Words = ("row_on_hit_crit_ratio", "On-Hit Damage Crit Ratio");
    pub(super) const ON_HIT_DAMAGE: Words = ("row_on_hit_damage", "On-Hit Damage");
    pub(super) const ON_HIT_DAMAGE_CAP: Words = ("row_on_hit_damage_cap", "On-Hit Damage Cap");
    pub(super) const OUT_OF_COMBAT_HEALING: Words = ("row_out_of_combat_healing", "Out of Combat Healing");
    pub(super) const OUT_OF_COMBAT_TIMER: Words = ("row_out_of_combat_timer", "Out of Combat Timer");
    pub(super) const PERMANENT_HEALTH: Words = ("row_permanent_health", "Permanent Health");
    pub(super) const RESURRECT_HEALTH: Words = ("row_resurrect_health", "Resurrect Health");
    pub(super) const SCALING: Words = ("row_scaling", "Scaling");
    pub(super) const SHIELDING: Words = ("row_shielding", "Shielding");
    pub(super) const SHIELD_DURATION: Words = ("row_shield_duration", "Shield Duration");
    pub(super) const SKILL_DAMAGE_REDUCTION: Words = ("row_skill_damage_reduction", "Skill Damage Reduction");
    pub(super) const SLOW: Words = ("row_slow", "Slow");
    pub(super) const SLOW_DURATION: Words = ("row_slow_duration", "Slow Duration");
    pub(super) const STACK_DURATION: Words = ("row_stack_duration", "Stack Duration");
    pub(super) const STASIS_DURATION: Words = ("row_stasis_duration", "Stasis Duration");
    pub(super) const STORM_DAMAGE: Words = ("row_storm_damage", "Storm Damage");
    pub(super) const TRUE_DAMAGE_RATIO: Words = ("row_true_damage_ratio", "True Damage Ratio");
}

// The words after a number: "15 seconds".
mod unit {
    use super::Words;

    pub(super) const AD: Words = ("unit_ad", " AD");
    pub(super) const HEALTH: Words = ("unit_health", " health");
    pub(super) const MAX_HP: Words = ("unit_max_hp", " max HP");
    pub(super) const MAX_HP_PER_SECOND: Words = ("unit_max_hp_per_second", " max HP per second");
    pub(super) const OF_DAMAGE_DEALT: Words = ("unit_of_damage_dealt", " of the damage dealt");
    pub(super) const OF_ORIGINAL_AMOUNT: Words = ("unit_of_original_amount", " of the original amount");
    pub(super) const OF_YOUR_DAMAGE_DEALT: Words = ("unit_of_your_damage_dealt", " of your damage dealt");
    pub(super) const PER_MOMENTUM_STACK: Words = ("unit_per_momentum_stack", " per <#92dc7bff>Momentum<> stack");
    pub(super) const PER_SECOND: Words = ("unit_per_second", " per second");
    pub(super) const PER_STACK: Words = ("unit_per_stack", " per stack");
    pub(super) const SECONDS: Words = ("unit_seconds", " seconds");
    pub(super) const SECONDS_PER_TARGET: Words = ("unit_seconds_per_target", " seconds per target");
    pub(super) const TARGET_CURRENT_HP: Words = ("unit_target_current_hp", " target current HP");
    pub(super) const TARGET_MAX_HP: Words = ("unit_target_max_hp", " target max HP");
}

// The words before a number: "up to 30%".
mod lead {
    use super::Words;

    pub(super) const UP_TO: Words = ("lead_up_to", "up to ");
}

// A row of [`ROW_FORMATS`]: `item`'s `field` under `label`, with no words
// round the number until [`RowFormat::unit`] or [`RowFormat::lead`] gives some.
const fn row(item: &'static str, field: &'static str, label: Words) -> RowFormat {
    RowFormat {
        item,
        field,
        label: Some(label),
        lead: None,
        unit: None,
    }
}

impl RowFormat {
    // The words after the number.
    const fn unit(self, unit: Words) -> Self {
        Self {
            unit: Some(unit),
            ..self
        }
    }

    // The words before it.
    const fn lead(self, lead: Words) -> Self {
        Self {
            lead: Some(lead),
            ..self
        }
    }
}

const FIELD_FORMATS: [FieldFormat; 5] = [
    FieldFormat {
        fields: &["effect_lethality", "effect_bonus_lethality"],
        label: label::LETHALITY,
        unit: None,
    },
    FieldFormat {
        fields: &["adaptive_force", "effect_adaptive_force"],
        label: label::ADAPTIVE_FORCE,
        unit: None,
    },
    FieldFormat {
        fields: &["effect_cooldown_seconds"],
        label: label::COOLDOWN,
        unit: Some(unit::SECONDS),
    },
    FieldFormat {
        fields: &["effect_duration_seconds", "effect_shield_seconds"],
        label: label::EFFECT_DURATION,
        unit: Some(unit::SECONDS),
    },
    FieldFormat {
        fields: &["effect_slow_amount"],
        label: label::SLOW,
        unit: None,
    },
];

const ROW_FORMATS: [RowFormat; 123] = [
    row("deaths_dance", "effect_delayed_damage_percent", label::DAMAGE_REDUCTION),
    row("echoes_of_helia", "effect_damage_conversion", label::DAMAGE_STORED),
    row("heartsteel", "effect_bonus_hp_percent_of_damage", label::PERMANENT_HEALTH)
        .unit(unit::OF_DAMAGE_DEALT),
    row("randuins_omen", "effect_crit_damage_percent_heal", label::CRIT_HEALING),
    row("hubris", "effect_stack_attack", label::SCALING).unit(unit::PER_STACK),
    row("zhonyas_hourglass", "effect_duration_seconds", label::STASIS_DURATION).unit(unit::SECONDS),
    row("hubris", "effect_bonus_flat_attack", label::BASE_AD_KILL_BONUS),
    row("cloak_of_starry_night", "effect_skill_damaged_reduce", label::BASE_SKILL_DAMAGE_REDUCTION),
    row("black_cleaver", "effect_percent_armor_shred", label::ARMOR_REDUCTION)
        .unit(unit::PER_STACK),
    row("bloodletters_curse", "effect_percent_mr_shred", label::MR_REDUCTION).unit(unit::PER_STACK),
    row("abyssal_mask", "effect_percent_mr_shred", label::MR_REDUCTION),
    row("frozen_heart", "effect_attack_speed_reduce", label::ATTACK_SPEED_REDUCTION),
    row("executioners_calling", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("mortal_reminder", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("chempunk_chainsword", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("chemtech_putrifier", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("morellonomicon", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("oblivion_orb", "effect_heal_reduce", label::HEALING_REDUCTION),
    row("bloodsong", "effect_damaged_amplify", label::DAMAGE_TAKEN_BONUS),
    row("imperial_mandate", "effect_damaged_amplify", label::DAMAGE_TAKEN_BONUS),
    row("celestial_opposition", "effect_damaged_reduce", label::DAMAGE_REDUCTION),
    row("spirit_visage", "effect_heal_mult", label::BONUS_HEALING_RECEIVED),
    row("lord_dominiks_regards", "effect_max_percent_bonus", label::BONUS_DAMAGE_CAP),
    row("eclipse", "effect_cooldown_seconds", label::COOLDOWN).unit(unit::SECONDS_PER_TARGET),
    row("heartsteel", "effect_cooldown_seconds", label::COOLDOWN).unit(unit::SECONDS_PER_TARGET),
    row("night_harvester", "effect_cooldown_seconds", label::COOLDOWN)
        .unit(unit::SECONDS_PER_TARGET),
    row("stormsurge", "effect_cooldown_seconds", label::COOLDOWN).unit(unit::SECONDS_PER_TARGET),
    row("sundered_sky", "effect_cooldown_seconds", label::COOLDOWN).unit(unit::SECONDS_PER_TARGET),
    row("celestial_opposition", "effect_duration_seconds", label::DAMAGE_REDUCTION_DURATION)
        .unit(unit::SECONDS),
    row("celestial_opposition", "effect_slow_seconds", label::SLOW_DURATION).unit(unit::SECONDS),
    row("eclipse", "effect_duration_seconds", label::MARK_DURATION).unit(unit::SECONDS),
    row("eclipse", "effect_shield_seconds", label::SHIELD_DURATION).unit(unit::SECONDS),
    row("opportunity", "effect_out_of_combat_seconds", label::OUT_OF_COMBAT_TIMER)
        .unit(unit::SECONDS),
    row("opportunity", "effect_duration_seconds", label::LETHALITY_DURATION).unit(unit::SECONDS),
    row("rite_of_ruin", "effect_shield_seconds", label::SHIELD_DURATION).unit(unit::SECONDS),
    row("rite_of_ruin", "effect_duration_seconds", label::STACK_DURATION).unit(unit::SECONDS),
    row("haunting_guise", "effect_out_of_combat_seconds", label::OUT_OF_COMBAT_TIMER)
        .unit(unit::SECONDS),
    row("warmogs_armor", "effect_duration_seconds", label::OUT_OF_COMBAT_TIMER).unit(unit::SECONDS),
    row("stormsurge", "effect_duration_seconds", label::BURST_WINDOW).unit(unit::SECONDS),
    row("grezs_spectral_lantern", "effect_percent_bonus_damage", label::BONUS_MAGIC_DAMAGE),
    row("spirit_stone", "effect_percent_bonus_damage", label::BONUS_MAGIC_DAMAGE),
    row("diamond_tipped_spear", "effect_max_percent_bonus", label::BONUS_DAMAGE).lead(lead::UP_TO),
    row("fiendhunter_bolts", "effect_percent_bonus_damage", label::BONUS_DAMAGE),
    row("haunting_guise", "effect_stack_percent_damage", label::BONUS_DAMAGE).unit(unit::PER_STACK),
    row("infinity_edge", "effect_crit_damage_bonus", label::BONUS_CRIT_DAMAGE),
    row("kraken_slayer", "effect_max_percent_bonus", label::MISSING_HEALTH_SCALING)
        .lead(lead::UP_TO),
    row("lord_dominiks_regards", "effect_percent_bonus_damage", label::BONUS_DAMAGE)
        .unit(unit::PER_STACK),
    row("sundered_sky", "effect_percent_bonus_damage", label::FIRST_ATTACK_MULTIPLIER),
    row("grezs_spectral_lantern", "effect_bonus_hp_percent_of_damage", label::HEALING)
        .unit(unit::OF_YOUR_DAMAGE_DEALT),
    row("spirit_stone", "effect_bonus_hp_percent_of_damage", label::HEALING)
        .unit(unit::OF_YOUR_DAMAGE_DEALT),
    row("feral_flare", "effect_bonus_flat_heal", label::HEALING).unit(unit::HEALTH),
    row("madreds_razors", "effect_bonus_flat_heal", label::HEALING).unit(unit::HEALTH),
    row("guardian_angel", "effect_caster_hp_percent_heal", label::RESURRECT_HEALTH)
        .unit(unit::MAX_HP),
    row("moonstone_renewer", "effect_heal_chain_percent", label::CHAINED_HEALING)
        .unit(unit::OF_ORIGINAL_AMOUNT),
    row("steraks_gage", "effect_caster_hp_percent_shield", label::SHIELDING).unit(unit::MAX_HP),
    row("warmogs_armor", "effect_caster_hp_percent_heal", label::OUT_OF_COMBAT_HEALING)
        .unit(unit::MAX_HP_PER_SECOND),
    row("guinsoos_rageblade", "effect_bonus_magic_damage", label::ON_HIT_DAMAGE),
    row("wits_end", "effect_bonus_magic_damage", label::ON_HIT_DAMAGE),
    row("ravenous_hydra", "effect_ad_percent_damage", label::ON_HIT_DAMAGE).unit(unit::AD),
    row("tiamat", "effect_ad_percent_damage", label::ON_HIT_DAMAGE).unit(unit::AD),
    row("hextech_alternator", "effect_bonus_flat_damage", label::BONUS_DAMAGE),
    row("scouts_slingshot", "effect_bonus_flat_damage", label::BONUS_DAMAGE),
    row("ardent_censer", "effect_attack_speed_mult", label::ATTACK_SPEED),
    row("ardent_censer", "effect_enemy_max_hp_damage", label::ON_HIT_DAMAGE)
        .unit(unit::TARGET_MAX_HP),
    row("blade_of_the_ruined_king", "effect_hp_percent_damage", label::ON_HIT_DAMAGE)
        .unit(unit::TARGET_CURRENT_HP),
    row("dead_mans_plate", "effect_max_bonus_damage", label::ON_HIT_DAMAGE_CAP),
    row("deaths_dance", "effect_burn_hp_percent_cap", label::DELAYED_DAMAGE_CAP)
        .unit(unit::MAX_HP_PER_SECOND),
    row("eclipse", "effect_hp_percent_damage", label::DAMAGE).unit(unit::TARGET_MAX_HP),
    row("essence_reaver", "effect_ad_percent_damage", label::ON_HIT_AD_RATIO),
    row("essence_reaver", "effect_crit_percent_damage", label::ON_HIT_CRIT_RATIO),
    row("fiendhunter_bolts", "effect_damage_conversion", label::TRUE_DAMAGE_RATIO),
    row("hamstringer", "effect_crit_percent_damage", label::BLEED_CRIT_RATIO),
    row("kraken_slayer", "effect_bonus_flat_damage", label::ON_HIT_DAMAGE),
    row("liandrys_torment", "effect_hp_percent_damage", label::BURN_DAMAGE)
        .unit(unit::TARGET_MAX_HP),
    row("night_harvester", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("stormrazor", "effect_bonus_flat_damage", label::ON_HIT_DAMAGE),
    row("stormrazor", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("titanic_hydra", "effect_caster_hp_percent_damage", label::ON_HIT_DAMAGE)
        .unit(unit::MAX_HP),
    row("titanic_hydra", "effect_splash_caster_hp_percent", label::AOE_DAMAGE).unit(unit::MAX_HP),
    row("voltaic_cyclosword", "effect_hp_percent_damage", label::ON_HIT_DAMAGE)
        .unit(unit::TARGET_CURRENT_HP),
    row("zekes_herald", "effect_bonus_magic_damage", label::STORM_DAMAGE).unit(unit::PER_SECOND),
    row("atmas_reckoning", "effect_stack_crit_chance", label::CRIT_CHANCE).unit(unit::PER_STACK),
    row("bandlepipes", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("bandlepipes", "effect_attack_speed_mult", label::ATTACK_SPEED),
    row("blackfire_torch", "effect_stack_magic_power", label::ABILITY_POWER).unit(unit::PER_STACK),
    row("cosmic_drive", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("dead_mans_plate", "effect_move_speed_per_stack", label::MOVEMENT_SPEED)
        .unit(unit::PER_MOMENTUM_STACK),
    row("dream_maker", "effect_skill_damaged_reduce", label::SKILL_DAMAGE_REDUCTION),
    row("endless_hunger", "effect_vamp", label::OMNIVAMP),
    row("experimental_hexplate", "effect_attack_speed_mult", label::ATTACK_SPEED),
    row("experimental_hexplate", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("fiendhunter_bolts", "effect_attack_speed_mult", label::ATTACK_SPEED),
    row("force_of_nature", "effect_bonus_magic_resistance", label::MAGIC_RESISTANCE),
    row("force_of_nature", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("grezs_spectral_lantern", "effect_stack_magic_power", label::ABILITY_POWER)
        .unit(unit::PER_STACK),
    row("guinsoos_rageblade", "effect_stack_attack_speed_mult", label::ATTACK_SPEED)
        .unit(unit::PER_STACK),
    row("hullbreaker", "effect_bonus_defence", label::ARMOR),
    row("hullbreaker", "effect_bonus_magic_resistance", label::MAGIC_RESISTANCE),
    row("jaksho_the_protean", "effect_stack_defence_mult", label::ARMOR).unit(unit::PER_STACK),
    row("jaksho_the_protean", "effect_stack_magic_resistance_mult", label::MAGIC_RESISTANCE)
        .unit(unit::PER_STACK),
    row("locket_of_the_iron_solari", "effect_bonus_defence", label::ARMOR),
    row("locket_of_the_iron_solari", "effect_bonus_magic_resistance", label::MAGIC_RESISTANCE),
    row("locket_of_the_iron_solari", "effect_bonus_hp_regen", label::HEALTH_REGENERATION),
    row("mirage_blade", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("overlords_bloodmail", "effect_caster_hp_percent_attack", label::ATTACK_DAMAGE)
        .unit(unit::MAX_HP),
    row("phage", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("philosophers_stone", "effect_stack_hp_mult", label::BONUS_HEALTH).unit(unit::PER_STACK),
    row("riftmaker", "effect_vamp", label::OMNIVAMP).unit(unit::PER_STACK),
    row("riftmaker", "effect_caster_hp_percent_power", label::ABILITY_POWER).unit(unit::MAX_HP),
    row("rite_of_ruin", "effect_stack_crit_chance", label::CRIT_CHANCE).unit(unit::PER_STACK),
    row("rod_of_ages", "effect_growth_hp", label::MAX_HEALTH).unit(unit::PER_STACK),
    row("rod_of_ages", "effect_growth_magic_power", label::ABILITY_POWER).unit(unit::PER_STACK),
    row("rod_of_ages", "effect_growth_skill_cooldown_mult", label::ABILITY_HASTE)
        .unit(unit::PER_STACK),
    row("seekers_armguard", "effect_stack_defence", label::ARMOR).unit(unit::PER_STACK),
    row("solstice_sleigh", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("spear_of_shojin", "effect_stack_attack_mult", label::ATTACK_DAMAGE).unit(unit::PER_STACK),
    row("staff_of_flowing_water", "effect_magic_power", label::ABILITY_POWER),
    row("staff_of_flowing_water", "effect_skill_cooldown_mult", label::ABILITY_HASTE),
    row("terminus", "effect_armor_pen_per_stack", label::ARMOR_PENETRATION).unit(unit::PER_STACK),
    row("terminus", "effect_magic_pen_per_stack", label::MR_PENETRATION).unit(unit::PER_STACK),
    row("warmogs_armor", "effect_move_speed_mult", label::MOVEMENT_SPEED),
    row("yun_tal_wildarrows", "effect_flurry_attack_speed_mult", label::ATTACK_SPEED),
    row("yun_tal_wildarrows", "effect_stack_crit_chance", label::CRIT_CHANCE).unit(unit::PER_STACK),
];

// A row of a passive's numbers that make one amount, a flat part and its shares
// ("Defy - Healing: 45 + 15% missing HP -> 50 + 15% missing HP"), written whole
// whichever part moved ([`amount_rows`]). `amount` has `{A}`/`{B}`/`{C}` in
// `fields` order, each number's part in square brackets ([`amount_text`]);
// `item` is the base key.
struct AmountFormat {
    item: &'static str,
    fields: &'static [&'static str],
    label: Words,
    amount: (&'static str, &'static str),
}

// The placeholders of an amount's numbers, in the order of its fields.
const AMOUNT_SLOTS: [&str; 3] = ["{A}", "{B}", "{C}"];

const AMOUNT_FORMATS: [AmountFormat; 26] = [
    AmountFormat {
        item: "deaths_dance",
        fields: &["effect_bonus_flat_heal", "effect_kill_heal_missing_percent"],
        label: label::HEALING,
        amount: ("amount_flat_missing_hp", "[{A}] + [{B}% missing HP]"),
    },
    AmountFormat {
        item: "dusk_and_dawn",
        fields: &["effect_caster_ap_percent_heal", "effect_caster_hp_percent_heal"],
        label: label::HEALING,
        amount: ("amount_ap_and_max_hp", "[{A}% AP] and [{B}% max HP]"),
    },
    AmountFormat {
        item: "eclipse",
        fields: &["effect_bonus_flat_shield", "effect_ad_percent_shield"],
        label: label::SHIELDING,
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "sundered_sky",
        fields: &["effect_bonus_flat_heal", "effect_caster_hp_percent_heal"],
        label: label::HEALING,
        amount: ("amount_flat_missing_hp", "[{A}] + [{B}% missing HP]"),
    },
    AmountFormat {
        item: "unending_despair",
        fields: &["effect_bonus_flat_heal", "effect_caster_hp_percent_heal"],
        label: label::HEALING,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "bamis_cinder",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: label::BURN_DAMAGE,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "hollow_radiance",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: label::BURN_DAMAGE,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "philosophers_stone",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: label::BURN_DAMAGE,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "dusk_and_dawn",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "lich_bane",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "bastionbreaker",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "heartsteel",
        fields: &["effect_bonus_flat_damage", "effect_caster_hp_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "hollow_radiance",
        fields: &["effect_explosion_flat_damage", "effect_explosion_caster_hp_percent"],
        label: label::DAMAGE,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
    AmountFormat {
        item: "feral_flare",
        fields: &["effect_bonus_magic_damage", "effect_stack_magic_damage"],
        label: label::DAMAGE,
        amount: ("amount_flat_per_feral_stack", "[{A}] ([+{B}] per <#92dc7bff>Feral<> stack)"),
    },
    AmountFormat {
        item: "hullbreaker",
        fields: &["effect_ad_percent_damage", "effect_caster_hp_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_ad_max_hp", "[{A}% AD] ([+{B}% max HP])"),
    },
    AmountFormat {
        item: "nashors_tooth",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "night_harvester",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: label::DAMAGE,
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "serpents_fang",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: label::BONUS_DAMAGE,
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "stormsurge",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage"],
        label: label::DAMAGE,
        amount: ("amount_flat_ap", "[{A}] + [{B}% AP]"),
    },
    AmountFormat {
        item: "trinity_force",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ad", "[{A}] + [{B}% AD]"),
    },
    AmountFormat {
        item: "terminus",
        fields: &["effect_bonus_flat_damage", "effect_ad_percent_damage", "effect_ap_percent_damage"],
        label: label::ON_HIT_DAMAGE,
        amount: ("amount_flat_ad_ap", "[{A}] ([+{B}% AD]) ([+{C}% AP])"),
    },
    AmountFormat {
        item: "zazzaks_realmspike",
        fields: &["effect_bonus_flat_damage", "effect_ap_percent_damage", "effect_enemy_max_hp_damage"],
        label: label::DAMAGE,
        amount: ("amount_flat_ap_target_max_hp", "[{A}] ([+{B}% AP]) ([+{C}% target max HP])"),
    },
    AmountFormat {
        item: "axiom_arc",
        fields: &["effect_ult_cooldown_mult", "effect_ult_cooldown_per_lethality"],
        label: label::COOLDOWN_REFUND,
        amount: ("amount_percent_per_lethality", "[{A}%] ([+{B}%] per <#ffdd8eff>1 <i#asset/base/ui/banpick/champion_stat_icon:armor_pen_0> Lethality<>)"),
    },
    AmountFormat {
        item: "endless_hunger",
        fields: &["effect_skill_cooldown_mult", "effect_ad_percent_haste"],
        label: label::ABILITY_HASTE,
        amount: ("amount_flat_ad_bracket", "[{A}] ([+{B}% AD])"),
    },
    AmountFormat {
        item: "protectors_vow",
        fields: &["effect_bonus_flat_hp", "effect_caster_defence_percent_hp"],
        label: label::BONUS_HEALTH,
        amount: ("amount_flat_armor", "[{A}] + [{B}% Armor]"),
    },
    AmountFormat {
        item: "protoplasm_harness",
        fields: &["effect_bonus_flat_hp", "effect_hp_percent_boost"],
        label: label::BONUS_HEALTH,
        amount: ("amount_flat_max_hp", "[{A}] + [{B}% max HP]"),
    },
];

// The amount `pattern` makes of `numbers` ([`AMOUNT_SLOTS`]): bracketed parts
// are drawn as the item's text draws them ([`Tint::draw`]), and an icon shared
// by two adjacent parts is put in once.
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

// The rows of the numbers `fields` that a note writes as one amount: a row a
// tier, the whole amount before and after, unmoved numbers read from `state`
// (before the patch). `None` where a tier has only some of them or no amount
// moved; they then get rows of their own.
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

// The rows of a level-scaling range, `low` to `high`, under the passive's name
// ("40 - 100 (based on level) -> 45 - 110 (based on level)"), whichever end
// moved ([`amount_rows`]); each end in its colour, behind the amount's icon
// once where both share it.
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

// The number whose row `key`'s `field` is written in: a range's low end, an
// amount's first number, else itself. Rows are counted the way they are
// written, for `max_changes_per_item`.
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

// A patch's items, an entry each: the buffs, then the nerfs, in the patch's
// order, a legendary and its radiant as separate entries. A number only brought
// level with the other tier's ([`Change::leader`]) is listed in that tier's
// entry ([`NoteEntry::followers`]) and does not decide its section.
//
// Each row is one changed number, flat stats first, then passives' numbers
// worded by [`ROW_FORMATS`] or [`FIELD_FORMATS`]; a level range
// ([`range_rows`]) and an amount ([`AMOUNT_FORMATS`]) are one row each.
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
        // A passive's number is called by its passive's name and what it does,
        // as the item's row format or else its field's format words it; with
        // neither, it is the bare number under the name.
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
        // A formatted passive number is drawn in its text's colour and behind
        // its icon; flat stats and unformatted numbers as they are.
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

// `text` as a line of one size: each run between style tags written whole as
// `<s#size><f#font><#colour>run<><f><s>`, icons included. One size span around
// the line fails, since the game draws whatever follows a closed colour at the
// label's own size.
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

// The news article for one patch: title, body, author, in the game's language
// when it lands. `state` is the balance before the patch, for numbers it did
// not move.
//
// The body uses the game's markup (`<#rrggbbaa>..<>`, `<f#asset>..<f>`,
// `<s#n>..<s>`, `<i#sheet:tag>`): Buffs, Adjustments and Nerfs headings, then
// per item its icon and name, a reason and its rows, with the other tier's
// levelled numbers last under that tier's name. An item is a buff where every
// number got stronger, a nerf where every one got weaker, else an adjustment.
//
// One article however long, scrolled by the mod's
// `news_component/simple_content.ui` override and [`sync_article_scroll`].
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

// What the window calls `key`'s `field`, and whether it is a percentage, as a
// patch note's row would ("Attack Damage", "Flux - Ultimate Haste"). An
// amount's numbers and a range's ends share words, told apart by their config
// field; empty where nothing names it.
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

// A plain article on the news screen (`news_component/simple_content`,
// mounted under its root's name): its date and its text.
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

// A height set that did not take, with the heights the nodes had instead: it is
// not set again, so a fix for the scroll cannot become what stops it.
static SCROLL_TRIED: Mutex<Option<(f32, f32, f32)>> = Mutex::new(None);

// What the scroll was last worked out from (the time shown, the row clicked)
// and the result ([`scroll_for`]), since finding the text can mean reading
// every inbox row.
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

// How tall an article's text draws: the layout's line height per line, wrapped
// lines included. Worked out from the text since the host does not measure it;
// the width guess runs a little wide, and only matters for the game's long
// articles, since nothing the mod writes wraps.
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

// Makes the open plain article scroll exactly as far as its text; every frame,
// one failed lookup off the news screen. The mod's layout override makes the
// body a scroll view whose length is a `#dummy` node's height and whose label
// box must be as long as the scroll or it is culled; both are set from the text
// ([`scroll_for`]), and kept at the layout's length where the text cannot be
// had.
pub(crate) fn sync_article_scroll(ctx: &mut StableClient<'_>) {
    if !listen_to_rows(ctx) {
        return;
    }
    // A plain article in the mod's layout is up: its length node is there.
    let Some((_, _, _, length)) = ctx.ui_node_rect(ARTICLE_EXTENT) else {
        return;
    };
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

// The scroll length for the open article: its text's height, never under the
// view. In the mod's layout the article's title and text do not read as
// labels, only its time does, so the article is the one whose inbox row was
// clicked last ([`on_news_row`]) if that row shows the open article's time,
// else the longest of the articles whose rows show that time. A game article
// (no plain text in the record) has no scroll; nothing found keeps the
// layout's length.
fn scroll_for(ctx: &StableClient<'_>) -> Option<f32> {
    let time = label_text(ctx, ARTICLE_TIME);
    let clicked = CLICKED.lock().ok().and_then(|clicked| *clicked);
    // What the answer was worked out from: while it is the same, so is the
    // answer.
    let asked = format!("{}\n{clicked:?}", time.as_deref().unwrap_or_default());
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
    let (length, from) = match &time {
        Some(time) => {
            // The clicked row, unless it shows another time: then the article
            // was opened some other way, and the rows at its time stand in.
            let rows = match clicked
                .filter(|&row| row_time(ctx, row).map_or(true, |shown| same_time(&shown, time)))
            {
                Some(row) => vec![row],
                None => rows_at(ctx, time),
            };
            let texts: Vec<Option<Option<String>>> =
                rows.iter().map(|&row| inbox_article(ctx, row)).collect();
            let longest = texts
                .iter()
                .flatten()
                .flatten()
                .map(|text| length_of(text.as_str()))
                .fold(None, |most: Option<f32>, length| Some(most.map_or(length, |most| most.max(length))));
            let from = format!("rows {rows:?} (clicked {clicked:?})");
            match longest {
                Some(length) => (Some(length), from),
                // Every one of them read, and none is plain text: a game
                // article, which does not scroll.
                None if !texts.is_empty() && texts.iter().all(Option::is_some) => {
                    (Some(ARTICLE_VIEW_H), from)
                }
                None => (None, from),
            }
        }
        None => (None, "nothing: the article's time cannot be read".to_string()),
    };
    super::log("patch.scroll.for", || {
        format!("time {time:?}: {length:?}px, from {from}")
    });
    if let Ok(mut known) = SCROLL_FOR.lock() {
        *known = Some((asked, length));
    }
    length
}

// The plain text of the article at `index` in the team's `news`: `Some(None)`
// for another kind of article, `None` where it could not be read.
fn inbox_article(ctx: &StableClient<'_>, index: usize) -> Option<Option<String>> {
    let team = ctx.player_team_id()?;
    let article = ctx.record_get_json(RecordKindV1::Team, team, &format!("news.{index}"))?;
    let article: Value = serde_json::from_str(&article).ok()?;
    Some(
        article
            .pointer("/ty/Simple/content")
            .and_then(Value::as_str)
            .map(str::to_string),
    )
}

// -- which article is open ---------------------------------------------------------

// The inbox list: a row per article, named by the article's place in the
// team's `news`, among date headers (`h_YYYYMMDD`).
const NEWS_ROWS: &str = "main.top.right.news.background.list.contents";

// Frames between two looks over the list for rows not yet listened to, where
// its length has not changed (another category shows as many).
const ROWS_LOOK_EVERY: u32 = 60;

// The inbox rows listened to ([`on_news_row`]).
static ROWS: crate::ui::Registered = crate::ui::Registered::new(on_news_row);

// The list's length when last looked over, and the frames since.
static ROWS_LOOKED: Mutex<Option<(usize, u32)>> = Mutex::new(None);

// The article whose inbox row was clicked last, by its place in the team's
// `news`; forgotten when the news screen closes.
static CLICKED: Mutex<Option<usize>> = Mutex::new(None);

// Listens for clicks on the inbox's rows, the only sure way to know which
// article is open. Whether the news screen is up: off it, everything here is
// forgotten.
fn listen_to_rows(ctx: &mut StableClient<'_>) -> bool {
    let Ok(mut looked) = ROWS_LOOKED.lock() else {
        return false;
    };
    let Some(count) = ctx.ui_child_count(NEWS_ROWS) else {
        if looked.take().is_some() {
            ROWS.forget();
            if let Ok(mut clicked) = CLICKED.lock() {
                *clicked = None;
            }
        }
        return false;
    };
    let due = match looked.as_mut() {
        Some((known, frames)) if *known == count && *frames < ROWS_LOOK_EVERY => {
            *frames += 1;
            false
        }
        _ => true,
    };
    if due {
        *looked = Some((count, 0));
        for name in ctx.ui_child_names(NEWS_ROWS) {
            if name.parse::<usize>().is_ok() {
                ROWS.register(ctx, &format!("{NEWS_ROWS}.{name}"));
            }
        }
    }
    true
}

// A click on an inbox row: that row's article is the one opening.
fn on_news_row(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    // An unreported kind counts as a click, as in `crate::ui`.
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    let Some(row) = event
        .path
        .strip_prefix(NEWS_ROWS)
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|name| name.parse::<usize>().ok())
    else {
        return;
    };
    if let Ok(mut clicked) = CLICKED.lock() {
        *clicked = Some(row);
    }
    super::log("patch.scroll.row", || format!("row {row} clicked"));
}

// The time an inbox row shows: its `time` label, a level or two down.
fn row_time(ctx: &StableClient<'_>, row: usize) -> Option<String> {
    let mut paths = vec![format!("{NEWS_ROWS}.{row}")];
    for _ in 0..3 {
        let mut next = Vec::new();
        for path in paths {
            for child in ctx.ui_child_names(&path) {
                let child_path = format!("{path}.{child}");
                if child == "time" {
                    return label_text(ctx, &child_path);
                }
                next.push(child_path);
            }
        }
        paths = next;
    }
    None
}

// The inbox rows showing `time`.
fn rows_at(ctx: &StableClient<'_>, time: &str) -> Vec<usize> {
    ctx.ui_child_names(NEWS_ROWS)
        .iter()
        .filter_map(|name| name.parse::<usize>().ok())
        .filter(|&row| row_time(ctx, row).is_some_and(|shown| same_time(&shown, time)))
        .collect()
}

// Whether two times shown are one: the same, or one the end of the other,
// should the article and its row write it at different lengths.
fn same_time(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty() && !b.is_empty() && (a == b || a.ends_with(b) || b.ends_with(a))
}
