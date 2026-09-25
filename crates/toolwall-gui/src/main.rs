//! toolwall GUI.
//!
//! Launched *inside* waywall via `waywall.exec()`, so it appears as a floating
//! window over the game, the same mechanism Ninjabrain Bot uses.
//!
//! It never talks to waywall directly. It edits `toolwall.json` and saves
//! through `toolwall_core::Store`, which trips waywall's hot reload. That is
//! the entire integration surface, and it is why this binary needs no patched
//! waywall and no IPC.

mod canvas;
#[cfg(feature = "screenshot")]
mod shots;
mod keys;
mod overlay;
mod setup;
mod ninb_keys;
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

use widgets::{FileBrowser, PickTarget};

/// Also the size re-asserted when waywall configures us to nothing.
const DEFAULT_SIZE: [f32; 2] = [936.0, 676.0];

/// The overlay: no frame, nothing painted behind it, and exactly as big as
/// waywall's window so a pixel here is a pixel there.
fn run_overlay(store: Store) -> Result<()> {
    let doc = store.load().unwrap_or_default();
    let size = overlay::Overlay::screen(&doc);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_title("toolwall overlay")
            .with_app_id("toolwall-overlay")
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "toolwall-overlay",
        options,
        Box::new(move |_cc| Ok(Box::new(overlay::Overlay::new(store, doc)))),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(())
}

/// The setup window, on a normal desktop rather than over the game: it runs
/// before anyone has a key to open anything with.
fn run_setup(store: Store) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([680.0, 480.0])
            .with_title("toolwall setup")
            // Wayland finds a window's icon by matching this against a
            // .desktop file's name, and egui-winit only sets it when you
            // hand it one. Without this the window has no app id at all and
            // the dock falls back to a grey placeholder.
            .with_app_id("toolwall-setup"),
        ..Default::default()
    };

    eframe::run_native(
        "toolwall-setup",
        options,
        Box::new(move |_cc| Ok(Box::new(setup::Setup::new(store)))),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(())
}

fn main() -> Result<()> {
    // Before anything opens a window. `--version` used to open the editor
    // and sit there, which is a surprising way to answer a question asked in
    // a terminal, and made the binary impossible to probe from a script.
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("toolwall-gui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "toolwall-gui {}\n\n\
             Usage: toolwall-gui [OPTIONS]\n\n\
             With no options it opens the editor, on the config waywall reads.\n\n\
             Options:\n  \
               --setup              Open the setup window instead\n  \
               --overlay            Place overlays over the running game\n  \
               -h, --help           Print this\n  \
               -V, --version        Print the version",
            env!("CARGO_PKG_VERSION"),
        );
        return Ok(());
    }

    let store = Store::at_default_path()?;

    // `--overlay` is the same editing, over the game instead of over a
    // drawing of it. Same binary so both share the canvas and the store;
    // waywall launches it as a second floating window.
    if args.iter().any(|a| a == "--overlay") {
        return run_overlay(store);
    }

    // `--setup` is the ten minutes before there is a config worth editing.
    // Its own window because it is a different job: it asks questions and
    // writes once, where the editor edits continuously.
    if args.iter().any(|a| a == "--setup") {
        return run_setup(store);
    }


    // Start from disk if possible, but a broken config must still open the
    // editor - that is precisely when you need it.
    let (doc, load_error) = match store.load() {
        Ok(doc) => (doc, None),
        Err(err) => (Document::default(), Some(err.to_string())),
    };

    #[cfg(feature = "screenshot")]
    if let Some(dir) = std::env::args()
        .skip_while(|a| a != "--screenshot")
        .nth(1)
    {
        return shots::Shooter::run(store, doc, dir.into(), DEFAULT_SIZE);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(DEFAULT_SIZE)
            .with_min_inner_size([480.0, 360.0])
            .with_title("toolwall")
            .with_app_id("toolwall")
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "toolwall",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(store, doc, load_error)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) enum Tab {
    Modes,
    Mirrors,
    Images,
    Keybinds,
    Theme,
    Ninb,
    Input,
    Layout,
    Screen,
}

pub(crate) struct App {
    store: Store,
    pub(crate) doc: Document,
    load_error: Option<String>,
    status: Option<(bool, String)>,
    pub(crate) tab: Tab,
    browser: FileBrowser,
    /// Index of the keybind currently swallowing the next keypress.
    capturing: Option<usize>,
    /// when the config on disk was last written, to spot outside edits
    seen_modified: Option<std::time::SystemTime>,
    last_checked: Instant,
    ninb_keys: ninb_keys::NinbKeys,
    remap_capture: Option<RemapCapture>,
    layout_edit: tabs::layout::LayoutEdit,
    screen_edit: tabs::screen::ScreenEdit,
    /// The layout last written to disk, so an unchanged one is not rewritten.
    written_layout: Option<toolwall_core::schema::CustomLayout>,
    pub(crate) advanced: bool,

    /// The document as last written, so an edit can be noticed without every
    /// widget having to report one.
    saved: String,
    pending_since: Option<Instant>,
    /// font path currently loaded, so it is only re-read when it changes
    font_loaded: String,
}

/// The whole look, in one place.
///
/// Shared with the setup window, which is a separate eframe app and was
/// therefore running on egui's stock dark theme: no border on a text field, no
/// fill on an unpicked segment, and dimmer text than the editor beside it.
pub(crate) fn appearance(ctx: &egui::Context, dark: bool, opacity: f32) {
        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        // Translucency has to be painted, not just requested: the window is
        // transparent, so every opaque surface we draw is one we chose to.
        let alpha = (opacity.clamp(0.15, 1.0) * 255.0) as u8;
        let tint = |c: egui::Color32| {
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
        };

        // Raise the ordinary and the dim text a step.
        //
        // egui's dark defaults put body text at #8c8c8c on a near-black panel,
        // which is under 5:1, and the weak text it derives from that is around
        // 2.5:1. This floats over a bright game on a laptop screen, so it has
        // to be readable at a glance rather than merely present.
        if dark {
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(205);
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_gray(215);
            // ui.weak() is this blended halfway to the panel, and it carries
            // load-bearing text: column headers, "Drag to move", every hint.
            // Left at egui's default it landed at 4.0:1, under AA.
            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_gray(110);
            // A checkbox you cannot see the edge of is not a control. 1.86:1
            // against the panel, measured.
            visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(58);
            // The selected tab and the accent.
            //
            // The pill's own label was the least legible text in the window:
            // egui puts light blue on this fill, which came to 3.5:1 while the
            // unselected tabs beside it were 13:1. Darker fill, white label.
            visuals.selection.bg_fill = egui::Color32::from_rgb(0, 86, 122);
            // egui takes the selected label's colour from here.
            visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        } else {
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(30);
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_gray(20);
            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_gray(150);
        }

        // A field has to look like a field.
        //
        // egui's dark theme fills a text box at near-panel-black with no
        // border, so "Cursor icon" and "Model" and "Rules" read as gaps in the
        // panel rather than as somewhere you can type.
        let edge = if dark {
            egui::Color32::from_gray(106)
        } else {
            egui::Color32::from_gray(160)
        };
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
        ] {
            widget.bg_stroke = egui::Stroke::new(1.0, edge);
        }

        // An unselected segment of a segmented control had no fill and no
        // border at all, so "Light" and "2560x1440" and "Always on" read as
        // captions sitting next to the one that happened to be filled.
        visuals.widgets.inactive.weak_bg_fill = if dark {
            egui::Color32::from_gray(38)
        } else {
            egui::Color32::from_gray(228)
        };
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, edge.gamma_multiply(1.4));

        // ui.weak() is this colour blended halfway to weak_bg_fill, so the
        // bump above is what carries the hints too.

        visuals.panel_fill = tint(if dark {
            egui::Color32::from_rgb(12, 12, 14)
        } else {
            egui::Color32::from_rgb(246, 246, 248)
        });
        visuals.window_fill = visuals.panel_fill;
        visuals.extreme_bg_color = tint(visuals.extreme_bg_color);
        visuals.faint_bg_color = tint(visuals.faint_bg_color);

        ctx.set_visuals(visuals);

        ctx.style_mut(|style| {
            // Solid, always there. A floating bar on a dark panel is invisible
            // until you are already scrolling, so a control row cut in half by
            // the status bar read as a crash rather than as "there is more".
            style.spacing.scroll = egui::style::ScrollStyle::solid();

            // A checkbox at egui's default 14px is both hard to see and hard
            // to hit. 24 is the smallest target worth shipping.
            style.spacing.icon_width = 18.0;
            style.spacing.icon_width_inner = 10.0;
            style.spacing.interact_size.y = style.spacing.interact_size.y.max(24.0);
        });
}

impl App {
    pub(crate) fn new(store: Store, doc: Document, load_error: Option<String>) -> Self {
        Self {
            seen_modified: store.modified(),
            saved: serde_json::to_string(&doc).unwrap_or_default(),
            advanced: doc.gui.appearance.advanced,
            store,
            doc,
            load_error,
            status: None,
            tab: Tab::Modes,
            browser: FileBrowser::default(),
            capturing: None,
            last_checked: Instant::now(),
            ninb_keys: ninb_keys::NinbKeys::default(),
            remap_capture: None,
            layout_edit: Default::default(),
            screen_edit: Default::default(),
            written_layout: None,
            pending_since: None,
            font_loaded: String::new(),
        }
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

    /// Push the document's appearance settings into egui, every frame, so
    /// edits in the Theme tab are visible as you make them.
    fn apply_appearance(&mut self, ctx: &egui::Context) {
        let look = &self.doc.gui.appearance;

        // reload the face only when the path changes, not every frame
        if look.font_path != self.font_loaded {
            self.font_loaded = look.font_path.clone();
            Self::load_font(ctx, &self.font_loaded);
        }

        appearance(ctx, look.dark, look.opacity);

        // Zoom scales the whole UI with the text, which keeps hit targets and
        // spacing proportional - setting a font size alone does not.
        let zoom = (look.font_size / 14.0).clamp(0.7, 2.2);
        if (ctx.zoom_factor() - zoom).abs() > 0.01 {
            ctx.set_zoom_factor(zoom);
        }
    }

    /// swap in a font from disk, falling back to the built-in one.
    fn load_font(ctx: &egui::Context, path: &str) {
        let mut fonts = egui::FontDefinitions::default();

        if !path.is_empty() {
            match std::fs::read(widgets::expand_tilde(path)) {
                Ok(bytes) => {
                    fonts.font_data.insert(
                        "custom".into(),
                        egui::FontData::from_owned(bytes).into(),
                    );
                    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        fonts.families.entry(family).or_default().insert(0, "custom".into());
                    }
                }
                // a bad path should not leave the editor unreadable
                Err(_) => {}
            }
        }

        ctx.set_fonts(fonts);
    }

    /// Apply edits shortly after they stop, so a change is visible in the
    /// game without hunting for a Save button.
    /// Notice the config being changed by something other than this editor.
    ///
    /// The editor writes the whole document, so whatever it holds in memory
    /// wins the next time anything is edited. A change made from the CLI, or
    /// by another copy of the editor, would be reverted without a trace: that
    /// is how `theme.ninb_hidden` came back on after being turned off.
    fn adopt_external_changes(&mut self, ctx: &egui::Context) {
        const CHECK_EVERY: Duration = Duration::from_millis(1000);

        if self.last_checked.elapsed() < CHECK_EVERY {
            ctx.request_repaint_after(CHECK_EVERY - self.last_checked.elapsed());
            return;
        }
        self.last_checked = Instant::now();

        let Some(modified) = self.store.modified() else { return };
        if Some(modified) == self.seen_modified {
            return;
        }
        self.seen_modified = Some(modified);

        // An edit still settling is about to be written anyway, and would be
        // what changed the file. Only a surprise is worth acting on.
        if self.pending_since.is_some() {
            return;
        }

        if let Ok(doc) = self.store.load() {
            let body = serde_json::to_string(&doc).unwrap_or_default();
            if body != self.saved {
                self.doc = doc;
                self.saved = body;
                self.status = Some((true, "Reloaded, it changed outside the editor".into()));
            }
        }
    }

    /// Keep `~/.config/xkb/symbols/<name>` matching the document.
    ///
    /// The document is the layout's home; the file is generated from it. Doing
    /// this on save means a layout edited in the editor cannot silently differ
    /// from the one waywall loads, which would look exactly like the editor
    /// not working.
    fn sync_layout_file(&mut self) -> anyhow::Result<()> {
        let Some(layout) = self.doc.input.custom_layout.clone() else {
            return Ok(());
        };

        // Rewriting an unchanged file on every save would churn its mtime for
        // nothing.
        if self.written_layout.as_ref() == Some(&layout) {
            return Ok(());
        }

        toolwall_core::xkb::write_symbols(&layout)?;
        self.written_layout = Some(layout);
        Ok(())
    }

    fn apply_when_settled(&mut self, ctx: &egui::Context) {
        let current = serde_json::to_string(&self.doc).unwrap_or_default();

        if current != self.saved {
            if self.pending_since.is_none() {
                self.pending_since = Some(Instant::now());
                // The last result is about the last write, not this one.
                self.status = None;
            }
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

        // The symbols file first, so it is already on disk when the save
        // trips waywall's reload and it rebuilds its keymap. The other order
        // reloads the old layout and needs a second save to take.
        if let Err(err) = self.sync_layout_file() {
            self.status = Some((false, format!("{err:#}")));
            return;
        }

        self.status = Some(match self.store.save(&self.doc) {
            Ok(()) => {
                self.saved = current;
                self.seen_modified = self.store.modified();
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

        if let Some((target, path)) = self.browser.show(ctx) {
            match target {
                PickTarget::Image(index) => {
                    if let Some(image) = self.doc.images.get_mut(index) {
                        image.path = path;
                    }
                }
                PickTarget::Background => self.doc.theme.background_png = path,
                PickTarget::NinbJar => self.doc.ninb.jar = path,
                PickTarget::Font => self.doc.gui.appearance.font_path = path,
            }
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                // Which tab you are on was carried by the fill alone, and the
                // fill is the one thing a colour-blind reader may not have.
                // The underline says it a second way.
                for (tab, label) in [
                    (Tab::Modes, "Modes"),
                    (Tab::Mirrors, "Mirrors"),
                    (Tab::Images, "Images"),
                    (Tab::Keybinds, "Keybinds"),
                    (Tab::Theme, "Theme"),
                    (Tab::Ninb, "Ninjabrain"),
                    (Tab::Input, "Input"),
                    (Tab::Layout, "Layout"),
                    (Tab::Screen, "Screen"),
                ] {
                    let here = self.tab == tab;
                    let response = ui.selectable_value(&mut self.tab, tab, label);

                    if here {
                        let rect = response.rect;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(rect.min.x, rect.max.y - 2.0),
                                egui::pos2(rect.max.x, rect.max.y + 1.0),
                            ),
                            0.0,
                            ui.visuals().selection.stroke.color,
                        );
                    }
                }

                // Right-aligned close. The editor floats over the game, so
                // dismissing it needs to be reachable without the keybind.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(2.0);
                    if ui
                        .add_sized([28.0, 24.0], egui::Button::new("×"))
                        .on_hover_text("Close (Esc)")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // A global mode that is easy to forget you left on, so
                    // it is said from every tab rather than only its own.
                    if self.doc.suspend_keybinds {
                        ui.separator();
                        if ui
                            .button(
                                egui::RichText::new("keybinds suspended")
                                    .color(egui::Color32::from_rgb(255, 170, 80)),
                            )
                            .on_hover_text("Click to turn them back on")
                            .clicked()
                        {
                            self.doc.suspend_keybinds = false;
                        }
                    }

                    // Everything most people need is in Basic; Advanced adds
                    // ids, layering and the things that break a setup.
                    //
                    // Greyed out on the two tabs that do not read it, rather
                    // than gone: a control that disappears looks like the bar
                    // moved, and a control that does nothing looks broken.
                    let reads_it = !matches!(self.tab, Tab::Screen | Tab::Layout);

                    ui.separator();
                    ui.add_space(6.0);
                    if reads_it {
                        ui.selectable_value(&mut self.advanced, true, "Advanced");
                        ui.selectable_value(&mut self.advanced, false, "Basic");
                    } else {
                        // Greyed out, it looked like a rendering fault. Say
                        // why instead, the way the disabled Calculate button
                        // in the wizard does.
                        ui.weak("everything on this tab is shown");
                    }

                    // Remembered, so it is not a click every time the editor
                    // opens. Written with the document like any other edit.
                    if self.doc.gui.appearance.advanced != self.advanced {
                        self.doc.gui.appearance.advanced = self.advanced;
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let blocked = !problems.is_empty();

                // The hint never changes and the state is only ever in one
                // place. These used to be two slots that could read
                // "Applying…" and "Applied" at the same time.
                ui.weak("Changes apply as you make them");

                if ui.button("Revert").clicked() {
                    self.revert();
                }

                ui.separator();

                // Every one of these carries a word as well as a colour, so
                // the bar still says what happened in greyscale.
                if let Some(err) = &self.load_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("⚠ load failed: {err}"));
                } else if blocked {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 120, 120),
                        if problems.len() == 1 {
                            "⚠ 1 problem, see the highlighted item".to_string()
                        } else {
                            format!("⚠ {} problems, see the highlighted items", problems.len())
                        },
                    );
                } else if self.pending_since.is_some() {
                    ui.weak("Applying…");
                } else if let Some((ok, message)) = &self.status {
                    let (color, mark) = if *ok {
                        (egui::Color32::LIGHT_GREEN, "•")
                    } else {
                        (egui::Color32::LIGHT_RED, "⚠")
                    };
                    ui.colored_label(color, format!("{mark} {message}"));
                } else {
                    // The full path is longer than the bar and used to run off
                    // the end mid-word. The file name is the part that says
                    // which config you are editing.
                    let path = self.store.path();
                    let short = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    ui.weak(short).on_hover_text(path.display().to_string());
                }
            });
            ui.add_space(2.0);
        });

        // Room under the last row, so the status bar never slices one in half.
        let frame = egui::Frame::central_panel(&ctx.style())
            .inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 8.0, bottom: 14.0 });

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
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
                Tab::Theme => {
                    tabs::theme::show(ui, &mut self.doc, self.advanced, &mut self.browser)
                }
                Tab::Ninb => tabs::ninb::show(
                    ui,
                    &mut self.doc,
                    &problems,
                    self.advanced,
                    &mut self.browser,
                    &mut self.ninb_keys,
                ),
                Tab::Input => tabs::input::show(
                    ui,
                    &mut self.doc,
                    &problems,
                    self.advanced,
                    &mut self.remap_capture,
                ),
                Tab::Layout => tabs::layout::show(ui, &mut self.doc, &mut self.layout_edit),
                Tab::Screen => tabs::screen::show(ui, &mut self.doc, &mut self.screen_edit),
            }
        });

        self.adopt_external_changes(ctx);
        self.apply_when_settled(ctx);
    }
}

