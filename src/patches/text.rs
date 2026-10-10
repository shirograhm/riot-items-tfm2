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

    /// The name of the passive that `item`'s `field` is a number of, and
    /// whether the sentence puts a percent sign after it. A passive's text
    /// opens with its name and a colon, in every language.
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
        let start = text[..at].rfind("\n\n").map_or(0, |gap| gap + 2);
        let paragraph = plain(&text[start..]);
        let paragraph = paragraph.trim();
        let name = paragraph.split([':', '\u{ff1a}']).next()?.trim();
        let named = !name.is_empty() && name.len() < paragraph.len() && name.chars().count() <= 40;
        named.then(|| (name.to_string(), percent))
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
/// Info and in the strategy screen's Item Info popup.
const DETAIL: &str = "item_detail";

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
                wanted.push(site_paths(format!("{content}.{DETAIL}"), ""));
                for inner in ctx.ui_child_names(&content) {
                    wanted.push(site_paths(format!("{content}.{inner}.{DETAIL}"), ""));
                }
            }
        }
    }
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
        let Some(key) = ctx
            .ui_text(&site.name)
            .and_then(|name| texts.key_of_name(&name).map(str::to_string))
        else {
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

/// One line of a note: what moved on one item, every tier of it.
fn note_line(texts: &Texts, lang: &str, change: &Change) -> Option<String> {
    let first = change.members.first()?;
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
    let (words, percent) = label.unwrap_or_else(|| (String::new(), false));
    let moves: Vec<String> = change
        .members
        .iter()
        .map(|moved| {
            format!(
                "{} -> {}",
                note_number(moved.before, percent),
                note_number(moved.after, percent)
            )
        })
        .collect();
    let name = texts.name(lang, &first.key);
    Some(if words.is_empty() {
        format!("- {name}: {}", moves.join(" / "))
    } else {
        format!("- {name}, {words}: {}", moves.join(" / "))
    })
}

/// The news article for one patch: title, body, author. In the game's
/// language as it is when the patch lands; an article is text once written.
pub(crate) fn news(patch: &Patch) -> (String, String, String) {
    let texts = texts();
    let lang = game_language();
    let title = texts
        .ui(&lang, "title", "Item Balance Patch {N}")
        .replace("{N}", &patch.number.to_string());
    let mut body = texts
        .ui(&lang, "intro", "Items were rebalanced after {Matches} matches on v{Version}.")
        .replace("{Matches}", &patch.matches.to_string())
        .replace("{Version}", &patch.version);
    for (buff, heading, fallback) in [(false, "nerfed", "Nerfed"), (true, "buffed", "Buffed")] {
        let lines: Vec<String> = patch
            .changes
            .iter()
            .filter(|change| change.buff == buff)
            .filter_map(|change| note_line(texts, &lang, change))
            .collect();
        if lines.is_empty() {
            continue;
        }
        body.push_str("\n\n");
        body.push_str(&texts.ui(&lang, heading, fallback));
        for line in lines {
            body.push('\n');
            body.push_str(&line);
        }
    }
    if patch.changes.is_empty() {
        body.push_str("\n\n");
        body.push_str(&texts.ui(&lang, "none", "No item was changed."));
    }
    let author = texts.ui(&lang, "author", "Balance Team");
    (title, body, author)
}
