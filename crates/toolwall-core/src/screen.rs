//! What the monitors say they are, for suggesting a window size.
//!
//! A suggestion and never a default, because the two numbers are not the same
//! thing. This reads the mode the display is running, in physical pixels.
//! waywall's window is measured in the compositor's logical pixels, and on a
//! scaled desktop those differ: a 2560x1600 laptop panel at 150% gives waywall
//! a 1707x1067 window. Filling that in automatically would place every overlay
//! off the side of the screen, which is the exact bug this is meant to help
//! with.
//!
//! So: offer it, say where it came from, and let them read the real number off
//! the Display line in F3.

/// One connected output and the mode it is running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// The connector, e.g. `eDP-1` or `DP-2`.
    pub name: String,
    pub width: u32,
    pub height: u32,
}

/// Every connected monitor, from the kernel rather than from a display server.
///
/// `/sys/class/drm` is there on any Linux with a graphics driver, which means
/// this works the same under X11, Wayland, and from a script with no session
/// at all. `xrandr` and `wlr-randr` each cover half of that and are often not
/// installed.
pub fn monitors() -> Vec<Monitor> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };

    let mut found = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        // Connector directories are card0-eDP-1 and the like. The bare
        // card0 has no status file, so it falls out below anyway.
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some((_, name)) = dir_name.split_once('-') else { continue };

        match std::fs::read_to_string(path.join("status")) {
            Ok(status) if status.trim() == "connected" => {}
            _ => continue,
        }

        let Ok(modes) = std::fs::read_to_string(path.join("modes")) else { continue };

        // The first line is the preferred mode, which is what it is running
        // unless somebody has changed it.
        let Some(first) = modes.lines().next() else { continue };
        let Some((w, h)) = first.trim().split_once('x') else { continue };

        let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.parse::<u32>()) else { continue };
        if width == 0 || height == 0 {
            continue;
        }

        found.push(Monitor { name: name.to_string(), width, height });
    }

    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// A one-line hint for a human, or nothing if there is nothing useful to say.
///
/// Deliberately hedged. Being wrong here costs somebody an afternoon.
pub fn hint() -> Option<String> {
    let found = monitors();
    if found.is_empty() {
        return None;
    }

    let list = found
        .iter()
        .map(|m| format!("{} is {}x{}", m.name, m.width, m.height))
        .collect::<Vec<_>>()
        .join(", ");

    Some(format!(
        "Your {list}. If your desktop is scaled, waywall's window is smaller \
         than that, and the Display line in F3 is the number you want."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_with_no_drm_says_nothing_rather_than_guessing() {
        // Containers and CI have no /sys/class/drm. An empty answer has to be
        // fine, because the alternative is inventing a resolution.
        let found = monitors();
        for m in &found {
            assert!(!m.name.is_empty(), "a connector with no name");
            assert!(m.width > 0 && m.height > 0, "a mode of {}x{}", m.width, m.height);
        }

        // hint() agrees with monitors() about whether there is anything to say.
        assert_eq!(hint().is_some(), !found.is_empty());
    }

    #[test]
    fn the_hint_never_claims_to_be_the_answer() {
        // The whole point: a scaled desktop makes the monitor size the wrong
        // number, so the wording has to send them to F3.
        if let Some(text) = hint() {
            assert!(text.contains("F3"), "the hint does not say where to look: {text}");
            assert!(text.contains("scaled"), "the hint does not mention scaling: {text}");
        }
    }
}
