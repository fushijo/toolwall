//! Modes: a resolution plus the overlays live while it is selected.
//!
//! Reference implementation for every other tab. Note what is absent: no
//! waywall calls, no IPC, no diffing. Mutate the document, hand it to the
//! store, done.

use toolwall_core::schema::{Mode, Resolution};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{optional_text, problems_for};

pub fn show(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem], advanced: bool) {
    // Collected up front: the attach lists need these while `doc.modes` is
    // borrowed mutably below.
    let mirror_ids: Vec<String> = doc.mirrors.iter().map(|m| m.id.clone()).collect();
    let image_ids: Vec<String> = doc.images.iter().map(|i| i.id.clone()).collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove: Option<usize> = None;

        for (index, mode) in doc.modes.iter_mut().enumerate() {
            let heading = mode.label.clone().unwrap_or_else(|| mode.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(index)
                .default_open(false)
                .show(ui, |ui| {
                    problems_for(ui, problems, &Scope::Mode(mode.id.clone()));

                    egui::Grid::new(format!("mode-{index}"))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            if advanced {
                                ui.label("ID").on_hover_text(
                                    "Used by keybinds to refer to this mode",
                                );
                                ui.text_edit_singleline(&mut mode.id);
                                ui.end_row();
                            }

                            ui.label("Name");
                            optional_text(ui, &mut mode.label);
                            ui.end_row();

                            ui.label("Width");
                            ui.add(
                                egui::DragValue::new(&mut mode.resolution.width).range(0..=16384),
                            );
                            ui.end_row();

                            ui.label("Height");
                            ui.add(
                                egui::DragValue::new(&mut mode.resolution.height).range(0..=16384),
                            );
                            ui.end_row();

                            ui.label("Sensitivity");
                            ui.horizontal(|ui| {
                                let mut overridden = mode.sensitivity.is_some();
                                if ui.checkbox(&mut overridden, "Override").changed() {
                                    mode.sensitivity = overridden.then_some(1.0);
                                }
                                if let Some(sens) = &mut mode.sensitivity {
                                    ui.add(
                                        egui::DragValue::new(sens).speed(0.01).range(0.0001..=1000.0),
                                    );
                                }
                            });
                            ui.end_row();

                            ui.label("Overlays").on_hover_text(
                                "Shown automatically while this mode is active",
                            );
                            ui.vertical(|ui| {
                                attach_list(ui, &mirror_ids, &mut mode.mirrors);
                                attach_list(ui, &image_ids, &mut mode.images);
                            });
                            ui.end_row();
                        });

                    if advanced && ui.button("Remove mode").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.modes.remove(index);
        }

        ui.separator();

        if ui.button("Add mode").clicked() {
            doc.modes.push(Mode {
                id: format!("mode{}", doc.modes.len() + 1),
                label: None,
                resolution: Resolution { width: 0, height: 0 },
                sensitivity: None,
                toggle: true,
                mirrors: Vec::new(),
                images: Vec::new(),
            });
        }
    });
}

/// Attach or detach overlays by id.
///
/// Only ids that exist are offered, so a mode cannot come to reference a
/// deleted overlay through this path. Anything already attached but missing is
/// still listed, so a dangling reference is visible and removable rather than
/// silently dropped.
fn attach_list(ui: &mut egui::Ui, available: &[String], attached: &mut Vec<String>) {
    ui.vertical(|ui| {
        if available.is_empty() && attached.is_empty() {
            ui.weak("none defined");
            return;
        }

        for id in available {
            let mut on = attached.iter().any(|a| a == id);
            if ui.checkbox(&mut on, id).changed() {
                if on {
                    attached.push(id.clone());
                } else {
                    attached.retain(|a| a != id);
                }
            }
        }

        let dangling: Vec<String> = attached
            .iter()
            .filter(|a| !available.iter().any(|b| &b == a))
            .cloned()
            .collect();

        for id in dangling {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 120), format!("⚠ {id}"));
                if ui.button("detach").clicked() {
                    attached.retain(|a| a != &id);
                }
            });
        }
    });
}
