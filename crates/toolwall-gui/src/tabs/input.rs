//! Input: sensitivity, key rebinds, and Ninjabrain Bot.

use std::collections::BTreeMap;

use toolwall_core::{Document, Problem};

use crate::keys;
use crate::widgets::settings_grid;


/// Which half of which rebind row is waiting for a keypress.
#[derive(PartialEq, Eq, Clone)]
pub struct RemapCapture {
    pub table: RemapTable,
    pub row: usize,
    pub to_side: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum RemapTable {
    Playing,
    Menu,
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    _problems: &[Problem],
    advanced: bool,
    capture: &mut Option<RemapCapture>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Mouse");
        settings_grid(ui, "input-grid", |ui| {
            ui.label("Sensitivity").on_hover_text(
                "Multiplies Minecraft's own setting. A god-sens setup keeps \
                 Minecraft very low and multiplies it back up here, so values \
                 well above 10 are normal.",
            );
            // log scale: god-sens setups sit near 13, normal ones near 1
            ui.add(
                egui::Slider::new(&mut doc.input.sensitivity, 0.01..=20.0)
                    .logarithmic(true)
                    .clamping(egui::SliderClamping::Never),
            );
            ui.end_row();

            ui.label("Lock cursor to the game");
            ui.checkbox(&mut doc.input.confine_pointer, "");
            ui.end_row();
        });

        ui.separator();
        ui.heading("Key rebinds");
        ui.weak("Press Set, then press the key you want. Left is the key you press.");
        remap_table(ui, RemapTable::Playing, &mut doc.input.remaps, capture);

        ui.add_space(10.0);
        ui.label("While the cursor is visible");
        ui.weak(
            "Used instead of the rebinds above in inventories, menus and while paused. \
             Leave empty to use one set everywhere.",
        );
        remap_table(ui, RemapTable::Menu, &mut doc.input.remaps_menu, capture);

        if advanced {
            ui.separator();
            ui.heading("Keyboard layout");
            ui.weak("Leave blank to inherit. For search crafting in another language.");

            settings_grid(ui, "layout-grid", |ui| {
                for (label, field) in [
                    ("Layout", &mut doc.input.layout),
                    ("Model", &mut doc.input.model),
                    ("Rules", &mut doc.input.rules),
                    ("Variant", &mut doc.input.variant),
                    ("Options", &mut doc.input.options),
                ] {
                    ui.label(label);
                    ui.text_edit_singleline(field);
                    ui.end_row();
                }

                ui.label("Repeat rate").on_hover_text("-1 inherits the system setting");
                ui.add(egui::DragValue::new(&mut doc.input.repeat_rate).range(-1..=255));
                ui.end_row();

                ui.label("Repeat delay").on_hover_text("-1 inherits the system setting");
                ui.add(egui::DragValue::new(&mut doc.input.repeat_delay).range(-1..=5000));
                ui.end_row();
            });
        }
    });
}

/// A rebind table with a key-capture button on each side.
///
/// Typing waywall's keysym names from memory is the kind of thing that sends
/// people to a lookup table, so both halves can be filled by pressing the key.
fn remap_table(
    ui: &mut egui::Ui,
    table: RemapTable,
    remaps: &mut BTreeMap<String, String>,
    capture: &mut Option<RemapCapture>,
) {
    // Edited as an ordered list so a row keeps its identity while you retype
    // the key it is filed under.
    let mut rows: Vec<(String, String)> =
        remaps.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    let mut changed = false;
    let mut remove = None;

    // A capture in progress swallows the next keypress into its field.
    //
    // captured_keycode, not captured: a rebind is matched against waywall's
    // keycode table, where the keybind spelling of anything but a letter or a
    // digit does not appear at all.
    if let Some(active) = capture.clone() {
        if active.table == table {
            if let Some(pressed) = keys::captured_keycode(ui.ctx()) {
                if let Some(row) = rows.get_mut(active.row) {
                    if active.to_side {
                        row.1 = pressed;
                    } else {
                        row.0 = pressed;
                    }
                    changed = true;
                }
                *capture = None;
            }
        }
    }

    for (index, row) in rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= capture_field(ui, table, index, false, &mut row.0, capture);
            ui.label("→");
            changed |= capture_field(ui, table, index, true, &mut row.1, capture);

            if ui.small_button("✕").on_hover_text("Remove").clicked() {
                remove = Some(index);
            }

            // Inline, because the cost of a bad name is the whole config
            // failing to load - not just this row going quiet.
            for half in [&mut row.0, &mut row.1] {
                changed |= warn_unknown(ui, half);
            }
        });
    }

    if let Some(index) = remove {
        rows.remove(index);
        changed = true;
        *capture = None;
    }

    if ui.button("Add rebind").clicked() {
        rows.push((String::new(), String::new()));
        changed = true;
        *capture = Some(RemapCapture { table, row: rows.len() - 1, to_side: false });
    }

    if changed {
        remaps.clear();
        for (from, to) in rows {
            // Both halves, not just the source. waywall rejects an empty
            // target outright, and rejecting it takes every other setting in
            // the document down with it, so a row being filled in is not a
            // state worth writing to disk.
            if !from.trim().is_empty() && !to.trim().is_empty() {
                remaps.insert(from, to);
            }
        }
    }
}

/// Flag a rebind name waywall will not parse, and offer the fix when there is
/// an obvious one.
fn warn_unknown(ui: &mut egui::Ui, value: &mut String) -> bool {
    if value.trim().is_empty() || toolwall_core::keycodes::is_valid(value) {
        return false;
    }

    match toolwall_core::keycodes::repair(value) {
        Some(fixed) => {
            let hint = format!(
                "waywall has no key called {value:?}. Rebinds use kernel key names, \
                 not the names keybinds use. Click to change it to {fixed}.",
            );
            if ui.button(format!("⚠ {fixed}?")).on_hover_text(hint).clicked() {
                *value = fixed.to_string();
                return true;
            }
        }
        None => {
            ui.label("⚠").on_hover_text(format!(
                "waywall has no key called {value:?}, and this rebind will stop \
                 the whole config from loading. Press Set and press the key instead.",
            ));
        }
    }

    false
}

fn capture_field(
    ui: &mut egui::Ui,
    table: RemapTable,
    row: usize,
    to_side: bool,
    value: &mut String,
    capture: &mut Option<RemapCapture>,
) -> bool {
    let waiting = capture
        .as_ref()
        .is_some_and(|c| c.table == table && c.row == row && c.to_side == to_side);

    let mut changed = ui
        .add(egui::TextEdit::singleline(value).desired_width(96.0))
        .changed();

    let label = if waiting { "press a key…" } else { "Set" };
    if ui.selectable_label(waiting, label).clicked() {
        *capture = if waiting {
            None
        } else {
            Some(RemapCapture { table, row, to_side })
        };
        changed = true;
    }

    changed
}
