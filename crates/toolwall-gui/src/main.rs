//! toolwall GUI.
//!
//! Launched *inside* waywall via `waywall.exec()`, so it appears as a floating
//! window over the game — the same mechanism Ninjabrain Bot uses.
//!
//! It never talks to waywall directly. It edits `toolwall.json` and saves
//! through `toolwall_core::Store`, which trips waywall's hot reload. That is
//! the entire integration surface, and it is why this binary needs no patched
//! waywall and no IPC.
//!
//! # Status
//!
//! Skeleton. The Modes tab is wired end to end as the reference pattern;
//! Mirrors, Images, Keybinds and Input follow the same shape.

use anyhow::Result;
use toolwall_core::{schema, Document, Store};

fn main() -> Result<()> {
    let store = Store::at_default_path()?;

    // Start from disk if possible, but a broken config must still open the
    // editor - that is precisely when you need it.
    let (doc, load_error) = match store.load() {
        Ok(doc) => (doc, None),
        Err(err) => (Document::default(), Some(err.to_string())),
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 520.0])
            .with_min_inner_size([480.0, 360.0])
            .with_title("toolwall"),
        ..Default::default()
    };

    eframe::run_native(
        "toolwall",
        options,
        Box::new(|_cc| Ok(Box::new(App { store, doc, load_error, status: None, tab: Tab::Modes }))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Modes,
    Mirrors,
    Images,
    Keybinds,
    Input,
}

struct App {
    store: Store,
    doc: Document,
    load_error: Option<String>,
    status: Option<(bool, String)>,
    tab: Tab,
}

impl App {
    fn save(&mut self) {
        self.status = Some(match self.store.save(&self.doc) {
            Ok(()) => (true, "Saved and reloaded".to_string()),
            Err(err) => (false, format!("{err:#}")),
        });
    }

    fn revert(&mut self) {
        self.status = Some(match self.store.load() {
            Ok(doc) => {
                self.doc = doc;
                (true, "Reverted to the file on disk".to_string())
            }
            Err(err) => (false, format!("{err:#}")),
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Modes, "Modes");
                ui.selectable_value(&mut self.tab, Tab::Mirrors, "Mirrors");
                ui.selectable_value(&mut self.tab, Tab::Images, "Images");
                ui.selectable_value(&mut self.tab, Tab::Keybinds, "Keybinds");
                ui.selectable_value(&mut self.tab, Tab::Input, "Input");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Revert").clicked() {
                    self.revert();
                }

                ui.separator();

                if let Some(err) = &self.load_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("load failed: {err}"));
                } else if let Some((ok, message)) = &self.status {
                    let color = if *ok {
                        egui::Color32::LIGHT_GREEN
                    } else {
                        egui::Color32::LIGHT_RED
                    };
                    ui.colored_label(color, message);
                } else {
                    ui.weak(self.store.path().display().to_string());
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Modes => modes_tab(ui, &mut self.doc),
            other => {
                let name = match other {
                    Tab::Mirrors => "Mirrors",
                    Tab::Images => "Images",
                    Tab::Keybinds => "Keybinds",
                    Tab::Input => "Input",
                    Tab::Modes => unreachable!(),
                };
                ui.weak(format!("{name}: not implemented yet."));
                ui.weak("Follow the pattern in modes_tab - edit self.doc in place, then Save.");
            }
        });
    }
}

/// Reference implementation for every other tab.
///
/// Note what is absent: no waywall calls, no IPC, no diffing. Mutate the
/// document, hand it to the store, done.
fn modes_tab(ui: &mut egui::Ui, doc: &mut Document) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut remove: Option<usize> = None;

        for (index, mode) in doc.modes.iter_mut().enumerate() {
            let heading = mode.label.clone().unwrap_or_else(|| mode.id.clone());

            egui::CollapsingHeader::new(heading)
                .id_salt(index)
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new(format!("mode-{index}"))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("id");
                            ui.text_edit_singleline(&mut mode.id);
                            ui.end_row();

                            ui.label("label");
                            let mut label = mode.label.clone().unwrap_or_default();
                            if ui.text_edit_singleline(&mut label).changed() {
                                mode.label = (!label.is_empty()).then_some(label);
                            }
                            ui.end_row();

                            ui.label("width");
                            ui.add(
                                egui::DragValue::new(&mut mode.resolution.width)
                                    .range(0..=16384),
                            );
                            ui.end_row();

                            ui.label("height");
                            ui.add(
                                egui::DragValue::new(&mut mode.resolution.height)
                                    .range(0..=16384),
                            );
                            ui.end_row();

                            ui.label("sensitivity");
                            ui.horizontal(|ui| {
                                let mut overridden = mode.sensitivity.is_some();
                                if ui.checkbox(&mut overridden, "override").changed() {
                                    mode.sensitivity = overridden.then_some(1.0);
                                }
                                if let Some(sens) = &mut mode.sensitivity {
                                    ui.add(
                                        egui::DragValue::new(sens)
                                            .speed(0.01)
                                            .range(0.01..=10.0),
                                    );
                                }
                            });
                            ui.end_row();

                            ui.label("toggle off on repress");
                            ui.checkbox(&mut mode.toggle, "");
                            ui.end_row();
                        });

                    if ui.button("Remove mode").clicked() {
                        remove = Some(index);
                    }
                });
        }

        if let Some(index) = remove {
            doc.modes.remove(index);
        }

        ui.separator();

        if ui.button("Add mode").clicked() {
            doc.modes.push(schema::Mode {
                id: format!("mode{}", doc.modes.len() + 1),
                label: None,
                resolution: schema::Resolution { width: 0, height: 0 },
                sensitivity: None,
                toggle: true,
                mirrors: Vec::new(),
                images: Vec::new(),
            });
        }
    });
}
