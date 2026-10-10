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
//! which `apply_config.ps1` wrote with the numbers already in the sentences.
//! Neither follows a patch, and the stable API can change neither, so the
//! text a tooltip is showing is corrected where it is shown: the label is
//! read, the unpatched stat lines and effect text are found in it, and it is
//! written back with the patched ones in their place ([`sync_tooltips`]).
//! Anything not found is left as it is, so a tooltip in a shape this does
//! not know shows the unpatched text rather than a broken one.
//!
//! # Where the patched sentences come from
//!
//! `item-templates.json`, beside the DLL: every item's effect text in every
//! language, with a slot naming the config field where each number goes. It
//! is made from `apply_config.ps1` itself by `tools/item_templates.py`, which
//! also proves it: filled with the numbers of `config-default.json` it gives
//! `text/item.i18n` back, letter for letter. The same check is run here at
//! start-up against the files as the player has them ([`Texts::unverified`]),
//! and an item that fails it is not patched in any number its text shows.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use mod_api_stable::*;
use serde_json::Value;

use super::state::{Change, Patch};

/// One config number in a slot, as the generator reads it.
pub(crate) struct Term {
    pub item: String,
    pub field: String,
    /// Read as `[int]`: the sentence shows a whole number.
    pub whole: bool,
}

enum Part {
    Text(String),
    /// One number, or the product of several (Haunting Guise's total is its
    /// share a stack times its stacks).
    Slot(Vec<Term>),
}

/// An item's effect text with its numbers still to be written in.
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

    /// The text with every number in, `value` giving each by config item and
    /// field. Written as the generator writes them: see [`shown`].
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

    /// What to call `item`'s `field` in a patch note, and whether the
    /// sentence puts a percent sign after the number. A passive's text opens
    /// with its name and a colon, in every language, and that name is it.
    /// A sentence with no name (the lethality an item gives is one of its
    /// own, "Gain 18 Lethality.") is its own label, less the number.
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
        // The paragraph the number is in, and no further: the first run of
        // this took the next paragraph's passive for the name of a sentence
        // that had none ("Collector, Gain 0 Lethality. Death: 10 -> 9").
        let start = text[..at].rfind("\n\n").map_or(0, |gap| gap + 2);
        let end = text[at..].find("\n\n").map_or(text.len(), |gap| at + gap);
        let before = plain(&text[start..at]);
        let label = match before.split_once([':', '\u{ff1a}']) {
            Some((name, _)) => name.trim().to_string(),
            None => {
                // `at` is where the number's one-letter stand-in sits.
                let after = plain(&text[at + 1..end]);
                let sentence = format!("{before} {after}");
                let words: Vec<&str> = sentence.split_whitespace().collect();
                words
                    .join(" ")
                    .trim_end_matches(['.', '\u{3002}'])
                    .to_string()
            }
        };
        let fits = !label.is_empty() && label.chars().count() <= 40;
        fits.then_some((label, percent))
    }

    /// The stat icon the text puts by `item`'s `field`, as (sheet, tag): the
    /// symbol of what the number is an amount of.
    ///
    /// Told by colour, which is how the sentences themselves pair a number
    /// with its stat. Axiom Arc's Flux reads "Gain 10 (+0.2 per 1 [armour
    /// penetration] Lethality) [haste] Ultimate Ability Haste": the icon
    /// nearest the 0.2 is lethality's and the right one is haste's, and what
    /// says so is that the 0.2 and "Ultimate Ability Haste" are the same
    /// blue (the user, 2026-10-10: "for axiom, that would be the haste symbol
    /// for flux"). So: the number's colour is the span it is in, or for a
    /// number in none the span that closed last before it ("grants 15 (+2
    /// per stack) bonus Attack Damage"); an icon's colour is the span it is
    /// in, or the span that opens straight after it; and the icon is the
    /// nearest one of the number's colour after it in its paragraph, else
    /// the nearest before. A number the text gives no symbol (a duration, a
    /// slow, plain damage) has none here either: a wrong icon says more than
    /// a missing one.
    ///
    /// Meant for the English text, which the rest are written from: a
    /// translation moves words and colours about (the same rule gives Flux
    /// lethality's icon in five of the six).
    fn icon_of(&self, item: &str, field: &str) -> Option<(String, String)> {
        // An icon: where it comes among the icons and numbers, what it
        // is, its colour.
        type Icon = (usize, String, Option<String>);
        fn pick(icons: &[Icon], slot: &(usize, Option<String>)) -> Option<(String, String)> {
            let colour = slot.1.as_ref()?;
            let same = |icon: &&Icon| icon.2.as_ref() == Some(colour);
            let icon = icons
                .iter()
                .filter(same)
                .find(|icon| icon.0 > slot.0)
                .or_else(|| icons.iter().rev().filter(same).find(|icon| icon.0 < slot.0))?;
            let (sheet, tag) = icon.1.rsplit_once(':')?;
            Some((sheet.to_string(), tag.to_string()))
        }

        let mut icons: Vec<Icon> = Vec::new();
        let mut slot: Option<(usize, Option<String>)> = None;
        let mut order = 0usize;
        // The colour spans open, and the one that closed last.
        let mut colours: Vec<String> = Vec::new();
        let mut closed: Option<String> = None;
        // Nothing but blanks since the last icon.
        let mut bare = false;
        for part in &self.parts {
            let text = match part {
                Part::Text(text) => text,
                Part::Slot(terms) => {
                    let wanted = terms.iter().any(|term| term.item == item && term.field == field);
                    if wanted && slot.is_none() {
                        slot = Some((order, colours.last().cloned().or_else(|| closed.clone())));
                    }
                    order += 1;
                    bare = false;
                    continue;
                }
            };
            let mut rest = text.as_str();
            while let Some(at) = rest.find(['<', '\n']) {
                if !rest[..at].trim().is_empty() {
                    bare = false;
                }
                let tail = &rest[at..];
                if tail.starts_with("\n\n") {
                    // The paragraph is over, and the number's with it.
                    if let Some(slot) = &slot {
                        return pick(&icons, slot);
                    }
                    icons.clear();
                    colours.clear();
                    closed = None;
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
                    if let Some(colour) = colours.pop() {
                        closed = Some(colour);
                    }
                    bare = false;
                } else if let Some(icon) = tag.strip_prefix("i#") {
                    icons.push((order, icon.to_string(), colours.last().cloned()));
                    order += 1;
                    bare = true;
                } else if let Some(colour) = tag.strip_prefix('#') {
                    let colour = colour.to_ascii_lowercase();
                    if let Some(icon) = icons.last_mut().filter(|icon| bare && icon.2.is_none()) {
                        icon.2 = Some(colour.clone());
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
        pick(&icons, slot.as_ref()?)
    }
}

/// `text` without its markup: colour spans and inline icons are `<...>`.
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

/// A slot's number as `apply_config.ps1` writes it into a sentence, which is
/// how PowerShell turns a number into text: `[int]` rounds a half to the even
/// number, and a `[double]` is written to fifteen significant digits.
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

/// A number to fifteen significant digits, with no trailing zeros.
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

/// Where one config number is written.
#[derive(Default)]
pub(crate) struct Shown {
    /// The items whose effect text has it, by text key. Usually the item
    /// itself; a few sentences quote another item's number.
    pub texts: Vec<String>,
    /// Some sentence writes it as a whole number, so it has to stay one.
    pub whole: bool,
}

/// Everything read from the mod's text files, once.
pub(crate) struct Texts {
    /// Language -> item key -> effect text, as the game was given it.
    options: HashMap<String, HashMap<String, String>>,
    /// Language -> item key -> name.
    names: HashMap<String, HashMap<String, String>>,
    /// Language -> stat field -> its tooltip line, `{Value}` in it. The
    /// mod's languages, then the base game's own for the rest.
    spec: Vec<(String, HashMap<String, String>)>,
    /// Language -> item key -> template.
    templates: HashMap<String, HashMap<String, Template>>,
    /// An item's name in any language -> its key.
    name_keys: HashMap<String, String>,
    shown: HashMap<(String, String), Shown>,
    /// Language -> the mod's own patch-note strings (`text/ui.i18n`,
    /// `item_patch`).
    ui: HashMap<String, HashMap<String, String>>,
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

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

        if let Some(file) = read_json(&dir.join("item-templates.json")) {
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

    /// Where `item`'s `field` is written, if any effect text has it.
    pub(crate) fn shown(&self, item: &str, field: &str) -> Option<&Shown> {
        self.shown.get(&(item.to_string(), field.to_string()))
    }

    /// Whether the effect text of `key` spells out a number of `field`,
    /// whichever config entry it takes it from.
    pub(crate) fn quotes(&self, key: &str, field: &str) -> bool {
        self.templates.values().any(|templates| {
            templates
                .get(key)
                .is_some_and(|template| template.terms().any(|term| term.field == field))
        })
    }

    /// Whether a tooltip has a stat line for `field`.
    pub(crate) fn has_line(&self, field: &str) -> bool {
        self.spec
            .iter()
            .any(|(_, lines)| lines.get(field).is_some_and(|line| line.contains("{Value}")))
    }

    /// The items whose effect text, in some language, is not what its
    /// template makes of the numbers `value` gives: a text file from another
    /// version than the templates, or a `config.json` that `apply_config`
    /// was not run for. What a patch would write into such a tooltip cannot
    /// be trusted to be the sentence that is in it.
    pub(crate) fn unverified(&self, value: &dyn Fn(&str, &str) -> f64) -> HashSet<String> {
        let mut out = HashSet::new();
        for (lang, templates) in &self.templates {
            let options = self.options.get(lang);
            for (key, template) in templates {
                let have = options.and_then(|options| options.get(key));
                if have.map(String::as_str) != Some(template.fill(value).as_str()) {
                    out.insert(key.clone());
                }
            }
        }
        out
    }

    /// The effect text of `key` in every language it has a template in: the
    /// one the game holds, and the one `value`'s numbers make.
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

    /// The key of the item a tooltip's name label is showing: the name in
    /// whatever language, or the reference a label was handed to resolve.
    fn key_of_name(&self, label: &str) -> Option<&str> {
        if let Some(key) = label
            .strip_prefix("#asset/base/text/item?")
            .and_then(|path| path.strip_suffix(".name"))
        {
            return self.name_keys.values().find(|known| known.as_str() == key).map(String::as_str);
        }
        self.name_keys.get(label.trim()).map(String::as_str)
    }

    /// An item's name in `lang`, in English where that language has none.
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

    /// The stat line of `field` in `lang` (English failing that), as a label
    /// for a patch note: its words, and whether it reads as a percentage.
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
    /// The icon a stat's tooltip line opens with, as (sheet, tag): the line
    /// is `<i#sheet:tag> {Value} words`.
    fn line_icon(&self, field: &str) -> Option<(String, String)> {
        let line = self
            .spec
            .iter()
            .find_map(|(_, lines)| lines.get(field).filter(|line| line.contains("<i#")))?;
        let inside = line.split_once("<i#")?.1.split_once('>')?.0;
        let (sheet, tag) = inside.rsplit_once(':')?;
        Some((sheet.to_string(), tag.to_string()))
    }

    /// The icon of what a passive's number is an amount of: the one the
    /// item's effect text puts by it ([`Template::icon_of`]), read off the
    /// English text whatever the game's language. None for a number the
    /// text gives no symbol, and for the game's own items, whose texts the
    /// mod has no templates of.
    fn passive_icon(&self, key: &str, field: &str) -> Option<(String, String)> {
        self.templates.get("en")?.get(key)?.icon_of(key, field)
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

/// The game's language, from its own settings file (`config/game/base.json`
/// beside the executable, `lang`). The stable API does not say, and answers
/// every text lookup in English. Read again every few seconds: the player
/// can change it in the options.
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

/// What a tooltip of one item has to say that the game will not: built by
/// [`super::live`] whenever the patch state changes.
#[derive(Default)]
pub(crate) struct Display {
    /// (language, the effect text the game holds, the patched one).
    pub options: Vec<(String, String, String)>,
    /// (stat field, the number the game's stat line shows, the patched one).
    /// Empty for the game's own items, whose stat lines follow their data.
    pub flats: Vec<(String, i64, i64)>,
}

impl Display {
    /// `desc` with this item's patched numbers in, or nothing where it holds
    /// none of the text they would replace.
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

/// The patched effect text of `key` in the game's language, for a tooltip
/// the mod draws itself. Nothing where the patch leaves that text alone.
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

/// One place the game shows an item's description.
struct Site {
    /// The node game code shows and hides.
    root: String,
    name: String,
    desc: String,
    /// What was last written to `desc`: while the label still says it,
    /// there is nothing to do.
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

/// Frames between two looks for tooltips not known yet.
const DISCOVER_FRAMES: u32 = 45;

/// The hover tooltip of an item slot, which three screens have under their
/// own root with the same nodes inside: in a match, on the match result
/// (whose layout's root is `main` too), and on the solo rank page.
const TOOLTIP: &str = "item_tooltip";
/// The description panel of the Item Info page (`item_info.ui`), under Game
/// Info and in the strategy screen's Item Info popup. Inside the page's
/// `#data`: the page is `main.top.right.item_info` (seen in the test log,
/// 2026-10-10, where looking for the panel straight under it found nothing).
const DETAIL: &str = "data.item_detail";

fn site_paths(root: String, inner: &str) -> (String, String, String) {
    let name = format!("{root}.{inner}name");
    let desc = format!("{root}.{inner}desc");
    (root, name, desc)
}

/// Finds the description labels now on screen. The two roots a layout puts
/// its tooltip under are known; where a screen is mounted inside the
/// management UI is game code's doing, so those are looked for one level
/// down from the places screens are put.
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

/// Puts the patched numbers into whatever item description the game is
/// showing. Every client frame; one atomic read while no patch shows
/// anything.
///
/// The label is written again whenever it stops saying what was written,
/// which covers both ways game code may fill it: once when the hover
/// starts, or every frame.
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

/// A number as a note shows it.
fn note_number(value: f64, percent: bool) -> String {
    format!("{}{}", number_text(value), if percent { "%" } else { "" })
}

/// What a note calls the number a change moved, and whether it reads as a
/// percentage: a flat stat by its tooltip line, a passive's number by the
/// passive's name. Empty where neither is found.
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

/// The sheet the item icons are in (the mod's own, by `mod.override_info`).
const ITEM_SHEET: &str = "asset/base/aseprite_resources/ingame/item_icons_18x18";
/// The arrow the game's own patch notes put between two numbers, in the
/// font they take it from (`text/news`, `patch.stat`).
const ARROW: &str = "<f#asset/base/font/set/symbol>\u{2192}<f>";
const BOLD: &str = "asset/base/font/set/bold";
/// The green of the game's "Buffs" mark (`ui/icons/up_patch`), a red for
/// "Nerfs", and the grey its notes give a reason in (`patch_reason_row`).
const BUFF_COLOR: &str = "#4ed5bdff";
const NERF_COLOR: &str = "#ff6b6bff";
const REASON_COLOR: &str = "#8b8d9aff";

/// One changed number of an entry: its icon, where it is a stat with one or
/// a passive's number whose text gives it one, and the line.
struct NoteRow {
    icon: Option<(String, String)>,
    text: String,
}

/// One item of a patch's buffs or nerfs: the legendary and its radiant
/// together, under the legendary's name and icon.
struct NoteEntry {
    family: String,
    /// The item it is drawn and named as: the legendary.
    key: String,
    /// Why the item was touched, as a designer would put it: see
    /// [`reason_pool`]. Empty where the text file has no such sentence.
    reason: String,
    rows: Vec<NoteRow>,
}

/// The tag an item draws in the item sheet: one of the mod's is its own key,
/// one of the game's has it in the settings file.
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

/// A holder's win share from which an item counts as winning, and under
/// which as losing; and holders a match from which it counts as popular.
const WINNING: f64 = 0.6;
const LOSING: f64 = 0.45;
const OFTEN_HELD: f64 = 0.8;

/// The reasons that fit one item's change, as keys of the `item_patch`
/// strings in `text/ui.i18n` (`why_nerf_early_1`, ...).
///
/// A reason is a designer's sentence, in the voice of the game's own
/// champion notes and of League's patch notes, and not the item's record
/// read out (the user, 2026-10-10, whose rule it also is that an item that
/// stacks is talked of as an early-game item and one that scales as a
/// late-game one; no sentence calls an item oppressive, a word they gave
/// as an example and then asked to have taken out). What picks the sentences is
/// still the record: an item nobody held gets one about being left on the
/// shelf, one that won and was held a lot gets one about the meta, and so
/// on; and the item's kind adds the early- or late-game ones.
fn reason_pool(
    buff: bool,
    tempo: super::base::Tempo,
    change: &Change,
    matches: u32,
) -> Vec<&'static str> {
    use super::base::Tempo;
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
        if change.games == 0 {
            pool.extend(["why_buff_unbuilt_1", "why_buff_unbuilt_2", "why_buff_unbuilt_3"]);
        }
        match tempo {
            Tempo::Early => pool.extend(["why_buff_early_1", "why_buff_early_2"]),
            Tempo::Late => pool.extend(["why_buff_late_1", "why_buff_late_2"]),
            Tempo::Neither => {}
        }
        if change.games > 0 {
            if share <= LOSING {
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
        pool.extend(match (share >= WINNING, held >= OFTEN_HELD) {
            (true, true) => ["why_nerf_meta_1", "why_nerf_meta_2"],
            (false, true) => ["why_nerf_popular_1", "why_nerf_popular_2"],
            _ => ["why_nerf_strong_1", "why_nerf_strong_2"],
        });
    }
    pool
}

/// A number from a patch and an item, to start a pick at.
fn seed_of(number: u32, text: &str) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(number);
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash >> 16) as usize
}

/// A patch's buffs or its nerfs, an entry an item, in the order the patch
/// has them. A row is one changed number in one tier: the legendary's reads
/// "Attack Damage: 20 -> 21", the radiant's "Attack Damage (Radiant): 33 ->
/// 34", the legendary's first.
fn note_entries(patch: &Patch, lang: &str, buff: bool) -> Vec<NoteEntry> {
    let texts = texts();
    let base = super::base::base();
    let mut entries: Vec<NoteEntry> = Vec::new();
    // Reasons given so far, so that no two items share one while another
    // fits.
    let mut used: Vec<&'static str> = Vec::new();
    for change in patch.changes.iter().filter(|change| change.buff == buff) {
        let Some(first) = change.members.first() else {
            continue;
        };
        let lead = base
            .families
            .get(&change.family)
            .and_then(|family| family.members.first())
            .map_or_else(|| first.key.clone(), |member| member.key.clone());
        let at = match entries.iter().position(|entry| entry.family == change.family) {
            Some(at) => at,
            None => {
                // One of the sentences that fit the item, a different one
                // for every item of the article as far as the pool goes, and
                // the same one whenever this patch's notes are written.
                let pool = reason_pool(buff, base.tempo(&lead), change, patch.matches);
                let from = seed_of(patch.number, &change.family) % pool.len().max(1);
                let key = (0..pool.len())
                    .map(|step| pool[(from + step) % pool.len()])
                    .find(|key| !used.contains(key))
                    .or_else(|| pool.get(from).copied());
                let reason = key.map_or_else(String::new, |key| {
                    used.push(key);
                    texts.ui(lang, key, "")
                });
                entries.push(NoteEntry {
                    family: change.family.clone(),
                    key: lead.clone(),
                    reason,
                    rows: Vec::new(),
                });
                entries.len() - 1
            }
        };
        let (label, percent) = label_of(texts, lang, change);
        // A stat's own icon; a passive's number has the icon of what it is
        // an amount of (the user, 2026-10-10: lethality armour penetration's,
        // Axiom Arc's Flux haste's), which is the one its text puts by it.
        let icon = if super::fields::FLAT.contains(&change.field.as_str()) {
            texts.line_icon(&change.field)
        } else {
            None
        }
        .or_else(|| texts.passive_icon(&first.key, &change.field));
        for moved in &change.members {
            let numbers = format!(
                "{} {ARROW} {}",
                note_number(moved.before, percent),
                note_number(moved.after, percent)
            );
            // A tier past the first is told apart by what its name has that
            // the first's does not: "Radiant", in whatever language.
            let tier = if moved.key == lead {
                String::new()
            } else {
                let name = texts.name(lang, &moved.key);
                let word = name.replace(&texts.name(lang, &lead), "").trim().to_string();
                if word.is_empty() {
                    name
                } else {
                    word
                }
            };
            let text = match (label.is_empty(), tier.is_empty()) {
                (true, true) => numbers,
                (true, false) => format!("{tier}: {numbers}"),
                (false, true) => format!("{label}: {numbers}"),
                (false, false) => format!("{label} ({tier}): {numbers}"),
            };
            entries[at].rows.push(NoteRow {
                icon: icon.clone(),
                text,
            });
        }
    }
    entries
}

/// The news article for one patch: title, body, author. In the game's
/// language as it is when the patch lands; an article is text once written.
///
/// The body is formatted in the markup the game's own text uses (a colour
/// `<#rrggbbaa>..<>`, a font `<f#asset>..<f>`, a size `<s#n>..<s>`, an inline
/// icon `<i#sheet:tag>`). The layout is the user's (2026-10-10): the buffs
/// and then the nerfs under a heading in their colour, and for each item
///
/// ```text
/// <icon> Trinity Force
/// <reason>
///     <stat icon> Attack Damage: 20 -> 21
///     <stat icon> Attack Damage (Radiant): 33 -> 34
/// ```
///
/// An item is one entry, its radiant's numbers in it and marked as such.
///
/// One article and one column, however long the patch: also the user's
/// choice, over an article cut into several and over the game's two columns
/// drawn on top of this text, both of which were tried that night. A long
/// one is read by scrolling, which a plain article has by the mod's override
/// of its layout (`ui/layout/news_component/simple_content.ui`) and
/// [`sync_article_scroll`].
pub(crate) fn news(patch: &Patch) -> (String, String, String) {
    let texts = texts();
    let lang = game_language();
    // Named as the game names its own, "Patch Notes v…", after the version
    // the game was on when it landed; a patch set off by hand is a numbered
    // hotfix of that version (the user's titles, 2026-10-10).
    let mut title = texts
        .ui(&lang, "title", "Item Patch Notes v{Version}")
        .replace("{Version}", &patch.announced);
    if patch.hotfix > 0 {
        title = texts
            .ui(&lang, "hotfix", "{Title} - Hotfix {N}")
            .replace("{Title}", &title)
            .replace("{N}", &patch.hotfix.to_string());
    }
    let mut body = texts
        .ui(&lang, "intro", "Items were rebalanced after {Matches} matches on v{Version}.")
        .replace("{Matches}", &patch.matches.to_string())
        .replace("{Version}", &patch.version);
    for (buff, heading, fallback, color) in [
        (true, "buffed", "Buffs", BUFF_COLOR),
        (false, "nerfed", "Nerfs", NERF_COLOR),
    ] {
        let entries = note_entries(patch, &lang, buff);
        if entries.is_empty() {
            continue;
        }
        body.push_str(&format!(
            "\n<s#24><f#{BOLD}><{color}>{}<><f><s>",
            texts.ui(&lang, heading, fallback)
        ));
        for entry in &entries {
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
            for row in &entry.rows {
                let mark = match &row.icon {
                    Some((sheet, tag)) => format!("<i#{sheet}:{tag}>"),
                    None => "-".to_string(),
                };
                body.push_str(&format!("\n<s#18>        {mark} {}<s>", row.text));
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

// -- a plain article's scroll ---------------------------------------------------

/// The news screen's article area, and a plain article in it
/// (`news_component/simple_content`, mounted under its root's name).
const ARTICLE_AREA: &str = "main.top.right.news.contents";
const ARTICLE_BODY: &str = "main.top.right.news.contents.simple_container.contents";
const ARTICLE_TEXT: &str = "main.top.right.news.contents.simple_container.contents.text";
/// The node whose height is how far the body scrolls.
const ARTICLE_EXTENT: &str = "main.top.right.news.contents.simple_container.contents.dummy";
/// The body's height on screen, which is the least its scroll is long.
const ARTICLE_VIEW_H: f32 = 774.0;
/// The text as the layout authors it: how wide, how high a line, and the
/// size it is written in where the text names no other.
const ARTICLE_TEXT_W: f32 = 1043.0;
const ARTICLE_LINE_H: f32 = 36.0;
const ARTICLE_TEXT_SIZE: f32 = 20.0;
/// Room under the last line: a line.
const ARTICLE_FOOT: f32 = 36.0;

/// A height set that the nodes then did not have, with the heights they had
/// instead (the scroll's, the text box's): the same is not set a second
/// time. Setting a height every frame that never takes is how a fix for the
/// scroll would become what stops it.
static SCROLL_TRIED: Mutex<Option<(f32, f32, f32)>> = Mutex::new(None);

/// Whether a character is drawn a full size wide (Chinese, Japanese, Korean
/// and the full-width forms) and not about half of one.
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

/// How tall an article's text is drawn: a line of the layout's height for
/// every line of the text, and for a line wider than the label as many as
/// it wraps to.
///
/// Worked out from the text because the host does not measure it: the
/// label's rect and its contents rect are both its authored box, and
/// `fit_height` does not grow it here (test log, 2026-10-10: a 35px box
/// whatever the text). The widths are a guess at the font, a little wide.
/// Nothing the mod writes wraps, so its own articles come out exact; the
/// guess only decides whether one of the game's long articles gets a short
/// scroll it did not have.
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

/// Makes the open article scroll as far as its text is long, and no
/// further. Every client frame; off the news screen it is two failed
/// lookups.
///
/// The mod's override of the plain article layout makes its body a scroll
/// view. Two things about one were learned from the user's runs on
/// 2026-10-10, neither of them to be had from the layouts:
///
/// - It is as long as a node in it says, and does not take its length from
///   a label: with only the label in it the article did not scroll. The
///   game's own scrolling article (`news_component/tutorial_last`) has a
///   `#dummy` node for the length, whose height game code sets, and the
///   override has the same node.
/// - A node whose box has left the view is not drawn, text and all. The
///   label was 35px high then, the text running out under it, and the first
///   turn of the wheel took the whole article away (its box at y 20 under a
///   view that starts at 120, in the test log). So the label's box is as
///   long as the scroll, always.
///
/// The layout gives both a height that is enough for the longest patch
/// notes, and this sets both to what the text takes: a short article does
/// not scroll at all, and one this cannot reach still scrolls, past its end.
///
/// For any plain article, the mod's own or not: the layout is every plain
/// article's.
pub(crate) fn sync_article_scroll(ctx: &mut StableClient<'_>) {
    let Some(text) = ctx.ui_text(ARTICLE_TEXT) else {
        // No plain article is open; or one is, and its text does not answer
        // by the path it should have, which the test log is told once.
        if ctx
            .ui_child_names(ARTICLE_AREA)
            .iter()
            .any(|child| child == "simple_container")
        {
            super::log("patch.scroll", || {
                format!(
                    "an article is open and {ARTICLE_TEXT} has no text; body exists={} is a {:?}, holds {:?}",
                    ctx.ui_exists(ARTICLE_BODY),
                    ctx.ui_runner_name(ARTICLE_BODY),
                    ctx.ui_child_names(ARTICLE_BODY),
                )
            });
        }
        return;
    };
    let (Some((_, _, _, length)), Some((_, _, _, text_box))) =
        (ctx.ui_node_rect(ARTICLE_EXTENT), ctx.ui_node_rect(ARTICLE_TEXT))
    else {
        // The layout in use is not the mod's: nothing to set.
        super::log("patch.scroll", || {
            format!(
                "the open article has no {ARTICLE_EXTENT}; its body is a {:?} and holds {:?}",
                ctx.ui_runner_name(ARTICLE_BODY),
                ctx.ui_child_names(ARTICLE_BODY),
            )
        });
        return;
    };
    let wanted = (article_text_height(&text) + ARTICLE_FOOT)
        .max(ARTICLE_VIEW_H)
        .ceil();
    let Ok(mut tried) = SCROLL_TRIED.lock() else {
        return;
    };
    if (length - wanted).abs() < 1.0 && (text_box - wanted).abs() < 1.0 {
        *tried = None;
        return;
    }
    if *tried == Some((wanted, length, text_box)) {
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
            "{} lines of text; the scroll was {length}px long and the text's box {text_box}px high, both set to {wanted}px: {set}",
            text.split('\n').count(),
        )
    });
}
