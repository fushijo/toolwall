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

use std::time::{Duration, Instant};

use anyhow::Result;
use toolwall_core::{problems, Document, Scope, Store};

use tabs::input::RemapCapture;

/// How long to wait after the last edit before applying it.
///
/// Every apply reloads waywall's config, so this batches a burst of nudges
/// into one reload instead of one per keystroke.
const APPLY_AFTER: Duration = Duration::from_millis(350);

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

    let saved = serde_json::to_string(&doc).unwrap_or_default();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(DEFAULT_SIZE)
            .with_min_inner_size([480.0, 360.0])
            .with_title("toolwall")
            .with_transparent(true),
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
                remap_capture: None,
                advanced: false,
                saved,
                pending_since: None,
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
    Theme,
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
    remap_capture: Option<RemapCapture>,
    advanced: bool,

    /// The document as last written, so an edit can be noticed without every
    /// widget having to report one.
    saved: String,
    pending_since: Option<Instant>,
}

impl App {
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

    /// Push the document's appearance settings into egui, every frame, so
    /// edits in the Theme tab are visible as you make them.
    fn apply_appearance(&mut self, ctx: &egui::Context) {
        let look = &self.doc.gui.appearance;

        let mut visuals = if look.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        // Translucency has to be painted, not just requested: the window is
        // transparent, so every opaque surface we draw is one we chose to.
        let alpha = (look.opacity.clamp(0.15, 1.0) * 255.0) as u8;
        let tint = |c: egui::Color32| {
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
        };

        visuals.panel_fill = tint(if look.dark {
            egui::Color32::from_rgb(12, 12, 14)
        } else {
            egui::Color32::from_rgb(246, 246, 248)
        });
        visuals.window_fill = visuals.panel_fill;
        visuals.extreme_bg_color = tint(visuals.extreme_bg_color);
        visuals.faint_bg_color = tint(visuals.faint_bg_color);

        ctx.set_visuals(visuals);

        // Zoom scales the whole UI with the text, which keeps hit targets and
        // spacing proportional - setting a font size alone does not.
        let zoom = (look.font_size / 14.0).clamp(0.7, 1.8);
        if (ctx.zoom_factor() - zoom).abs() > 0.01 {
            ctx.set_zoom_factor(zoom);
        }
    }

    /// Apply edits shortly after they stop, so a change is visible in the
    /// game without hunting for a Save button.
    fn apply_when_settled(&mut self, ctx: &egui::Context) {
        let current = serde_json::to_string(&self.doc).unwrap_or_default();

        if current != self.saved {
            self.pending_since.get_or_insert_with(Instant::now);
        }

        let Some(since) = self.pending_since else { return };

        if since.elapsed() < APPLY_AFTER {
            // Come back when the debounce is up, even without further input.
            ctx.request_repaint_after(APPLY_AFTER - since.elapsed());
            return;
        }

        self.pending_since = None;

        // Never write a document the runtime would reject; the error stays on
        // screen and the edit stays in the editor until it is fixed.
        if !problems(&self.doc).is_empty() {
            return;
        }

        self.status = Some(match self.store.save(&self.doc) {
            Ok(()) => {
                self.saved = current;
                (true, "Applied".to_string())
            }
            Err(err) => (false, format!("{err:#}")),
        });
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
        self.apply_appearance(ctx);

        // Escape dismisses the editor rather than falling through to the game.
        // While a key is being captured it cancels that instead, which is the
        // more local meaning.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.capturing.is_some() || self.remap_capture.is_some() {
                self.capturing = None;
                self.remap_capture = None;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

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
                ui.selectable_value(&mut self.tab, Tab::Theme, "Theme");
                ui.selectable_value(&mut self.tab, Tab::Input, "Input");

                // Right-aligned close. The editor floats over the game, so
                // dismissing it needs to be reachable without the keybind.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").on_hover_text("Close (Esc)").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // Everything most people need is in Basic; Advanced adds
                    // ids, layering and the things that break a setup.
                    ui.separator();
                    ui.selectable_value(&mut self.advanced, true, "Advanced");
                    ui.selectable_value(&mut self.advanced, false, "Basic");
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let blocked = !problems.is_empty();

                ui.label(if blocked {
                    "Not applied"
                } else if self.pending_since.is_some() {
                    "Applying…"
                } else {
                    "Changes apply as you make them"
                });

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
                Tab::Modes => tabs::modes::show(ui, &mut self.doc, &problems, self.advanced),
                Tab::Mirrors => tabs::mirrors::show(ui, &mut self.doc, &problems, self.advanced),
                Tab::Images => {
                    tabs::images::show(ui, &mut self.doc, &problems, &mut self.browser, self.advanced)
                }
                Tab::Keybinds => {
                    tabs::keybinds::show(ui, &mut self.doc, &problems, &mut self.capturing, self.advanced)
                }
                Tab::Theme => tabs::theme::show(ui, &mut self.doc, self.advanced),
                Tab::Input => tabs::input::show(
                    ui,
                    &mut self.doc,
                    &problems,
                    self.advanced,
                    &mut self.remap_capture,
                ),
            }
        });

        self.apply_when_settled(ctx);
    }
}

