//! The setup window: `toolwall-gui --setup`.
//!
//! The editor assumes you already have a config worth editing. This is for
//! the ten minutes before that, and it exists because the first person to
//! install the beta ended up with a config they had not chosen, no way to
//! open the editor, and no idea which of their old settings had survived.
//!
//! It never writes anything until the last step. Every choice is applied to a
//! document held in memory, so backing out costs nothing.

mod sens;

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use toolwall_core::preset::{self, Choices, OverlayState};
use toolwall_core::{problems, Document, Store};

use crate::keys;
use crate::widgets::{color_field, path_field, settings_grid, FileBrowser, PickTarget};

/// Where the import leaves its notes, next to the config.
const REPORT_NAME: &str = "toolwall-import-report.txt";

const GORE_URL: &str = "https://github.com/arjuncgore/waywall_generic_config";

/// Screens people actually have, so most setups are one click.
const COMMON_SCREENS: &[(u32, u32, &str)] = &[
    (1920, 1080, "1920x1080"),
    (2560, 1440, "2560x1440"),
    (3840, 2160, "3840x2160"),
    (3440, 1440, "3440x1440"),
    (1366, 768, "1366x768"),
    (1600, 900, "1600x900"),
];

#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) enum Step {
    Start,
    Screens,
    Overlays,
    Keys,
    Look,
    Sens,
    Ninb,
    Finish,
}

pub(crate) const STEPS: &[(Step, &str)] = &[
    (Step::Start, "Where to start"),
    (Step::Screens, "Screens"),
    (Step::Overlays, "Overlays"),
    (Step::Keys, "Keys"),
    (Step::Look, "Look"),
    (Step::Sens, "Sensitivity"),
    (Step::Ninb, "Ninjabrain Bot"),
    (Step::Finish, "Finish"),
];

/// Which key is swallowing the next press.
#[derive(PartialEq, Eq, Clone)]
enum Capture {
    Screen(usize),
    Overlay(usize),
    Editor,
    Reset,
    Chat,
}

pub struct Setup {
    store: Store,
    /// Everything the choices are applied on top of.
    base: Document,
    choices: Choices,
    /// Where `base` came from, for saying so on the last step.
    origin: String,
    /// How big waywall's window is. Everything downstream is placed against
    /// this, so it is the first question on the first step.
    screen: (u32, u32),
    /// What the monitors say they are, as a line of text to show. A hint
    /// only: see where it is drawn for why it is never filled in.
    monitor_hint: Option<String>,
    /// Whether the config still needs reshaping for that screen.
    ///
    /// Only the preset does. An imported config was already written for the
    /// machine it was imported on, and scaling it would be inventing a
    /// problem to solve.
    fit_preset: bool,

    pub(crate) step: Step,
    capture: Option<Capture>,
    browser: FileBrowser,
    sens: sens::SensState,

    /// The import notes install.sh left behind, if there are any.
    report: Option<String>,
    /// What was on disk when this opened, for the first step to describe.
    /// Not `base`, which changes the moment you pick something else.
    had_config: Option<(usize, usize)>,

    busy: Option<String>,
    status: Option<(bool, String)>,
    written: bool,
}

impl Setup {
    pub fn new(store: Store) -> Self {
        // A config that will not parse is exactly when this is needed, so a
        // failed load starts from the preset. refusing to open would be an
        // unhelpful moment to pick.
        let (base, origin, had_config) = match store.load() {
            Ok(doc) if !doc.modes.is_empty() || !doc.keybinds.is_empty() => {
                let counts = (doc.modes.len(), doc.keybinds.len());
                (doc, "the config already on disk".to_string(), Some(counts))
            }
            Ok(_) => (preset::preset(), "the toolwall preset".to_string(), None),
            Err(_) => (preset::preset(), "the toolwall preset".to_string(), None),
        };

        let choices = preset::choices_for(&base);
        let mut sens = sens::SensState::default();
        sens.seed(
            base.gui.screen.h.max(1),
            base.modes.iter().map(|m| m.resolution.height).max(),
        );

        let screen = (base.gui.screen.w.max(1), base.gui.screen.h.max(1));

        Self {
            report: read_report(&store),
            store,
            base,
            choices,
            origin,
            screen,
            monitor_hint: toolwall_core::screen::hint(),
            fit_preset: had_config.is_none(),
            step: Step::Start,
            capture: None,
            browser: FileBrowser::default(),
            sens,
            had_config,
            busy: None,
            status: None,
            written: false,
        }
    }

    /// What the config would be if you finished right now.
    ///
    /// The screen fitting happens here and not when a source is picked, so it
    /// is re-derived from the untouched preset every frame. Choosing a
    /// different screen size therefore cannot scale an already scaled config
    /// a second time.
    fn preview(&self) -> Document {
        let mut doc = preset::build(&self.base, &self.choices);
        let (w, h) = self.screen;

        if self.fit_preset {
            preset::fit_to_screen(&mut doc, w, h);
        } else {
            doc.gui.screen = toolwall_core::schema::Size { w, h };
        }
        doc
    }

    fn adopt(&mut self, doc: Document, origin: &str) {
        self.fit_preset = origin.contains("preset");
        self.choices = preset::choices_for(&doc);
        self.sens.seed(
            doc.gui.screen.h.max(1) as u32,
            doc.modes.iter().map(|m| m.resolution.height).max(),
        );
        self.base = doc;
        self.origin = origin.to_string();
    }

    fn config_dir(&self) -> PathBuf {
        self.store.path().parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

/// The notes from the last import, if one ran.
fn read_report(store: &Store) -> Option<String> {
    let path = store.path().parent()?.join(REPORT_NAME);
    std::fs::read_to_string(path).ok()
}

/// Download gore's generic config and convert it.
///
/// Both halves already exist: uninstall.sh clones the same repository, and
/// install.sh puts the importer next to the runtime so it is still here once
/// the tarball is gone. This is those two, from a button.
fn import_gore(config_dir: &Path) -> Result<(Document, String), String> {
    let lua = ["luajit", "lua5.1", "lua"]
        .into_iter()
        .find(|exe| which(exe))
        .ok_or("no lua interpreter found, so a waywall config cannot be read")?;

    if !which("git") {
        return Err("git is not installed, so the config cannot be downloaded".into());
    }

    let importer = config_dir.join("toolwall/import.lua");
    if !importer.is_file() {
        return Err(format!(
            "the importer is not installed at {}; re-run install.sh",
            importer.display()
        ));
    }

    let work = std::env::temp_dir().join(format!("toolwall-gore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);

    let clone = Process::new("git")
        .args(["clone", "--depth", "1", "--quiet", GORE_URL])
        .arg(&work)
        .output()
        .map_err(|err| format!("could not run git: {err}"))?;

    if !clone.status.success() {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!(
            "downloading the config failed: {}",
            String::from_utf8_lossy(&clone.stderr).trim()
        ));
    }

    // Its overlays are referenced by absolute path, so they have to be where
    // the config says they are before the config means anything.
    let resources = config_dir.join("resources");
    let _ = std::fs::create_dir_all(&resources);
    copy_into(&work.join("resources"), &resources);

    let out = work.join("toolwall.json");
    let run = Process::new(lua)
        .arg(&importer)
        .arg(&work)
        .arg(&out)
        .output()
        .map_err(|err| format!("could not run {lua}: {err}"))?;

    let log = String::from_utf8_lossy(&run.stdout).to_string();

    if !run.status.success() {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!(
            "reading the config failed: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }

    let body = std::fs::read_to_string(&out)
        .map_err(|err| format!("the import wrote nothing: {err}"))?;
    let doc: Document =
        serde_json::from_str(&body).map_err(|err| format!("the import is not readable: {err}"))?;

    let _ = std::fs::remove_dir_all(&work);
    Ok((doc, log))
}

/// Keep the end of a string, which for a path is the part worth reading.
///
/// Nobody has ever needed to be told they are still inside /home.
fn elide(text: &str, most: usize) -> String {
    let count = text.chars().count();
    if count <= most {
        return text.to_string();
    }

    let tail: String = text.chars().skip(count - most.saturating_sub(1)).collect();
    format!("…{tail}")
}

fn which(exe: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| dir.join(exe).is_file())
}

/// Copy files that are not already there, one level deep. Enough for a
/// resources directory and not enough to be a surprise.
///
/// One level. We are not here to recursively adopt somebody's home folder.
fn copy_into(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else { return };

    for entry in entries.flatten() {
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let Some(name) = source.file_name() else { continue };
        let target = to.join(name);
        if target.exists() {
            continue;
        }
        let _ = std::fs::copy(&source, &target);
    }
}

impl eframe::App for Setup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl Setup {
    /// The whole window, panels and all, from nothing but a context.
    ///
    /// Split out from `update` so a test can run it: eframe::Frame cannot be
    /// built without a window, and the panel layout is the half most worth
    /// checking. Text that does not fit and a step that draws nothing both
    /// look fine in a per-widget test.
    pub(crate) fn draw(&mut self, ctx: &egui::Context) {
        // A capture in progress swallows the next keypress.
        if let Some(target) = self.capture.clone() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capture = None;
            } else if let Some(input) = keys::captured(ctx) {
                match target {
                    Capture::Screen(index) => {
                        if let Some(screen) = self.choices.screens.get_mut(index) {
                            screen.input = input;
                        }
                    }
                    Capture::Overlay(index) => {
                        if let Some(overlay) = self.choices.overlays.get_mut(index) {
                            overlay.state = OverlayState::Bound(input);
                        }
                    }
                    Capture::Editor => self.choices.editor_key = input,
                    Capture::Reset => self.choices.reset_key = input,
                    Capture::Chat => self.choices.chat_key = input,
                }
                self.capture = None;
            }
        }

        if let Some((target, path)) = self.browser.show(ctx) {
            match target {
                PickTarget::Font => self.choices.font_path = path,
                PickTarget::NinbJar => self.choices.ninb_jar = path,
                _ => {}
            }
        }

        egui::SidePanel::left("steps").exact_width(190.0).show(ctx, |ui| {
            ui.add_space(10.0);
            ui.heading("Set up toolwall");
            ui.add_space(10.0);

            for (index, (step, name)) in STEPS.iter().enumerate() {
                let label = format!("{}.  {name}", index + 1);
                // Whatever the last step had to say about itself is not news
                // on the next one, and a red line that never goes away reads
                // as something still being wrong.
                if ui.selectable_value(&mut self.step, *step, label).clicked() {
                    self.status = None;
                }
            }
        });

        egui::TopBottomPanel::bottom("nav").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let at = STEPS.iter().position(|(s, _)| *s == self.step).unwrap_or(0);

                if at > 0 && ui.button("Back").clicked() {
                    self.step = STEPS[at - 1].0;
                    self.status = None;
                }
                if at + 1 < STEPS.len() && ui.button("Next").clicked() {
                    self.step = STEPS[at + 1].0;
                    self.status = None;
                }

                // Whatever is worth saying, at the right hand end.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, color) = match (&self.status, &self.busy) {
                        (Some((ok, message)), _) => (
                            message.clone(),
                            Some(if *ok {
                                egui::Color32::LIGHT_GREEN
                            } else {
                                egui::Color32::from_rgb(255, 120, 120)
                            }),
                        ),
                        (None, Some(busy)) => (busy.clone(), None),
                        (None, None) => (self.store.path().display().to_string(), None),
                    };

                    // Shortened before it is laid out, not only by the
                    // truncation: a long save error would otherwise push the
                    // Back and Next buttons off their own panel.
                    let mut rich = egui::RichText::new(elide(&text, 52));
                    match color {
                        Some(color) => rich = rich.color(color),
                        None => rich = rich.weak(),
                    }

                    ui.add(egui::Label::new(rich).truncate()).on_hover_text(text);
                });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.step {
                Step::Start => self.start(ui),
                Step::Screens => self.screens(ui),
                Step::Overlays => self.overlays(ui),
                Step::Keys => self.keys(ui),
                Step::Look => self.look(ui),
                Step::Sens => {
                    if let Some(result) = sens::show(ui, &mut self.sens) {
                        self.choices.sensitivity = Some(result);
                        self.choices.normal_height =
                            self.sens.normal_height.trim().parse().unwrap_or(1080);
                        self.status = Some((true, "Sensitivity noted".into()));
                    }
                }
                Step::Ninb => self.ninb(ui),
                Step::Finish => self.finish(ui, ctx),
            });
        });
    }
}

impl Setup {
    fn start(&mut self, ui: &mut egui::Ui) {
        ui.heading("Where to start");
        ui.add_space(6.0);

        if let Some((screens, keys)) = self.had_config {
            ui.label(format!(
                "There is already a config here, with {screens} screen(s) and \
                 {keys} keybind(s). The steps after this one let you change it, \
                 or start over from one of these."
            ));
        } else {
            ui.label(
                "Nothing is set up yet. Pick where the config comes from, then work \
                 through the steps down the side.",
            );
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Asked first, because every overlay is placed against it.
        ui.strong("How big is waywall's window?");
        ui.label(
            "Usually your monitor. It is the Display line in F3, and getting it \
             wrong is what puts the pie chart off the side of the screen.",
        );
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            for (w, h, name) in COMMON_SCREENS {
                let picked = self.screen == (*w, *h);
                if ui.selectable_label(picked, *name).clicked() {
                    self.screen = (*w, *h);
                    self.sens.seed(*h, None);
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.screen.0).range(320..=16384));
            ui.label("x");
            ui.add(egui::DragValue::new(&mut self.screen.1).range(240..=16384));
            if self.fit_preset {
                ui.weak("the preset is resized to fit");
            }
        });

        // Offered, never filled in. The monitor's mode is in physical pixels
        // and waywall's window is in the compositor's logical ones, so on a
        // scaled desktop they differ and using this would put every overlay
        // off the side of the screen.
        if let Some(hint) = &self.monitor_hint {
            ui.add_space(4.0);
            ui.weak(hint);
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(10.0);

        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Use the toolwall preset")
                .on_hover_text(
                    "fushijo's config: thin, wide and an eye measuring screen, with \
                     a pie chart and entity counter. You pick the keys next.",
                )
                .clicked()
            {
                let doc = preset::preset();
                self.adopt(doc, "the toolwall preset");
                self.status = Some((true, "Started from the preset".into()));
            }

            let gore = ui.button("Download gore's generic config").on_hover_text(GORE_URL);
            if gore.clicked() {
                self.busy = Some("Downloading...".into());
                match import_gore(&self.config_dir()) {
                    Ok((doc, log)) => {
                        self.report = Some(log);
                        self.adopt(doc, "gore's generic config");
                        self.status = Some((true, "Converted gore's config".into()));
                    }
                    Err(message) => self.status = Some((false, message)),
                }
                self.busy = None;
            }
        });

        ui.add_space(4.0);
        ui.weak(format!("Currently starting from {}.", self.origin));

        ui.add_space(6.0);
        ui.label(
            "The download fetches that repository and converts it. Its overlay \
             images are copied in alongside, because its config points at them by \
             absolute path.",
        );

        if let Some(report) = &self.report {
            ui.add_space(14.0);
            ui.separator();
            ui.heading("What did not come across");
            ui.label(
                "A waywall config can do things toolwall has no setting for. \
                 Anything in this list was left out, so it is worth reading before \
                 you decide you are finished.",
            );
            ui.add_space(6.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                for line in report.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Some(note) = line.trim().strip_prefix("- ") {
                        ui.label(format!("• {note}"));
                    } else {
                        ui.weak(line);
                    }
                }
            });
        }
    }

    fn screens(&mut self, ui: &mut egui::Ui) {
        ui.heading("Screens");
        ui.label(
            "The resolutions you switch between. Each one needs a key, and the key \
             puts you in it until you press it again or reset.",
        );
        ui.add_space(10.0);

        let mut capture = None;

        for (index, screen) in self.choices.screens.iter_mut().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut screen.enabled, "");
                    ui.strong(&screen.label);
                });

                ui.add_enabled_ui(screen.enabled, |ui| {
                    settings_grid(ui, format!("screen-{index}"), |ui| {
                        ui.label("Size");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut screen.width).range(1..=16384));
                            ui.label("x");
                            ui.add(egui::DragValue::new(&mut screen.height).range(1..=16384));
                        });
                        ui.end_row();

                        ui.label("Key");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut screen.input);
                            if ui.button("Capture").clicked() {
                                capture = Some(Capture::Screen(index));
                            }
                        });
                        ui.end_row();
                    });
                });
            });
            ui.add_space(4.0);
        }

        if capture.is_some() {
            self.capture = capture;
        }
        if matches!(self.capture, Some(Capture::Screen(_))) {
            ui.colored_label(egui::Color32::LIGHT_BLUE, "press a key, or Esc to cancel");
        }
    }

    fn overlays(&mut self, ui: &mut egui::Ui) {
        ui.heading("Overlays");
        ui.label(
            "The things drawn on top of the game: the pie chart, the entity counter, \
             the measuring grid. Each one is either on a key, on the whole time its \
             screen is, or not here at all.",
        );
        ui.add_space(10.0);

        let mut capture = None;

        for (index, overlay) in self.choices.overlays.iter_mut().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.strong(&overlay.label);
                if overlay.modes.is_empty() {
                    ui.weak("not on any screen");
                } else {
                    ui.weak(format!("on {}", overlay.modes.join(", ")));
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let bound = matches!(overlay.state, OverlayState::Bound(_));

                    if ui.selectable_label(bound, "On a key").clicked() && !bound {
                        overlay.state = OverlayState::Bound(String::new());
                    }
                    if ui
                        .selectable_label(overlay.state == OverlayState::AlwaysOn, "Always on")
                        .clicked()
                    {
                        overlay.state = OverlayState::AlwaysOn;
                    }
                    if ui.selectable_label(overlay.state == OverlayState::Off, "Off").clicked() {
                        overlay.state = OverlayState::Off;
                    }
                });

                if let OverlayState::Bound(input) = &mut overlay.state {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(input);
                        if ui.button("Capture").clicked() {
                            capture = Some(Capture::Overlay(index));
                        }
                    });
                }
            });
            ui.add_space(4.0);
        }

        if capture.is_some() {
            self.capture = capture;
        }
        if matches!(self.capture, Some(Capture::Overlay(_))) {
            ui.colored_label(egui::Color32::LIGHT_BLUE, "press a key, or Esc to cancel");
        }
    }

    fn keys(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keys");
        ui.add_space(6.0);

        let mut capture = None;

        settings_grid(ui, "setup-keys", |ui| {
            ui.label("Open the editor").on_hover_text(
                "The one key worth remembering. Everything else can be changed from \
                 inside the editor, including this.",
            );
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.choices.editor_key);
                if ui.button("Capture").clicked() {
                    capture = Some(Capture::Editor);
                }
            });
            ui.end_row();

            ui.label("Back to normal").on_hover_text("Leave whichever screen you are in.");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.choices.reset_key);
                if ui.button("Capture").clicked() {
                    capture = Some(Capture::Reset);
                }
            });
            ui.end_row();

            ui.label("Type in chat").on_hover_text(
                "Turns your rebinds off and puts your normal keyboard layout \
                 back, so chat gets what you typed. Press it again to play. \
                 Menus and chat already do this on their own if the instance \
                 has the State Output mod, so this is the one to reach for \
                 when it does not.",
            );
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.choices.chat_key);
                if ui.button("Capture").clicked() {
                    capture = Some(Capture::Chat);
                }
            });
            ui.end_row();
        });

        if capture.is_some() {
            self.capture = capture;
        }
        if matches!(
            self.capture,
            Some(Capture::Editor) | Some(Capture::Reset) | Some(Capture::Chat)
        ) {
            ui.colored_label(egui::Color32::LIGHT_BLUE, "press a key, or Esc to cancel");
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Everything on a key");

        let doc = self.preview();
        for bind in &doc.keybinds {
            ui.horizontal(|ui| {
                ui.monospace(&bind.input);
                ui.label(bind.label.clone().unwrap_or_else(|| format!("{:?}", bind.command)));
            });
        }

        // Only the assignments this window controls can clash; build() drops
        // the loser, so the warning has to come from the choices themselves.
        let mut wanted: Vec<String> = vec![self.choices.editor_key.clone()];
        if !self.choices.reset_key.trim().is_empty() {
            wanted.push(self.choices.reset_key.clone());
        }
        if !self.choices.chat_key.trim().is_empty() {
            wanted.push(self.choices.chat_key.clone());
        }
        wanted.extend(
            self.choices
                .screens
                .iter()
                .filter(|s| s.enabled && !s.input.trim().is_empty())
                .map(|s| s.input.clone()),
        );
        wanted.extend(self.choices.overlays.iter().filter_map(|o| match &o.state {
            OverlayState::Bound(input) if !input.trim().is_empty() => Some(input.clone()),
            _ => None,
        }));

        let mut clashes: Vec<String> = Vec::new();
        for (index, key) in wanted.iter().enumerate() {
            if wanted[..index].iter().any(|other| other.eq_ignore_ascii_case(key)) {
                clashes.push(key.clone());
            }
        }
        clashes.dedup();

        if !clashes.is_empty() {
            ui.add_space(8.0);
            ui.colored_label(
                egui::Color32::from_rgb(255, 170, 80),
                format!(
                    "Two things are on {}. Only one of them will work, so change one.",
                    clashes.join(", ")
                ),
            );
        }
    }

    fn look(&mut self, ui: &mut egui::Ui) {
        ui.heading("Look");
        ui.add_space(6.0);

        settings_grid(ui, "setup-look", |ui| {
            ui.label("Background").on_hover_text(
                "What fills the window around the game when you are in a screen \
                 narrower than your monitor.",
            );
            color_field(ui, &mut self.choices.background);
            ui.end_row();

            ui.label("Editor font").on_hover_text(
                "A .ttf or .otf to use in the editor. Leave it empty for the built-in one.",
            );
            ui.vertical(|ui| {
                path_field(
                    ui,
                    &mut self.choices.font_path,
                    &mut self.browser,
                    PickTarget::Font,
                    "ttf",
                );
                if !self.choices.font_path.is_empty() && ui.small_button("Clear").clicked() {
                    self.choices.font_path.clear();
                }
            });
            ui.end_row();
        });
    }

    fn ninb(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ninjabrain Bot");
        ui.label(
            "toolwall can start Ninjabrain Bot with the game and hold it in a corner, \
             so it is not a window you alt-tab to.",
        );
        ui.add_space(10.0);

        settings_grid(ui, "setup-ninb", |ui| {
            ui.label("Jar");
            path_field(
                ui,
                &mut self.choices.ninb_jar,
                &mut self.browser,
                PickTarget::NinbJar,
                "jar",
            );
            ui.end_row();
        });

        ui.add_space(6.0);
        ui.weak("Leave it empty if you do not use it. Nothing else here depends on it.");

        ui.add_space(16.0);
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(60, 40, 10))
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 200, 100),
                    "Read this before you go looking for the readout",
                );
                ui.add_space(6.0);
                ui.label(
                    "toolwall can also draw Ninjabrain Bot's numbers into the game \
                     itself, instead of you reading them off the window. That part \
                     needs a patched waywall.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Stock waywall has no way to fill a rectangle, so there is nothing \
                     to draw the panel behind the text or the outline around it. The \
                     patch adds one. It is in patches/ in the repository, it is not in \
                     the release tarball, and applying it means building waywall \
                     yourself.",
                );
                ui.add_space(6.0);
                ui.strong(
                    "Do not do this on your first setup. Everything else works without \
                     it, and a half-built waywall is a much worse problem than not \
                     having the readout.",
                );
            });
    }

    fn finish(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Finish");
        ui.add_space(6.0);

        let doc = self.preview();
        let found = problems(&doc);

        ui.label(format!("Built from {}.", self.origin));
        ui.add_space(8.0);

        settings_grid(ui, "setup-summary", |ui| {
            ui.label("Window");
            ui.label(format!("{}x{}", self.screen.0, self.screen.1));
            ui.end_row();

            ui.label("Screens");
            ui.label(
                doc.modes
                    .iter()
                    .map(|m| {
                        format!(
                            "{} ({}x{})",
                            m.label.clone().unwrap_or_else(|| m.id.clone()),
                            m.resolution.width,
                            m.resolution.height
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            ui.end_row();

            ui.label("Overlays");
            ui.label(format!("{} mirror(s), {} image(s)", doc.mirrors.len(), doc.images.len()));
            ui.end_row();

            ui.label("Keys");
            ui.label(format!("{}", doc.keybinds.len()));
            ui.end_row();

            ui.label("Opens the editor");
            ui.monospace(&self.choices.editor_key);
            ui.end_row();

            ui.label("Sensitivity");
            match self.choices.sensitivity {
                Some(sens) => ui.label(format!(
                    "{:.6} normal, Minecraft set to {}",
                    sens.normal, sens.mc
                )),
                None => ui.weak("left as it was"),
            };
            ui.end_row();
        });

        ui.add_space(12.0);

        if !found.is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 120, 120),
                "This config would not load. Go back and fix these:",
            );
            for problem in &found {
                ui.label(format!("• {}", problem.message));
            }
            return;
        }

        if self.choices.editor_key.trim().is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 120, 120),
                "Nothing opens the editor. Set a key on the Keys step first.",
            );
            return;
        }

        if self.written {
            ui.colored_label(egui::Color32::LIGHT_GREEN, "Written.");
            ui.add_space(6.0);
            ui.label(format!(
                "Launch your instance and press {} to open the editor.",
                self.choices.editor_key
            ));
            if let Some(sens) = self.choices.sensitivity {
                ui.label(format!(
                    "Set Minecraft's own sensitivity to {} as well, or the numbers \
                     here will not match what you feel.",
                    sens.mc
                ));
            }
            ui.add_space(10.0);
            if ui.button("Close").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        if self.had_config.is_some() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 170, 80),
                format!("This replaces {}.", self.store.path().display()),
            );
            ui.add_space(6.0);
        }

        if ui.button("Write the config").clicked() {
            self.status = Some(match self.store.save(&doc) {
                Ok(()) => {
                    self.written = true;
                    (true, "Saved".to_string())
                }
                Err(err) => (false, format!("{err:#}")),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("toolwall-setup-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fresh(name: &str) -> (Setup, PathBuf) {
        let dir = scratch(name);
        let path = dir.join("toolwall.json");
        (Setup::new(Store::new(&path)), dir)
    }

    /// One layout pass over every step, as egui would do with a window.
    ///
    /// Panics, id collisions and out of range indexing all show up here, and
    /// none of them need pixels. A step that is never laid out is a step
    /// nothing is checking.
    fn draw_every_step(setup: &mut Setup) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for (step, _) in STEPS {
                    setup.step = *step;
                    match step {
                        Step::Start => setup.start(ui),
                        Step::Screens => setup.screens(ui),
                        Step::Overlays => setup.overlays(ui),
                        Step::Keys => setup.keys(ui),
                        Step::Look => setup.look(ui),
                        Step::Sens => {
                            sens::show(ui, &mut setup.sens);
                        }
                        Step::Ninb => setup.ninb(ui),
                        Step::Finish => {
                            let ctx = ui.ctx().clone();
                            setup.finish(ui, &ctx);
                        }
                    }
                }
            });
        });
    }

    #[test]
    fn every_step_lays_out_from_a_clean_machine() {
        let (mut setup, dir) = fresh("clean");

        // Nothing on disk, so it should have fallen back to the preset rather
        // than opening empty. That is the case darvz was in.
        assert!(setup.had_config.is_none());
        assert!(!setup.base.modes.is_empty(), "started with no screens");

        draw_every_step(&mut setup);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn it_writes_a_config_that_loads() {
        let (setup, dir) = fresh("write");
        let doc = setup.preview();

        assert!(problems(&doc).is_empty(), "{:?}", problems(&doc));
        setup.store.save(&doc).unwrap();

        // The real check: the store validates on load, so a round trip here
        // is the same thing the runtime does.
        let back = setup.store.load().unwrap();
        assert_eq!(back.modes.len(), doc.modes.len());
        assert!(
            back.keybinds.iter().any(|b| b.command == toolwall_core::schema::Command::GuiToggle),
            "no way to open the editor"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_config_that_will_not_parse_still_opens_the_window() {
        // This is exactly when someone needs it, so refusing to start would
        // be the worst possible moment to do it.
        let dir = scratch("broken");
        let path = dir.join("toolwall.json");
        std::fs::write(&path, "{ not json at all").unwrap();

        let mut setup = Setup::new(Store::new(&path));
        assert!(setup.had_config.is_none(), "a broken file is not a config to keep");
        assert!(!setup.base.modes.is_empty(), "fell back to nothing");

        draw_every_step(&mut setup);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_import_report_is_shown_when_there_is_one() {
        let dir = scratch("report");
        let path = dir.join("toolwall.json");
        std::fs::write(
            dir.join(REPORT_NAME),
            "toolwall import of /home/someone/.config/waywall\n\nworth knowing:\n  \
             - keybind \"Insert\" was left out: toggles a second rebind set\n",
        )
        .unwrap();

        let mut setup = Setup::new(Store::new(&path));
        let report = setup.report.clone().expect("the report was not picked up");
        assert!(report.contains("Insert"), "the notes are not in it");

        draw_every_step(&mut setup);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn turning_every_screen_off_still_leaves_a_config_that_loads() {
        // Someone will do this, and the result has to be a config waywall
        // accepts, not one with keybinds pointing at nothing.
        let (mut setup, dir) = fresh("noscreens");

        for screen in &mut setup.choices.screens {
            screen.enabled = false;
        }

        let doc = setup.preview();
        assert!(problems(&doc).is_empty(), "{:?}", problems(&doc));
        assert!(doc.modes.is_empty(), "the screens are gone");
        assert!(doc.mirrors.is_empty(), "and so are the overlays nothing shows");
        assert!(
            doc.keybinds.iter().any(|b| b.command == toolwall_core::schema::Command::GuiToggle),
            "but the editor key survives, or there is no way back in"
        );

        draw_every_step(&mut setup);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_editor_key_cannot_be_given_away() {
        let (mut setup, dir) = fresh("clash");

        setup.choices.editor_key = "Ctrl-I".into();
        for screen in &mut setup.choices.screens {
            screen.input = "Ctrl-I".into();
        }

        let doc = setup.preview();
        let editor: Vec<_> = doc
            .keybinds
            .iter()
            .filter(|b| b.command == toolwall_core::schema::Command::GuiToggle)
            .collect();

        assert_eq!(editor.len(), 1, "exactly one editor bind");
        assert_eq!(editor[0].input, "Ctrl-I");
        assert_eq!(
            doc.keybinds.iter().filter(|b| b.input == "Ctrl-I").count(),
            1,
            "and nothing else is on that key"
        );

        // And the window says so instead of quietly eating one of them.
        draw_every_step(&mut setup);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_sensitivity_step_fills_in_from_an_options_file() {
        let dir = scratch("sens");
        let options = dir.join("options.txt");
        std::fs::write(&options, "mouseSensitivity:0.5\n").unwrap();

        assert_eq!(toolwall_core::minecraft::read_sensitivity(&options), Some(0.5));

        // And the numbers that come out are the calculator's, which
        // toolwall-core checks against its JavaScript.
        let result = toolwall_core::sens::boat_eye(0.5, 1080, 16384, 30.0);
        assert!((result.normal - 12.800000599064097).abs() < 1e-9);

        std::fs::remove_dir_all(dir).unwrap();
    }
    /// Nothing the window draws may run off the edge of it.
    ///
    /// Found the status bar doing exactly that: the config path was laid out
    /// from x=892 in a 900 wide window, so most of it was simply not on
    /// screen. A layout pass on its own does not care, which is why this
    /// looks at where the text actually ended up.
    #[test]
    fn no_text_is_laid_out_past_the_edge_of_the_window() {
        let (mut setup, dir) = fresh("bounds");
        let mut bad: Vec<String> = Vec::new();

        for (step, name) in STEPS {
            setup.step = *step;

            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            };

            let out = ctx.run(input, |ctx| setup.draw(ctx));

            for clipped in &out.shapes {
                check(&clipped.shape, clipped.clip_rect, name, &mut bad);
            }
        }

        assert!(bad.is_empty(), "text off the edge:\n  {}", bad.join("\n  "));
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn check(shape: &egui::epaint::Shape, clip: egui::Rect, step: &str, bad: &mut Vec<String>) {
        use egui::epaint::Shape;

        match shape {
            Shape::Text(text) => {
                // `pos` is the anchor, not the left edge. A right aligned
                // galley has a rect of [-width, 0] hung off it, so adding the
                // width to the anchor measures from the wrong end and calls
                // every right aligned label an overflow.
                let right = text.pos.x + text.galley.rect.max.x;
                // A pixel of slack: rounding puts a flush-right label a
                // hair over now and then, and that is not the bug.
                if right > clip.right() + 1.0 {
                    bad.push(format!(
                        "{step}: {:?} ends at {right:.0}, past {:.0}",
                        text.galley.text(),
                        clip.right()
                    ));
                }
            }
            Shape::Vec(shapes) => {
                for inner in shapes {
                    check(inner, clip, step, bad);
                }
            }
            _ => {}
        }
    }

    /// Prints what one step draws. `TOOLWALL_DUMP=<step number>`.
    ///
    /// Not a check, a pair of eyes: this window is OpenGL, and a screenshot
    /// of an OpenGL window under XWayland comes back as a rectangle of black
    /// no matter which tool takes it.
    #[test]
    fn dump_a_step() {
        let Ok(want) = std::env::var("TOOLWALL_DUMP") else { return };
        let want: usize = want.parse().unwrap_or(1);

        let (mut setup, dir) = fresh("dump");
        setup.step = STEPS[want.saturating_sub(1).min(STEPS.len() - 1)].0;

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| setup.draw(ctx));

        for clipped in &out.shapes {
            print_text(&clipped.shape);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn print_text(shape: &egui::epaint::Shape) {
        use egui::epaint::Shape;
        match shape {
            Shape::Text(t) => println!(
                "[{:4.0},{:4.0}] {}",
                t.pos.x + t.galley.rect.min.x,
                t.pos.y,
                t.galley.text()
            ),
            Shape::Vec(v) => v.iter().for_each(print_text),
            _ => {}
        }
    }
}
