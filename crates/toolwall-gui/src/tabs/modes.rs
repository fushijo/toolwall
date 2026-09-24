//! Modes: a resolution plus the overlays live while it is selected.
//!
//! Reference implementation for every other tab. Note what is absent: no
//! waywall calls, no IPC, no diffing. Mutate the document, hand it to the
//! store, done.

use toolwall_core::schema::{Mode, Resolution};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{optional_text, problems_for, settings_grid,
    scroll_body,
};

/// Columns of the collapsed row, so the list reads down.
const NAME_COLUMN: f32 = 150.0;
const SIZE_COLUMN: f32 = 150.0;

pub fn show(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem], advanced: bool) {
    // Collected up front: the attach lists need these while `doc.modes` is
    // borrowed mutably below.
    let mirror_ids: Vec<String> = doc.mirrors.iter().map(|m| m.id.clone()).collect();
    let image_ids: Vec<String> = doc.images.iter().map(|i| i.id.clone()).collect();

    // The key that puts you in each mode, taken before doc.modes is borrowed.
    // It is the first thing you want to know about a mode and the row used to
    // be three collapsed words with none of it.
    let keys_for: Vec<(String, String)> = doc
        .keybinds
        .iter()
        .filter(|b| b.command == toolwall_core::schema::Command::ModeSet)
        .filter_map(|b| {
            let mode = b.args.as_ref()?.get("mode")?.as_str()?.to_string();
            Some((mode, crate::keys::pretty(&b.input)))
        })
        .collect();

    scroll_body(ui, |ui| {
        // waywall's own window, above the sizes it is the backdrop for. It
        // used to be on the Theme tab under the cursor settings, which split
        // window sizing across three tabs.
        if advanced {
            ui.collapsing("waywall's own window", |ui| {
                ui.weak("0 x 0 follows the monitor.");
                settings_grid(ui, "window-grid", |ui| {
                    ui.label("Fullscreen width");
                    ui.add(egui::DragValue::new(&mut doc.window.fullscreen_width).range(0..=16384));
                    ui.end_row();

                    ui.label("Fullscreen height");
                    ui.add(
                        egui::DragValue::new(&mut doc.window.fullscreen_height).range(0..=16384),
                    );
                    ui.end_row();

                    ui.label("Go fullscreen on start").on_hover_text(
                        "The size above only applies while fullscreen. Without \
                         this it does nothing until you press a key for it.",
                    );
                    ui.checkbox(&mut doc.window.fullscreen_on_start, "");
                    ui.end_row();
                });
            });
            ui.add_space(6.0);
        }

        if !doc.modes.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.scope(|ui| {
                    ui.set_min_width(NAME_COLUMN);
                    ui.weak("Mode");
                });
                ui.scope(|ui| {
                    ui.set_min_width(SIZE_COLUMN);
                    ui.weak("Size");
                });
                ui.weak("Key");
            });
        }

        let mut remove: Option<usize> = None;

        for (index, mode) in doc.modes.iter_mut().enumerate() {
            let heading = mode.label.clone().unwrap_or_else(|| mode.id.clone());
            let size = if mode.resolution.width == 0 || mode.resolution.height == 0 {
                "stretched to the window".to_string()
            } else {
                format!("{} x {}", mode.resolution.width, mode.resolution.height)
            };
            let key = keys_for
                .iter()
                .find(|(id, _)| *id == mode.id)
                .map(|(_, k)| k.clone());

            let id = ui.make_persistent_id(("mode", index));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.scope(|ui| {
                        ui.set_min_width(NAME_COLUMN);
                        ui.label(heading);
                    });
                    ui.scope(|ui| {
                        ui.set_min_width(SIZE_COLUMN);
                        ui.weak(size);
                    });
                    match key {
                        Some(key) => ui.monospace(key),
                        None => ui.weak("no key"),
                    };
                })
                .body(|ui| {
                    problems_for(ui, problems, &Scope::Mode(mode.id.clone()));

                    settings_grid(ui, format!("mode-{index}"), |ui| {
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

                        ui.label("Width").on_hover_text("0 stretches to the window");
                        ui.add(
                            egui::Slider::new(&mut mode.resolution.width, 0..=3840)
                                .clamping(egui::SliderClamping::Never),
                        );
                        ui.end_row();

                        ui.label("Height").on_hover_text(
                            "0 stretches to the window. tall modes go well past the \
                             slider; type the number in.",
                        );
                        ui.add(
                            egui::Slider::new(&mut mode.resolution.height, 0..=2160)
                                .clamping(egui::SliderClamping::Never),
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
                                    egui::Slider::new(sens, 0.01..=20.0)
                                        .logarithmic(true)
                                        .clamping(egui::SliderClamping::Never),
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

                    if ui.button("Remove mode").clicked() {
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
