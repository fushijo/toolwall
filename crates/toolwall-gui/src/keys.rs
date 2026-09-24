//! Turning a real keypress into a waywall input string.
//!
//! waywall parses an input string by splitting on `-` and matching each
//! element case-insensitively against its modifier names and libxkbcommon
//! keysyms. So `Ctrl-Shift-N` and `shift-ctrl-n` are equivalent, and element
//! order does not matter to the parser - we just emit a consistent one.

/// The waywall keysym name for an egui key, where one exists.
///
/// Anything unmapped falls through to the free-text field, which stays
/// editable precisely because this table cannot cover every keysym.
pub fn keysym(key: egui::Key) -> Option<&'static str> {
    use egui::Key::*;

    Some(match key {
        ArrowDown => "Down",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",

        Escape => "Escape",
        Tab => "Tab",
        Backspace => "BackSpace",
        Enter => "Return",
        Space => "space",
        Insert => "Insert",
        Delete => "Delete",
        Home => "Home",
        End => "End",
        PageUp => "Page_Up",
        PageDown => "Page_Down",

        Minus => "minus",
        Equals => "equal",
        Comma => "comma",
        Period => "period",
        Semicolon => "semicolon",
        Colon => "colon",
        Backslash => "backslash",
        Slash => "slash",
        Pipe => "bar",
        Questionmark => "question",
        OpenBracket => "bracketleft",
        CloseBracket => "bracketright",
        Backtick => "grave",
        Quote => "apostrophe",
        Plus => "plus",

        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",

        A => "A",
        B => "B",
        C => "C",
        D => "D",
        E => "E",
        F => "F",
        G => "G",
        H => "H",
        I => "I",
        J => "J",
        K => "K",
        L => "L",
        M => "M",
        N => "N",
        O => "O",
        P => "P",
        Q => "Q",
        R => "R",
        S => "S",
        T => "T",
        U => "U",
        V => "V",
        W => "W",
        X => "X",
        Y => "Y",
        Z => "Z",

        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",

        _ => return None,
    })
}

/// Format a captured key plus its held modifiers as a waywall input string.
pub fn format(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    let name = keysym(key)?;

    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    // egui reports the Super/Meta key as `command` on non-Mac too.
    if modifiers.command && !modifiers.ctrl {
        parts.push("Super");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.push(name);

    Some(parts.join("-"))
}

/// Read the first real keypress this frame, ignoring bare modifier presses.
pub fn captured(ctx: &egui::Context) -> Option<String> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key { key, pressed: true, modifiers, .. } => format(*key, *modifiers),
            _ => None,
        })
    })
}

/// The waywall *keycode* name for an egui key, where one exists.
///
/// Remaps are not keybinds. waywall matches a remap half against
/// `util_keycodes`, which are the names from `linux/input-event-codes.h`, so
/// the key that a keybind calls `Escape` a remap has to call `ESC`. Letters,
/// digits and the function keys are spelled the same in both, which is what
/// makes the difference so easy to miss: the remaps people try first work,
/// and the first punctuation key they add takes the whole config down.
///
/// Shifted punctuation resolves to the physical key underneath it. A keycode
/// names a key, not a character, so `?` is the same input as `/`.
pub fn keycode(key: egui::Key) -> Option<&'static str> {
    use egui::Key::*;

    Some(match key {
        ArrowDown => "DOWN",
        ArrowLeft => "LEFT",
        ArrowRight => "RIGHT",
        ArrowUp => "UP",

        Escape => "ESC",
        Tab => "TAB",
        Backspace => "BACKSPACE",
        Enter => "ENTER",
        Space => "SPACE",
        Insert => "INSERT",
        Delete => "DELETE",
        Home => "HOME",
        End => "END",
        PageUp => "PAGEUP",
        PageDown => "PAGEDOWN",

        Minus => "MINUS",
        Equals => "EQUAL",
        Comma => "COMMA",
        Period => "DOT",
        Semicolon => "SEMICOLON",
        Backslash => "BACKSLASH",
        Slash => "SLASH",
        OpenBracket => "LEFTBRACE",
        CloseBracket => "RIGHTBRACE",
        Backtick => "GRAVE",
        Quote => "APOSTROPHE",

        // Shifted characters, mapped to the key you actually press.
        Colon => "SEMICOLON",
        Pipe => "BACKSLASH",
        Questionmark => "SLASH",
        Plus => "EQUAL",

        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",

        A => "A",
        B => "B",
        C => "C",
        D => "D",
        E => "E",
        F => "F",
        G => "G",
        H => "H",
        I => "I",
        J => "J",
        K => "K",
        L => "L",
        M => "M",
        N => "N",
        O => "O",
        P => "P",
        Q => "Q",
        R => "R",
        S => "S",
        T => "T",
        U => "U",
        V => "V",
        W => "W",
        X => "X",
        Y => "Y",
        Z => "Z",

        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",

        _ => return None,
    })
}

/// Read the first real keypress this frame as a remap name.
///
/// Modifiers are dropped rather than joined on. A remap half is matched
/// whole, so `Ctrl-N` reads as a name of its own, one waywall does not have,
/// and writing it aborts the entire config load.
pub fn captured_keycode(ctx: &egui::Context) -> Option<String> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key { key, pressed: true, .. } => {
                keycode(*key).map(str::to_string)
            }
            egui::Event::PointerButton { button, pressed: true, .. } => match button {
                egui::PointerButton::Extra1 => Some("mb4".to_string()),
                egui::PointerButton::Extra2 => Some("mb5".to_string()),
                egui::PointerButton::Middle => Some("mmb".to_string()),
                _ => None,
            },
            _ => None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name this can emit has to be one waywall will actually parse.
    #[test]
    fn captured_remap_names_are_all_valid() {
        use egui::Key;

        for key in Key::ALL {
            if let Some(name) = keycode(*key) {
                assert!(
                    toolwall_core::keycodes::is_valid(name),
                    "{key:?} maps to {name:?}, which waywall does not accept",
                );
            }
        }

        for button in ["mb4", "mb5", "mmb"] {
            assert!(toolwall_core::keycodes::is_valid(button));
        }
    }

    /// The bug this table exists to fix: keysyms and keycodes diverge for
    /// everything that is not a letter, a digit or a function key.
    #[test]
    fn keycodes_are_not_keysyms() {
        assert_eq!(keysym(egui::Key::Escape), Some("Escape"));
        assert_eq!(keycode(egui::Key::Escape), Some("ESC"));

        assert_eq!(keysym(egui::Key::OpenBracket), Some("bracketleft"));
        assert_eq!(keycode(egui::Key::OpenBracket), Some("LEFTBRACE"));

        // ... and agree for the ones that made the bug hard to spot.
        assert_eq!(keysym(egui::Key::A), keycode(egui::Key::A));
        assert_eq!(keysym(egui::Key::F3), keycode(egui::Key::F3));
    }
}

/// A key as a person would write it, for display only.
///
/// The config stores X11 keysyms, which is what waywall reads, so a keybind
/// list shows you `backslash` and `equal` and `Caps_Lock` when the key on your
/// keyboard says `\` and `=` and `Caps Lock`. Nothing here changes what is
/// written to the file.
pub fn pretty(input: &str) -> String {
    input.split('-').map(pretty_part).collect::<Vec<_>>().join(" + ")
}

fn pretty_part(part: &str) -> String {
    match part {
        "minus" => "-",
        "equal" => "=",
        "comma" => ",",
        "period" => ".",
        "semicolon" => ";",
        "colon" => ":",
        "backslash" => "\\",
        "slash" => "/",
        "bar" => "|",
        "question" => "?",
        "bracketleft" => "[",
        "bracketright" => "]",
        "grave" => "`",
        "apostrophe" => "'",
        "plus" => "+",
        "space" => "Space",
        "Return" => "Enter",
        "BackSpace" => "Backspace",
        "Prior" => "Page Up",
        "Next" => "Page Down",
        // Left_Alt and friends: the underscore is the only thing between the
        // keysym and the label on the key.
        other => return other.replace('_', " "),
    }
    .to_string()
}

#[cfg(test)]
mod pretty_tests {
    use super::pretty;

    #[test]
    fn a_keysym_reads_as_the_key_it_is_printed_on() {
        assert_eq!(pretty("backslash"), "\\");
        assert_eq!(pretty("Caps_Lock"), "Caps Lock");
        assert_eq!(pretty("Shift-Z"), "Shift + Z");
        assert_eq!(pretty("Ctrl-bracketleft"), "Ctrl + [");
        assert_eq!(pretty("F7"), "F7");

        // The wildcard modifier is not a key and must survive untouched.
        assert_eq!(pretty("*-F3"), "* + F3");
    }
}
