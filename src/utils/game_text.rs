//! Champion and item names in every language, as the game shows them: its own
//! text out of its asset bundle, with the text of every enabled mod that
//! merges into the same documents laid over it. The stable API answers `i18n`
//! in English whatever the game's language, and a name drawn by reference
//! cannot be searched, so the Build Editor's lists take their names from here.
//!
//! `bundle.game_data`, beside the executable, is a record count and then that
//! many records of three length-prefixed fields (a little-endian u32 length,
//! then that many bytes): the asset's type, its path, its content. The text
//! documents are JSON with every language in one: `{ "ko": {..}, "en": {..} }`.
//! Only the record headers are walked, so finding the two documents costs a
//! few thousand small reads, not the bundle's gigabyte. A mod's text has the
//! same shape, in the file its `mod.override_info` remaps the document to.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;

// The two documents read, by asset path.
const CHAMPION_DOC: &str = "asset/base/text/champion";
const ITEM_DOC: &str = "asset/base/text/item";

// Longest type and path a record may have. Anything longer means the bundle is
// not laid out as this reads it, and nothing is taken from it.
const MAX_TYPE_LEN: u32 = 64;
const MAX_PATH_LEN: u32 = 1024;

// This mod's id, whose text `crate::patches::text` reads from the copy that is
// running; another install of it is not read here.
const OWN_MOD_ID: &str = "riot_items_tfm2";

// The game's Steam app id, under which the Workshop keeps its mods.
const WORKSHOP_APP_ID: &str = "3009300";

// Language -> champion id or item key -> name.
type Names = HashMap<String, HashMap<String, String>>;

struct GameText {
    champions: Names,
    items: Names,
}

fn game_text() -> &'static GameText {
    static TEXT: OnceLock<GameText> = OnceLock::new();
    TEXT.get_or_init(GameText::load)
}

// Reads the bundle now, on a thread of its own, so the first screen to ask
// does not wait on the disk. A caller that asks before it is done waits for it.
pub(crate) fn warm() {
    let _ = std::thread::Builder::new()
        .name("riot-items-game-text".to_string())
        .spawn(|| {
            game_text();
        });
}

// What the game calls champion `id` in `lang` (`description.<id>.name`), its
// own champions' names and those of champion mods alike, if any has one.
pub(crate) fn champion_name(lang: &str, id: &str) -> Option<&'static str> {
    game_text().champions.get(lang)?.get(id).map(String::as_str)
}

// What the game calls item `key` in `lang` (`<key>.name`), with other item
// mods' names over its own; this mod's own text comes before this
// ([`crate::patches::text::item_name_in`]).
pub(crate) fn item_name(lang: &str, key: &str) -> Option<&'static str> {
    game_text().items.get(lang)?.get(key).map(String::as_str)
}

impl GameText {
    fn load() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let mut docs = exe_dir
            .as_deref()
            .and_then(|dir| read_docs(&dir.join("bundle.game_data"), &[CHAMPION_DOC, ITEM_DOC]))
            .unwrap_or_default();
        let mut text = Self {
            champions: docs
                .remove(CHAMPION_DOC)
                .map(|doc| names_in(&doc, Some("description")))
                .unwrap_or_default(),
            items: docs
                .remove(ITEM_DOC)
                .map(|doc| names_in(&doc, None))
                .unwrap_or_default(),
        };
        for (doc, path) in exe_dir.as_deref().map(mod_text_files).unwrap_or_default() {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if doc == CHAMPION_DOC {
                lay_over(&mut text.champions, names_in(&bytes, Some("description")));
            } else {
                lay_over(&mut text.items, names_in(&bytes, None));
            }
        }
        text
    }
}

// Each language's `name`s in a text document: the entries under `section`, or
// directly under the language, that have one.
fn names_in(doc: &[u8], section: Option<&str>) -> Names {
    let doc = doc.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(doc);
    let Ok(Value::Object(langs)) = serde_json::from_slice::<Value>(doc) else {
        return Names::new();
    };
    langs
        .iter()
        .filter_map(|(lang, entries)| {
            let entries = match section {
                Some(section) => entries.get(section)?,
                None => entries,
            }
            .as_object()?;
            let names: HashMap<String, String> = entries
                .iter()
                .filter_map(|(key, entry)| {
                    Some((key.clone(), entry.get("name")?.as_str()?.to_string()))
                })
                .collect();
            Some((lang.clone(), names))
        })
        .collect()
}

// Lays a mod's names over those so far, as its merge lays its text over the
// game's.
fn lay_over(names: &mut Names, over: Names) {
    for (lang, entries) in over {
        names.entry(lang).or_default().extend(entries);
    }
}

// The files enabled mods merge into the two documents, as (document, file), in
// the order of [`other_mod_folders`], so that a later mod's names win as its
// merge would.
fn mod_text_files(exe_dir: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut found = Vec::new();
    for folder in other_mod_folders(exe_dir) {
        let Some(overrides) = read_json(&folder.join("mod.override_info")) else {
            continue;
        };
        for doc in [CHAMPION_DOC, ITEM_DOC] {
            let Some(remapping) = overrides
                .get(doc)
                .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("merge"))
                .and_then(|entry| entry.get("remapping")?.as_str())
            else {
                continue;
            };
            // `asset/<mod>/text/champion` is `text/champion.i18n` in the
            // mod's folder.
            let Some((_, file)) = remapping
                .strip_prefix("asset/")
                .and_then(|rest| rest.split_once('/'))
            else {
                continue;
            };
            found.push((doc, folder.join(format!("{file}.i18n"))));
        }
    }
    found
}

// The folder of every enabled mod but this one, from the game's `mods` folder
// and the Workshop's alike, in the order the game lists the mods as enabled;
// every installed mod is taken when that list cannot be read.
pub(crate) fn other_mod_folders(exe_dir: &Path) -> Vec<PathBuf> {
    let enabled: Option<Vec<String>> =
        read_json(&exe_dir.join("config").join("game").join("mods.json")).and_then(|mods| {
            let ids = mods.get("enabled_mods")?.as_array()?;
            Some(
                ids.iter()
                    .filter_map(|id| Some(id.as_str()?.to_string()))
                    .collect(),
            )
        });
    // `<library>/steamapps/common/<game>` -> `<library>/steamapps/workshop`.
    let workshop = exe_dir
        .parent()
        .and_then(Path::parent)
        .map(|apps| apps.join("workshop").join("content").join(WORKSHOP_APP_ID));

    let mut found = Vec::new();
    for root in [Some(exe_dir.join("mods")), workshop].into_iter().flatten() {
        let Ok(folders) = std::fs::read_dir(&root) else {
            continue;
        };
        for folder in folders.flatten().map(|entry| entry.path()) {
            let Some(id) = read_json(&folder.join("mod.mod_info")).and_then(|info| {
                Some(
                    info.get("mod_id")
                        .or_else(|| info.get("id"))?
                        .as_str()?
                        .to_string(),
                )
            }) else {
                continue;
            };
            if id == OWN_MOD_ID {
                continue;
            }
            let rank = match &enabled {
                Some(ids) => match ids.iter().position(|listed| *listed == id) {
                    Some(rank) => rank,
                    None => continue,
                },
                None => 0,
            };
            found.push((rank, folder));
        }
    }
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, folder)| folder).collect()
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

// The content of each `i18n` record in `wanted` in the bundle at `path`, by
// asset path; `None` where the bundle cannot be opened or is not laid out as
// expected.
fn read_docs(path: &Path, wanted: &[&str]) -> Option<HashMap<String, Vec<u8>>> {
    let file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut reader = BufReader::new(file);

    let mut found = HashMap::new();
    for _ in 0..read_u32(&mut reader)? {
        let kind = read_field(&mut reader, MAX_TYPE_LEN)?;
        let asset = String::from_utf8_lossy(&read_field(&mut reader, MAX_PATH_LEN)?).into_owned();
        let len = read_u32(&mut reader)?;
        if u64::from(len) > size {
            return None;
        }
        if kind == b"i18n" && wanted.contains(&asset.as_str()) {
            let mut content = vec![0; len as usize];
            reader.read_exact(&mut content).ok()?;
            found.insert(asset, content);
            if found.len() == wanted.len() {
                break;
            }
        } else {
            reader.seek_relative(i64::from(len)).ok()?;
        }
    }
    Some(found)
}

fn read_u32(reader: &mut impl Read) -> Option<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}

// One length-prefixed field no longer than `max`.
fn read_field(reader: &mut impl Read, max: u32) -> Option<Vec<u8>> {
    let len = read_u32(reader)?;
    if len > max {
        return None;
    }
    let mut bytes = vec![0; len as usize];
    reader.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
