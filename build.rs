//! Compiles `text/item_classes.json` into the `CATEGORY_OF` table that
//! `src/item_catalog.rs` includes.
//!
//! That file is the one place an item's class is written down. This pack reads
//! it here, at build time; `item_scroller_tfm2` reads the shipped copy at
//! runtime to class this pack's items in its filter. A malformed file stops the
//! build rather than reaching the game.

use std::path::Path;

const CLASS_FILE: &str = "text/item_classes.json";

fn main() {
    println!("cargo:rerun-if-changed={CLASS_FILE}");

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CLASS_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{CLASS_FILE}: {err}"));
    let root: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .unwrap_or_else(|err| panic!("{CLASS_FILE}: {err}"));

    let mut entries: Vec<(&str, &str)> = root
        .iter()
        .map(|(slug, class)| {
            let class = class
                .as_str()
                .unwrap_or_else(|| panic!("{CLASS_FILE}: {slug:?} has a non-string class"));
            (slug.as_str(), class)
        })
        .collect();
    // `category_of` binary searches, so the table is sorted whatever order the
    // file is written in.
    entries.sort_unstable();

    let mut table = String::from("const CATEGORY_OF: &[(&str, &str)] = &[\n");
    for (slug, class) in entries {
        table.push_str(&format!("    ({slug:?}, {class:?}),\n"));
    }
    table.push_str("];\n");

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("item_classes.rs");
    std::fs::write(out, table).unwrap();
}
