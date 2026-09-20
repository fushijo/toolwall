//! The config the setup window builds, and the choices that shape it.
//!
//! The starting point is `examples/default.json`, compiled in rather than
//! read from disk: it has to be there on a machine where the tarball was
//! deleted an hour ago, and it is already covered by a test that loads and
//! validates it.
//!
//! Everything here is deliberately outside the GUI. Deciding what a config
//! ends up containing is the part worth testing, and a test should not have
//! to open a window to do it.

use crate::schema::{Command, Document, Keybind, Mode};
use crate::sens::Sens;

/// The prebuilt config, as shipped.
pub const PRESET_JSON: &str = include_str!("../../../examples/default.json");

/// A fresh copy of it.
///
/// Panics only if the shipped file stops parsing, which a test catches long
/// before anyone runs this.
pub fn preset() -> Document {
    serde_json::from_str(PRESET_JSON).expect("the shipped preset does not parse")
}

/// How an overlay is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayState {
    /// On a key, which is how most of them start.
    Bound(String),
    /// In the scene whenever its mode is, with no key to hide it.
    AlwaysOn,
    /// Not in the config at all.
    Off,
}

/// One resolution you can switch to, and the key that switches to it.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenChoice {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
    pub input: String,
}

/// One mirror or overlay image.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayChoice {
    pub id: String,
    pub label: String,
    /// The modes it appears in, for showing next to it.
    pub modes: Vec<String>,
    pub state: OverlayState,
}

/// Everything the setup window collects.
#[derive(Debug, Clone, PartialEq)]
pub struct Choices {
    pub screens: Vec<ScreenChoice>,
    pub overlays: Vec<OverlayChoice>,
    /// The key that opens the editor. Never allowed to be empty.
    pub editor_key: String,
    /// Back to the normal resolution. Empty removes it.
    pub reset_key: String,
    pub background: String,
    pub font_path: String,
    /// Normal and tall waywall sensitivities, once the calculator has run.
    pub sensitivity: Option<Sens>,
    /// Framebuffer height a normal resolution has, for deciding which modes
    /// count as tall.
    pub normal_height: u32,
    pub ninb_jar: String,
}

fn label_of(id: &str, label: &Option<String>) -> String {
    label.clone().unwrap_or_else(|| id.to_string())
}

/// Read a document back into the choices that would rebuild it.
///
/// So the window can start from the preset, or from the config an import
/// just produced, without caring which.
pub fn choices_for(doc: &Document) -> Choices {
    let bind_for = |want: Command, arg: &str, value: &str| -> Option<String> {
        doc.keybinds
            .iter()
            .find(|b| {
                b.command == want
                    && b.args
                        .as_ref()
                        .and_then(|a| a.get(arg))
                        .and_then(|v| v.as_str())
                        == Some(value)
            })
            .map(|b| b.input.clone())
    };

    let screens = doc
        .modes
        .iter()
        .map(|mode| ScreenChoice {
            id: mode.id.clone(),
            label: label_of(&mode.id, &mode.label),
            enabled: true,
            width: mode.resolution.width,
            height: mode.resolution.height,
            input: bind_for(Command::ModeSet, "mode", &mode.id).unwrap_or_default(),
        })
        .collect();

    let mut overlays = Vec::new();
    for (id, label) in doc
        .mirrors
        .iter()
        .map(|m| (&m.id, &m.label))
        .chain(doc.images.iter().map(|i| (&i.id, &i.label)))
    {
        let modes: Vec<String> = doc
            .modes
            .iter()
            .filter(|m| m.mirrors.contains(id) || m.images.contains(id))
            .map(|m| label_of(&m.id, &m.label))
            .collect();

        let state = match bind_for(Command::OverlayToggle, "overlay", id) {
            Some(input) => OverlayState::Bound(input),
            None => OverlayState::AlwaysOn,
        };

        overlays.push(OverlayChoice { id: id.clone(), label: label_of(id, label), modes, state });
    }

    let key_for = |want: Command| {
        doc.keybinds.iter().find(|b| b.command == want).map(|b| b.input.clone())
    };

    Choices {
        screens,
        overlays,
        editor_key: key_for(Command::GuiToggle).unwrap_or_else(|| "Ctrl-I".into()),
        reset_key: key_for(Command::ModeReset).unwrap_or_default(),
        background: doc.theme.background.clone(),
        font_path: doc.gui.appearance.font_path.clone(),
        sensitivity: None,
        normal_height: doc.gui.screen.h.max(1) as u32,
        ninb_jar: doc.ninb.jar.clone(),
    }
}

/// Build the config those choices describe.
///
/// Starts from `base` so everything nobody was asked about, the mirror
/// rectangles most of all, survives untouched.
pub fn build(base: &Document, choices: &Choices) -> Document {
    let mut doc = base.clone();

    // ---- overlays that are off leave the scene entirely ----
    let dropped: Vec<&str> = choices
        .overlays
        .iter()
        .filter(|o| o.state == OverlayState::Off)
        .map(|o| o.id.as_str())
        .collect();

    doc.mirrors.retain(|m| !dropped.contains(&m.id.as_str()));
    doc.images.retain(|i| !dropped.contains(&i.id.as_str()));
    doc.base_overlays.retain(|id| !dropped.contains(&id.as_str()));
    for mode in &mut doc.modes {
        mode.mirrors.retain(|id| !dropped.contains(&id.as_str()));
        mode.images.retain(|id| !dropped.contains(&id.as_str()));
    }

    // ---- screens ----
    let keep: Vec<&str> = choices
        .screens
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.as_str())
        .collect();

    doc.modes.retain(|m| keep.contains(&m.id.as_str()));

    for screen in choices.screens.iter().filter(|s| s.enabled) {
        if let Some(mode) = doc.modes.iter_mut().find(|m| m.id == screen.id) {
            mode.resolution.width = screen.width;
            mode.resolution.height = screen.height;
        }
    }

    if doc.default_mode.as_deref().is_some_and(|id| !keep.contains(&id)) {
        doc.default_mode = None;
    }

    // A mirror nothing shows any more is dead weight in the file.
    prune_unused(&mut doc);

    // ---- keybinds, rebuilt rather than edited ----
    //
    // Editing in place would leave behind binds for modes and overlays that
    // are no longer here, which is how you end up with a config the validator
    // refuses over something the window never showed you.
    let mut binds = Vec::new();

    binds.push(Keybind {
        f3_safe: true,
        ingame_only: false,
        input: choices.editor_key.clone(),
        command: Command::GuiToggle,
        args: None,
        label: Some("Open settings".into()),
    });

    for screen in choices.screens.iter().filter(|s| s.enabled) {
        if screen.input.trim().is_empty() {
            continue;
        }
        binds.push(Keybind {
            f3_safe: true,
            ingame_only: false,
            input: screen.input.clone(),
            command: Command::ModeSet,
            args: Some(serde_json::json!({ "mode": screen.id })),
            label: Some(screen.label.clone()),
        });
    }

    if !choices.reset_key.trim().is_empty() {
        binds.push(Keybind {
            f3_safe: true,
            ingame_only: false,
            input: choices.reset_key.clone(),
            command: Command::ModeReset,
            args: None,
            label: Some("Reset".into()),
        });
    }

    for overlay in &choices.overlays {
        let OverlayState::Bound(input) = &overlay.state else { continue };
        if input.trim().is_empty() {
            continue;
        }
        // An overlay whose every mode went away has nothing to toggle.
        if !doc.mirrors.iter().any(|m| m.id == overlay.id)
            && !doc.images.iter().any(|i| i.id == overlay.id)
        {
            continue;
        }
        binds.push(Keybind {
            f3_safe: true,
            ingame_only: false,
            input: input.clone(),
            command: Command::OverlayToggle,
            args: Some(serde_json::json!({ "overlay": overlay.id })),
            label: Some(overlay.label.clone()),
        });
    }

    // Anything the window does not offer, kept as it was: a ninb key, an
    // exec an import carried across, a bind someone added by hand.
    for bind in &base.keybinds {
        let ours = matches!(
            bind.command,
            Command::GuiToggle | Command::ModeSet | Command::ModeReset | Command::OverlayToggle
        );
        if !ours {
            binds.push(bind.clone());
        }
    }

    // Two binds on one key is a config the runtime warns about and half
    // ignores. Last one in loses, which keeps the editor key.
    let mut seen = std::collections::HashSet::new();
    binds.retain(|b| seen.insert(b.input.to_lowercase()));

    doc.keybinds = binds;

    // ---- the rest ----
    doc.theme.background = choices.background.clone();
    doc.gui.appearance.font_path = choices.font_path.clone();
    doc.ninb.jar = choices.ninb_jar.clone();

    if let Some(sens) = choices.sensitivity {
        apply_sensitivity(&mut doc, sens, choices.normal_height);
    }

    doc
}

/// Drop mirrors and images that nothing shows.
fn prune_unused(doc: &mut Document) {
    let used: std::collections::HashSet<String> = doc
        .modes
        .iter()
        .flat_map(|m| m.mirrors.iter().chain(m.images.iter()))
        .chain(doc.base_overlays.iter())
        .cloned()
        .collect();

    doc.mirrors.retain(|m| used.contains(&m.id));
    doc.images.retain(|i| used.contains(&i.id));
}

/// Put the calculator's answer where waywall reads it.
///
/// The normal coefficient is the document-wide one. The tall coefficient only
/// goes on modes whose framebuffer is taller than the screen, because that is
/// the only case the calculation describes: it comes from how much of the
/// vertical FOV a taller framebuffer squeezes into the same window. A wide
/// mode is a different shape of problem and gets left inheriting the normal
/// value rather than a number nobody derived.
pub fn apply_sensitivity(doc: &mut Document, sens: Sens, normal_height: u32) {
    doc.input.sensitivity = sens.normal;

    for mode in &mut doc.modes {
        if mode.resolution.height > normal_height {
            mode.sensitivity = Some(crate::sens::boat_eye(
                // Re-derive per mode: two tall modes of different heights do
                // not share a coefficient.
                mc_for(sens.normal),
                normal_height,
                mode.resolution.height,
                crate::sens::DEFAULT_VFOV,
            ).tall);
        } else {
            mode.sensitivity = None;
        }
    }
}

/// The Minecraft sensitivity that produces this normal coefficient.
///
/// `boat_eye` only needs the coefficient to scale, and the scale is linear in
/// the effective sensitivity, so going back through it is exact rather than a
/// search.
fn mc_for(normal: f64) -> f64 {
    let effective_target = normal * (crate::sens::BOAT_EYE_MC_SENS * 0.6 + 0.2).powf(3.0) * 8.0;
    (((effective_target / 8.0).cbrt()) - 0.2) / 0.6
}

/// The modes a tall sensitivity would be applied to, for showing in the UI.
pub fn tall_modes(doc: &Document, normal_height: u32) -> Vec<&Mode> {
    doc.modes.iter().filter(|m| m.resolution.height > normal_height).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problems;

    #[test]
    fn the_preset_round_trips_through_its_own_choices() {
        // Changing nothing has to produce a config that still works, or every
        // other test here is measuring the wrong thing.
        let base = preset();
        let choices = choices_for(&base);
        let built = build(&base, &choices);

        assert!(problems(&built).is_empty(), "{:?}", problems(&built));
        assert_eq!(built.modes.len(), base.modes.len(), "same modes");
        assert_eq!(built.mirrors.len(), base.mirrors.len(), "same mirrors");

        for bind in &base.keybinds {
            assert!(
                built.keybinds.iter().any(|b| b.input == bind.input && b.command == bind.command),
                "lost the bind on {}",
                bind.input
            );
        }
    }

    #[test]
    fn dropping_a_screen_takes_its_overlays_with_it() {
        let base = preset();
        let mut choices = choices_for(&base);

        // eye is the only mode carrying the measuring grid.
        for screen in &mut choices.screens {
            screen.enabled = screen.id != "eye";
        }

        let built = build(&base, &choices);

        assert!(built.modes.iter().all(|m| m.id != "eye"), "the mode is gone");
        assert!(built.images.is_empty(), "and the image only it used");
        assert!(
            built.mirrors.iter().all(|m| m.id != "eye_zoom"),
            "and the mirror only it used"
        );

        // entity_count was in thin as well, so it stays.
        assert!(built.mirrors.iter().any(|m| m.id == "entity_count"));

        // And no keybind is left pointing at any of it.
        for bind in &built.keybinds {
            let arg = bind.args.as_ref().and_then(|a| a.get("mode")).and_then(|v| v.as_str());
            assert_ne!(arg, Some("eye"), "a bind still switches to the dropped mode");
        }
        assert!(problems(&built).is_empty(), "{:?}", problems(&built));
    }

    #[test]
    fn an_overlay_can_be_on_a_key_always_on_or_gone() {
        let base = preset();
        let mut choices = choices_for(&base);

        for overlay in &mut choices.overlays {
            overlay.state = match overlay.id.as_str() {
                "pie_chart" => OverlayState::AlwaysOn,
                "entity_count" => OverlayState::Off,
                _ => overlay.state.clone(),
            };
        }

        let built = build(&base, &choices);

        // always on: still in the scene, no key for it
        assert!(built.mirrors.iter().any(|m| m.id == "pie_chart"));
        assert!(built.modes.iter().any(|m| m.mirrors.contains(&"pie_chart".to_string())));
        assert!(!built.keybinds.iter().any(|b| {
            b.args.as_ref().and_then(|a| a.get("overlay")).and_then(|v| v.as_str())
                == Some("pie_chart")
        }));

        // off: gone from the mirrors and from every mode that listed it
        assert!(built.mirrors.iter().all(|m| m.id != "entity_count"));
        for mode in &built.modes {
            assert!(!mode.mirrors.contains(&"entity_count".to_string()));
        }
        assert!(problems(&built).is_empty(), "{:?}", problems(&built));
    }

    #[test]
    fn keys_are_taken_from_the_choices() {
        let base = preset();
        let mut choices = choices_for(&base);

        choices.editor_key = "Ctrl-semicolon".into();
        choices.reset_key = String::new();
        for screen in &mut choices.screens {
            if screen.id == "thin" {
                screen.input = "Alt_L".into();
            }
        }

        let built = build(&base, &choices);

        let editor = built.keybinds.iter().find(|b| b.command == Command::GuiToggle).unwrap();
        assert_eq!(editor.input, "Ctrl-semicolon");

        assert!(
            !built.keybinds.iter().any(|b| b.command == Command::ModeReset),
            "an empty key removes the bind rather than writing a blank one"
        );

        let thin = built
            .keybinds
            .iter()
            .find(|b| {
                b.args.as_ref().and_then(|a| a.get("mode")).and_then(|v| v.as_str()) == Some("thin")
            })
            .unwrap();
        assert_eq!(thin.input, "Alt_L");
        assert!(problems(&built).is_empty(), "{:?}", problems(&built));
    }

    #[test]
    fn two_things_cannot_end_up_on_one_key() {
        let base = preset();
        let mut choices = choices_for(&base);

        // The runtime warns and ignores one of them, so the window must not
        // be able to produce this.
        choices.editor_key = "B".into();
        for screen in &mut choices.screens {
            if screen.id == "thin" {
                screen.input = "b".into(); // and case does not save it
            }
        }

        let built = build(&base, &choices);
        let on_b = built.keybinds.iter().filter(|b| b.input.eq_ignore_ascii_case("b")).count();
        assert_eq!(on_b, 1, "one bind on B");
        assert_eq!(
            built.keybinds.iter().filter(|b| b.command == Command::GuiToggle).count(),
            1,
            "and the editor is the one that kept it"
        );
    }

    #[test]
    fn a_bind_the_window_never_offered_survives() {
        let mut base = preset();
        base.ninb.jar = "/opt/ninb.jar".into();
        base.keybinds.push(Keybind {
            f3_safe: true,
            ingame_only: false,
            input: "F7".into(),
            command: Command::NinbToggle,
            args: None,
            label: Some("Ninjabrain".into()),
        });

        let choices = choices_for(&base);
        let built = build(&base, &choices);

        assert!(
            built.keybinds.iter().any(|b| b.command == Command::NinbToggle && b.input == "F7"),
            "the ninb key was dropped"
        );
    }

    #[test]
    fn the_tall_sensitivity_only_goes_on_modes_that_are_tall() {
        let base = preset();
        let mut choices = choices_for(&base);
        choices.normal_height = 1080;
        choices.sensitivity = Some(crate::sens::boat_eye(0.5, 1080, 16384, crate::sens::DEFAULT_VFOV));

        let built = build(&base, &choices);

        assert!((built.input.sensitivity - 12.800000599064097).abs() < 1e-9, "normal");

        for mode in &built.modes {
            match mode.id.as_str() {
                // 384x16384
                "eye" => {
                    let tall = mode.sensitivity.expect("the tall mode needs its own");
                    assert!((tall - 0.8634803836976988).abs() < 1e-9, "tall: {tall}");
                }
                // 340x1080 and 1920x300 are not taller than the screen
                _ => assert_eq!(mode.sensitivity, None, "{} should inherit", mode.id),
            }
        }
    }

    #[test]
    fn two_tall_modes_of_different_heights_do_not_share_a_coefficient() {
        let mut base = preset();
        base.modes.push(Mode {
            id: "taller".into(),
            label: Some("Taller".into()),
            resolution: crate::schema::Resolution { width: 384, height: 8192 },
            sensitivity: None,
            toggle: true,
            mirrors: Vec::new(),
            images: Vec::new(),
        });

        let mut choices = choices_for(&base);
        choices.normal_height = 1080;
        choices.sensitivity = Some(crate::sens::boat_eye(0.5, 1080, 16384, crate::sens::DEFAULT_VFOV));

        let built = build(&base, &choices);

        let eye = built.modes.iter().find(|m| m.id == "eye").unwrap().sensitivity.unwrap();
        let taller = built.modes.iter().find(|m| m.id == "taller").unwrap().sensitivity.unwrap();

        assert!((eye - 0.8634803836976988).abs() < 1e-9, "16384");
        assert!((taller - 1.7264224556016685).abs() < 1e-9, "8192");
    }
}
