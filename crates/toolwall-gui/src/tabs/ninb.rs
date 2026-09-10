//! Ninjabrain Bot: the jar, the API, and the readout drawn into the scene.

use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{color_field, path_field, problems_for, FileBrowser, PickTarget};

const COORDS: &[(&str, &str)] = &[("chunk", "Chunk"), ("block", "Block")];

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    problems: &[Problem],
    advanced: bool,
    browser: &mut FileBrowser,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        problems_for(ui, problems, &Scope::Ninb);

        ui.heading("Ninjabrain Bot");
        egui::Grid::new("ninb-setup")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Jar file");
                path_field(ui, &mut doc.ninb.jar, browser, PickTarget::NinbJar, "jar");
                ui.end_row();

                ui.label("Open on startup");
                ui.checkbox(&mut doc.ninb.autostart, "");
                ui.end_row();

                if advanced {
                    ui.label("Launch command")
                        .on_hover_text("{jar} is replaced with the path above");
                    ui.text_edit_singleline(&mut doc.ninb.command);
                    ui.end_row();

                    ui.label("API port").on_hover_text("Enable the API in ninb's settings");
                    ui.add(egui::DragValue::new(&mut doc.ninb.port).range(1..=65535));
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("Readout");
        ui.weak("Drawn into the game, so it does not hide with the other floating windows.");

        let o = &mut doc.ninb.overlay;

        egui::Grid::new("ninb-readout")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Enabled");
                ui.checkbox(&mut o.enabled, "");
                ui.end_row();

                ui.label("Position");
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut o.x).speed(1.0));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut o.y).speed(1.0));
                });
                ui.end_row();

                ui.label("Text size");
                ui.add(egui::Slider::new(&mut o.size, 1..=8));
                ui.end_row();

                ui.label("Row spacing");
                ui.add(egui::Slider::new(&mut o.row_spacing, 6..=64));
                ui.end_row();

                ui.label("Predictions shown");
                ui.add(egui::Slider::new(&mut o.shown_predictions, 1..=5));
                ui.end_row();

                ui.label("Eye throw rows").on_hover_text("0 hides the throw list");
                ui.add(egui::Slider::new(&mut o.throw_rows, 0..=10));
                ui.end_row();

                ui.label("Coordinates");
                ui.horizontal(|ui| {
                    for (value, label) in COORDS {
                        if ui.selectable_label(o.coords == *value, *label).clicked() {
                            o.coords = (*value).to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label("Line format").on_hover_text(
                    "{x} {z} {certainty} {overworldDistance} {chunkX} {chunkZ} {throws}",
                );
                ui.text_edit_singleline(&mut o.template);
                ui.end_row();

                ui.label("Before any throw");
                ui.text_edit_singleline(&mut o.idle_text);
                ui.end_row();
            });

        ui.separator();
        ui.heading("Colours");

        egui::Grid::new("ninb-colours")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Text");
                color_field(ui, &mut o.color);
                ui.end_row();

                ui.label("Secondary").on_hover_text("Idle text and throw rows");
                color_field(ui, &mut o.header_color);
                ui.end_row();

                ui.label("Certainty high");
                color_field(ui, &mut o.certainty_high_color);
                ui.end_row();

                ui.label("Certainty medium");
                color_field(ui, &mut o.certainty_mid_color);
                ui.end_row();

                ui.label("Certainty low");
                color_field(ui, &mut o.certainty_low_color);
                ui.end_row();

                if advanced {
                    ui.label("High above (%)");
                    ui.add(egui::Slider::new(&mut o.certainty_high_above, 0.0..=100.0));
                    ui.end_row();

                    ui.label("Medium above (%)");
                    ui.add(egui::Slider::new(&mut o.certainty_mid_above, 0.0..=100.0));
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("Panel");
        ui.weak("waywall has no rectangle, so this is a solid image recoloured behind the text.");

        egui::Grid::new("ninb-panel")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Background");
                ui.checkbox(&mut o.background, "");
                ui.end_row();

                ui.label("Background colour").on_hover_text("Alpha sets the opacity");
                color_field(ui, &mut o.background_color);
                ui.end_row();

                ui.label("Padding");
                ui.add(egui::Slider::new(&mut o.padding, 0..=40));
                ui.end_row();

                ui.label("Border width");
                ui.add(egui::Slider::new(&mut o.border_width, 0..=8));
                ui.end_row();

                ui.label("Border colour");
                color_field(ui, &mut o.border_color);
                ui.end_row();
            });

        if advanced {
            ui.separator();
            ui.heading("Refresh");

            egui::Grid::new("ninb-refresh")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Poll every (ms)");
                    ui.add(egui::Slider::new(&mut o.poll_ms, 100..=2000));
                    ui.end_row();

                    ui.label("Hide when stale (ms)").on_hover_text("0 keeps it on screen");
                    ui.add(egui::Slider::new(&mut o.hide_after_ms, 0..=60000));
                    ui.end_row();
                });
        }

        ui.add_space(6.0);
        ui.weak(
            "Font and text outline are not adjustable: waywall draws scene text in a \
             bundled face with no outline parameter.",
        );
    });
}
