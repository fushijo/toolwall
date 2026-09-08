//! Input: sensitivity, key repeat, pointer confinement and remaps.

use std::collections::BTreeMap;

use toolwall_core::Document;

pub fn show(ui: &mut egui::Ui, doc: &mut Document) {
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
