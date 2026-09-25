//! Headless checks for the tab code.
//!
//! egui runs perfectly well without a window, so every tab can be driven
//! through a real layout pass with no compositor, GPU or display. That catches
//! the failures that actually happen when editing this code, panics, id
//! collisions between repeated widgets, and index handling in the
//! add/remove paths, none of which need pixels to detect.

use serde_json::json;
use toolwall_core::schema::{
    ColorKey, Command, Image, Keybind, Mirror, Mode, Rect, Resolution, Shader,
};
use toolwall_core::{problems, Document};

use crate::{keys, tabs, widgets::FileBrowser};

/// A document exercising every branch the tabs have: overlays attached and
/// dangling, a colour key, a shader, and one keybind per argument shape.
fn sample() -> Document {
    let mut doc = Document {
        modes: vec![
            Mode {
                id: "thin".into(),
                label: Some("Thin BT".into()),
                resolution: Resolution { width: 320, height: 1080 },
                sensitivity: Some(0.5),
                toggle: true,
                mirrors: vec!["eye".into()],
                images: vec!["grid".into()],
            },
            Mode {
                id: "wide".into(),
                label: None,
                resolution: Resolution { width: 1920, height: 300 },
                sensitivity: None,
                toggle: true,
                // Deliberately dangling, to exercise the warning path.
                mirrors: vec!["missing".into()],
                images: vec![],
            },
        ],
        mirrors: vec![Mirror {
            id: "eye".into(),
            src_anchor: None,
            dst_anchor: None,
            color_keys: Vec::new(),
            outline: None,
            label: Some("Boat eye".into()),
            src: Rect { x: 0, y: 0, w: 100, h: 100 },
            dst: Rect { x: 0, y: 300, w: 300, h: 300 },
            depth: Some(1),
            shader: Some("invert".into()),
            color_key: Some(ColorKey {
                input: "#fff".into(),
                output: "#f00".into(),
                threshold: toolwall_core::schema::DEFAULT_THRESHOLD,
            }),
        }],
        images: vec![Image {
            id: "grid".into(),
            label: None,
            path: "~/.config/waywall/overlays/nope.png".into(),
            dst_anchor: None,
            dst: Rect { x: 0, y: 0, w: 10, h: 10 },
            depth: None,
            shader: None,
        }],
        keybinds: vec![
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-T".into(),
                command: Command::ModeSet,
                args: Some(json!({ "mode": "thin" })),
                label: Some("Thin".into()),
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-C".into(),
                command: Command::ModeCycle,
                args: Some(json!({ "modes": ["thin", "wide"] })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-S".into(),
                command: Command::SensSet,
                args: Some(json!({ "sensitivity": 0.6 })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-K".into(),
                command: Command::KeymapSet,
                args: Some(json!({ "layout": "de" })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-R".into(),
                command: Command::RemapsSet,
                args: Some(json!({ "remaps": { "MB4": "HOME" } })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Shift-P".into(),
                command: Command::KeyPress,
                args: Some(json!({ "key": "F3" })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "grave".into(),
                command: Command::NinbToggle,
                args: None,
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Ctrl-E".into(),
                command: Command::Exec,
                args: Some(json!({ "command": "ls" })),
                label: None,
            },
            Keybind {
                f3_safe: true,
                ingame_only: false,
                input: "Ctrl-I".into(),
                command: Command::GuiToggle,
                args: None,
                label: None,
            },
        ],
        ..Default::default()
    };

    doc.shaders.insert(
        "invert".into(),
        Shader { vertex: None, fragment: Some("invert.frag".into()) },
    );
    doc.input.remaps.insert("MB4".into(), "Home".into());
    doc.input.remaps_menu.insert("MB4".into(), "ESC".into());
    doc.ninb.jar = "~/Ninjabrain-Bot.jar".into();
    doc
}

/// Run one layout pass over a tab, as egui would with a real window.
fn render(doc: &mut Document, mut body: impl FnMut(&mut egui::Ui, &mut Document)) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| body(ui, doc));
    });
}

/// Bare modifiers are the reason the picker exists.
///
/// No keypress can name Left Alt: winit merges the two alts into one flag
/// before egui sees it, and egui has no `Key` variant for a modifier either.
/// waywall remaps them fine - `try_remap_key` matches the raw keycode before
/// any modifier handling - so the editor has to offer them by name.
#[test]
fn a_bare_modifier_can_be_rebound() {
    use toolwall_core::keycodes;

    for name in ["LEFTALT", "RIGHTSHIFT", "RIGHTCTRL", "LEFTMETA", "CAPSLOCK"] {
        assert!(keycodes::is_valid(name), "{name} rejected");

        // No capture could ever produce it, which is the whole problem.
        let capturable = egui::Key::ALL.iter().any(|k| keys::keycode(*k) == Some(name));
        assert!(!capturable, "{name} is capturable after all; the picker may be redundant");
    }

    // And once named, it survives into the document waywall is handed.
    let mut doc = sample();
    doc.input.remaps.insert("LEFTALT".into(), "F3".into());
    doc.input.remaps.insert("RIGHTSHIFT".into(), "mb4".into());

    let round_tripped: Document =
        serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();

    assert_eq!(round_tripped.input.remaps["LEFTALT"], "F3");
    assert_eq!(round_tripped.input.remaps["RIGHTSHIFT"], "mb4");

    let complaints: Vec<_> = problems(&round_tripped)
        .into_iter()
        .filter(|p| matches!(p.scope, toolwall_core::Scope::Remap(_)))
        .collect();
    assert!(complaints.is_empty(), "{complaints:?}");
}

/// The picker list draws every group, including the modifiers.
#[test]
fn the_key_picker_renders() {
    let mut doc = sample();

    render(&mut doc, |ui, _doc| {
        let id = ui.make_persistent_id("picker-test");
        tabs::input::picker_list(ui, id);

        // Searching narrows it without dropping the group it lives in.
        ui.data_mut(|d| d.insert_temp(id, "alt".to_string()));
        tabs::input::picker_list(ui, id);
    });
}

/// A rebind row with the picker armed renders without panicking, and the
/// popup stays shut until it is opened by a click.
#[test]
fn a_rebind_row_with_the_picker_armed_renders() {
    use tabs::input::{RemapCapture, RemapEntry, RemapTable};

    let mut doc = sample();
    doc.input.remaps.insert("MB4".into(), "HOME".into());

    let mut capture = Some(RemapCapture {
        table: RemapTable::Playing,
        row: 0,
        to_side: false,
        how: RemapEntry::Picking,
    });

    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::input::show(ui, doc, &found, true, &mut capture);
    });

    // Nothing opened it, so the row gave up on picking rather than leaving a
    // popup that cannot be dismissed.
    assert!(capture.is_none());
}

#[test]
fn every_tab_renders() {
    let mut doc = sample();
    let found = problems(&doc);
    assert!(!found.is_empty(), "sample should have the dangling overlay problem");

    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::modes::show(ui, doc, &found, true);
    });
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::mirrors::show(ui, doc, &found, true);
    });
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        let mut browser = FileBrowser::default();
        tabs::images::show(ui, doc, &found, &mut browser, true);
    });
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        let mut capturing = None;
        tabs::keybinds::show(ui, doc, &found, &mut capturing, true);
    });
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::input::show(ui, doc, &found, true, &mut None);
    });
    render(&mut doc, |ui, doc| {
        let mut state = tabs::screen::ScreenEdit::default();
        tabs::screen::show(ui, doc, &mut state);
    });
    render(&mut doc, |ui, doc| tabs::theme::show(ui, doc, true, &mut FileBrowser::default(), &mut Default::default()));
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::ninb::show(
            ui,
            doc,
            &found,
            true,
            &mut FileBrowser::default(),
            &mut crate::ninb_keys::NinbKeys::default(),
        );
    });
}

#[test]
fn tabs_render_an_empty_document() {
    let mut doc = Document::default();

    render(&mut doc, |ui, doc| tabs::modes::show(ui, doc, &[], false));
    render(&mut doc, |ui, doc| tabs::mirrors::show(ui, doc, &[], false));
    render(&mut doc, |ui, doc| {
        let mut browser = FileBrowser::default();
        tabs::images::show(ui, doc, &[], &mut browser, false);
    });
    render(&mut doc, |ui, doc| {
        let mut capturing = None;
        tabs::keybinds::show(ui, doc, &[], &mut capturing, false);
    });
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::input::show(ui, doc, &found, true, &mut None);
    });
    render(&mut doc, |ui, doc| {
        let mut state = tabs::screen::ScreenEdit::default();
        tabs::screen::show(ui, doc, &mut state);
    });
    render(&mut doc, |ui, doc| tabs::theme::show(ui, doc, true, &mut FileBrowser::default(), &mut Default::default()));
    render(&mut doc, |ui, doc| {
        let found = problems(doc);
        tabs::ninb::show(
            ui,
            doc,
            &found,
            true,
            &mut FileBrowser::default(),
            &mut crate::ninb_keys::NinbKeys::default(),
        );
    });
}

/// waywall configures floating windows with `xdg_toplevel.configure(0, 0)`,
/// which winit clamps to a 1x1 surface: the GUI renders every frame correctly
/// into a single pixel and looks like it never launched. Confirmed from a
/// WAYLAND_DEBUG trace inside waywall, where every buffer was created 1x1.
#[test]
fn a_degenerate_window_size_is_re_requested() {
    fn viewport_commands(screen: egui::Vec2) -> Vec<egui::ViewportCommand> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
            ..Default::default()
        };

        let output = ctx.run(input, |ctx| {
            crate::App::ensure_usable_size(ctx);
        });

        output
            .viewport_output
            .into_values()
            .flat_map(|v| v.commands)
            .collect()
    }

    let squashed = viewport_commands(egui::vec2(1.0, 1.0));
    assert!(
        squashed.iter().any(|c| matches!(c, egui::ViewportCommand::InnerSize(_))),
        "a 1x1 window must ask for a real size, got {squashed:?}"
    );

    // Once usable, stop asking - otherwise this would fight the user resizing.
    let normal = viewport_commands(egui::vec2(720.0, 520.0));
    assert!(
        !normal.iter().any(|c| matches!(c, egui::ViewportCommand::InnerSize(_))),
        "a usable window must be left alone, got {normal:?}"
    );
}

#[test]
fn captured_keys_format_as_waywall_input_strings() {
    let ctrl = egui::Modifiers { ctrl: true, ..Default::default() };
    let ctrl_shift = egui::Modifiers { ctrl: true, shift: true, ..Default::default() };
    let none = egui::Modifiers::default();

    assert_eq!(keys::format(egui::Key::I, ctrl).as_deref(), Some("Ctrl-I"));
    assert_eq!(keys::format(egui::Key::N, ctrl_shift).as_deref(), Some("Ctrl-Shift-N"));
    assert_eq!(keys::format(egui::Key::T, none).as_deref(), Some("T"));
    assert_eq!(keys::format(egui::Key::F3, none).as_deref(), Some("F3"));

    // waywall's keysym names, which are not always the obvious ones.
    assert_eq!(keys::format(egui::Key::Enter, none).as_deref(), Some("Return"));
    assert_eq!(keys::format(egui::Key::Backspace, none).as_deref(), Some("BackSpace"));
    assert_eq!(keys::format(egui::Key::PageUp, none).as_deref(), Some("Page_Up"));
    assert_eq!(keys::format(egui::Key::Space, none).as_deref(), Some("space"));
}

#[test]
fn a_document_edited_through_the_tabs_still_round_trips() {
    // The GUI hands the store a typed Document; if that cannot serialise and
    // deserialise cleanly, a save would write something the runtime rejects.
    let doc = sample();
    let body = serde_json::to_string_pretty(&doc).expect("serialise");
    let back: Document = serde_json::from_str(&body).expect("deserialise");

    assert_eq!(back.modes.len(), doc.modes.len());
    assert_eq!(back.keybinds.len(), doc.keybinds.len());
    assert_eq!(back.mirrors[0].color_key.as_ref().unwrap().input, "#fff");
    assert_eq!(problems(&back).len(), problems(&doc).len());
}

/// An overlay can be put in every mode from the editor.
///
/// `base_overlays` has been in the schema and the runtime since the start and
/// had no UI at all, so the only way to use it was editing the JSON. Somebody
/// asked how to add a mirror to the base mode and the honest answer was "you
/// cannot", which is the wrong answer to a question about a feature that
/// exists.
#[test]
fn an_overlay_can_be_shown_in_every_mode_from_the_editor() {
    let mut doc = sample();
    assert!(doc.base_overlays.is_empty(), "nothing is in the base set yet");

    let problems = problems(&doc);
    let mut browser = FileBrowser::default();

    // Tick the box on the one mirror the sample has.
    let id = doc.mirrors[0].id.clone();
    render(&mut doc, |ui, doc| {
        tabs::mirrors::show(ui, doc, &problems, true);
        let _ = &mut browser;
    });

    // The checkbox needs a click to fire, which a layout pass cannot do, so
    // drive the same transition the handler performs and check the tab then
    // renders it as on.
    doc.base_overlays.push(id.clone());

    render(&mut doc, |ui, doc| {
        tabs::mirrors::show(ui, doc, &problems, true);
    });

    assert!(doc.base_overlays.contains(&id), "still in the base set");
    assert!(
        toolwall_core::problems(&doc).iter().all(|p| !p.message.contains("base_overlays")),
        "a base overlay naming a real mirror is valid"
    );
}

#[test]
fn a_base_overlay_naming_nothing_is_a_problem() {
    // The validator is what stops a stale id becoming an invisible overlay.
    let mut doc = sample();
    doc.base_overlays.push("not-a-thing".into());

    let found = toolwall_core::problems(&doc);
    assert!(
        found.iter().any(|p| p.message.contains("not-a-thing")),
        "an unknown base overlay was not reported: {found:?}"
    );
}

/// Dragging an anchored overlay has to leave it anchored.
///
/// The canvas works in screen coordinates. An anchored `dst` stores a
/// distance from an edge instead, so without a conversion both ways an
/// anchored overlay is drawn in the wrong place and a drag writes a position
/// into a field that means a distance. It would look like the overlay jumped
/// on the next reload, which is a horrible thing to debug.
#[test]
fn an_anchored_overlay_survives_being_dragged() {
    use toolwall_core::schema::{Anchor, Rect};
    use tabs::screen::{placed, stored};

    let screen = (2560, 1440);
    let rect = Rect { x: 300, y: 120, w: 400, h: 200 };

    for anchor in [
        None,
        Some(Anchor::TopLeft),
        Some(Anchor::TopRight),
        Some(Anchor::BottomLeft),
        Some(Anchor::BottomRight),
    ] {
        let on_screen = placed(rect, anchor, screen);
        let mut kept = anchor;
        let back = stored(on_screen, &mut kept, screen);

        assert_eq!(back, rect, "{anchor:?} did not round trip");
        assert_eq!(kept, anchor, "{anchor:?} was not kept");
    }
}

#[test]
fn a_right_anchored_overlay_is_drawn_against_the_right_edge() {
    use toolwall_core::schema::{Anchor, Rect};
    use tabs::screen::placed;

    // Stored x is the distance from the right edge to the near edge, which is
    // how src_anchor already reads, so both ends mean the same thing.
    let rect = Rect { x: 740, y: 100, w: 400, h: 200 };
    let on_screen = placed(rect, Some(Anchor::TopRight), (2560, 1440));

    assert_eq!(on_screen.x, 2560 - 740);
    assert_eq!(on_screen.y, 100, "y is untouched by a top anchor");
}

#[test]
fn dragging_a_centred_overlay_stops_it_being_centred() {
    use toolwall_core::schema::{Anchor, Rect};
    use tabs::screen::stored;

    // There is no offset that means "the middle", so keeping the anchor would
    // snap it straight back and the drag would look broken.
    let mut anchor = Some(Anchor::Center);
    let dropped = Rect { x: 80, y: 90, w: 100, h: 100 };
    let back = stored(dropped, &mut anchor, (2560, 1440));

    assert_eq!(anchor, Some(Anchor::TopLeft), "it picked up a real anchor");
    assert_eq!(back, dropped, "and landed where it was dropped");
}

/// Every non-ASCII character in the UI has to survive egui's font fallback.
///
/// The custom font someone picks goes in front of egui's bundled fonts, not
/// instead of them, so a glyph missing from their font still renders if one of
/// the fallbacks has it. What does not render is a glyph missing from all of
/// them, and that shows up as a blank box with no warning anywhere.
///
/// Three got through that way: the picker button and the remove button and the
/// arrow between the two halves of a rebind, all reported from a screenshot
/// rather than by anything here. Hence this.
///
/// The allowlist is what Ubuntu-Light, NotoEmoji and emoji-icon-font cover
/// between them, checked against their cmap tables. Adding a character means
/// checking it the same way first.
#[test]
fn ui_text_uses_no_glyph_the_bundled_fonts_lack() {
    const SAFE: &str = "…×⚠⬆−•⊗✖❌⌨🗀📁📂";

    let sources = [
        ("main.rs", include_str!("main.rs")),
        ("keys.rs", include_str!("keys.rs")),
        ("canvas.rs", include_str!("canvas.rs")),
        ("overlay.rs", include_str!("overlay.rs")),
        ("setup/mod.rs", include_str!("setup/mod.rs")),
        ("setup/sens.rs", include_str!("setup/sens.rs")),
        ("tabs/screen.rs", include_str!("tabs/screen.rs")),
        ("widgets.rs", include_str!("widgets.rs")),
        ("tabs/input.rs", include_str!("tabs/input.rs")),
        ("tabs/layout.rs", include_str!("tabs/layout.rs")),
        ("tabs/keybinds.rs", include_str!("tabs/keybinds.rs")),
        ("tabs/modes.rs", include_str!("tabs/modes.rs")),
        ("tabs/mirrors.rs", include_str!("tabs/mirrors.rs")),
        ("tabs/images.rs", include_str!("tabs/images.rs")),
        ("tabs/theme.rs", include_str!("tabs/theme.rs")),
        ("tabs/ninb.rs", include_str!("tabs/ninb.rs")),
    ];

    let mut bad: Vec<String> = Vec::new();

    for (name, src) in sources {
        for (number, line) in src.lines().enumerate() {
            // Comments are for us, not for the renderer.
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }

            for ch in line.chars() {
                if ch.is_ascii() || SAFE.contains(ch) {
                    continue;
                }
                bad.push(format!("{name}:{} {ch:?} (U+{:04X})", number + 1, ch as u32));
            }
        }
    }

    assert!(bad.is_empty(), "glyphs that may not render:\n  {}", bad.join("\n  "));
}
