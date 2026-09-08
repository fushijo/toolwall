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
