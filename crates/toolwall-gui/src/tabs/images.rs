//! Images: a PNG overlay drawn over the instance.

use std::path::Path;

use toolwall_core::schema::{Image, Rect};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{
    depth_editor, optional_text, path_field, problems_for, rect_editor, shader_picker,
    FileBrowser, PickTarget,
};

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    problems: &[Problem],
    browser: &mut FileBrowser,
    advanced: bool,
) {
    let shaders: Vec<String> = doc.shaders.keys().cloned().collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove = None;

        for (index, image) in doc.images.iter_mut().enumerate() {
            let heading = image.label.clone().unwrap_or_else(|| image.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(("image", index))
                .show(ui, |ui| {
                    problems_for(ui, problems, &Scope::Image(image.id.clone()));

                    egui::Grid::new(("image-grid", index))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            if advanced {
                                ui.label("ID");
                                ui.text_edit_singleline(&mut image.id);
                                ui.end_row();
                            }

                            ui.label("Name");
                            optional_text(ui, &mut image.label);
                            ui.end_row();

                            ui.label("Image file");
                            path_field(
                                ui,
                                &mut image.path,
                                browser,
                                PickTarget::Image(index),
                                "png",
                            );
                            ui.end_row();

                            ui.label("Draw at");
                            rect_editor(ui, "dst", &mut image.dst);
                            ui.end_row();

                            if advanced {
                                ui.label("Layer");
                                depth_editor(ui, &mut image.depth);
                                ui.end_row();

                                ui.label("Shader");
                                shader_picker(
                                    ui,
                                    &format!("image-shader-{index}"),
                                    &mut image.shader,
                                    &shaders,
                                );
                                ui.end_row();
                            }
                        });

                    if advanced && ui.button("Remove image").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.images.remove(index);
        }

        ui.separator();

        if advanced && ui.button("Add image").clicked() {
            doc.images.push(Image {
                id: format!("image{}", doc.images.len() + 1),
                label: None,
                path: String::new(),
                dst: Rect { x: 0, y: 0, w: 100, h: 100 },
                depth: None,
                shader: None,
            });
        }
    });
}
