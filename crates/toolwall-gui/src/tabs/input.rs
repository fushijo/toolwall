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

/// Width of one half of a rebind row: the field, Set and List.
const REMAP_COLUMN: f32 = 218.0;

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
            "Press Set, then press the key. A modifier on its own needs List: \
             left and right arrive here already merged, so listening cannot \
             tell them apart.",
        );
        remap_table(ui, RemapTable::Playing, &mut doc.input.remaps, capture);

        ui.add_space(10.0);
        ui.label("While the cursor is visible");
        ui.weak("Used in inventories, menus and while paused. Needs the State Output mod.");
        ui.collapsing("When these apply", |ui| {
            ui.weak(
                "They replace the set above, they do not add to it, and an \
                 empty list means one set everywhere.",
            );
            ui.weak(
                "Chat counts as a menu to Minecraft, so toolwall watches the \
                 key that opens chat and leaves your rebinds off there. Set \
                 that key on the Keybinds tab if chat is not on T.",
            );
            ui.weak(
                "To change what a key types without losing what it does, use \
                 the Layout tab.",
            );
        });
        remap_table(ui, RemapTable::Menu, &mut doc.input.remaps_menu, capture);

        if advanced {
            ui.separator();
            // The Layout tab is also about the keyboard layout. This half is
            // the xkb names waywall is handed; that half builds one.
            ui.heading("Keyboard language");
            ui.weak(
                "The layout your system already has, by name. To build one of \
                 your own instead, use the Layout tab.",
            );

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

        // Room under the last row, so the status bar never cuts one in half.
        ui.add_space(24.0);
    });
}

/// The rebind rows as they are being edited, which is not the same set as the
/// rows that can be saved.
///
/// A row you have only half filled in cannot live in the document. waywall
/// refuses an empty remap target, and refusing it aborts the whole config
/// load, so the map is only ever allowed to hold complete pairs. But a new row
/// starts empty by definition, so with the document as the only storage
/// "Add rebind" wrote a blank row, the save filter dropped it, and the next
/// frame rebuilt the list without it. The button looked dead.
///
/// So the draft lives here, in editor state, and the document gets only the
/// rows that are finished.
#[derive(Clone, Default)]
pub(crate) struct RemapDraft {
    rows: Vec<(String, String)>,
    /// The last thing this widget wrote, so an edit from anywhere else - a
    /// reload, an import, the file changing on disk - can be told apart from
    /// its own output and reseed the rows.
    committed: BTreeMap<String, String>,
}

impl RemapDraft {
    /// Take up the document's rows, unless we are mid-edit on them already.
    fn sync(&mut self, remaps: &BTreeMap<String, String>) {
        if &self.committed == remaps {
            return;
        }

        self.rows = remaps.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self.committed = remaps.clone();
    }

    fn add(&mut self) {
        self.rows.push((String::new(), String::new()));
    }

    fn remove(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
    }

    /// Write the finished rows through, leaving the unfinished ones alone.
    fn commit_into(&mut self, remaps: &mut BTreeMap<String, String>) {
        let mut out = BTreeMap::new();

        for (from, to) in &self.rows {
            if !from.trim().is_empty() && !to.trim().is_empty() {
                out.insert(from.clone(), to.clone());
            }
        }

        *remaps = out.clone();
        self.committed = out;
    }
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
    let draft_id = ui.make_persistent_id(("remap-draft", table as u8));
    let mut draft: RemapDraft = ui.data(|d| d.get_temp(draft_id).unwrap_or_default());
    draft.sync(remaps);

    let mut changed = false;
    let mut remove = None; // farewell, little rebind

    // A capture in progress swallows the next keypress into its field.
    //
    // captured_keycode, not captured: a rebind is matched against waywall's
    // keycode table, where the keybind spelling of anything but a letter or a
    // digit does not appear at all.
    if let Some(active) = capture.clone() {
        if active.table == table && active.how == RemapEntry::Listening {
            if let Some(pressed) = keys::captured_keycode(ui.ctx()) {
                if let Some(row) = draft.rows.get_mut(active.row) {
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

    if !draft.rows.is_empty() {
        // Which half is which was only said in a paragraph further up.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            ui.scope(|ui| {
                ui.set_min_width(REMAP_COLUMN);
                ui.weak("You press");
            });
            ui.weak("Minecraft gets");
        });
    }

    for (index, row) in draft.rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= capture_field(ui, table, index, false, &mut row.0, capture);
            ui.weak("becomes");
            changed |= capture_field(ui, table, index, true, &mut row.1, capture);

            ui.add_space(8.0);
            if ui
                .add_sized([26.0, 24.0], egui::Button::new("×"))
                .on_hover_text("Remove this rebind")
                .clicked()
            {
                remove = Some(index);
            }

            // Inline, because the cost of a bad name is the whole config
            // failing to load - not just this row going quiet.
            for half in [&mut row.0, &mut row.1] {
                changed |= warn_unknown(ui, half);
            }

            // An unfinished row is normal while you are filling it in, so this
            // says what is missing rather than complaining.
            if row.0.trim().is_empty() || row.1.trim().is_empty() {
                ui.weak("unfinished").on_hover_text(
                    "Both halves are needed before this rebind is saved. \
                     waywall rejects a rebind with no target, and rejecting \
                     one stops the whole config from loading.",
                );
            }
        });
    }

    if let Some(index) = remove {
        draft.remove(index);
        changed = true;
        *capture = None;
    }

    if ui.button("Add rebind").clicked() {
        draft.add();
        changed = true;
        *capture = Some(RemapCapture {
            table,
            row: draft.rows.len() - 1,
            to_side: false,
            how: RemapEntry::Listening,
        });
    }

    if changed {
        draft.commit_into(remaps);
    }

    ui.data_mut(|d| d.insert_temp(draft_id, draft));
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

    if ui
        .add(crate::tabs::keybinds::listen_button(ui, listening, "Set"))
        .on_hover_text(
            "Press this, then press the key. A modifier on its own cannot be \
             read this way, so use List for those.",
        )
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
    let list = egui::Button::new("List…").fill(if picking {
        ui.visuals().selection.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    });
    let picker = ui
        .add(list)
        .on_hover_text("Every key waywall knows, including Left Alt, Right Shift and the rest");

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
             on its own cannot be captured. Use the browse button next to \
             Set instead.",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The reported bug: "Add rebind" appeared to do nothing.
    ///
    /// A new row is empty by definition, and the save filter drops incomplete
    /// rows because waywall aborts its whole config load over a rebind with no
    /// target. With the document as the only storage those two rules met in
    /// the middle and the row was gone before it could be drawn.
    #[test]
    fn adding_a_rebind_leaves_a_row_to_fill_in() {
        let mut remaps = BTreeMap::new();
        let mut draft = RemapDraft::default();

        draft.sync(&remaps);
        draft.add();
        draft.commit_into(&mut remaps);

        // Next frame.
        draft.sync(&remaps);
        assert_eq!(draft.rows.len(), 1, "the new row did not survive the frame");
        assert!(remaps.is_empty(), "an unfinished rebind must not reach the document");
    }

    #[test]
    fn a_row_reaches_the_document_once_both_halves_are_set() {
        let mut remaps = BTreeMap::new();
        let mut draft = RemapDraft::default();

        draft.sync(&remaps);
        draft.add();
        draft.commit_into(&mut remaps);
        draft.sync(&remaps);

        draft.rows[0].0 = "Q".into();
        draft.commit_into(&mut remaps);
        draft.sync(&remaps);
        assert!(remaps.is_empty(), "still half filled");
        assert_eq!(draft.rows.len(), 1, "and still being edited");

        draft.rows[0].1 = "O".into();
        draft.commit_into(&mut remaps);
        draft.sync(&remaps);

        assert_eq!(remaps.get("Q").map(String::as_str), Some("O"));
        assert_eq!(draft.rows.len(), 1);
    }

    /// Several new rows at once, which is what happens when you add three
    /// rebinds before filling any of them in.
    #[test]
    fn unfinished_rows_do_not_collide() {
        let mut remaps = BTreeMap::new();
        let mut draft = RemapDraft::default();
        draft.sync(&remaps);

        for _ in 0..3 {
            draft.add();
            draft.commit_into(&mut remaps);
            draft.sync(&remaps);
        }

        assert_eq!(draft.rows.len(), 3, "empty rows collapsed into one another");
    }

    #[test]
    fn removing_a_row_removes_it() {
        let mut remaps = BTreeMap::new();
        remaps.insert("MB4".to_string(), "HOME".to_string());
        remaps.insert("P".to_string(), "F3".to_string());

        let mut draft = RemapDraft::default();
        draft.sync(&remaps);
        assert_eq!(draft.rows.len(), 2);

        draft.remove(0);
        draft.commit_into(&mut remaps);
        draft.sync(&remaps);

        assert_eq!(draft.rows.len(), 1);
        assert!(!remaps.contains_key("MB4"));
    }

    /// A change from outside the widget wins over the draft: loading another
    /// config, or the file changing on disk, must not be papered over by rows
    /// left from the previous document.
    #[test]
    fn an_edit_from_elsewhere_reseeds_the_rows() {
        let mut remaps = BTreeMap::new();
        let mut draft = RemapDraft::default();
        draft.sync(&remaps);
        draft.add();
        draft.rows[0] = ("Q".into(), "O".into());
        draft.commit_into(&mut remaps);

        // Something else replaces the document's rebinds entirely.
        remaps.clear();
        remaps.insert("LEFTALT".to_string(), "F3".to_string());
        draft.sync(&remaps);

        assert_eq!(draft.rows, vec![("LEFTALT".to_string(), "F3".to_string())]);
    }
}
