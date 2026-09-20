//! Screen: drag your overlays where you want them.
//!
//! One picture of waywall's window with every overlay on it, dragged and
//! resized directly instead of typed as four numbers. Edits go straight into
//! the document, so the real overlay follows about a third of a second later.
//!
//! None of this needs the waywall patch. The readout's panel and outline do,
//! but where it sits and how big its text is are ordinary config fields that
//! unpatched waywall reads the same way.

use toolwall_core::schema::Rect;
use toolwall_core::Document;

use crate::canvas::{Canvas, Item, Resize, State};

/// waywall's scene text is Terminus at a fixed 8x16, scaled by a whole number.
const CHAR_W: i32 = 8;
const CHAR_H: i32 = 16;

#[derive(Clone, Default)]
pub struct ScreenEdit {
    pub canvas: State,
    /// Which mode's overlays are in focus. Empty means the base set.
    pub mode: String,
    pub snapping: bool,
    seeded: bool,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Document, state: &mut ScreenEdit) {
    if !state.seeded {
        state.snapping = true;
        state.mode = doc.modes.first().map(|m| m.id.clone()).unwrap_or_default();
        state.seeded = true;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        toolbar(ui, doc, state);
        ui.add_space(8.0);

        let items = collect(doc, &state.mode);
        if items.is_empty() {
            ui.weak("Nothing to place yet. Add a mirror or an image first.");
            return;
        }

        let screen = (doc.gui.screen.w.max(1) as i32, doc.gui.screen.h.max(1) as i32);

        // Alt is the usual "ignore the grid" modifier, and matching it costs
        // nothing.
        let snapping = state.snapping && !ui.input(|i| i.modifiers.alt);

        let action = Canvas { screen, items: &items, snapping }.show(ui, &mut state.canvas);
        apply(doc, action);

        ui.add_space(8.0);
        inspector(ui, doc, state);
    });
}

fn toolbar(ui: &mut egui::Ui, doc: &mut Document, state: &mut ScreenEdit) {
    ui.horizontal(|ui| {
        ui.label("Mode");
        egui::ComboBox::from_id_salt("screen-mode")
            .selected_text(if state.mode.is_empty() {
                "Base".to_string()
            } else {
                state.mode.clone()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.mode, String::new(), "Base");
                for mode in &doc.modes {
                    let label = mode.label.clone().unwrap_or_else(|| mode.id.clone());
                    ui.selectable_value(&mut state.mode, mode.id.clone(), label);
                }
            });

        ui.add_space(12.0);
        ui.checkbox(&mut state.snapping, "Snap")
            .on_hover_text("Edges and centres. Hold Alt to ignore it for one drag.");

        ui.add_space(12.0);
        ui.label("Screen").on_hover_text(
            "How big waywall's window is. Overlay coordinates are window \
             pixels, not monitor pixels, and nothing reports the size, so read \
             it off the Display: line in F3.",
        );
        ui.add(egui::DragValue::new(&mut doc.gui.screen.w).range(320..=16384).speed(4));
        ui.label("x");
        ui.add(egui::DragValue::new(&mut doc.gui.screen.h).range(240..=16384).speed(4));
    });

    ui.weak(
        "Drag to move, corners to resize. Overlays from other modes are shown \
         faded so you can line things up against them.",
    );
}

/// Everything that can be placed, with the ones outside this mode faded.
fn collect(doc: &Document, mode: &str) -> Vec<Item> {
    let active: Vec<&str> = doc
        .modes
        .iter()
        .find(|m| m.id == mode)
        .map(|m| m.mirrors.iter().chain(m.images.iter()).map(String::as_str).collect())
        .unwrap_or_default();

    let base: Vec<&str> = doc.base_overlays.iter().map(String::as_str).collect();
    let shown = |id: &str| active.contains(&id) || base.contains(&id);

    let mut items = Vec::new();

    for mirror in &doc.mirrors {
        items.push(Item {
            key: format!("mirror:{}", mirror.id),
            label: mirror.label.clone().unwrap_or_else(|| mirror.id.clone()),
            rect: mirror.dst,
            // A mirrored capture stretched off its own aspect is a lie about
            // what the game looks like, so corners hold the shape.
            resize: Resize::Aspect,
            muted: !shown(&mirror.id),
        });
    }

    for image in &doc.images {
        items.push(Item {
            key: format!("image:{}", image.id),
            label: image.label.clone().unwrap_or_else(|| image.id.clone()),
            rect: image.dst,
            resize: Resize::Free,
            muted: !shown(&image.id),
        });
    }

    if doc.ninb.overlay.enabled {
        let o = &doc.ninb.overlay;
        items.push(Item {
            key: "readout".into(),
            label: "Ninjabrain readout".into(),
            rect: readout_rect(doc),
            resize: Resize::Steps { min: 1, max: 6, current: o.size.max(1) },
            muted: false,
        });
    }

    items
}

/// Roughly where the readout lands.
///
/// Only the top left is a real setting; the rest depends on what ninb is
/// saying at the time, so this is sized from the rows that are switched on at
/// the configured scale. Close enough to place it against other overlays,
/// which is the job.
fn readout_rect(doc: &Document) -> Rect {
    let o = &doc.ninb.overlay;
    let size = o.size.max(1) as i32;
    let gap = o.line_gap as i32;

    let mut rows = 0;
    for on in [
        o.show_location,
        o.show_certainty,
        o.show_nether,
        o.show_distance,
        o.show_angle,
        o.show_info,
    ] {
        if on {
            rows += 1;
        }
    }
    if o.throw_rows > 0 {
        rows += o.throw_rows as i32 + if o.show_throw_header { 1 } else { 0 };
    }
    rows = rows.max(1);

    let cols = if o.layout == "compact" {
        o.template.chars().count().max(12) as i32
    } else {
        o.wrap_width.max(20) as i32
    };

    let pad = o.padding as i32;
    Rect {
        x: o.x - pad,
        y: o.y - pad,
        w: (cols * CHAR_W * size + pad * 2).max(1) as u32,
        h: (rows * (CHAR_H * size + gap) - gap + pad * 2).max(1) as u32,
    }
}

fn apply(doc: &mut Document, action: crate::canvas::Action) {
    use crate::canvas::Action;

    match action {
        Action::Moved { key, rect } => {
            if let Some(id) = key.strip_prefix("mirror:") {
                if let Some(m) = doc.mirrors.iter_mut().find(|m| m.id == id) {
                    m.dst = rect;
                }
            } else if let Some(id) = key.strip_prefix("image:") {
                if let Some(i) = doc.images.iter_mut().find(|i| i.id == id) {
                    i.dst = rect;
                }
            } else if key == "readout" {
                // The box drawn includes the panel padding, so take it back
                // off or the readout creeps by `padding` on every drag.
                let pad = doc.ninb.overlay.padding as i32;
                doc.ninb.overlay.x = rect.x + pad;
                doc.ninb.overlay.y = rect.y + pad;
            }
        }
        Action::Scaled { key, steps } => {
            if key == "readout" {
                doc.ninb.overlay.size = steps.max(1);
            }
        }
        Action::Selected(_) | Action::None => {}
    }
}

fn inspector(ui: &mut egui::Ui, doc: &mut Document, state: &mut ScreenEdit) {
    let Some(key) = state.canvas.selected.clone() else {
        ui.weak("Click something to see its numbers.");
        return;
    };

    ui.group(|ui| {
        if key == "readout" {
            ui.horizontal(|ui| {
                ui.strong("Ninjabrain readout");
                if ui.small_button("×").on_hover_text("Deselect").clicked() {
                    state.canvas.selected = None;
                }
            });

            let o = &mut doc.ninb.overlay;
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut o.x).speed(1));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut o.y).speed(1));
                ui.label("Text size");
                ui.add(egui::DragValue::new(&mut o.size).range(1..=6));
            });
            ui.weak(
                "The box is an estimate: its real width depends on what ninb \
                 is saying. Placement and text size are exact.",
            );
            return;
        }

        let (kind, id) = match key.split_once(':') {
            Some(pair) => pair,
            None => return,
        };

        let dst = match kind {
            "mirror" => doc.mirrors.iter_mut().find(|m| m.id == id).map(|m| &mut m.dst),
            "image" => doc.images.iter_mut().find(|i| i.id == id).map(|i| &mut i.dst),
            _ => None,
        };

        let Some(dst) = dst else {
            state.canvas.selected = None;
            return;
        };

        ui.horizontal(|ui| {
            ui.strong(id);
            if ui.small_button("×").on_hover_text("Deselect").clicked() {
                state.canvas.selected = None;
            }
        });

        ui.horizontal(|ui| {
            ui.label("X");
            ui.add(egui::DragValue::new(&mut dst.x).speed(1));
            ui.label("Y");
            ui.add(egui::DragValue::new(&mut dst.y).speed(1));
            ui.label("W");
            ui.add(egui::DragValue::new(&mut dst.w).range(1..=16384).speed(1));
            ui.label("H");
            ui.add(egui::DragValue::new(&mut dst.h).range(1..=16384).speed(1));
        });

        // The measuring window only reads true at particular widths, so say so
        // where someone is dragging it rather than in the docs they are not
        // reading right now.
        if id.contains("measure") || id.contains("eye") {
            if dst.w % 60 != 0 {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 160, 60),
                    format!(
                        "Width {} is not a multiple of 60, so the grid bands land on \
                         fractional pixels and drift. Nearest: {} or {}.",
                        dst.w,
                        dst.w / 60 * 60,
                        (dst.w / 60 + 1) * 60
                    ),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use toolwall_core::schema::{Image, Mirror, Mode, Resolution};

    fn doc_with_overlays() -> Document {
        let mut doc = Document::default();
        doc.mirrors.push(Mirror {
            id: "pie".into(),
            label: Some("Pie chart".into()),
            src_anchor: None,
            src: Rect { x: 0, y: 0, w: 10, h: 10 },
            dst: Rect { x: 100, y: 100, w: 200, h: 200 },
            depth: None,
            shader: None,
            color_key: None,
            color_keys: vec![],
        });
        doc.images.push(Image {
            id: "grid".into(),
            label: None,
            path: "/tmp/grid.png".into(),
            dst: Rect { x: 30, y: 300, w: 600, h: 338 },
            depth: None,
            shader: None,
        });
        doc.modes.push(Mode {
            id: "thin".into(),
            label: None,
            resolution: Resolution { width: 340, height: 1080 },
            sensitivity: None,
            toggle: true,
            mirrors: vec!["pie".into()],
            images: vec![],
        });
        doc
    }

    #[test]
    fn overlays_outside_the_mode_are_shown_but_faded() {
        let doc = doc_with_overlays();
        let items = collect(&doc, "thin");

        let pie = items.iter().find(|i| i.key == "mirror:pie").unwrap();
        let grid = items.iter().find(|i| i.key == "image:grid").unwrap();

        assert!(!pie.muted, "pie is in thin");
        assert!(grid.muted, "grid is not in thin, so it should be faded");
    }

    #[test]
    fn a_move_writes_back_to_the_right_overlay() {
        let mut doc = doc_with_overlays();
        apply(
            &mut doc,
            crate::canvas::Action::Moved {
                key: "mirror:pie".into(),
                rect: Rect { x: 5, y: 6, w: 200, h: 200 },
            },
        );
        assert_eq!(doc.mirrors[0].dst, Rect { x: 5, y: 6, w: 200, h: 200 });
        assert_eq!(doc.images[0].dst.x, 30, "the image must not have moved");
    }

    /// The drawn box includes the panel padding, so a round trip has to take
    /// it back off or the readout walks away by `padding` every drag.
    #[test]
    fn dragging_the_readout_does_not_drift() {
        let mut doc = Document::default();
        doc.ninb.overlay.enabled = true;
        doc.ninb.overlay.x = 400;
        doc.ninb.overlay.y = 300;
        doc.ninb.overlay.padding = 10;

        for _ in 0..5 {
            let rect = readout_rect(&doc);
            apply(&mut doc, crate::canvas::Action::Moved { key: "readout".into(), rect });
        }

        assert_eq!(doc.ninb.overlay.x, 400);
        assert_eq!(doc.ninb.overlay.y, 300);
    }

    #[test]
    fn the_readout_box_grows_with_text_size() {
        let mut doc = Document::default();
        doc.ninb.overlay.enabled = true;
        doc.ninb.overlay.size = 1;
        let small = readout_rect(&doc);
        doc.ninb.overlay.size = 3;
        let big = readout_rect(&doc);

        assert!(big.w > small.w && big.h > small.h, "{small:?} vs {big:?}");
    }

    /// The readout is listed whether or not waywall is patched. Only its panel
    /// and outline need that, and neither is what this tab edits.
    #[test]
    fn the_readout_is_placeable_without_the_patch() {
        let mut doc = Document::default();
        doc.ninb.overlay.enabled = true;
        doc.ninb.overlay.background = false;
        doc.ninb.overlay.outline = 0;

        let items = collect(&doc, "");
        assert!(items.iter().any(|i| i.key == "readout"));
    }

    #[test]
    fn scaling_only_touches_the_readout() {
        let mut doc = doc_with_overlays();
        doc.ninb.overlay.enabled = true;
        let before = doc.mirrors[0].dst;

        apply(&mut doc, crate::canvas::Action::Scaled { key: "readout".into(), steps: 4 });

        assert_eq!(doc.ninb.overlay.size, 4);
        assert_eq!(doc.mirrors[0].dst, before);
    }
}
