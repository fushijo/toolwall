//! Editing Ninjabrain Bot's own hotkeys from the Ninjabrain tab.
//!
//! ninb watches the keyboard globally rather than being sent keys, so these
//! cannot be toolwall keybinds. They live in ninb's settings window, which is
//! exactly what `theme.ninb_hidden` takes away, so they are edited here.
//!
//! ninb reads its preferences once at startup and writes them back when it
//! exits, so editing the file under a running ninb would be undone the moment
//! it closes. Saving therefore restarts it.

use std::process::Command;

use toolwall_core::ninb_prefs::{self, Hotkey};

use crate::widgets::expand_tilde;

#[derive(Default)]
pub struct NinbKeys {
    /// None until the file has been read, so the tab can tell "not loaded yet"
    /// apart from "ninb has never been run".
    pub hotkeys: Option<Vec<(String, Hotkey)>>,
    pub capturing: Option<usize>,
    pub status: Option<String>,
}

impl NinbKeys {
    pub fn load(&mut self) {
        self.capturing = None;

        let Some(path) = ninb_prefs::path() else {
            self.status = Some("no HOME, so ninb's settings cannot be found".into());
            return;
        };

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.hotkeys = Some(ninb_prefs::read(&text));
                self.status = None;
            }
            Err(err) => {
                self.status = Some(format!("{}: {}", path.display(), err));
            }
        }
    }

    /// Write the hotkeys back, then restart ninb so it reloads them and does
    /// not overwrite the file on the way out.
    pub fn save(&mut self, launch: Option<&str>) {
        let Some(hotkeys) = &self.hotkeys else { return };
        let Some(path) = ninb_prefs::path() else { return };

        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.status = Some(format!("could not read {}: {}", path.display(), err));
                return;
            }
        };

        if let Err(err) = std::fs::write(&path, ninb_prefs::write(&text, hotkeys)) {
            self.status = Some(format!("could not write {}: {}", path.display(), err));
            return;
        }

        self.status = Some(match restart(launch) {
            Ok(true) => "Saved, and Ninjabrain Bot restarted.".into(),
            Ok(false) => "Saved. Start Ninjabrain Bot again to pick them up.".into(),
            Err(err) => format!("Saved, but the restart failed: {}. Restart it yourself.", err),
        });
    }
}

/// Stop ninb and start it again. Returns false when there was no launch
/// command to start it with, which is not a failure: the new settings are on
/// disk either way, they just will not be read until ninb next starts.
fn restart(launch: Option<&str>) -> Result<bool, String> {
    Command::new("pkill")
        .args(["-f", "Ninjabrain-Bot"])
        .status()
        .map_err(|e| e.to_string())?;

    let Some(command) = launch.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(false);
    };

    // ninb writes its preferences on the way out, so it has to be gone before
    // the new file can be considered safe.
    std::thread::sleep(std::time::Duration::from_millis(600));

    // setsid so it outlives this editor. waywall's own launcher checks the
    // process table before starting one, so this cannot cause a second copy.
    Command::new("setsid")
        .args(["sh", "-c", command])
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(true)
}

/// The launch command for ninb, with the jar path filled in.
pub fn launch_command(jar: &str, template: &str) -> Option<String> {
    if jar.trim().is_empty() {
        return None;
    }

    let template = if template.trim().is_empty() { "java -jar {jar}" } else { template };
    Some(template.replace("{jar}", &expand_tilde(jar)))
}

/// egui's key names are not JNativeHook's, so they are matched by the label
/// toolwall shows. Anything with no entry in that table cannot be bound.
pub fn key_name(key: egui::Key) -> Option<&'static str> {
    use egui::Key::*;

    let name = match key {
        ArrowDown => "Down",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        Escape => "Escape",
        Tab => "Tab",
        Backspace => "Backspace",
        Enter => "Enter",
        Space => "Space",
        Insert => "Insert",
        Delete => "Delete",
        Home => "Home",
        End => "End",
        PageUp => "Page Up",
        PageDown => "Page Down",
        Minus => "-",
        Equals => "=",
        OpenBracket => "[",
        CloseBracket => "]",
        Backslash => "\\",
        Semicolon => ";",
        Quote => "'",
        Comma => ",",
        Period => ".",
        Slash => "/",
        Backtick => "`",
        Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
        Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",
        A => "A", B => "B", C => "C", D => "D", E => "E", F => "F", G => "G",
        H => "H", I => "I", J => "J", K => "K", L => "L", M => "M", N => "N",
        O => "O", P => "P", Q => "Q", R => "R", S => "S", T => "T", U => "U",
        V => "V", W => "W", X => "X", Y => "Y", Z => "Z",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
        F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11",
        F12 => "F12",
        _ => return None,
    };

    Some(name)
}

/// Turn a captured keypress into what ninb stores.
///
/// Modifiers go in as the left-hand key. ninb requires every modifier bit it
/// saved to be held, and the bit for left control is not the bit for right
/// control, so a hotkey saved here fires on the left one.
pub fn capture(ctx: &egui::Context) -> Option<Hotkey> {
    ctx.input(|input| {
        input.events.iter().find_map(|event| {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = event else {
                return None;
            };

            let name = key_name(*key)?;

            let mut mask = 0;
            if modifiers.ctrl {
                mask |= ninb_prefs::MOD_CTRL;
            }
            if modifiers.alt {
                mask |= ninb_prefs::MOD_ALT;
            }
            if modifiers.command && !modifiers.ctrl {
                mask |= ninb_prefs::MOD_META;
            }
            if modifiers.shift {
                mask |= ninb_prefs::MOD_SHIFT;
            }

            Hotkey::new(name, mask)
        })
    })
}
