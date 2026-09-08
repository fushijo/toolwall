//! Mirrors: a region of the Minecraft window drawn somewhere else.

use toolwall_core::schema::{ColorKey, Mirror, Rect};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{depth_editor, optional_text, problems_for, rect_editor, shader_picker};

pub fn show(ui: &mut egui::Ui, doc: &mut Document, problems: &[Problem]) {
    let shaders: Vec<String> = doc.shaders.keys().cloned().collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove = None;

        for (index, mirror) in doc.mirrors.iter_mut().enumerate() {
            let heading = mirror.label.clone().unwrap_or_else(|| mirror.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(("mirror", index))
                .show(ui, |ui| {
                    problems_for(ui, problems, &Scope::Mirror(mirror.id.clone()));

                    egui::Grid::new(("mirror-grid", index))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("id");
                            ui.text_edit_singleline(&mut mirror.id);
                            ui.end_row();

                            ui.label("label");
                            optional_text(ui, &mut mirror.label);
                            ui.end_row();

                            ui.label("src")
                                .on_hover_text("Region of the Minecraft window to copy from");
                            rect_editor(ui, "src", &mut mirror.src);
                            ui.end_row();

                            ui.label("dst").on_hover_text("Where to draw it");
                            rect_editor(ui, "dst", &mut mirror.dst);
                            ui.end_row();

                            ui.label("depth");
                            depth_editor(ui, &mut mirror.depth);
                            ui.end_row();

                            ui.label("shader");
                            shader_picker(ui, &format!("mirror-shader-{index}"), &mut mirror.shader, &shaders);
                            ui.end_row();

                            ui.label("colour key");
                            color_key_editor(ui, &mut mirror.color_key);
                            ui.end_row();
                        });

                    if ui.button("Remove mirror").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.mirrors.remove(index);
        }

        ui.separator();

        if ui.button("Add mirror").clicked() {
            doc.mirrors.push(Mirror {
                id: format!("mirror{}", doc.mirrors.len() + 1),
                label: None,
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
