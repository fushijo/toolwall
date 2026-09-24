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
use crate::widgets::{optional_text, problems_for, settings_grid};

/// Every command, as what it does and as what it is called in the file.
///
/// The identifier is what waywall and `docs/schema.md` use, and it is the only
/// thing anyone reading the JSON will see, so it stays: as hover text, and in
/// Advanced. But `floating.toggle` in a dropdown labelled Basic asks a runner
/// to know the waywall Lua API before they can bind a key.
const COMMANDS: &[(Command, &str, &str)] = &[
    (Command::ModeSet, "Switch to a mode", "mode.set"),
    (Command::ModeReset, "Back to the normal size", "mode.reset"),
    (Command::ModeCycle, "Step through the modes", "mode.cycle"),
    (Command::SensSet, "Set the mouse sensitivity", "sens.set"),
    (Command::KeymapSet, "Switch keyboard layout", "keymap.set"),
    (Command::RemapsSet, "Switch to a set of rebinds", "remaps.set"),
    (Command::KeyPress, "Press a key in the game", "key.press"),
    (Command::FullscreenToggle, "Fullscreen on and off", "fullscreen.toggle"),
    (Command::FloatingToggle, "Floating windows on and off", "floating.toggle"),
    (Command::FloatingShow, "Show the floating windows", "floating.show"),
    (Command::FloatingHide, "Hide the floating windows", "floating.hide"),
    (Command::GuiToggle, "Open this editor", "gui.toggle"),
    (Command::ScreenEdit, "Place overlays over the game", "screen.edit"),
    (Command::RemapsToggle, "Type in chat", "remaps.toggle"),
    (Command::NinbToggle, "Ninjabrain Bot's window", "ninb.toggle"),
    (Command::NinbOverlay, "Ninjabrain readout on and off", "ninb.overlay"),
    (Command::OverlayToggle, "Show or hide overlays", "overlay.toggle"),
    (Command::Exec, "Run a command", "exec"),
];

/// The overlays a bind toggles, from either shape the config can be in.
fn overlay_args(args: &Option<Value>) -> Vec<String> {
    let Some(args) = args else { return Vec::new() };

    if let Some(list) = args.get("overlays").and_then(Value::as_array) {
        return list.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    }

    args.get("overlay")
        .and_then(Value::as_str)
        .map(|id| vec![id.to_string()])
        .unwrap_or_default()
}

fn clear_arg(args: &mut Option<Value>, key: &str) {
    if let Some(Value::Object(map)) = args {
        map.remove(key);
    }
}

/// Width of the key column. Wide enough for "Shift + Caps Lock", which is
/// about as long as a real bind gets.
const KEY_COLUMN: f32 = 120.0;

/// The Capture / Set / List buttons, in both states.
///
/// Filled with the selection colour while it is swallowing the next key, so
/// "this is listening" is a fill and a word rather than a shade of grey.
pub(crate) fn listen_button(ui: &egui::Ui, listening: bool, idle: &str) -> egui::Button<'static> {
    if listening {
        egui::Button::new(egui::RichText::new("press a key…").strong())
            .fill(ui.visuals().selection.bg_fill)
    } else {
        egui::Button::new(idle.to_string())
    }
}

/// What it does, for anywhere a person reads it.
fn command_name(command: Command) -> &'static str {
    COMMANDS.iter().find(|(c, ..)| *c == command).map(|(_, name, _)| *name).unwrap_or("?")
}

/// What it is called in the file, for the hover and for Advanced.
fn command_id(command: Command) -> &'static str {
    COMMANDS.iter().find(|(c, ..)| *c == command).map(|(.., id)| *id).unwrap_or("?")
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    problems: &[Problem],
    capturing: &mut Option<usize>,
    advanced: bool,
) {
    let mode_ids: Vec<String> = doc.modes.iter().map(|m| m.id.clone()).collect();
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
        suspend_switch(ui, doc);

        // Above the list. Below it, a config with ten binds put it off the
        // bottom of the panel behind a scrollbar.
        if ui.button("Add keybind").clicked() {
            doc.keybinds.push(Keybind {
                f3_safe: true,
                ingame_only: false,
                input: String::new(),
                command: Command::ModeReset,
                args: None,
                label: None,
            });
            *capturing = Some(doc.keybinds.len() - 1);
        }
        ui.add_space(4.0);

        let mut remove = None;

        for (index, bind) in doc.keybinds.iter_mut().enumerate() {
            let id = ui.make_persistent_id(("keybind", index));
            let what = match &bind.label {
                Some(label) if !label.trim().is_empty() => label.clone(),
                _ => command_name(bind.command).to_string(),
            };

            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                index == 0,
            )
                .show_header(ui, |ui| {
                    // A column of keys, so the list can be read down rather
                    // than word by word. The key is what you are looking for.
                    ui.scope(|ui| {
                        ui.set_min_width(KEY_COLUMN);
                        ui.monospace(keys::pretty(&bind.input));
                    });
                    ui.label(what);
                })
                .body(|ui| {
                    problems_for(ui, problems, &Scope::Keybind(bind.input.clone()));

                    settings_grid(ui, ("keybind-grid", index), |ui| {
                        ui.label("Key");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut bind.input);

                            // A frame, so it reads as something you press.
                            // selectable_label put it at the same weight and
                            // the same colour as the row's own label.
                            let capturing_this = *capturing == Some(index);
                            if ui.add(listen_button(ui, capturing_this, "Capture")).clicked() {
                                *capturing = if capturing_this { None } else { Some(index) };
                            }
                        });
                        ui.end_row();

                        ui.label("Name");
                        optional_text(ui, &mut bind.label);
                        ui.end_row();

                        ui.label("Ignore while F3 is held").on_hover_text(
                            "So F3 combos reach Minecraft. Shift, Ctrl and Alt \
                             are already safe: waywall matches modifiers exactly.",
                        );
                        ui.checkbox(&mut bind.f3_safe, "");
                        ui.end_row();

                        ui.label("Only while unpaused in a world").on_hover_text(
                            "gore's config calls this ingame_only. Keeps the key \
                             from firing on the title screen or in a menu. Needs \
                             the State Output mod.",
                        );
                        ui.checkbox(&mut bind.ingame_only, "");
                        ui.end_row();

                        ui.label("Does");
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("command", index))
                                .selected_text(command_name(bind.command))
                                .width(230.0)
                                .show_ui(ui, |ui| {
                                    for (command, name, id) in COMMANDS {
                                        if ui
                                            .selectable_label(bind.command == *command, *name)
                                            .on_hover_text(*id)
                                            .clicked()
                                            && bind.command != *command
                                        {
                                            bind.command = *command;
                                            bind.args = None;
                                        }
                                    }
                                });

                            if advanced {
                                ui.weak(command_id(bind.command));
                            }
                        });
                        ui.end_row();

                        args_editor(ui, index, bind, &mode_ids, &overlay_ids, allow_exec);
                    });

                    if advanced && ui.button("Remove keybind").clicked() {
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

        // Room under the last row, so the status bar never cuts one in half.
        ui.add_space(24.0);
    });
}

/// Argument shape per command, from `docs/schema.md`.
fn args_editor(
    ui: &mut egui::Ui,
    index: usize,
    bind: &mut Keybind,
    mode_ids: &[String],
    overlay_ids: &[String],
    allow_exec: bool,
) {
    match bind.command {
        Command::ModeSet => {
            ui.label("mode");
            let current = arg_str(&bind.args, "mode");
            egui::ComboBox::from_id_salt(("arg-mode", index))
                .selected_text(if current.is_empty() { "-".into() } else { current.clone() })
                .show_ui(ui, |ui| {
                    for id in mode_ids {
                        if ui.selectable_label(&current == id, id).clicked() {
                            set_arg(&mut bind.args, "mode", json!(id));
                        }
                    }
                });
            ui.end_row();
        }

        Command::OverlayToggle => {
            ui.label("Overlays").on_hover_text(
                "Shown on top of whatever mode is active, until toggled off. \
                 Tick several and one key brings all of them up together.",
            );
            ui.vertical(|ui| {
                let mut selected = overlay_args(&bind.args);

                let mut changed = false;
                for id in overlay_ids {
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
                    set_arg(&mut bind.args, "overlays", json!(selected));
                    // The single-id form is what older configs carry. Once
                    // the list exists it is the only one read, so leaving it
                    // behind would just be a stale copy in the file.
                    clear_arg(&mut bind.args, "overlay");
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
        | Command::GuiToggle
        | Command::ScreenEdit
        | Command::RemapsToggle
        | Command::NinbToggle
        | Command::NinbOverlay => {}
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

            ui.label("->");

            let mut to_edit = to.as_str().unwrap_or_default().to_owned();
            if ui
                .add(egui::TextEdit::singleline(&mut to_edit).desired_width(90.0))
                .changed()
            {
                *to = json!(to_edit);
                changed = true;
            }

            if ui.button("×").clicked() {
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
        .on_hover_text("Source key or mouse button -> output key")
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

/// Turn every keybind off except the one that opens this editor.
///
/// For when toolwall's keys are in the way, of the game or of another tool.
/// It is meant to be turned off again, so it says so plainly and the top bar
/// keeps saying so from whichever tab you wander to.
fn suspend_switch(ui: &mut egui::Ui, doc: &mut Document) {
    ui.checkbox(&mut doc.suspend_keybinds, "Suspend all keybinds");

    if doc.suspend_keybinds {
        let escape = doc
            .keybinds
            .iter()
            .find(|b| b.command == Command::GuiToggle)
            .map(|b| b.input.clone());

        match escape {
            Some(input) => {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 170, 80),
                    format!("Only {input} still does anything. Everything else reaches the game."),
                );
            }
            None => {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 120, 120),
                    "Nothing here opens this editor, so closing it strands you. \
                     Bind a key to gui.toggle, or run: toolwall set suspend_keybinds false",
                );
            }
        }
    }

    ui.separator();
}
