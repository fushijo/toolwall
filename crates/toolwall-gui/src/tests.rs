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
            color_keys: Vec::new(),
            label: Some("Boat eye".into()),
            src: Rect { x: 0, y: 0, w: 100, h: 100 },
            dst: Rect { x: 0, y: 300, w: 300, h: 300 },
            depth: Some(1),
            shader: Some("invert".into()),
            color_key: Some(ColorKey { input: "#fff".into(), output: "#f00".into() }),
        }],
        images: vec![Image {
            id: "grid".into(),
            label: None,
            path: "~/.config/waywall/overlays/nope.png".into(),
            dst: Rect { x: 0, y: 0, w: 10, h: 10 },
            depth: None,
            shader: None,
        }],
        keybinds: vec![
            Keybind {
                input: "Shift-T".into(),
                command: Command::ModeSet,
                args: Some(json!({ "mode": "thin" })),
                label: Some("Thin".into()),
            },
            Keybind {
                input: "Shift-C".into(),
                command: Command::ModeCycle,
                args: Some(json!({ "modes": ["thin", "wide"] })),
                label: None,
            },
            Keybind {
                input: "Shift-S".into(),
                command: Command::SensSet,
                args: Some(json!({ "sensitivity": 0.6 })),
                label: None,
            },
            Keybind {
                input: "Shift-K".into(),
                command: Command::KeymapSet,
                args: Some(json!({ "layout": "de" })),
                label: None,
            },
            Keybind {
                input: "Shift-R".into(),
                command: Command::RemapsSet,
                args: Some(json!({ "remaps": { "MB4": "Home" } })),
                label: None,
            },
            Keybind {
                input: "Shift-P".into(),
                command: Command::KeyPress,
                args: Some(json!({ "key": "F3" })),
                label: None,
            },
            Keybind {
                input: "grave".into(),
                command: Command::NinbToggle,
                args: None,
                label: None,
            },
            Keybind {
                input: "Ctrl-E".into(),
                command: Command::Exec,
                args: Some(json!({ "command": "ls" })),
                label: None,
            },
            Keybind {
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
    doc.input.remaps_menu.insert("MB4".into(), "Escape".into());
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
        tabs::input::show(ui, doc, &found, true, &mut None, &mut FileBrowser::default());
    });
    render(&mut doc, |ui, doc| tabs::theme::show(ui, doc, true, &mut FileBrowser::default()));
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
        tabs::input::show(ui, doc, &found, true, &mut None, &mut FileBrowser::default());
    });
    render(&mut doc, |ui, doc| tabs::theme::show(ui, doc, true, &mut FileBrowser::default()));
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
