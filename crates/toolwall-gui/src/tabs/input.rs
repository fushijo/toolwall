//! Input: sensitivity, key repeat, pointer confinement and remaps.

use std::collections::BTreeMap;

use toolwall_core::schema::App;
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{optional_text, problems_for};

pub fn show(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem]) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("input-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("sensitivity");
                ui.add(
                    egui::DragValue::new(&mut doc.input.sensitivity)
                        .speed(0.01)
                        .range(0.01..=10.0),
                );
                ui.end_row();

                ui.label("repeat rate")
                    .on_hover_text("-1 inherits the host Wayland session");
                ui.add(egui::DragValue::new(&mut doc.input.repeat_rate).range(-1..=255));
                ui.end_row();

                ui.label("repeat delay")
                    .on_hover_text("-1 inherits the host Wayland session");
                ui.add(egui::DragValue::new(&mut doc.input.repeat_delay).range(-1..=5000));
                ui.end_row();

                ui.label("confine pointer");
                ui.checkbox(&mut doc.input.confine_pointer, "");
                ui.end_row();
            });

        ui.separator();
        ui.heading("Keyboard layout");
        ui.weak("Leave blank to inherit. Used for search crafting in another language.");

        egui::Grid::new("layout-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (label, field) in [
                    ("layout", &mut doc.input.layout),
                    ("model", &mut doc.input.model),
                    ("rules", &mut doc.input.rules),
                    ("variant", &mut doc.input.variant),
                    ("options", &mut doc.input.options),
                ] {
                    ui.label(label);
                    ui.text_edit_singleline(field);
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("Remaps");
        ui.weak("Source key or mouse button → output key, e.g. MB4 → Home.");
        remap_editor(ui, &mut doc.input.remaps);

        ui.add_space(6.0);
        ui.label("While the cursor is visible");
        ui.weak(
            "Used instead of the set above in inventories, menus and while paused. \
             Leave empty to use one set everywhere. Needs the State Output mod.",
        );
        remap_editor(ui, &mut doc.input.remaps_menu);

        ui.separator();
        ui.heading("Floating apps");
        ui.weak(
            "Programs hosted as floating windows, e.g. Ninjabrain Bot. Bind one to a key \
             with the app.toggle command.",
        );
        apps_editor(ui, doc, problems);

        ui.separator();
        ui.heading("GUI");

        egui::Grid::new("gui-grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("command").on_hover_text(
                    "waywall exec()s this with its own PATH, which usually lacks \
                     ~/.cargo/bin - prefer an absolute path or one starting with ~",
                );
                ui.text_edit_singleline(&mut doc.gui.command);
                ui.end_row();

                ui.label("launch delay (ms)");
                ui.add(egui::DragValue::new(&mut doc.gui.launch_delay_ms).range(0..=5000));
                ui.end_row();

                ui.label("allow exec").on_hover_text(
                    "Enables the arbitrary `exec` keybind command. Off by default so a \
                     shared config cannot run commands on load.",
                );
                ui.checkbox(&mut doc.gui.allow_exec, "");
                ui.end_row();
            });
    });
}

fn remap_editor(ui: &mut egui::Ui, remaps: &mut BTreeMap<String, String>) {
    let mut remove = None;
    let mut rename: Option<(String, String)> = None;

    for (from, to) in remaps.iter_mut() {
        ui.horizontal(|ui| {
            let mut from_edit = from.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut from_edit).desired_width(110.0))
                .changed()
            {
                rename = Some((from.clone(), from_edit));
            }

            ui.label("→");
            ui.add(egui::TextEdit::singleline(to).desired_width(110.0));

            if ui.button("✕").clicked() {
                remove = Some(from.clone());
            }
        });
    }

    // A rename is a remove plus an insert, so it cannot happen during iteration.
    if let Some((old, new)) = rename {
        if let Some(value) = remaps.remove(&old) {
            remaps.insert(new, value);
        }
    }

    if let Some(key) = remove {
        remaps.remove(&key);
    }

    if ui.button("Add remap").clicked() {
        remaps.insert(String::new(), String::new());
    }
}

/// Apps are launched by `app.toggle`, which is what makes a "toggle
/// Ninjabrain Bot" keybind actually start Ninjabrain Bot - `floating.toggle`
/// only changes the visibility of things already running.
fn apps_editor(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem]) {
    let mut remove = None;

    for (index, app) in doc.apps.iter_mut().enumerate() {
        let heading = app.label.clone().unwrap_or_else(|| app.id.clone());

        egui::CollapsingHeader::new(heading)
            .id_salt(("app", index))
            .show(ui, |ui| {
                problems_for(ui, problems, &Scope::App(app.id.clone()));

                egui::Grid::new(("app-grid", index))
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("id");
                        ui.text_edit_singleline(&mut app.id);
                        ui.end_row();

                        ui.label("label");
                        optional_text(ui, &mut app.label);
                        ui.end_row();

                        ui.label("command").on_hover_text(
                            "Split on spaces into arguments by waywall. ~ is expanded.",
                        );
                        ui.text_edit_singleline(&mut app.command);
                        ui.end_row();
                    });

                if ui.button("Remove app").clicked() {
                    remove = Some(index);
                }
            });
    }

    if let Some(index) = remove {
        doc.apps.remove(index);
    }

    if ui.button("Add app").clicked() {
        doc.apps.push(App {
            id: format!("app{}", doc.apps.len() + 1),
            label: None,
            command: String::new(),
        });
    }
}
