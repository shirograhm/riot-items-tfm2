//! The game's own champion and item names in every language, read out of its
//! asset bundle. The stable API answers `i18n` in English whatever the game's
//! language, and a name drawn by reference cannot be searched, so the Build
//! Editor's lists take their names from here.
//!
//! `bundle.game_data`, beside the executable, is a record count and then that
//! many records of three length-prefixed fields (a little-endian u32 length,
//! then that many bytes): the asset's type, its path, its content. The text
//! documents are JSON with every language in one: `{ "ko": {..}, "en": {..} }`.
//! Only the record headers are walked, so finding the two documents costs a
//! few thousand small reads, not the bundle's gigabyte.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::OnceLock;

use serde_json::Value;

// The two documents read, by asset path.
const CHAMPION_DOC: &str = "asset/base/text/champion";
const ITEM_DOC: &str = "asset/base/text/item";

// Longest type and path a record may have. Anything longer means the bundle is
// not laid out as this reads it, and nothing is taken from it.
const MAX_TYPE_LEN: u32 = 64;
const MAX_PATH_LEN: u32 = 1024;

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

// What the game calls champion `id` in `lang` (`description.<id>.name`), if
// its own text has it: another mod's champions are not in the bundle.
pub(crate) fn champion_name(lang: &str, id: &str) -> Option<&'static str> {
    game_text().champions.get(lang)?.get(id).map(String::as_str)
}

// What the game calls item `key` in `lang` (`<key>.name`), as it ships: the
// mod's own `text/item.i18n` is merged over this in game and comes first.
pub(crate) fn item_name(lang: &str, key: &str) -> Option<&'static str> {
    game_text().items.get(lang)?.get(key).map(String::as_str)
}

impl GameText {
    fn load() -> Self {
        let mut docs = read_docs(&[CHAMPION_DOC, ITEM_DOC]).unwrap_or_default();
        Self {
            champions: docs
                .remove(CHAMPION_DOC)
                .map(|doc| names_in(&doc, Some("description")))
                .unwrap_or_default(),
            items: docs
                .remove(ITEM_DOC)
                .map(|doc| names_in(&doc, None))
                .unwrap_or_default(),
        }
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

// The content of each `i18n` record in `wanted`, by path; `None` where the
// bundle cannot be opened or is not laid out as expected.
fn read_docs(wanted: &[&str]) -> Option<HashMap<String, Vec<u8>>> {
    let path = std::env::current_exe()
        .ok()?
        .parent()?
        .join("bundle.game_data");
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
