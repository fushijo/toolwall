//! Keybinds: an input string bound to a command from a closed set.
//!
//! Commands are an enum on purpose. A keybind never carries Lua source, so a
//! config downloaded from a stranger cannot execute arbitrary code on load.
//! That is why this tab offers a picker and typed argument fields rather than
//! a script box.

use serde_json::{json, Value};
use toolwall_core::schema::{Command, Keybind};
use toolwall_core::{Document, Problem, Scope};

use crate::keys;
use crate::widgets::{optional_text, problems_for};

const COMMANDS: &[(Command, &str)] = &[
    (Command::ModeSet, "mode.set"),
    (Command::ModeReset, "mode.reset"),
    (Command::ModeCycle, "mode.cycle"),
    (Command::SensSet, "sens.set"),
    (Command::KeymapSet, "keymap.set"),
    (Command::RemapsSet, "remaps.set"),
    (Command::KeyPress, "key.press"),
    (Command::FullscreenToggle, "fullscreen.toggle"),
    (Command::FloatingToggle, "floating.toggle"),
    (Command::FloatingShow, "floating.show"),
    (Command::FloatingHide, "floating.hide"),
    (Command::GuiToggle, "gui.toggle"),
    (Command::AppToggle, "app.toggle"),
    (Command::OverlayToggle, "overlay.toggle"),
    (Command::Exec, "exec"),
];

fn command_name(command: Command) -> &'static str {
    COMMANDS.iter().find(|(c, _)| *c == command).map(|(_, n)| *n).unwrap_or("?")
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    problems: &[Problem],
    capturing: &mut Option<usize>,
) {
    let mode_ids: Vec<String> = doc.modes.iter().map(|m| m.id.clone()).collect();
    let app_ids: Vec<String> = doc.apps.iter().map(|a| a.id.clone()).collect();
    let overlay_ids: Vec<String> = doc
        .mirrors
        .iter()
        .map(|m| m.id.clone())
        .chain(doc.images.iter().map(|i| i.id.clone()))
        .collect();
    let allow_exec = doc.gui.allow_exec;

    // A capture in progress swallows the next keypress into the binding.
    if let Some(index) = *capturing {
        if let Some(input) = keys::captured(ui.ctx()) {
            if let Some(bind) = doc.keybinds.get_mut(index) {
                bind.input = input;
            }
            *capturing = None;
        }
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove = None;

        for (index, bind) in doc.keybinds.iter_mut().enumerate() {
            let heading = match &bind.label {
                Some(label) => format!("{}  ({})", label, bind.input),
                None => format!("{}  {}", bind.input, command_name(bind.command)),
            };

            egui::CollapsingHeader::new(heading)
                .id_salt(("keybind", index))
                .show(ui, |ui| {
                    problems_for(ui, problems, &Scope::Keybind(bind.input.clone()));

                    egui::Grid::new(("keybind-grid", index))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("input");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut bind.input);

                                let capturing_this = *capturing == Some(index);
                                let button = if capturing_this {
                                    "press a key…"
                                } else {
                                    "Capture"
                                };
                                if ui.selectable_label(capturing_this, button).clicked() {
                                    *capturing = if capturing_this { None } else { Some(index) };
                                }
                            });
                            ui.end_row();

                            ui.label("label");
                            optional_text(ui, &mut bind.label);
                            ui.end_row();

                            ui.label("command");
                            egui::ComboBox::from_id_salt(("command", index))
                                .selected_text(command_name(bind.command))
                                .show_ui(ui, |ui| {
                                    for (command, name) in COMMANDS {
                                        if ui
                                            .selectable_label(bind.command == *command, *name)
                                            .clicked()
                                            && bind.command != *command
                                        {
                                            bind.command = *command;
                                            bind.args = None;
                                        }
                                    }
                                });
                            ui.end_row();

                            args_editor(ui, index, bind, &mode_ids, &app_ids, &overlay_ids, allow_exec);
                        });

                    if ui.button("Remove keybind").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.keybinds.remove(index);
            if *capturing == Some(index) {
                *capturing = None;
            }
        }

        ui.separator();

        if ui.button("Add keybind").clicked() {
            doc.keybinds.push(Keybind {
                input: String::new(),
                command: Command::ModeReset,
                args: None,
                label: None,
            });
            *capturing = Some(doc.keybinds.len() - 1);
        }
    });
}

/// Argument shape per command, from `docs/schema.md`.
fn args_editor(
    ui: &mut egui::Ui,
    index: usize,
    bind: &mut Keybind,
    mode_ids: &[String],
    app_ids: &[String],
    overlay_ids: &[String],
    allow_exec: bool,
) {
    match bind.command {
        Command::ModeSet => {
            ui.label("mode");
            let current = arg_str(&bind.args, "mode");
            egui::ComboBox::from_id_salt(("arg-mode", index))
                .selected_text(if current.is_empty() { "—".into() } else { current.clone() })
                .show_ui(ui, |ui| {
                    for id in mode_ids {
                        if ui.selectable_label(&current == id, id).clicked() {
                            set_arg(&mut bind.args, "mode", json!(id));
                        }
                    }
                });
            ui.end_row();
        }

        Command::AppToggle => {
            ui.label("app").on_hover_text(
                "Launches the app if it is not running yet, then reveals it",
            );
            ui.vertical(|ui| {
                let current = arg_str(&bind.args, "app");
                egui::ComboBox::from_id_salt(("arg-app", index))
                    .selected_text(if current.is_empty() { "—".into() } else { current.clone() })
                    .show_ui(ui, |ui| {
                        for id in app_ids {
                            if ui.selectable_label(&current == id, id).clicked() {
                                set_arg(&mut bind.args, "app", json!(id));
                            }
                        }
                    });
                if app_ids.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 170, 80),
                        "⚠ no apps defined - add one in the Input tab",
                    );
                }
            });
            ui.end_row();
        }

        Command::OverlayToggle => {
            ui.label("overlay")
                .on_hover_text("Shown on top of whatever mode is active, until toggled off");
            let current = arg_str(&bind.args, "overlay");
            egui::ComboBox::from_id_salt(("arg-overlay", index))
                .selected_text(if current.is_empty() { "—".into() } else { current.clone() })
                .show_ui(ui, |ui| {
                    for id in overlay_ids {
                        if ui.selectable_label(&current == id, id).clicked() {
                            set_arg(&mut bind.args, "overlay", json!(id));
                        }
                    }
                });
            ui.end_row();
        }

        Command::ModeCycle => {
            ui.label("modes")
                .on_hover_text("Leave all unchecked to cycle every mode in document order");
            ui.vertical(|ui| {
                let mut selected: Vec<String> = bind
                    .args
                    .as_ref()
                    .and_then(|a| a.get("modes"))
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();

                let mut changed = false;
                for id in mode_ids {
                    let mut on = selected.iter().any(|s| s == id);
                    if ui.checkbox(&mut on, id).changed() {
                        changed = true;
                        if on {
                            selected.push(id.clone());
                        } else {
                            selected.retain(|s| s != id);
                        }
                    }
                }
                if changed {
                    set_arg(&mut bind.args, "modes", json!(selected));
                }
            });
            ui.end_row();
        }

        Command::SensSet => {
            ui.label("sensitivity").on_hover_text("0 restores input.sensitivity");
            let mut value = bind
                .args
                .as_ref()
                .and_then(|a| a.get("sensitivity"))
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            if ui
                .add(egui::DragValue::new(&mut value).speed(0.01).range(0.0..=10.0))
                .changed()
            {
                set_arg(&mut bind.args, "sensitivity", json!(value));
            }
            ui.end_row();
        }

        Command::KeymapSet => {
            for field in ["layout", "model", "rules", "variant", "options"] {
                ui.label(field);
                let mut text = arg_str(&bind.args, field);
                if ui.text_edit_singleline(&mut text).changed() {
                    set_arg(&mut bind.args, field, json!(text));
                }
                ui.end_row();
            }
        }

        Command::RemapsSet => {
            ui.label("remaps");
            ui.vertical(|ui| {
                let mut map = bind
                    .args
                    .as_ref()
                    .and_then(|a| a.get("remaps"))
                    .and_then(Value::as_object)
                    .map(Clone::clone)
                    .unwrap_or_default();

                if remap_rows(ui, ("bind-remaps", index), &mut map) {
                    set_arg(&mut bind.args, "remaps", Value::Object(map));
                }
            });
            ui.end_row();
        }

        Command::KeyPress => {
            ui.label("key").on_hover_text("A waywall keycode, e.g. F3");
            let mut text = arg_str(&bind.args, "key");
            if ui.text_edit_singleline(&mut text).changed() {
                set_arg(&mut bind.args, "key", json!(text));
            }
            ui.end_row();
        }

        Command::Exec => {
            ui.label("command");
            ui.vertical(|ui| {
                let mut text = arg_str(&bind.args, "command");
                if ui.text_edit_singleline(&mut text).changed() {
                    set_arg(&mut bind.args, "command", json!(text));
                }
                if !allow_exec {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 170, 80),
                        "⚠ exec is disabled - enable gui.allow_exec in the Input tab",
                    );
                }
            });
            ui.end_row();
        }

        Command::ModeReset
        | Command::FullscreenToggle
        | Command::FloatingToggle
        | Command::FloatingShow
        | Command::FloatingHide
        | Command::GuiToggle => {}
    }
}

/// Shared by the keybind `remaps.set` args and the Input tab.
pub fn remap_rows(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Clone,
    map: &mut serde_json::Map<String, Value>,
) -> bool {
    let mut changed = false;
    let mut remove = None;
    let mut rename: Option<(String, String)> = None;

    for (from, to) in map.iter_mut() {
        ui.horizontal(|ui| {
            let mut from_edit = from.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut from_edit).desired_width(90.0))
                .changed()
            {
                rename = Some((from.clone(), from_edit));
            }

            ui.label("→");

            let mut to_edit = to.as_str().unwrap_or_default().to_owned();
            if ui
                .add(egui::TextEdit::singleline(&mut to_edit).desired_width(90.0))
                .changed()
            {
                *to = json!(to_edit);
                changed = true;
            }

            if ui.button("✕").clicked() {
                remove = Some(from.clone());
            }
        });
    }

    if let Some((old, new)) = rename {
        if let Some(value) = map.remove(&old) {
            map.insert(new, value);
            changed = true;
        }
    }

    if let Some(key) = remove {
        map.remove(&key);
        changed = true;
    }

    if ui
        .button("Add remap")
        .on_hover_text("Source key or mouse button → output key")
        .clicked()
    {
        let _ = salt;
        map.insert(String::new(), json!(""));
        changed = true;
    }

    changed
}

fn arg_str(args: &Option<Value>, key: &str) -> String {
    args.as_ref()
        .and_then(|a| a.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn set_arg(args: &mut Option<Value>, key: &str, value: Value) {
    match args {
        Some(Value::Object(map)) => {
            map.insert(key.to_owned(), value);
        }
        _ => {
            let mut map = serde_json::Map::new();
            map.insert(key.to_owned(), value);
            *args = Some(Value::Object(map));
        }
    }
}
