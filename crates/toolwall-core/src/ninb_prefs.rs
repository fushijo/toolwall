//! Ninjabrain Bot's own hotkeys, which live outside toolwall's config.
//!
//! ninb keeps its settings in a Java preferences file, and its hotkeys are
//! global: it watches the keyboard itself rather than being sent keys. That
//! makes them unreachable from toolwall's keybinds, and unreachable full stop
//! once ninb's window is hidden, which is why they are edited here.
//!
//! A hotkey is stored as two integers. The code is JNativeHook's key code with
//! the key location in the high 16 bits, and the modifier is a mask of the
//! modifier keys that must be held. The masks are per side: there is a bit for
//! left shift and a different one for right shift, and ninb requires every bit
//! it stored to be present, so a hotkey saved with left control does not fire
//! on right control.

use std::path::PathBuf;

/// JNativeHook's `KEY_LOCATION_STANDARD`. Everything this editor can pick is a
/// standard key; the numpad and the left/right pairs are deliberately left out.
const LOCATION_STANDARD: i32 = 1;

/// Modifier masks. A hotkey is saved with one side, and this editor saves the
/// left one, but ninb itself will have saved whichever side was pressed, so
/// both have to be understood when reading a file back.
pub const MOD_SHIFT: i32 = 1;
pub const MOD_CTRL: i32 = 2;
pub const MOD_META: i32 = 4;
pub const MOD_ALT: i32 = 8;
pub const MOD_SHIFT_R: i32 = 16;
pub const MOD_CTRL_R: i32 = 32;
pub const MOD_META_R: i32 = 64;
pub const MOD_ALT_R: i32 = 128;

/// Either side of each modifier, for reading rather than writing.
pub const ANY_SHIFT: i32 = MOD_SHIFT | MOD_SHIFT_R;
pub const ANY_CTRL: i32 = MOD_CTRL | MOD_CTRL_R;
pub const ANY_META: i32 = MOD_META | MOD_META_R;
pub const ANY_ALT: i32 = MOD_ALT | MOD_ALT_R;

/// The hotkeys ninb exposes, as (preference name, label).
pub const ACTIONS: &[(&str, &str)] = &[
    ("increment", "Increase last angle"),
    ("decrement", "Decrease last angle"),
    ("undo", "Undo throw"),
    ("redo", "Redo throw"),
    ("reset", "Reset"),
    ("minimize", "Minimise ninb"),
];

/// JNativeHook key codes, by the name toolwall shows for them.
pub const KEYS: &[(&str, i32)] = &[
    ("Escape", 1),
    ("F1", 59), ("F2", 60), ("F3", 61), ("F4", 62), ("F5", 63), ("F6", 64),
    ("F7", 65), ("F8", 66), ("F9", 67), ("F10", 68), ("F11", 87), ("F12", 88),
    ("`", 41),
    ("1", 2), ("2", 3), ("3", 4), ("4", 5), ("5", 6),
    ("6", 7), ("7", 8), ("8", 9), ("9", 10), ("0", 11),
    ("-", 12), ("=", 13), ("Backspace", 14), ("Tab", 15),
    ("A", 30), ("B", 48), ("C", 46), ("D", 32), ("E", 18), ("F", 33),
    ("G", 34), ("H", 35), ("I", 23), ("J", 36), ("K", 37), ("L", 38),
    ("M", 50), ("N", 49), ("O", 24), ("P", 25), ("Q", 16), ("R", 19),
    ("S", 31), ("T", 20), ("U", 22), ("V", 47), ("W", 17), ("X", 45),
    ("Y", 21), ("Z", 44),
    ("[", 26), ("]", 27), ("\\", 43), (";", 39), ("'", 40),
    ("Enter", 28), (",", 51), (".", 52), ("/", 53), ("Space", 57),
    ("Caps Lock", 58), ("Scroll Lock", 70), ("Num Lock", 69),
    ("Print Screen", 3639), ("Pause", 3653),
    ("Insert", 3666), ("Delete", 3667), ("Home", 3655), ("End", 3663),
    ("Page Up", 3657), ("Page Down", 3665),
    ("Up", 57416), ("Left", 57419), ("Right", 57421), ("Down", 57424),
    ("Shift", 42), ("Ctrl", 29), ("Alt", 56), ("Super", 3675),
    ("Menu", 3677),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hotkey {
    pub code: i32,
    pub modifier: i32,
}

impl Hotkey {
    pub fn new(key: &str, modifier: i32) -> Option<Self> {
        let code = KEYS.iter().find(|(name, _)| *name == key)?.1;
        Some(Self { code: code | (LOCATION_STANDARD << 16), modifier })
    }

    /// The key on its own, without the location or the modifiers.
    pub fn key(&self) -> Option<&'static str> {
        let code = self.code & 0xFFFF;
        KEYS.iter().find(|(_, value)| *value == code).map(|(name, _)| *name)
    }

    /// How the hotkey reads to a person, eg "Ctrl-Page Up".
    pub fn label(&self) -> String {
        if self.code == 0 {
            return "unset".into();
        }

        let mut parts = Vec::new();
        if self.modifier & ANY_CTRL != 0 {
            parts.push("Ctrl".to_string());
        }
        if self.modifier & ANY_ALT != 0 {
            parts.push("Alt".to_string());
        }
        if self.modifier & ANY_META != 0 {
            parts.push("Super".to_string());
        }
        if self.modifier & ANY_SHIFT != 0 {
            parts.push("Shift".to_string());
        }

        match self.key() {
            Some(name) => parts.push(name.to_string()),
            // an unknown code still has to be shown, or an editor that cannot
            // name a key would silently drop it
            None => parts.push(format!("key {}", self.code & 0xFFFF)),
        }

        parts.join("-")
    }
}

/// Where ninb keeps its settings. Java's preference store, not XDG.
pub fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".java/.userPrefs/ninjabrainbot/prefs.xml"))
}

fn entry(text: &str, key: &str) -> Option<String> {
    let needle = format!("key=\"{}\" value=\"", key);
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

/// Read every hotkey out of a preferences file. Missing entries come back as
/// zero, which reads as "unset" rather than as an error: ninb only writes the
/// ones that have been set.
pub fn read(text: &str) -> Vec<(String, Hotkey)> {
    ACTIONS
        .iter()
        .map(|(name, _)| {
            let hotkey = Hotkey {
                code: entry(text, &format!("hotkey_{}_code", name))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                modifier: entry(text, &format!("hotkey_{}_modifier", name))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
            };
            ((*name).to_string(), hotkey)
        })
        .collect()
}

fn set_entry(text: &str, key: &str, value: i32) -> String {
    let needle = format!("key=\"{}\" value=\"", key);

    if let Some(start) = text.find(&needle) {
        let from = start + needle.len();
        if let Some(len) = text[from..].find('"') {
            return format!("{}{}{}", &text[..from], value, &text[from + len..]);
        }
    }

    // Not set yet. Java does not care about order, so the end of the map will
    // do, and the file keeps the rest of ninb's settings untouched.
    let line = format!("  <entry key=\"{}\" value=\"{}\"/>\n", key, value);
    match text.rfind("</map>") {
        Some(at) => format!("{}{}{}", &text[..at], line, &text[at..]),
        None => text.to_string(),
    }
}

/// Write hotkeys back, leaving every other preference exactly as it was.
pub fn write(text: &str, hotkeys: &[(String, Hotkey)]) -> String {
    let mut out = text.to_string();

    for (name, hotkey) in hotkeys {
        out = set_entry(&out, &format!("hotkey_{}_code", name), hotkey.code);
        out = set_entry(&out, &format!("hotkey_{}_modifier", name), hotkey.modifier);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n",
        "<!DOCTYPE map SYSTEM \"http://java.sun.com/dtd/preferences.dtd\">\n",
        "<map MAP_XML_VERSION=\"1.0\">\n",
        "  <entry key=\"hotkey_increment_code\" value=\"69175\"/>\n",
        "  <entry key=\"hotkey_increment_modifier\" value=\"0\"/>\n",
        "  <entry key=\"sensitivity\" value=\"0.02291165\"/>\n",
        "</map>\n",
    );

    #[test]
    fn reads_what_ninb_wrote() {
        let keys = read(SAMPLE);
        let increment = keys.iter().find(|(n, _)| n == "increment").unwrap().1;

        // 69175 is print screen at the standard key location
        assert_eq!(increment.code, 69175);
        assert_eq!(increment.key(), Some("Print Screen"));
        assert_eq!(increment.label(), "Print Screen");

        // absent entries are unset, not an error
        let undo = keys.iter().find(|(n, _)| n == "undo").unwrap().1;
        assert_eq!(undo.code, 0);
        assert_eq!(undo.label(), "unset");
    }

    #[test]
    fn writing_leaves_the_rest_of_the_file_alone() {
        let hotkeys = vec![("increment".to_string(), Hotkey::new("Page Up", 0).unwrap())];
        let out = write(SAMPLE, &hotkeys);

        assert!(out.contains("key=\"hotkey_increment_code\" value=\"69193\""));
        assert!(out.contains("key=\"sensitivity\" value=\"0.02291165\""));
        assert!(out.contains("<!DOCTYPE map"));
        assert!(out.ends_with("</map>\n"));
    }

    #[test]
    fn a_hotkey_ninb_never_set_is_added() {
        let hotkeys = vec![("reset".to_string(), Hotkey::new("F7", MOD_CTRL).unwrap())];
        let out = write(SAMPLE, &hotkeys);

        assert!(out.contains("key=\"hotkey_reset_code\" value=\"65601\""));
        assert!(out.contains("key=\"hotkey_reset_modifier\" value=\"2\""));
        assert!(out.ends_with("</map>\n"));
    }

    #[test]
    fn a_modifier_ninb_saved_on_the_right_is_still_named() {
        // ninb records the side that was pressed, so a file can hold masks
        // this editor never writes
        let key = Hotkey { code: 200283, modifier: MOD_META_R };
        assert_eq!(key.label(), "Super-Super");
        assert_eq!(key.key(), Some("Super"));
    }

    #[test]
    fn modifiers_round_trip_through_the_label() {
        let key = Hotkey::new("Page Down", MOD_CTRL | MOD_SHIFT).unwrap();
        assert_eq!(key.label(), "Ctrl-Shift-Page Down");
        assert_eq!(key.key(), Some("Page Down"));
    }
}
