//! Theme: how the compositor looks around the game, plus the editor's own look.

use toolwall_core::schema::NinbAnchor;
use toolwall_core::Document;

use crate::widgets::{color_field, path_field, settings_grid, FileBrowser, PickTarget,
    scroll_body, segment_value};

/// Stored as one word, shown as two. waywall reads the left-hand spelling.
const ANCHORS: &[(&str, &str)] = &[
    ("topleft", "Top left"),
    ("top", "Top"),
    ("topright", "Top right"),
    ("left", "Left"),
    ("right", "Right"),
    ("bottomleft", "Bottom left"),
    ("bottomright", "Bottom right"),
];

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    advanced: bool,
    browser: &mut FileBrowser,
    updates: &mut crate::updates::Updates,
) {
    scroll_body(ui, |ui| {
        ui.heading("Around the game");

        settings_grid(ui, "theme-grid", |ui| {
            ui.label("Background colour");
            color_field(ui, &mut doc.theme.background);
            ui.end_row();

            ui.label("Background image");
            path_field(
                ui,
                &mut doc.theme.background_png,
                browser,
                PickTarget::Background,
                "png",
            );
            ui.end_row();

            if advanced {
                ui.label("Cursor theme");
                ui.text_edit_singleline(&mut doc.theme.cursor_theme);
                ui.end_row();

                ui.label("Cursor icon");
                ui.text_edit_singleline(&mut doc.theme.cursor_icon);
                ui.end_row();

                ui.label("Cursor size").on_hover_text("0 inherits");
                ui.add(egui::DragValue::new(&mut doc.theme.cursor_size).range(0..=128));
                ui.end_row();
            }
        });

        // waywall's own window size lives on the Modes tab, next to the sizes
        // it is the backdrop for, and Ninjabrain Bot's position lives on the
        // Ninjabrain tab with the rest of it.

        ui.separator();
        ui.heading("toolwall itself");
        crate::updates::show(ui, updates);

        ui.separator();
        ui.heading("This editor");
        ui.weak("Applies as you change it.");

        let look = &mut doc.gui.appearance;

        settings_grid(ui, "appearance-grid", |ui| {
            ui.label("Opacity").on_hover_text("See the game through the editor");
            ui.add(egui::Slider::new(&mut look.opacity, 0.25..=1.0).fixed_decimals(2));
            ui.end_row();

            ui.label("Theme");
            ui.horizontal(|ui| {
                segment_value(ui, &mut look.dark, true, "Dark");
                segment_value(ui, &mut look.dark, false, "Light");
            });
            ui.end_row();

            ui.label("Font size");
            ui.add(egui::Slider::new(&mut look.font_size, 10.0..=32.0).fixed_decimals(0));
            ui.end_row();

            ui.label("Font").on_hover_text("a .ttf or .otf. blank uses the built-in one.");
            path_field(ui, &mut look.font_path, browser, PickTarget::Font, "ttf");
            ui.end_row();
        });

    });
}

/// `ninb_anchor` is either a bare position name or a position with offsets.
/// Editing an offset promotes the bare form, so the two shapes stay one control.
pub(crate) fn anchor_editor(ui: &mut egui::Ui, doc: &mut Document) {
    let (mut position, mut x, mut y) = match &doc.theme.ninb_anchor {
        Some(NinbAnchor::Named(name)) => (name.clone(), 0, 0),
        Some(NinbAnchor::Offset { position, x, y }) => {
            (position.clone(), x.unwrap_or(0), y.unwrap_or(0))
        }
        None => (String::new(), 0, 0),
    };

    let mut changed = false;

    ui.horizontal(|ui| {
        let shown = ANCHORS
            .iter()
            .find(|(stored, _)| *stored == position)
            .map(|(_, shown)| *shown)
            .unwrap_or("Wherever it opens");

        egui::ComboBox::from_id_salt("ninb-anchor")
            .selected_text(shown)
            .width(170.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(position.is_empty(), "Wherever it opens").clicked() {
                    position.clear();
                    changed = true;
                }
                for (stored, shown) in ANCHORS {
                    if ui.selectable_label(position == *stored, *shown).clicked() {
                        position = (*stored).to_string();
                        changed = true;
                    }
                }
            });

        ui.add_enabled_ui(!position.is_empty(), |ui| {
            ui.label("Offset X");
            changed |= ui.add(egui::DragValue::new(&mut x).speed(1.0)).changed();
            ui.label("Y");
            changed |= ui.add(egui::DragValue::new(&mut y).speed(1.0)).changed();
        });
    });

    if changed {
        doc.theme.ninb_anchor = if position.is_empty() {
            // waywall rejects an empty string here; absent is how "no anchor"
            // is spelled, and the runtime translates accordingly.
            None
        } else if x == 0 && y == 0 {
            Some(NinbAnchor::Named(position))
        } else {
            Some(NinbAnchor::Offset { position, x: Some(x), y: Some(y) })
        };
    }
}
