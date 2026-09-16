//! Input: sensitivity, key rebinds, and Ninjabrain Bot.

use std::collections::BTreeMap;

use toolwall_core::{Document, Problem};

use crate::keys;
use crate::widgets::settings_grid;


/// Which half of which rebind row the editor is currently filling in.
#[derive(PartialEq, Eq, Clone)]
pub struct RemapCapture {
    pub table: RemapTable,
    pub row: usize,
    pub to_side: bool,
    pub how: RemapEntry,
}

/// The two ways to name a key, because one of them cannot cover everything.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum RemapEntry {
    /// Wait for a keypress and write down what was pressed.
    Listening,
    /// Choose from waywall's list.
    ///
    /// Needed for the keys a keypress cannot name. Winit merges left and
    /// right modifiers into one flag before egui sees them, and egui has no
    /// `Key` variant for a modifier in the first place, so pressing Left Alt
    /// produces nothing to capture. waywall is happy to remap it - keys are
    /// matched on the raw keycode, before modifier handling - so the only
    /// thing missing was a way to say so.
    Picking,
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
        ui.weak(
            "Press Set, then press the key you want. Left is the key you press. \
             For Left Alt, Right Shift and the other modifiers, use ▾ - they \
             reach the editor already merged and cannot be told apart by \
             listening.",
        );
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

/// A rebind table with a key-capture button and a picker on each side.
///
/// Typing waywall's key names from memory is the kind of thing that sends
/// people to a lookup table, so both halves can be filled by pressing the key
/// - and, for the keys no keypress can name, chosen from a list.
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
        if active.table == table && active.how == RemapEntry::Listening {
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
        *capture =
            Some(RemapCapture { table, row: rows.len() - 1, to_side: false, how: RemapEntry::Listening });
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
    let active = capture
        .as_ref()
        .filter(|c| c.table == table && c.row == row && c.to_side == to_side)
        .map(|c| c.how);

    let mut changed = ui
        .add(egui::TextEdit::singleline(value).desired_width(96.0))
        .changed();

    let listening = active == Some(RemapEntry::Listening);
    let label = if listening { "press a key…" } else { "Set" };

    if ui
        .selectable_label(listening, label)
        .on_hover_text("Press this, then press the key. Modifiers on their own cannot be read this way - use the list.")
        .clicked()
    {
        *capture = if listening {
            None
        } else {
            Some(RemapCapture { table, row, to_side, how: RemapEntry::Listening })
        };
        changed = true;
    }

    let picking = active == Some(RemapEntry::Picking);
    let picker = ui
        .selectable_label(picking, "▾")
        .on_hover_text("Choose from waywall's list, including Left Alt, Right Shift and the rest");

    let popup_id = ui.make_persistent_id(("remap-picker", table as u8, row, to_side));

    if picker.clicked() {
        if picking {
            ui.memory_mut(|m| m.close_popup());
            *capture = None;
        } else {
            *capture = Some(RemapCapture { table, row, to_side, how: RemapEntry::Picking });
            ui.memory_mut(|m| m.open_popup(popup_id));
        }
    }

    if picking {
        match key_picker(ui, &picker, popup_id) {
            Picked::One(chosen) => {
                *value = chosen;
                *capture = None;
                changed = true;
            }
            // Clicking away closes the popup itself. Without noticing that,
            // the next frame would reopen it and the list could not be
            // dismissed by clicking off it.
            Picked::Dismissed => *capture = None,
            Picked::StillOpen => {}
        }
    }

    // A modifier held with nothing else, while listening, is the case the
    // capture cannot serve. Say so where it happened rather than leaving the
    // button sitting there looking broken.
    if listening && ui.ctx().input(|i| i.modifiers.any()) {
        ui.label("⌨").on_hover_text(
            "Left and right modifiers arrive here already merged, so a modifier \
             on its own cannot be captured. Pick it from the ▾ list instead.",
        );
    }

    changed
}

/// The list of every name waywall accepts, grouped and searchable.
///
/// This is the only way to name a key that produces no keypress event, which
/// is every modifier: winit merges left and right into one flag and egui has
/// no `Key` for them at all. waywall remaps them happily, matching the raw
/// keycode before any modifier handling, so the list is the whole fix.
/// The searchable, grouped list of names.
///
/// Split out from the popup so it can be rendered on its own in a test: the
/// popup needs a click to open, and the part worth checking is that every
/// group draws and the search narrows it.
pub(crate) fn picker_list(ui: &mut egui::Ui, search_id: egui::Id) -> Option<String> {
    let mut chosen = None;

    ui.set_min_width(260.0);

    let mut query: String = ui.data(|d| d.get_temp(search_id).unwrap_or_default());
    let response = ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("search")
            .desired_width(f32::INFINITY),
    );
    response.request_focus();
    ui.data_mut(|d| d.insert_temp(search_id, query.clone()));

    let needle = query.trim().to_ascii_lowercase();

    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        for (group, names) in toolwall_core::keycodes::groups() {
            let matching: Vec<&str> = names
                .into_iter()
                .filter(|n| needle.is_empty() || n.to_ascii_lowercase().contains(&needle))
                .collect();

            if matching.is_empty() {
                continue;
            }

            ui.add_space(4.0);
            ui.weak(group);

            for name in matching {
                if ui.selectable_label(false, name).clicked() {
                    chosen = Some(name.to_string());
                }
            }
        }
    });

    chosen
}

enum Picked {
    One(String),
    Dismissed,
    StillOpen,
}

fn key_picker(ui: &mut egui::Ui, below: &egui::Response, popup_id: egui::Id) -> Picked {
    if !ui.memory(|m| m.is_popup_open(popup_id)) {
        return Picked::Dismissed;
    }

    let mut chosen = None;
    let search_id = popup_id.with("search");

    egui::popup_below_widget(
        ui,
        popup_id,
        below,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| chosen = picker_list(ui, search_id),
    );

    match chosen {
        Some(name) => {
            ui.data_mut(|d| d.insert_temp(search_id, String::new()));
            ui.memory_mut(|m| m.close_popup());
            Picked::One(name)
        }
        None => Picked::StillOpen,
    }
}
