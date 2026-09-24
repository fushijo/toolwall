//! Mirrors: a region of the Minecraft window drawn somewhere else.

use toolwall_core::schema::{ColorKey, Mirror, Outline, Rect, DEFAULT_THRESHOLD};
use toolwall_core::{Document, Problem, Scope};

use crate::widgets::{
    anchor_picker,
    color_field,
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

        // Taken before the mutable borrow, because the checkbox below needs
        // to know what is already in there while doc.mirrors is held.
        let in_base: Vec<String> = doc.base_overlays.clone();
        let mut toggle_base: Option<String> = None;

        for (index, mirror) in doc.mirrors.iter_mut().enumerate() {
            // Older configs carry one key in a field of its own. Fold it into
            // the list so there is only ever one place to edit. Only when the
            // list is empty: taking it unconditionally would throw it away
            // wherever both are set.
            if mirror.color_keys.is_empty() {
                if let Some(single) = mirror.color_key.take() {
                    mirror.color_keys.push(single);
                }
            }

            let heading = mirror.label.clone().unwrap_or_else(|| mirror.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(("mirror", index))
                .default_open(index == 0)
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

                        ui.label("Show in every mode").on_hover_text(
                            "Normally an overlay appears only in the modes that \
                             list it. This puts it on the screen whatever mode \
                             you are in, including none.",
                        );
                        let mut base = in_base.contains(&mirror.id);
                        if ui.checkbox(&mut base, "").changed() {
                            toggle_base = Some(mirror.id.clone());
                        }
                        ui.end_row();

                        ui.label("Capture from")
                            .on_hover_text("Region of the game to copy");
                        rect_editor(ui, "src", &mut mirror.src);
                        ui.end_row();

                        ui.label("Measured from").on_hover_text(
                            "Which corner of the game the capture offsets start \
                             at. Minecraft pins its debug HUD to the corners, so \
                             an unanchored capture is only right at one \
                             resolution.",
                        );
                        anchor_picker(ui, &format!("mirror-src-anchor-{index}"), &mut mirror.src_anchor);
                        ui.end_row();

                        ui.label("Draw at").on_hover_text("Where on screen to draw it");
                        rect_editor(ui, "dst", &mut mirror.dst);
                        ui.end_row();

                        ui.label("Anchored to").on_hover_text(
                            "Which corner of the screen the offsets start at. \
                             Without one, this lands in the wrong place on any \
                             screen that is not the size you set it up at.",
                        );
                        anchor_picker(ui, &format!("mirror-dst-anchor-{index}"), &mut mirror.dst_anchor);
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

                            ui.label("Border").on_hover_text(
                                "Needs the rect patch (patches/apply.sh). \
                                 Ignored without it.",
                            );
                            outline_editor(ui, &mut mirror.outline);
                            ui.end_row();
                        }
                    });

                    colors_section(ui, index, &mut mirror.color_keys);

                    if advanced && ui.button("Remove mirror").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.mirrors.remove(index);
        }

        if let Some(id) = toggle_base {
            match doc.base_overlays.iter().position(|b| *b == id) {
                Some(at) => {
                    doc.base_overlays.remove(at);
                }
                None => doc.base_overlays.push(id),
            }
        }

        ui.separator();

        if ui.button("Add mirror").clicked() {
            doc.mirrors.push(Mirror {
                id: format!("mirror{}", doc.mirrors.len() + 1),
                label: None,
                src_anchor: None,
                dst_anchor: None,
                color_keys: Vec::new(),
                src: Rect { x: 0, y: 0, w: 100, h: 100 },
                dst: Rect { x: 0, y: 0, w: 100, h: 100 },
                depth: None,
                shader: None,
                outline: None,
                color_key: None,
            });
        }
    });
}

/// Colour keying, which does not do what its name suggests.
///
/// Every pixel that does not match is made fully transparent, so this is
/// "keep only these colours", not "recolour this". The pie chart is the
/// reason it takes a list: one entry per slice you want to keep.
fn colors_section(ui: &mut egui::Ui, index: usize, keys: &mut Vec<ColorKey>) {
    egui::CollapsingHeader::new("Keep only certain colours")
        .id_salt(("mirror-colors", index))
        .default_open(!keys.is_empty())
        .show(ui, |ui| {
            ui.weak(
                "Everything else in the capture is made see-through. Use one \
                 row per colour you want to keep, like one per pie chart slice.",
            );
            ui.add_space(6.0);

            let mut drop = None;

            for (row, key) in keys.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label("Keep").on_hover_text("The colour to look for in the capture");
                    color_field(ui, &mut key.input);

                    ui.label("draw as").on_hover_text(
                        "What to paint it instead. The same colour leaves it as it is.",
                    );
                    color_field(ui, &mut key.output);

                    if ui
                        .button("×")
                        .on_hover_text("Remove this colour")
                        .clicked()
                    {
                        drop = Some(row);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Tolerance").on_hover_text(
                        "How far a pixel may be from that colour and still \
                         count. Raise it for anything with a soft or shaded \
                         edge; lower it if colours you did not want are \
                         getting through.",
                    );
                    ui.add(
                        egui::Slider::new(&mut key.threshold, 0.0..=0.5)
                            .fixed_decimals(3)
                            .custom_formatter(|v, _| format!("{:.0}/255", v * 255.0))
                            .custom_parser(|s| s.trim_end_matches("/255").parse().ok()),
                    );
                    if key.threshold != DEFAULT_THRESHOLD {
                        ui.weak("uses a generated shader");
                    }
                });

                ui.add_space(4.0);
            }

            if let Some(row) = drop {
                keys.remove(row);
            }

            if ui.button("Add a colour").clicked() {
                keys.push(ColorKey {
                    input: "#ffffffff".into(),
                    output: "#ffffffff".into(),
                    threshold: DEFAULT_THRESHOLD,
                });
            }
        });
}

fn outline_editor(ui: &mut egui::Ui, outline: &mut Option<Outline>) {
    ui.horizontal(|ui| {
        let mut on = outline.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *outline = on.then(Outline::default);
        }

        if let Some(outline) = outline {
            ui.add(
                egui::DragValue::new(&mut outline.width)
                    .range(0..=32)
                    .suffix(" px"),
            )
            .on_hover_text("Thickness");
            color_field(ui, &mut outline.color);
        }

        // Room under the last row, so the status bar never cuts one in half.
        ui.add_space(24.0);
    });
}
