//! The config the setup window builds, and the choices that shape it.
//!
//! The starting point is `examples/default.json`, compiled into the binary.
//! It has to still be there on a machine where the tarball was deleted an
//! hour ago, and a test already loads and validates it.
//!
//! Everything here is deliberately outside the GUI. Deciding what a config
//! ends up containing is the part worth testing, and a test should not have
//! to open a window to do it.

use crate::schema::{Anchor, Command, Document, Keybind, Mode, Size};
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
    /// Chat mode: rebinds off, custom layout off, until you press it again.
    /// Empty removes it.
    pub chat_key: String,
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

        // Only a key that toggles this one overlay on its own. A key that
        // brings up a set is not something this window can describe, so it is
        // left alone below and the overlay reads as always on.
        let state = match doc
            .keybinds
            .iter()
            .find(|b| b.command == Command::OverlayToggle && b.overlay_ids() == [id.as_str()])
        {
            Some(bind) => OverlayState::Bound(bind.input.clone()),
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
        chat_key: key_for(Command::RemapsToggle).unwrap_or_default(),
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

    // ---- keybinds, rebuilt from scratch ----
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

    if !choices.chat_key.trim().is_empty() {
        binds.push(Keybind {
            f3_safe: true,
            // Chat mode off the title screen would leave the rebinds off with
            // nothing to put them back, since the automatic swap defers to it.
            ingame_only: false,
            input: choices.chat_key.clone(),
            command: Command::RemapsToggle,
            args: None,
            label: Some("Type in chat".into()),
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
            Command::GuiToggle
                | Command::ModeSet
                | Command::ModeReset
                | Command::RemapsToggle
        ) || (bind.command == Command::OverlayToggle && bind.overlay_ids().len() <= 1);
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

/// Reshape a config written for one screen so it lands the same way on
/// another.
///
/// The preset is authored at 1920x1080. Left alone on a 1366 wide laptop its
/// pie chart is off the edge of the screen, and on a 3440 ultrawide it is
/// stranded somewhere in the middle. Three separate people hit that in one
/// week in the waywall discord, which is how this came to exist.
///
/// Two things happen. Sizes and offsets scale by the height ratio, uniformly,
/// so nothing changes shape. And every overlay picks up a `dst_anchor` for
/// whichever side of the screen it was already nearer, so it stays put from
/// then on even if the screen changes again.
///
/// Anchoring by itself would keep things in the right corner at the wrong
/// size. Scaling by itself would be correct once and wrong after the next
/// monitor. It wants both.
pub fn fit_to_screen(doc: &mut Document, width: u32, height: u32) {
    let from = doc.gui.screen;
    if width == 0 || height == 0 || from.w == 0 || from.h == 0 {
        return;
    }

    let k = height as f64 / from.h as f64;
    let scale = |v: u32| ((v as f64 * k).round() as u32).max(1);
    let shift = |v: i32| (v as f64 * k).round() as i32;

    let old_width = from.w as i32;

    for (anchor, rect) in doc
        .mirrors
        .iter_mut()
        .map(|m| (&mut m.dst_anchor, &mut m.dst))
        .chain(doc.images.iter_mut().map(|i| (&mut i.dst_anchor, &mut i.dst)))
    {
        // Already anchored means somebody chose, and this is not the place to
        // argue with them.
        if anchor.is_some() {
            continue;
        }

        // Which edge it belongs to, decided while x is still a position.
        let to_right = rect.x + rect.w as i32 / 2 > old_width / 2;
        let from_right = old_width - rect.x;

        rect.w = scale(rect.w);
        rect.h = scale(rect.h);
        rect.y = shift(rect.y);

        // A right hand x becomes a distance from the right edge, which is
        // what the anchor convention means by x, and matches src_anchor.
        rect.x = if to_right { shift(from_right) } else { shift(rect.x) };
        *anchor = Some(if to_right { Anchor::TopRight } else { Anchor::TopLeft });
    }

    // The Ninjabrain readout is placed by hand and was not moving with
    // everything else, so rescaling a config left it wherever it had been in
    // the old coordinates. Reported as the readout ending up in the middle of
    // the screen: it had been against the right edge at 1707 wide, and 1700 is
    // the middle of 2560.
    //
    // Clamped too, because an offset already off the edge scales to further
    // off the edge, and an invisible readout looks the same as a broken one.
    {
        let o = &mut doc.ninb.overlay;
        o.x = shift(o.x).clamp(0, width.saturating_sub(1) as i32);
        o.y = shift(o.y).clamp(0, height.saturating_sub(1) as i32);
    }

    // Text is positioned, not sized, so only its offsets move.
    for text in &mut doc.text {
        text.x = shift(text.x);
        text.y = shift(text.y);
        if let Some(size) = text.size {
            text.size = Some(scale(size).max(1));
        }
    }

    doc.gui.screen = Size { w: width, h: height };
}

/// Drop mirrors and images that nothing shows.
///
/// A pie chart in a config with no screen to put it on is just a rectangle
/// having a quiet think to itself.
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
/// value, because nobody has derived a number for it.
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
/// the effective sensitivity, so going back through it is exact. no search,
/// no fudge factor.
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

    /// Where an anchored rectangle actually lands, the way the runtime works
    /// it out. Keeps the arithmetic in the tests honest.
    fn placed(rect: &crate::schema::Rect, anchor: Option<Anchor>, screen: Size) -> (i32, i32) {
        match anchor {
            Some(Anchor::TopRight) | Some(Anchor::BottomRight) => {
                (screen.w as i32 - rect.x, rect.y)
            }
            Some(Anchor::Center) => (
                (screen.w as i32 - rect.w as i32) / 2,
                (screen.h as i32 - rect.h as i32) / 2,
            ),
            _ => (rect.x, rect.y),
        }
    }

    #[test]
    fn fitting_to_a_bigger_screen_keeps_the_layout() {
        let mut doc = preset();

        // What it looks like at the size it was written for.
        let before: Vec<(String, i32, i32, u32)> = doc
            .mirrors
            .iter()
            .map(|m| (m.id.clone(), m.dst.x, m.dst.y, m.dst.w))
            .collect();

        fit_to_screen(&mut doc, 2560, 1440);
        let screen = Size { w: 2560, h: 1440 };
        let k = 1440.0 / 1080.0;

        assert_eq!(doc.gui.screen, screen, "the screen size is recorded");

        for (id, was_x, was_y, was_w) in before {
            let m = doc.mirrors.iter().find(|m| m.id == id).unwrap();
            let (x, y) = placed(&m.dst, m.dst_anchor, screen);

            assert_eq!(m.dst.w, (was_w as f64 * k).round() as u32, "{id} width scaled");
            assert_eq!(y, (was_y as f64 * k).round() as i32, "{id} y scaled");

            // The margin to whichever edge it was nearer scales with it.
            let was_right = was_x + was_w as i32 / 2 > 960;
            if was_right {
                let was_gap = 1920 - (was_x + was_w as i32);
                let gap = 2560 - (x + m.dst.w as i32);
                assert_eq!(gap, (was_gap as f64 * k).round() as i32, "{id} right margin");
            } else {
                assert_eq!(x, (was_x as f64 * k).round() as i32, "{id} left margin");
            }
        }
    }

    #[test]
    fn nothing_runs_off_the_edge_of_a_small_screen() {
        // The case that started this: a 1366x768 laptop, where the preset's
        // pie chart sits at x=1180 and is simply not on the screen.
        let mut doc = preset();
        fit_to_screen(&mut doc, 1366, 768);
        let screen = Size { w: 1366, h: 768 };

        for m in &doc.mirrors {
            let (x, y) = placed(&m.dst, m.dst_anchor, screen);
            assert!(x >= 0, "{} starts at {x}", m.id);
            assert!(
                x + m.dst.w as i32 <= 1366,
                "{} ends at {} past the right edge",
                m.id,
                x + m.dst.w as i32
            );
            assert!(y + m.dst.h as i32 <= 768, "{} runs off the bottom", m.id);
        }

        assert!(problems(&doc).is_empty(), "{:?}", problems(&doc));
    }

    #[test]
    fn the_readout_moves_with_everything_else() {
        // It was placed by hand and left behind, so a config rescaled from
        // 1707 to 2560 wide put a readout that had been against the right
        // edge into the middle of the screen.
        let mut doc = preset();
        doc.gui.screen = Size { w: 1707, h: 1067 };
        doc.ninb.overlay.x = 1700;
        doc.ninb.overlay.y = 900;

        fit_to_screen(&mut doc, 2560, 1600);

        let k: f64 = 1600.0 / 1067.0;
        assert_eq!(doc.ninb.overlay.x, (1700.0 * k).round() as i32, "x scaled");
        assert_eq!(doc.ninb.overlay.y, (900.0 * k).round() as i32, "y scaled");

        // Still near the right edge, which is where it started.
        assert!(doc.ninb.overlay.x > 2400, "it drifted away from the edge");
    }

    #[test]
    fn a_readout_already_off_the_edge_is_brought_back() {
        // Scaling an out of bounds offset puts it further out, and a readout
        // nobody can see looks exactly like a broken one.
        let mut doc = preset();
        doc.gui.screen = Size { w: 1707, h: 1067 };
        doc.ninb.overlay.x = 1700;
        doc.ninb.overlay.y = 1200; // already past the bottom at 1067 tall

        fit_to_screen(&mut doc, 2560, 1600);

        assert!(doc.ninb.overlay.y < 1600, "still off the bottom");
        assert!(doc.ninb.overlay.x < 2560, "still off the right");
        assert!(doc.ninb.overlay.y >= 0 && doc.ninb.overlay.x >= 0);
    }

    #[test]
    fn everything_comes_out_anchored() {
        // Anchors are the half that survives the next monitor. Scaling alone
        // would be right once and wrong afterwards.
        let mut doc = preset();
        assert!(doc.mirrors.iter().all(|m| m.dst_anchor.is_none()), "starts absolute");

        fit_to_screen(&mut doc, 2560, 1440);

        assert!(doc.mirrors.iter().all(|m| m.dst_anchor.is_some()), "mirrors anchored");
        assert!(doc.images.iter().all(|i| i.dst_anchor.is_some()), "images anchored");

        // The pie chart lives on the right, the eye overlay on the left.
        let pie = doc.mirrors.iter().find(|m| m.id == "pie_chart").unwrap();
        let eye = doc.mirrors.iter().find(|m| m.id == "eye_zoom").unwrap();
        assert_eq!(pie.dst_anchor, Some(Anchor::TopRight));
        assert_eq!(eye.dst_anchor, Some(Anchor::TopLeft));
    }

    #[test]
    fn fitting_to_the_same_screen_changes_only_the_anchors() {
        let mut doc = preset();
        let before: Vec<_> = doc.mirrors.iter().map(|m| (m.dst.x, m.dst.w)).collect();

        fit_to_screen(&mut doc, 1920, 1080);

        for (m, (was_x, was_w)) in doc.mirrors.iter().zip(before) {
            assert_eq!(m.dst.w, was_w, "{} width untouched", m.id);
            let (x, _) = placed(&m.dst, m.dst_anchor, Size { w: 1920, h: 1080 });
            assert_eq!(x, was_x, "{} did not move", m.id);
        }
    }

    #[test]
    fn an_anchor_somebody_already_set_is_left_alone() {
        let mut doc = preset();
        doc.mirrors[0].dst_anchor = Some(Anchor::Center);
        let was = doc.mirrors[0].dst;

        fit_to_screen(&mut doc, 2560, 1440);

        assert_eq!(doc.mirrors[0].dst_anchor, Some(Anchor::Center), "kept");
        assert_eq!(doc.mirrors[0].dst.w, was.w, "and not resized either");
    }

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
            "an empty key removes the bind, it does not write a blank one"
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
    fn a_key_that_toggles_several_overlays_is_left_alone() {
        // The window can only describe one overlay per key, so rebuilding
        // would otherwise quietly drop a group somebody made in the editor.
        let base = preset();
        let mut with_group = base.clone();

        let group: Vec<String> = with_group.mirrors.iter().map(|m| m.id.clone()).collect();
        assert!(group.len() > 1, "the preset has enough mirrors to group");

        with_group.keybinds.push(Keybind {
            f3_safe: true,
            ingame_only: false,
            input: "F8".into(),
            command: Command::OverlayToggle,
            args: Some(serde_json::json!({ "overlays": group })),
            label: Some("Everything".into()),
        });

        let built = build(&with_group, &choices_for(&with_group));

        let kept = built
            .keybinds
            .iter()
            .find(|b| b.input == "F8")
            .expect("the group survived the rebuild");
        assert_eq!(kept.overlay_ids().len(), group.len());
        assert!(problems(&built).is_empty(), "{:?}", problems(&built));
    }

    #[test]
    fn the_chat_key_is_a_choice_and_survives_a_round_trip() {
        // Without one, chat mode is a feature you have to know exists and go
        // and bind by hand, which is how the first tester found it: by not
        // finding it.
        let base = preset();
        let mut choices = choices_for(&base);

        assert!(choices.chat_key.is_empty(), "the preset does not bind one for you");

        choices.chat_key = "Insert".into();
        let built = build(&base, &choices);

        let chat = built.keybinds.iter().find(|b| b.command == Command::RemapsToggle).unwrap();
        assert_eq!(chat.input, "Insert");
        assert!(problems(&built).is_empty(), "{:?}", problems(&built));

        assert_eq!(choices_for(&built).chat_key, "Insert", "and reading it back finds it");

        // Rebuilding drops the binds this window owns and writes them again.
        // A chat key left blank has to actually go, not turn into a bind on
        // no key at all.
        let mut cleared = choices_for(&built);
        cleared.chat_key = String::new();
        let built = build(&built, &cleared);
        assert!(!built.keybinds.iter().any(|b| b.command == Command::RemapsToggle));
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
