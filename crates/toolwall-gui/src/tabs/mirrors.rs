//! Mirrors: a region of the Minecraft window drawn somewhere else.

use toolwall_core::schema::{ColorKey, Mirror, Rect};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{
    depth_editor,
    optional_text,
    problems_for,
    rect_editor,
    settings_grid,
    shader_picker,
};

pub fn show(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem], advanced: bool) {
    let shaders: Vec<String> = doc.shaders.keys().cloned().collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove = None;

        for (index, mirror) in doc.mirrors.iter_mut().enumerate() {
            let heading = mirror.label.clone().unwrap_or_else(|| mirror.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(("mirror", index))
                .show(ui, |ui| {
                    problems_for(ui, problems, &Scope::Mirror(mirror.id.clone()));

                    settings_grid(ui, ("mirror-grid", index), |ui| {
                        if advanced {
                            ui.label("ID");
                            ui.text_edit_singleline(&mut mirror.id);
                            ui.end_row();
                        }

                        ui.label("Name");
                        optional_text(ui, &mut mirror.label);
                        ui.end_row();

                        ui.label("Capture from")
                            .on_hover_text("Region of the game to copy");
                        rect_editor(ui, "src", &mut mirror.src);
                        ui.end_row();

                        ui.label("Draw at").on_hover_text("Where on screen to draw it");
                        rect_editor(ui, "dst", &mut mirror.dst);
                        ui.end_row();

                        if advanced {
                            ui.label("Layer").on_hover_text(
                                "Higher draws in front. Leave unset unless \
                                 two overlays are fighting over the same spot.",
                            );
                            depth_editor(ui, &mut mirror.depth);
                            ui.end_row();

                            ui.label("Shader");
                            shader_picker(
                                ui,
                                &format!("mirror-shader-{index}"),
                                &mut mirror.shader,
                                &shaders,
                            );
                            ui.end_row();

                            ui.label("Colour key").on_hover_text(
                                "Replace one colour with another as it is drawn",
                            );
                            color_key_editor(ui, &mut mirror.color_key);
                            ui.end_row();
                        }
                    });

                    if advanced && ui.button("Remove mirror").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.mirrors.remove(index);
        }

        ui.separator();

        if advanced && ui.button("Add mirror").clicked() {
            doc.mirrors.push(Mirror {
                id: format!("mirror{}", doc.mirrors.len() + 1),
                label: None,
                src_anchor: None,
                color_keys: Vec::new(),
                src: Rect { x: 0, y: 0, w: 100, h: 100 },
                dst: Rect { x: 0, y: 0, w: 100, h: 100 },
                depth: None,
                shader: None,
                color_key: None,
            });
        }
    });
}

fn color_key_editor(ui: &mut egui::Ui, key: &mut Option<ColorKey>) {
    ui.vertical(|ui| {
        let mut enabled = key.is_some();
        if ui.checkbox(&mut enabled, "enabled").changed() {
            *key = enabled.then(|| ColorKey {
                input: "#ffffff".into(),
                output: "#ff0000".into(),
            });
        }

        if let Some(key) = key {
            ui.horizontal(|ui| {
                ui.label("in");
                ui.text_edit_singleline(&mut key.input);
                ui.label("out");
                ui.text_edit_singleline(&mut key.output);
            });
        }
    });
}
