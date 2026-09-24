//! The editor as a sheet of glass over the game.
//!
//! Same dragging as the Screen tab, except the window is the size of waywall's
//! own and has nothing painted behind it, so you are moving the outline of a
//! mirror across the actual mirror rather than across a scaled drawing of one.
//!
//! What makes this possible, and it is not obvious from outside:
//!
//! - A window launched from inside waywall is a floating window of waywall's,
//!   the same as Ninjabrain Bot.
//! - waywall ignores `set_opaque_region`, calling it an optimization hint, so
//!   a client with an alpha channel really is see-through.
//! - Floating views and the scene are siblings under the same parent surface
//!   and the game is explicitly placed below both, so a floating window draws
//!   on top of the mirrors it is editing rather than behind them.
//! - `on_keyboard_key` runs waywall's own key actions before handing anything
//!   to the focused window, so a keybind can always close this even while it
//!   covers everything.
//!
//! The one thing it cannot do is work out how big to be. waywall hands its
//! clients a fixed 8192x8192 output and answers a fullscreen request with a
//! 1x1 configure, so the size comes from `gui.screen` like everywhere else.

use std::time::{Duration, Instant};

use toolwall_core::{Document, Store};

use crate::canvas::{Canvas, State};
use crate::tabs::screen::{apply, collect};

const APPLY_AFTER: Duration = Duration::from_millis(350);

pub struct Overlay {
    store: Store,
    doc: Document,
    canvas: State,
    mode: String,
    snapping: bool,
    saved: String,
    pending_since: Option<Instant>,
    status: Option<String>,
}

impl Overlay {
    pub fn new(store: Store, doc: Document) -> Self {
        let saved = serde_json::to_string(&doc).unwrap_or_default();
        let mode = doc.modes.first().map(|m| m.id.clone()).unwrap_or_default();

        Overlay {
            store,
            doc,
            canvas: State::default(),
            mode,
            snapping: true,
            saved,
            pending_since: None,
            status: None,
        }
    }

    /// How big the window should be, which is also the coordinate space every
    /// overlay position is written in.
    pub fn screen(doc: &Document) -> [f32; 2] {
        [doc.gui.screen.w.max(320) as f32, doc.gui.screen.h.max(240) as f32]
    }

    fn save_when_settled(&mut self, ctx: &egui::Context) {
        let current = serde_json::to_string(&self.doc).unwrap_or_default();
        if current != self.saved {
            self.pending_since.get_or_insert_with(Instant::now);
        }

        let Some(since) = self.pending_since else { return };
        if since.elapsed() < APPLY_AFTER {
            ctx.request_repaint_after(APPLY_AFTER - since.elapsed());
            return;
        }

        self.pending_since = None;

        if !toolwall_core::problems(&self.doc).is_empty() {
            return;
        }

        self.status = match self.store.save(&self.doc) {
            Ok(()) => {
                self.saved = current;
                None
            }
            Err(err) => Some(format!("{err:#}")),
        };
    }

    /// A small bar in the corner, kept out of the way of the overlays.
    fn bar(&mut self, ctx: &egui::Context) -> bool {
        let mut done = false;

        egui::Area::new("overlay-bar".into())
            .fixed_pos(egui::pos2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Placing overlays");
                        ui.separator();

                        egui::ComboBox::from_id_salt("overlay-mode")
                            .width(110.0)
                            .selected_text(if self.mode.is_empty() {
                                "Base".to_string()
                            } else {
                                self.mode.clone()
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.mode, String::new(), "Base");
                                for mode in &self.doc.modes {
                                    let label =
                                        mode.label.clone().unwrap_or_else(|| mode.id.clone());
                                    ui.selectable_value(&mut self.mode, mode.id.clone(), label);
                                }
                            });

                        ui.checkbox(&mut self.snapping, "Snap");
                        ui.separator();

                        if ui.button("Done").clicked() {
                            done = true;
                        }
                        ui.weak("or Esc");
                    });

                    if let Some(err) = &self.status {
                        ui.colored_label(egui::Color32::from_rgb(220, 90, 90), err);
                    }
                });
            });

        done
    }
}

impl eframe::App for Overlay {
    /// Nothing behind the window, so the game shows through.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let close = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        if self.bar(ctx) || close {
            // Write anything outstanding before the window goes away, or the
            // last nudge before Esc is the one that gets thrown out.
            self.pending_since = Some(Instant::now() - APPLY_AFTER);
            self.save_when_settled(ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let items = collect(&self.doc, &self.mode);
                if items.is_empty() {
                    return;
                }

                let screen = (
                    self.doc.gui.screen.w.max(1) as i32,
                    self.doc.gui.screen.h.max(1) as i32,
                );
                let snapping = self.snapping && !ui.input(|i| i.modifiers.alt);

                let action = Canvas { screen, items: &items, snapping, over_the_real_thing: true }
                    .show(ui, &mut self.canvas);
                apply(&mut self.doc, action);
            });

        self.save_when_settled(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::App;

    fn doc_with_screen(w: u32, h: u32) -> Document {
        let mut doc = Document::default();
        doc.gui.screen = toolwall_core::schema::Size { w, h };
        doc
    }

    /// The window has to be exactly waywall's window, or the one thing this
    /// buys over the Screen tab - a pixel here being a pixel there - is gone.
    #[test]
    fn the_window_is_the_size_of_waywalls() {
        assert_eq!(Overlay::screen(&doc_with_screen(1707, 1067)), [1707.0, 1067.0]);
    }

    /// A zero would be taken literally by winit and clamped to 1x1, which is
    /// how you get a window that looks like it never opened.
    #[test]
    fn a_nonsense_screen_size_does_not_produce_a_nonsense_window() {
        let tiny = Overlay::screen(&doc_with_screen(0, 0));
        assert!(tiny[0] >= 320.0 && tiny[1] >= 240.0, "{tiny:?}");
    }

    #[test]
    fn it_draws_nothing_behind_itself() {
        let doc = doc_with_screen(1707, 1067);
        let store = Store::new(std::env::temp_dir().join("toolwall-overlay-test.json"));
        let overlay = Overlay::new(store, doc);

        assert_eq!(
            overlay.clear_color(&egui::Visuals::dark())[3],
            0.0,
            "an opaque clear colour hides the game it is supposed to sit over"
        );
    }

    #[test]
    fn it_renders_with_overlays_and_without() {
        let path = std::env::temp_dir().join("toolwall-overlay-render.json");

        for doc in [doc_with_screen(1280, 800), {
            let mut d = doc_with_screen(1280, 800);
            d.mirrors.push(toolwall_core::schema::Mirror {
                id: "pie".into(),
                label: None,
                src_anchor: None,
                dst_anchor: None,
                src: toolwall_core::schema::Rect { x: 0, y: 0, w: 10, h: 10 },
                dst: toolwall_core::schema::Rect { x: 100, y: 100, w: 200, h: 200 },
                depth: None,
                shader: None,
                color_key: None,
                color_keys: vec![],
                outline: None,
            });
            d
        }] {
            let mut overlay = Overlay::new(Store::new(&path), doc);
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                // eframe::Frame cannot be built outside eframe, so drive the
                // parts that do not need it.
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
                        let items = collect(&overlay.doc, &overlay.mode);
                        let screen = (
                            overlay.doc.gui.screen.w as i32,
                            overlay.doc.gui.screen.h as i32,
                        );
                        Canvas {
                            screen,
                            items: &items,
                            snapping: true,
                            over_the_real_thing: true,
                        }
                        .show(ui, &mut overlay.canvas);
                    });
            });
        }
    }
}
