//! Ninjabrain Bot: the jar, the API, and the readout drawn into the scene.

use toolwall_core::ninb_prefs::ACTIONS;
use toolwall_core::{Document, Problem, Scope, NINB_PRESETS};

use crate::ninb_keys::{self, NinbKeys};
use crate::widgets::{color_field, path_field, problems_for, FileBrowser, PickTarget};

const COORDS: &[(&str, &str, &str)] = &[
    ("block", "Block", "the block at the centre of the stronghold chunk"),
    ("chunk", "Chunk", "the chunk numbers ninb reports"),
    ("nether", "Nether", "overworld coordinates divided by eight"),
];

const LAYOUTS: &[(&str, &str)] = &[("ninbot", "Ninjabrain Bot"), ("compact", "Compact")];

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    problems: &[Problem],
    advanced: bool,
    browser: &mut FileBrowser,
    keys: &mut NinbKeys,
) {
    // A capture in progress swallows the next keypress into the hotkey.
    if let Some(index) = keys.capturing {
        if let Some(hotkey) = ninb_keys::capture(ui.ctx()) {
            if let Some(list) = keys.hotkeys.as_mut() {
                if let Some(slot) = list.get_mut(index) {
                    slot.1 = hotkey;
                }
            }
            keys.capturing = None;
        }
    }

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
        hotkeys(ui, doc, keys);

        ui.separator();
        ui.heading("Readout");
        ui.weak("Drawn into the game, so it does not hide with the other floating windows.");

        let o = &mut doc.ninb.overlay;

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Preset");
            for (id, label, hint) in NINB_PRESETS {
                if ui.button(*label).on_hover_text(*hint).clicked() {
                    o.apply_preset(id);
                }
            }
        });
        ui.add_space(4.0);

        egui::Grid::new("ninb-readout")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Enabled");
                ui.checkbox(&mut o.enabled, "");
                ui.end_row();

                ui.label("Style");
                ui.horizontal(|ui| {
                    for (value, label) in LAYOUTS {
                        if ui.selectable_label(o.layout == *value, *label).clicked() {
                            o.layout = (*value).to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label("X");
                ui.add(egui::Slider::new(&mut o.x, 0..=1920));
                ui.end_row();

                ui.label("Y");
                ui.add(egui::Slider::new(&mut o.y, 0..=1200));
                ui.end_row();

                ui.label("Text size").on_hover_text("A whole multiple of the 8x16 face");
                ui.add(egui::Slider::new(&mut o.size, 1..=6));
                ui.end_row();

                ui.label("Line gap").on_hover_text("Extra pixels between rows");
                ui.add(egui::Slider::new(&mut o.line_gap, 0..=24));
                ui.end_row();

                ui.label("Coordinates");
                ui.horizontal(|ui| {
                    for (value, label, hint) in COORDS {
                        if ui
                            .selectable_label(o.coords == *value, *label)
                            .on_hover_text(*hint)
                            .clicked()
                        {
                            o.coords = (*value).to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label("Before any throw");
                ui.text_edit_singleline(&mut o.idle_text);
                ui.end_row();
            });

        ui.separator();

        if o.layout == "compact" {
            ui.heading("Lines");
            egui::Grid::new("ninb-compact")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Predictions shown");
                    ui.add(egui::Slider::new(&mut o.shown_predictions, 1..=5));
                    ui.end_row();

                    ui.label("Line format").on_hover_text(PLACEHOLDERS);
                    ui.text_edit_singleline(&mut o.template);
                    ui.end_row();
                });
            ui.weak(PLACEHOLDERS);
        } else {
            ui.heading("Rows");
            egui::Grid::new("ninb-rows")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Location");
                    ui.checkbox(&mut o.show_location, "");
                    ui.end_row();

                    ui.label("Certainty");
                    ui.checkbox(&mut o.show_certainty, "");
                    ui.end_row();

                    ui.label("Nether coords")
                        .on_hover_text("Where to travel to, at an eighth of the scale");
                    ui.checkbox(&mut o.show_nether, "");
                    ui.end_row();

                    ui.label("Distances");
                    ui.checkbox(&mut o.show_distance, "");
                    ui.end_row();

                    ui.label("Current angle")
                        .on_hover_text("Your heading, and how far to turn to face the stronghold");
                    ui.checkbox(&mut o.show_angle, "");
                    ui.end_row();

                    ui.label("Hints").on_hover_text(
                        "ninb's own advice, eg \"go left 1 block for ~95% after the next throw\"",
                    );
                    ui.checkbox(&mut o.show_info, "");
                    ui.end_row();

                    if o.show_info && advanced {
                        ui.label("Wrap hints at");
                        ui.add(egui::Slider::new(&mut o.wrap_width, 20..=100).suffix(" chars"));
                        ui.end_row();
                    }

                    ui.label("Eye throw rows").on_hover_text("0 hides the throw table");
                    ui.add(egui::Slider::new(&mut o.throw_rows, 0..=10));
                    ui.end_row();

                    if o.throw_rows > 0 {
                        ui.label("Throw table header");
                        ui.checkbox(&mut o.show_throw_header, "");
                        ui.end_row();

                        ui.label("Show nudges").on_hover_text(
                            "A throw you nudged reads as \"120.02+2\": what you measured, \
                             and how many increments you moved it",
                        );
                        ui.checkbox(&mut o.show_correction, "");
                        ui.end_row();
                    }
                });
        }

        ui.separator();
        ui.heading("Colours");

        egui::Grid::new("ninb-colours")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Values");
                color_field(ui, &mut o.color);
                ui.end_row();

                if o.layout != "compact" {
                    ui.label("Labels").on_hover_text("The \"Location:\" column");
                    color_field(ui, &mut o.label_color);
                    ui.end_row();
                }

                ui.label("Secondary").on_hover_text("Table headers, hints, the idle line");
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
                    ui.label("High above");
                    ui.add(egui::Slider::new(&mut o.certainty_high_above, 0.0..=100.0).suffix("%"));
                    ui.end_row();

                    ui.label("Medium above");
                    ui.add(egui::Slider::new(&mut o.certainty_mid_above, 0.0..=100.0).suffix("%"));
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("Outline and panel");
        ui.weak("Both need the waywall patch from patches/. Without it they are ignored.");

        egui::Grid::new("ninb-panel")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Text outline").on_hover_text("Width in pixels. 0 is off.");
                ui.add(egui::Slider::new(&mut o.outline, 0..=4));
                ui.end_row();

                if o.outline > 0 {
                    ui.label("Outline colour");
                    color_field(ui, &mut o.outline_color);
                    ui.end_row();
                }

                ui.label("Separators").on_hover_text("Rules between the sections");
                ui.checkbox(&mut o.separators, "");
                ui.end_row();

                if o.separators {
                    ui.label("Separator colour");
                    color_field(ui, &mut o.separator_color);
                    ui.end_row();

                    ui.label("Separator width");
                    ui.add(egui::Slider::new(&mut o.separator_width, 1..=6));
                    ui.end_row();
                }

                ui.label("Background");
                ui.checkbox(&mut o.background, "");
                ui.end_row();

                if o.background {
                    ui.label("Background colour").on_hover_text("Alpha sets the opacity");
                    color_field(ui, &mut o.background_color);
                    ui.end_row();

                    ui.label("Padding");
                    ui.add(egui::Slider::new(&mut o.padding, 0..=40));
                    ui.end_row();

                    ui.label("Border width");
                    ui.add(egui::Slider::new(&mut o.border_width, 0..=8));
                    ui.end_row();

                    if o.border_width > 0 {
                        ui.label("Border colour");
                        color_field(ui, &mut o.border_color);
                        ui.end_row();
                    }
                }
            });

        if advanced {
            ui.separator();
            ui.heading("Refresh");

            egui::Grid::new("ninb-refresh")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Live").on_hover_text(
                        "Hold ninb's event stream open so changes arrive as they happen, \
                         instead of asking again every tick",
                    );
                    ui.checkbox(&mut o.live, "");
                    ui.end_row();

                    ui.label(if o.live { "Redraw check" } else { "Poll every" })
                        .on_hover_text("Nothing is redrawn unless the readout changed");
                    ui.add(egui::Slider::new(&mut o.poll_ms, 16..=2000).suffix(" ms"));
                    ui.end_row();

                    ui.label("Hide when stale").on_hover_text("0 keeps it on screen");
                    ui.add(egui::Slider::new(&mut o.hide_after_ms, 0..=60000).suffix(" ms"));
                    ui.end_row();
                });
        }

        ui.add_space(6.0);
        ui.weak(
            "The face is waywall's bundled 8x16 terminus and cannot be changed: \
             scene text has no font parameter.",
        );
    });
}

const PLACEHOLDERS: &str = "{x} {z} {certainty} {distance} {netherX} {netherZ} \
                            {netherDistance} {chunkX} {chunkZ} {blockX} {blockZ} \
                            {angle} {angleDelta} {throws} {n}";

/// Ninjabrain Bot's own hotkeys.
///
/// These are not toolwall keybinds and cannot be: ninb watches the keyboard
/// itself. They normally live in ninb's settings window, which is unreachable
/// once that window is hidden, so they are edited here and written straight
/// into ninb's preferences.
fn hotkeys(ui: &mut egui::Ui, doc: &mut Document, keys: &mut NinbKeys) {
    ui.heading("Its hotkeys");
    ui.weak(
        "Ninjabrain Bot watches the keyboard itself, so these are its own \
         keys, not toolwall's.",
    );

    ui.add_space(4.0);
    let mut shown = !doc.theme.ninb_hidden;
    if ui
        .checkbox(&mut shown, "Show its window")
        .on_hover_text("Its own settings window, for anything not listed here")
        .changed()
    {
        doc.theme.ninb_hidden = !shown;
    }
    if shown {
        ui.weak("Reveal floating windows to see it, the same key that opens this editor.");
    }

    if keys.hotkeys.is_none() {
        ui.add_space(4.0);
        if ui.button("Load from Ninjabrain Bot").clicked() {
            keys.load();
        }
        if let Some(status) = &keys.status {
            ui.colored_label(egui::Color32::from_rgb(255, 120, 120), status);
        }
        return;
    }

    ui.add_space(4.0);

    let mut rows: Vec<(usize, &'static str, String, bool, Option<String>)> = Vec::new();
    if let Some(list) = &keys.hotkeys {
        for (index, (name, hotkey)) in list.iter().enumerate() {
            let label = ACTIONS
                .iter()
                .find(|(id, _)| id == name)
                .map(|(_, label)| *label)
                .unwrap_or("");
            let clash = hotkey.key().and_then(|key| {
                doc.keybinds
                    .iter()
                    .find(|bind| ninb_keys::ninb_key_for_input(&bind.input) == Some(key))
                    .map(|bind| bind.input.clone())
            });

            rows.push((index, label, hotkey.label(), grabbed(hotkey), clash));
        }
    }

    let mut capture_clicked = None;

    egui::Grid::new("ninb-hotkeys")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (index, label, current, taken, clash) in &rows {
                ui.label(*label);
                ui.horizontal(|ui| {
                    let capturing = keys.capturing == Some(*index);
                    let text = if capturing { "press a key…" } else { current.as_str() };

                    if ui.selectable_label(capturing, text).clicked() {
                        capture_clicked = Some(*index);
                    }

                    if capturing {
                        return;
                    }

                    if *taken {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 170, 80),
                            "⚠ your desktop takes this key",
                        );
                    }

                    // ninb watches the keyboard, so a key bound here as well
                    // does both things on every press
                    if let Some(bind) = clash {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 120, 120),
                            format!("⚠ toolwall also binds {}", bind),
                        );
                    }
                });
                ui.end_row();
            }
        });

    if let Some(index) = capture_clicked {
        keys.capturing = if keys.capturing == Some(index) { None } else { Some(index) };
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Save and restart ninb").clicked() {
            let launch = ninb_keys::launch_command(&doc.ninb.jar, &doc.ninb.command);
            keys.save(launch.as_deref());
        }
        if ui.button("Reload").clicked() {
            keys.load();
        }
    });

    if let Some(status) = &keys.status {
        ui.add_space(4.0);
        ui.weak(status);
    }

    ui.add_space(4.0);
    ui.weak(
        "Saving restarts ninb, because it rewrites its settings when it exits \
         and would undo the change.",
    );
}

/// Keys the desktop compositor keeps for itself, which therefore never reach
/// waywall and never reach ninb. This is the usual reason a hotkey "does
/// nothing": it is being answered somewhere else entirely.
fn grabbed(hotkey: &toolwall_core::ninb_prefs::Hotkey) -> bool {
    if hotkey.modifier & toolwall_core::ninb_prefs::ANY_META != 0 {
        return true;
    }

    matches!(hotkey.key(), Some("Print Screen") | Some("Super") | Some("Menu"))
}
