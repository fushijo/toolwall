//! `--screenshot <dir>`: render every tab to a PNG and exit.
//!
//! Behind the `screenshot` feature, so a release build does not carry it.
//! It exists so the interface can be looked at as pixels rather than read as
//! layout code, which is the only way to catch text that overflows its column
//! or a control that lands off the bottom of a panel.

use std::path::PathBuf;

use toolwall_core::{Document, Store};

use crate::setup::{Setup, Step, STEPS};
use crate::{App, Tab};

/// Every tab, plus the name its file gets.
const SHOTS: &[(Tab, &str)] = &[
    (Tab::Modes, "modes"),
    (Tab::Mirrors, "mirrors"),
    (Tab::Images, "images"),
    (Tab::Keybinds, "keybinds"),
    (Tab::Theme, "theme"),
    (Tab::Ninb, "ninjabrain"),
    (Tab::Input, "input"),
    (Tab::Layout, "layout"),
    (Tab::Screen, "screen"),
];

/// Frames to draw before asking for the picture.
///
/// egui is immediate mode and lays out in the frame after it first measures,
/// so the first frame of a tab has collapsing headers at the wrong height.
const SETTLE: u32 = 3;

/// What is on screen for one shot.
enum Frame {
    /// A tab of the editor, in Basic then in Advanced.
    Tab(Tab, &'static str, bool),
    /// A step of the setup window.
    Step(Step, &'static str),
}

fn plan() -> Vec<Frame> {
    let mut out = Vec::new();
    for advanced in [false, true] {
        for (tab, name) in SHOTS {
            out.push(Frame::Tab(*tab, name, advanced));
        }
    }
    for (step, name) in STEPS {
        out.push(Frame::Step(*step, name));
    }
    out
}

pub struct Shooter {
    app: App,
    setup: Setup,
    plan: Vec<Frame>,
    dir: PathBuf,
    at: usize,
    settled: u32,
    /// A request already in flight. Without this the request repeats every
    /// frame until the reply lands, and the extra replies are pictures of the
    /// tab before the one they get filed under.
    waiting: bool,
    /// The size to render at, so a screenshot is comparable between runs.
    size: [f32; 2],
}

impl Shooter {
    pub fn run(store: Store, doc: Document, dir: PathBuf, size: [f32; 2]) -> anyhow::Result<()> {
        std::fs::create_dir_all(&dir)?;

        let setup = Setup::new(Store::new(store.path().to_path_buf()));
        let app = App::new(store, doc, None);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(size)
                .with_title("toolwall screenshots")
                .with_app_id("toolwall"),
            ..Default::default()
        };

        eframe::run_native(
            "toolwall-screenshots",
            options,
            Box::new(move |_cc| {
                Ok(Box::new(Shooter {
                    app,
                    setup,
                    plan: plan(),
                    dir,
                    at: 0,
                    settled: 0,
                    waiting: false,
                    size,
                }))
            }),
        )
        .map_err(|err| anyhow::anyhow!("{err}"))?;

        Ok(())
    }
}

impl eframe::App for Shooter {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Some(frame_spec) = self.plan.get(self.at) else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };

        let name = match frame_spec {
            Frame::Tab(tab, name, advanced) => {
                self.app.tab = *tab;
                // Both, or the editor sees the toggle change every frame and
                // spends the run saving instead of settling.
                self.app.advanced = *advanced;
                self.app.doc.gui.appearance.advanced = *advanced;
                self.app.update(ctx, frame);
                format!("{name}-{}", if *advanced { "advanced" } else { "basic" })
            }
            Frame::Step(step, name) => {
                self.setup.step = *step;
                self.setup.draw(ctx);
                format!("setup-{}", name.to_lowercase().replace(' ', "-"))
            }
        };

        // Whatever the compositor gave us, render at the size asked for.
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            self.size[0],
            self.size[1],
        )));

        for event in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let path = self.dir.join(format!("{:02}-{name}.png", self.at + 1));
                let buffer = image::RgbaImage::from_raw(
                    image.width() as u32,
                    image.height() as u32,
                    image.as_raw().to_vec(),
                );

                match buffer.and_then(|b| b.save(&path).ok()) {
                    Some(()) => println!("wrote {}", path.display()),
                    None => eprintln!("could not write {}", path.display()),
                }

                self.at += 1;
                self.settled = 0;
                self.waiting = false;
            }
        }

        if self.settled >= SETTLE {
            if !self.waiting {
                self.waiting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
        } else {
            self.settled += 1;
        }

        ctx.request_repaint();
    }
}
