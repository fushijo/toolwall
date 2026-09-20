//! The starter config is what a fresh install runs, so it is worth a test.
//!
//! Nothing else loads `examples/default.json`: it gets copied into place by
//! install.sh and is not read again until waywall reads it. A typo in there
//! reaches every new user and nobody else.

use toolwall_core::schema::Command;
use toolwall_core::{problems, Document};

fn starter() -> Document {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/default.json");
    let body = std::fs::read_to_string(path).expect("examples/default.json is missing");
    serde_json::from_str(&body).expect("examples/default.json does not parse")
}

#[test]
fn the_starter_config_is_valid() {
    let doc = starter();
    let found = problems(&doc);
    assert!(found.is_empty(), "starter config has problems: {found:?}");
}

#[test]
fn the_starter_config_can_open_the_editor() {
    // A config with no gui.toggle bind leaves you with no way in. The first
    // person to try the beta hit that on the import path and went hunting for
    // the binary in ~/.cargo/bin.
    let doc = starter();

    let editor: Vec<&str> = doc
        .keybinds
        .iter()
        .filter(|bind| bind.command == Command::GuiToggle)
        .map(|bind| bind.input.as_str())
        .collect();

    assert_eq!(
        editor,
        vec!["Ctrl-I"],
        "the readme tells people to press Ctrl+I, so that is what this has to be"
    );
}
