//! Theme: how the compositor looks around the game, plus the editor's own look.

use toolwall_core::schema::NinbAnchor;
use toolwall_core::Document;

use crate::widgets::{color_field, path_field, FileBrowser, PickTarget};

const ANCHORS: &[&str] = &[
    "topleft", "top", "topright",
    "left", "right",
    "bottomleft", "bottomright",
];

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    advanced: bool,
    browser: &mut FileBrowser,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Around the game");

        egui::Grid::new("theme-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
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

        ui.separator();
        ui.heading("Ninjabrain Bot position");
        ui.horizontal(|ui| {
            ui.checkbox(&mut doc.ninb.autostart, "Open Ninjabrain Bot on startup");
        });
        ui.add_space(4.0);

        anchor_editor(ui, doc);

        ui.add_space(4.0);
        egui::Grid::new("ninb-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Opacity");
                ui.add(
                    egui::Slider::new(&mut doc.theme.ninb_opacity, 0.1..=1.0).fixed_decimals(2),
                );
                ui.end_row();

                ui.label("Hide its window").on_hover_text(
                    "Keeps ninb running for the API but never shows it, so opening this \
                     editor no longer reveals it too. Needs the waywall patch.",
                );
                ui.checkbox(&mut doc.theme.ninb_hidden, "");
                ui.end_row();
            });

        if doc.theme.ninb_hidden {
            ui.weak("Its window stays hidden, so read it from the Ninjabrain tab's readout.");
        }

        if advanced {
        ui.separator();
        ui.heading("Window size");
        ui.weak("0 x 0 follows the monitor.");

        egui::Grid::new("window-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Fullscreen width");
                ui.add(egui::DragValue::new(&mut doc.window.fullscreen_width).range(0..=16384));
                ui.end_row();

                ui.label("Fullscreen height");
                ui.add(egui::DragValue::new(&mut doc.window.fullscreen_height).range(0..=16384));
                ui.end_row();
            });
        }

        ui.separator();
        ui.heading("This editor");
        ui.weak("Applies as you change it.");

        let look = &mut doc.gui.appearance;

        egui::Grid::new("appearance-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Opacity").on_hover_text("See the game through the editor");
                ui.add(egui::Slider::new(&mut look.opacity, 0.25..=1.0).fixed_decimals(2));
                ui.end_row();

                ui.label("Theme");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut look.dark, true, "Dark");
                    ui.selectable_value(&mut look.dark, false, "Light");
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
fn anchor_editor(ui: &mut egui::Ui, doc: &mut Document) {
    let (mut position, mut x, mut y) = match &doc.theme.ninb_anchor {
        Some(NinbAnchor::Named(name)) => (name.clone(), 0, 0),
        Some(NinbAnchor::Offset { position, x, y }) => {
            (position.clone(), x.unwrap_or(0), y.unwrap_or(0))
        }
        None => (String::new(), 0, 0),
    };

    let mut changed = false;

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("ninb-anchor")
            .selected_text(if position.is_empty() { "none".into() } else { position.clone() })
            .show_ui(ui, |ui| {
                if ui.selectable_label(position.is_empty(), "none").clicked() {
                    position.clear();
                    changed = true;
                }
                for name in ANCHORS {
                    if ui.selectable_label(position == *name, *name).clicked() {
                        position = (*name).to_string();
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
