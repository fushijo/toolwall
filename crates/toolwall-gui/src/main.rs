//! toolwall GUI.
//!
//! Launched *inside* waywall via `waywall.exec()`, so it appears as a floating
//! window over the game — the same mechanism Ninjabrain Bot uses.
//!
//! It never talks to waywall directly. It edits `toolwall.json` and saves
//! through `toolwall_core::Store`, which trips waywall's hot reload. That is
//! the entire integration surface, and it is why this binary needs no patched
//! waywall and no IPC.

mod keys;
mod tabs;
mod widgets;

use anyhow::Result;
use toolwall_core::{problems, Document, Scope, Store};

use widgets::FileBrowser;

/// Also the size re-asserted when waywall configures us to nothing.
const DEFAULT_SIZE: [f32; 2] = [720.0, 520.0];

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
            .with_inner_size(DEFAULT_SIZE)
            .with_min_inner_size([480.0, 360.0])
            .with_title("toolwall"),
        ..Default::default()
    };

    eframe::run_native(
        "toolwall",
        options,
        Box::new(|_cc| {
            Ok(Box::new(App {
                store,
                doc,
                load_error,
                status: None,
                tab: Tab::Modes,
                browser: FileBrowser::default(),
                capturing: None,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[cfg(test)]
mod tests;

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
    browser: FileBrowser,
    /// Index of the keybind currently swallowing the next keypress.
    capturing: Option<usize>,
}

impl App {
    fn save(&mut self) {
        self.status = Some(match self.store.save(&self.doc) {
            Ok(()) => (true, "Saved and reloaded".to_string()),
            Err(err) => (false, format!("{err:#}")),
        });
    }

    /// Re-assert our size while the compositor has left us degenerately small.
    ///
    /// waywall configures floating windows with `xdg_toplevel.configure(0, 0)`,
    /// which in xdg-shell means "choose your own size". winit takes the zero
    /// literally and clamps the surface to 1x1, so the GUI renders every frame
    /// correctly into a single pixel and looks like it never launched at all.
    /// A normal desktop compositor sends a real size, which is why this only
    /// happens inside waywall.
    ///
    /// Re-requesting stops as soon as we have a usable size, so it does not
    /// fight the user resizing the window afterwards.
    fn ensure_usable_size(ctx: &egui::Context) {
        const MIN_USABLE: f32 = 64.0;

        let size = ctx.screen_rect().size();
        if size.x >= MIN_USABLE && size.y >= MIN_USABLE {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            DEFAULT_SIZE[0],
            DEFAULT_SIZE[1],
        )));
        ctx.request_repaint();
    }

    fn revert(&mut self) {
        self.status = Some(match self.store.load() {
            Ok(doc) => {
                self.doc = doc;
                self.load_error = None;
                (true, "Reverted to the file on disk".to_string())
            }
            Err(err) => (false, format!("{err:#}")),
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::ensure_usable_size(ctx);

        // Cheap for a document this size, and it keeps every inline warning
        // honest as you type rather than only at save time.
        let problems = problems(&self.doc);

        if let Some((index, path)) = self.browser.show(ctx) {
            if let Some(image) = self.doc.images.get_mut(index) {
                image.path = path;
            }
        }

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
                // Save is refused rather than allowed to fail in the store:
                // a bad write here hot-reloads into a live session.
                let blocked = !problems.is_empty();
                let save = ui.add_enabled(!blocked, egui::Button::new("Save"));
                if save.clicked() {
                    self.save();
                }
                if blocked {
                    save.on_hover_text("Fix the highlighted problems first");
                }

                if ui.button("Revert").clicked() {
                    self.revert();
                }

                ui.separator();

                if let Some(err) = &self.load_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("load failed: {err}"));
                } else if blocked {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 120, 120),
                        format!("{} problem(s) — see the highlighted items", problems.len()),
                    );
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

        egui::CentralPanel::default().show(ctx, |ui| {
            // Document-wide problems have no item to sit next to.
            for problem in problems.iter().filter(|p| p.scope == Scope::Document) {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 120, 120),
                    format!("⚠ {}", problem.message),
                );
            }

            match self.tab {
                Tab::Modes => tabs::modes::show(ui, &mut self.doc, &problems),
                Tab::Mirrors => tabs::mirrors::show(ui, &mut self.doc, &problems),
                Tab::Images => {
                    tabs::images::show(ui, &mut self.doc, &problems, &mut self.browser)
                }
                Tab::Keybinds => {
                    tabs::keybinds::show(ui, &mut self.doc, &problems, &mut self.capturing)
                }
                Tab::Input => tabs::input::show(ui, &mut self.doc),
            }
        });
    }
}

