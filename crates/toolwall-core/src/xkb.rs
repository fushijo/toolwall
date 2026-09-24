//! Building a custom keyboard layout, written out as an XKB symbols file.
//!
//! This is the [xkbedit](https://xkbedit.github.io/) workflow, brought inside
//! the editor: a picture of a keyboard, click a key, say what it should type.
//! Runners want it for search crafting in another language, where the point is
//! to get at characters the US layout has no key for without giving up a
//! layout they can still play on.
//!
//! # Why this is not the same thing as a rebind
//!
//! A rebind (`input.remaps`) swaps one whole key for another, before Minecraft
//! or anything else sees it. A layout changes what a key *types*, and only
//! where typing happens. Pressing a key still sends that key, so a layout
//! never costs you a keybind: Q can type an umlaut in the crafting-book search
//! box and still drop items, because dropping is bound to the key, not to the
//! character it produces.
//!
//! # The four levels
//!
//! XKB gives every key four symbols, chosen by which modifiers are held:
//! base, Shift, AltGr, and Shift+AltGr. Levels past the second are what make
//! this useful - the extra characters live on AltGr and leave normal typing
//! alone.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::schema::CustomLayout;

/// One physical key: where it sits, what it says on the cap, and what US
/// QWERTY has it produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The XKB name, e.g. `AD01` for the key US QWERTY calls Q. These are
    /// positional: `AD01` is the first key of that row on every keyboard,
    /// whatever is printed on it.
    pub code: &'static str,
    /// What is printed on a US cap, for drawing the key.
    pub label: &'static str,
    pub base: Option<&'static str>,
    pub shift: Option<&'static str>,
}

/// The typing block, row by row.
///
/// Function keys, navigation and the arrows are left out on purpose: they have
/// no symbols worth editing, and `partial alphanumeric_keys` does not cover
/// them either.
pub const ROWS: &[&[Key]] = &[
    &[
        Key { code: "TLDE", label: r#"`"#, base: Some(r#"`"#), shift: Some(r#"~"#) },
        Key { code: "AE01", label: r#"1"#, base: Some(r#"1"#), shift: Some(r#"!"#) },
        Key { code: "AE02", label: r#"2"#, base: Some(r#"2"#), shift: Some(r#"@"#) },
        Key { code: "AE03", label: r#"3"#, base: Some(r#"3"#), shift: Some(r#"#"#) },
        Key { code: "AE04", label: r#"4"#, base: Some(r#"4"#), shift: Some(r#"$"#) },
        Key { code: "AE05", label: r#"5"#, base: Some(r#"5"#), shift: Some(r#"%"#) },
        Key { code: "AE06", label: r#"6"#, base: Some(r#"6"#), shift: Some(r#"^"#) },
        Key { code: "AE07", label: r#"7"#, base: Some(r#"7"#), shift: Some(r#"&"#) },
        Key { code: "AE08", label: r#"8"#, base: Some(r#"8"#), shift: Some(r#"*"#) },
        Key { code: "AE09", label: r#"9"#, base: Some(r#"9"#), shift: Some(r#"("#) },
        Key { code: "AE10", label: r#"0"#, base: Some(r#"0"#), shift: Some(r#")"#) },
        Key { code: "AE11", label: r#"-"#, base: Some(r#"-"#), shift: Some(r#"_"#) },
        Key { code: "AE12", label: r#"="#, base: Some(r#"="#), shift: Some(r#"+"#) },
        Key { code: "BKSP", label: r#"Backspace"#, base: Some(r#"BackSpace"#), shift: None },
    ],
    &[
        Key { code: "TAB", label: r#"Tab"#, base: Some(r#"Tab"#), shift: None },
        Key { code: "AD01", label: r#"Q"#, base: Some(r#"q"#), shift: Some(r#"Q"#) },
        Key { code: "AD02", label: r#"W"#, base: Some(r#"w"#), shift: Some(r#"W"#) },
        Key { code: "AD03", label: r#"E"#, base: Some(r#"e"#), shift: Some(r#"E"#) },
        Key { code: "AD04", label: r#"R"#, base: Some(r#"r"#), shift: Some(r#"R"#) },
        Key { code: "AD05", label: r#"T"#, base: Some(r#"t"#), shift: Some(r#"T"#) },
        Key { code: "AD06", label: r#"Y"#, base: Some(r#"y"#), shift: Some(r#"Y"#) },
        Key { code: "AD07", label: r#"U"#, base: Some(r#"u"#), shift: Some(r#"U"#) },
        Key { code: "AD08", label: r#"I"#, base: Some(r#"i"#), shift: Some(r#"I"#) },
        Key { code: "AD09", label: r#"O"#, base: Some(r#"o"#), shift: Some(r#"O"#) },
        Key { code: "AD10", label: r#"P"#, base: Some(r#"p"#), shift: Some(r#"P"#) },
        Key { code: "AD11", label: r#"["#, base: Some(r#"["#), shift: Some(r#"{"#) },
        Key { code: "AD12", label: r#"]"#, base: Some(r#"]"#), shift: Some(r#"}"#) },
        Key { code: "BKSL", label: r#"\"#, base: Some(r#"\"#), shift: Some(r#"|"#) },
    ],
    &[
        Key { code: "CAPS", label: r#"Caps"#, base: Some(r#"Caps_Lock"#), shift: None },
        Key { code: "AC01", label: r#"A"#, base: Some(r#"a"#), shift: Some(r#"A"#) },
        Key { code: "AC02", label: r#"S"#, base: Some(r#"s"#), shift: Some(r#"S"#) },
        Key { code: "AC03", label: r#"D"#, base: Some(r#"d"#), shift: Some(r#"D"#) },
        Key { code: "AC04", label: r#"F"#, base: Some(r#"f"#), shift: Some(r#"F"#) },
        Key { code: "AC05", label: r#"G"#, base: Some(r#"g"#), shift: Some(r#"G"#) },
        Key { code: "AC06", label: r#"H"#, base: Some(r#"h"#), shift: Some(r#"H"#) },
        Key { code: "AC07", label: r#"J"#, base: Some(r#"j"#), shift: Some(r#"J"#) },
        Key { code: "AC08", label: r#"K"#, base: Some(r#"k"#), shift: Some(r#"K"#) },
        Key { code: "AC09", label: r#"L"#, base: Some(r#"l"#), shift: Some(r#"L"#) },
        Key { code: "AC10", label: r#";"#, base: Some(r#";"#), shift: Some(r#":"#) },
        Key { code: "AC11", label: r#"'"#, base: Some(r#"'"#), shift: Some(r#"""#) },
        Key { code: "RTRN", label: r#"Enter"#, base: Some(r#"Return"#), shift: None },
    ],
    &[
        Key { code: "LFSH", label: r#"Shift"#, base: Some(r#"Shift_L"#), shift: None },
        Key { code: "AB01", label: r#"Z"#, base: Some(r#"z"#), shift: Some(r#"Z"#) },
        Key { code: "AB02", label: r#"X"#, base: Some(r#"x"#), shift: Some(r#"X"#) },
        Key { code: "AB03", label: r#"C"#, base: Some(r#"c"#), shift: Some(r#"C"#) },
        Key { code: "AB04", label: r#"V"#, base: Some(r#"v"#), shift: Some(r#"V"#) },
        Key { code: "AB05", label: r#"B"#, base: Some(r#"b"#), shift: Some(r#"B"#) },
        Key { code: "AB06", label: r#"N"#, base: Some(r#"n"#), shift: Some(r#"N"#) },
        Key { code: "AB07", label: r#"M"#, base: Some(r#"m"#), shift: Some(r#"M"#) },
        Key { code: "AB08", label: r#","#, base: Some(r#","#), shift: Some(r#"<"#) },
        Key { code: "AB09", label: r#"."#, base: Some(r#"."#), shift: Some(r#">"#) },
        Key { code: "AB10", label: r#"/"#, base: Some(r#"/"#), shift: Some(r#"?"#) },
        Key { code: "RTSH", label: r#"Shift"#, base: Some(r#"Shift_R"#), shift: None },
    ],
    &[
        Key { code: "LCTL", label: r#"Ctrl"#, base: Some(r#"Control_L"#), shift: None },
        Key { code: "LWIN", label: r#"Super"#, base: Some(r#"Super_L"#), shift: None },
        Key { code: "LALT", label: r#"Alt"#, base: Some(r#"Alt_L"#), shift: None },
        Key { code: "SPCE", label: r#"Space"#, base: Some(r#"space"#), shift: None },
        Key { code: "RALT", label: r#"Alt"#, base: Some(r#"ISO_Level3_Shift"#), shift: None },
        Key { code: "RWIN", label: r#"Menu"#, base: Some(r#"Menu"#), shift: None },
        Key { code: "RCTL", label: r#"Ctrl"#, base: Some(r#"Control_R"#), shift: None },
    ],
];

/// How many levels XKB gives a key. Four. It is always four.
pub const LEVELS: usize = 4;

/// What each level is called, in order.
pub const LEVEL_NAMES: [&str; LEVELS] = ["Normal", "Shift", "AltGr", "Shift+AltGr"];

/// Every key in the table, flattened.
pub fn keys() -> impl Iterator<Item = &'static Key> {
    ROWS.iter().copied().flatten()
}

pub fn key(code: &str) -> Option<&'static Key> {
    keys().find(|k| k.code == code)
}

/// What US QWERTY puts on this level, if anything.
pub fn default_symbol(key: &Key, level: usize) -> Option<&'static str> {
    match level {
        0 => key.base,
        1 => key.shift,
        // A plain US layout puts nothing on AltGr, which leaves it free for
        // a second alphabet.
        _ => None,
    }
}

/// Turn what someone typed into a keysym XKB will accept.
///
/// A bare name goes through untouched, so `Escape` and `adiaeresis` still work
/// for anyone who knows them. Anything else is emitted as its Unicode code
/// point, which is the escape hatch that puts every character in reach whether
/// or not it has a name.
pub fn to_keysym(symbol: &str) -> String {
    let trimmed = symbol.trim();
    if trimmed.is_empty() {
        return "VoidSymbol".to_string();
    }

    if trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return trimmed.to_string();
    }

    match trimmed.chars().next() {
        Some(c) => format!("U{:04X}", c as u32),
        None => "VoidSymbol".to_string(),
    }
}

/// The inverse, for showing a stored keysym back on a key cap.
pub fn from_keysym(keysym: &str) -> String {
    let trimmed = keysym.trim();
    if trimmed.is_empty() || trimmed == "VoidSymbol" {
        return String::new();
    }

    if let Some(hex) = trimmed.strip_prefix('U') {
        if (4..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                return c.to_string();
            }
        }
    }

    trimmed.to_string()
}

/// The levels this key should be written with, or `None` to leave it out.
///
/// A key that matches US QWERTY is omitted entirely. The file is an overlay on
/// a layout that already exists, so writing out the keys nobody changed would
/// just be a longer way of saying nothing.
fn export_levels(key: &Key, layout: &CustomLayout) -> Option<Vec<String>> {
    let set = layout.keys.get(key.code);

    let highest = set
        .map(|levels| {
            levels
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(i, _)| i as i32)
                .max()
                .unwrap_or(-1)
        })
        .unwrap_or(-1);

    // Always emit at least base and Shift: a key with only an AltGr symbol
    // still has to restate the two below it, or XKB fills them with nothing
    // and the key stops typing at all.
    let count = (highest + 1).max(2) as usize;

    let levels: Vec<String> = (0..count)
        .map(|level| {
            set.and_then(|s| s.get(level))
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .or_else(|| default_symbol(key, level).map(str::to_string))
                .unwrap_or_default()
        })
        .collect();

    let unchanged = levels.iter().enumerate().all(|(level, value)| {
        value.as_str() == default_symbol(key, level).unwrap_or_default()
    });

    if unchanged {
        return None;
    }

    Some(levels)
}

/// Does this layout put anything on AltGr or above?
///
/// Worth knowing because reaching those levels needs a key that acts as
/// `ISO_Level3_Shift`, and on a US layout nothing does.
pub fn uses_level3(layout: &CustomLayout) -> bool {
    layout
        .keys
        .values()
        .any(|levels| levels.iter().skip(2).any(|v| !v.trim().is_empty()))
}

/// Render the symbols file.
///
/// Same shape xkbedit emits, so a layout built in either one is recognisable
/// in the other.
pub fn symbols_file(layout: &CustomLayout) -> String {
    let mut out = String::new();

    out.push_str("// Written by toolwall. Edited from the Layout tab.\n");
    out.push_str("partial alphanumeric_keys\n");
    out.push_str("xkb_symbols \"basic\" {\n");

    // Everything not mentioned below comes from here.
    //
    // Required, not tidiness. An xkb_symbols section replaces the alphanumeric
    // block rather than adding to it, so without this every key the layout does
    // not name loses its symbol: xkbcomp prints "No symbols defined for <AE02>"
    // and you get a keyboard that types only what you edited.
    let base = layout.base.trim();
    if !base.is_empty() {
        out.push_str(&format!("    include \"{base}\"\n"));
    }

    // Make Right Alt actually be AltGr, but only if the layout uses those
    // levels.
    //
    // Four symbols on a key is not enough to reach the last two. Something has
    // to emit ISO_Level3_Shift, and on a US layout nothing does - Right Alt is
    // plain Alt_R. The keymap compiles either way and the extra characters
    // simply never appear, which is the kind of silence that reads as "this
    // feature does not work".
    //
    // Guarded, so a layout that only touches the first two levels leaves Right
    // Alt alone and keeps working as a normal Alt in game.
    if uses_level3(layout) {
        out.push_str("    include \"level3(ralt_switch)\"\n");
    }

    out.push('\n');

    out.push_str(&format!("    name[Group1]= \"{}\";\n\n", layout.name));

    for key in keys() {
        if let Some(levels) = export_levels(key, layout) {
            let rendered: Vec<String> = levels.iter().map(|s| to_keysym(s)).collect();
            out.push_str(&format!("    key <{}> {{ [ {} ] }};\n", key.code, rendered.join(", ")));
        }
    }

    out.push_str("};\n");
    out
}

/// Where libxkbcommon looks for a layout by name.
///
/// It searches `$XDG_CONFIG_HOME/xkb` before the system tree, which is what
/// lets a layout live in the user's own config with no root involved.
pub fn symbols_dir() -> Result<PathBuf> {
    let base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(xdg) if !xdg.is_empty() => PathBuf::from(xdg),
        _ => directories::BaseDirs::new()
            .context("cannot determine home directory")?
            .home_dir()
            .join(".config"),
    };

    Ok(base.join("xkb").join("symbols"))
}

/// A layout name that is safe as a filename and legal to xkbcommon.
pub fn sanitise_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();

    if cleaned.is_empty() { "custom".to_string() } else { cleaned }
}

/// Write the symbols file, returning where it landed.
pub fn write_symbols(layout: &CustomLayout) -> Result<PathBuf> {
    write_symbols_in(layout, &symbols_dir()?)
}

pub fn write_symbols_in(layout: &CustomLayout, dir: &Path) -> Result<PathBuf> {
    let name = sanitise_name(&layout.name);
    let path = dir.join(&name);

    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(&path, symbols_file(layout))
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn layout_with(code: &str, levels: &[&str]) -> CustomLayout {
        let mut keys = BTreeMap::new();
        keys.insert(code.to_string(), levels.iter().map(|s| s.to_string()).collect());
        CustomLayout { name: "mc".into(), base: "us".into(), keys, enabled: true }
    }

    #[test]
    fn the_table_is_a_keyboard() {
        assert_eq!(ROWS.len(), 5);
        assert_eq!(keys().count(), 60);

        // Positional codes, so this is Q's place whatever the cap says.
        assert_eq!(key("AD01").map(|k| k.base), Some(Some("q")));
        assert_eq!(key("AD01").map(|k| k.shift), Some(Some("Q")));
        assert_eq!(key("SPCE").map(|k| k.base), Some(Some("space")));

        // No duplicate codes, or an edit would land on two keys at once.
        let mut seen = std::collections::HashSet::new();
        for k in keys() {
            assert!(seen.insert(k.code), "{} appears twice", k.code);
        }
    }

    #[test]
    fn named_keysyms_survive_and_characters_become_code_points() {
        assert_eq!(to_keysym("q"), "q");
        assert_eq!(to_keysym("Escape"), "Escape");
        assert_eq!(to_keysym("ISO_Level3_Shift"), "ISO_Level3_Shift");

        // No name, so the code point is the way in.
        assert_eq!(to_keysym("\u{f6}"), "U00F6");
        assert_eq!(to_keysym("\u{259}"), "U0259");
        assert_eq!(to_keysym("!"), "U0021");
        assert_eq!(to_keysym(""), "VoidSymbol");
    }

    #[test]
    fn keysyms_round_trip() {
        for symbol in ["q", "Escape", "\u{f6}", "\u{259}", "\u{65e5}"] {
            assert_eq!(from_keysym(&to_keysym(symbol)), symbol, "{symbol}");
        }
        assert_eq!(from_keysym("VoidSymbol"), "");
    }

    #[test]
    fn a_layout_that_changes_nothing_writes_no_keys() {
        let empty =
            CustomLayout { name: "mc".into(), base: "us".into(), keys: BTreeMap::new(), enabled: true };
        let file = symbols_file(&empty);

        assert!(!file.contains("key <"), "{file}");
        assert!(file.contains("xkb_symbols \"basic\""));
    }

    #[test]
    fn a_key_left_at_its_default_is_not_written() {
        // Saying "q types q" is not a change.
        let layout = layout_with("AD01", &["q", "Q"]);
        assert!(!symbols_file(&layout).contains("key <AD01>"));
    }

    #[test]
    fn an_altgr_symbol_restates_the_levels_below_it() {
        // XKB fills unstated levels with nothing, so a key that only sets
        // AltGr has to carry its own base and Shift or typing q breaks.
        let layout = layout_with("AD01", &["", "", "\u{f6}"]);
        let file = symbols_file(&layout);

        assert!(file.contains("key <AD01> { [ q, Q, U00F6 ] };"), "{file}");
    }

    #[test]
    fn all_four_levels_are_written_when_used() {
        let layout = layout_with("AD01", &["q", "Q", "\u{e4}", "\u{c4}"]);
        let file = symbols_file(&layout);

        assert!(file.contains("key <AD01> { [ q, Q, U00E4, U00C4 ] };"), "{file}");
    }

    #[test]
    fn the_file_is_shaped_the_way_xkb_expects() {
        let layout = layout_with("AD01", &["", "", "\u{f6}"]);
        let file = symbols_file(&layout);
        let lines: Vec<&str> = file.lines().collect();

        assert_eq!(lines[1], "partial alphanumeric_keys");
        assert_eq!(lines[2], "xkb_symbols \"basic\" {");
        assert_eq!(lines[3], "    include \"us\"");
        assert_eq!(lines[4], "    include \"level3(ralt_switch)\"");
        assert_eq!(lines[6], "    name[Group1]= \"mc\";");
        assert_eq!(lines.last(), Some(&"};"));
    }

    /// Without the include, xkbcomp reports "No symbols defined for <AE02>"
    /// for every key the layout does not mention, and the keyboard types
    /// only the handful that were edited.
    #[test]
    fn the_base_layout_is_always_included() {
        let layout = layout_with("AD01", &["", "", "\u{f6}"]);
        let file = symbols_file(&layout);

        assert!(file.contains("include \"us\""), "{file}");

        // Before the name, so the name wins over the base layout's own.
        let include_at = file.find("include").unwrap();
        let name_at = file.find("name[Group1]").unwrap();
        assert!(include_at < name_at);
    }

    #[test]
    fn a_layout_can_be_built_on_something_other_than_us() {
        let mut layout = layout_with("AD01", &["", "", "\u{f6}"]);
        layout.base = "de".into();
        assert!(symbols_file(&layout).contains("include \"de\""));

        // And opting out of a base is possible, for someone who really means
        // it. The level3 include is a separate question and stays.
        layout.base = String::new();
        let file = symbols_file(&layout);
        assert!(!file.contains("include \"de\""));
        assert!(!file.contains("include \"us\""));
    }

    /// Four symbols on a key are unreachable unless something emits
    /// ISO_Level3_Shift, and a US layout has nothing that does.
    #[test]
    fn altgr_is_wired_up_when_the_layout_uses_it() {
        let using = layout_with("AD01", &["", "", "\u{f6}"]);
        assert!(uses_level3(&using));
        assert!(symbols_file(&using).contains("level3(ralt_switch)"));
    }

    /// And left alone when it does not, so Right Alt stays a normal Alt for
    /// anyone who only reordered the first two levels.
    #[test]
    fn altgr_is_left_alone_when_the_layout_does_not_use_it() {
        let not_using = layout_with("AD01", &["z", "Z"]);
        assert!(!uses_level3(&not_using));
        assert!(!symbols_file(&not_using).contains("level3"));
    }

    /// The derived Default would leave `base` empty, which produces a layout
    /// where every key not explicitly set types nothing.
    #[test]
    fn a_default_layout_still_has_a_base() {
        let fresh = CustomLayout::default();
        assert_eq!(fresh.base, "us");
        assert!(symbols_file(&fresh).contains("include \"us\""));
    }

    #[test]
    fn a_name_cannot_escape_the_symbols_directory() {
        // The property that matters is that nothing which can traverse a
        // path survives, not the exact replacement.
        for hostile in ["../../etc/passwd", "a/b", "..", "x\0y"] {
            let clean = sanitise_name(hostile);
            assert!(!clean.contains('/'), "{clean}");
            assert!(!clean.contains('.'), "{clean}");
            assert!(!clean.is_empty());
        }

        assert_eq!(sanitise_name("my layout"), "my_layout");
        assert_eq!(sanitise_name("  "), "custom");
        assert_eq!(sanitise_name("mc"), "mc");
    }

    #[test]
    fn writing_lands_where_xkbcommon_looks() {
        let dir = std::env::temp_dir().join(format!("toolwall-xkb-{}", std::process::id()));
        let layout = layout_with("AD01", &["", "", "\u{f6}"]);

        let path = write_symbols_in(&layout, &dir).unwrap();
        assert_eq!(path.file_name().unwrap(), "mc");

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("U00F6"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
