//! "Is there a newer toolwall, and will you install it."
//!
//! Every step runs on its own thread and reports back through a channel, so a
//! check that cannot reach GitHub and a build that takes two minutes both
//! leave the editor drawing. It floats over a running game; freezing it for a
//! cargo build would be freezing the thing you were playing on.

use std::sync::mpsc::{Receiver, Sender};

use toolwall_core::update;

enum Message {
    /// The newest released version, and whether it beats ours.
    Found { latest: String, newer: bool },
    /// A step of the install, for showing while it runs.
    Step(String),
    Done(String),
    Failed(String),
}

#[derive(Default)]
pub struct Updates {
    inbox: Option<Receiver<Message>>,
    /// What the last check found, once it has come back.
    pub latest: Option<String>,
    pub newer: bool,
    /// What is happening now, or how it ended.
    pub status: Option<String>,
    pub failed: bool,
    pub busy: bool,
    pub finished: bool,
}

impl Updates {
    /// Take whatever the worker has said since the last frame.
    pub fn poll(&mut self) {
        let Some(inbox) = &self.inbox else { return };

        while let Ok(message) = inbox.try_recv() {
            match message {
                Message::Found { latest, newer } => {
                    self.newer = newer;
                    self.status = Some(if newer {
                        format!("{latest} is out")
                    } else {
                        format!("{latest} is the newest, and you are on it")
                    });
                    self.latest = Some(latest);
                    self.busy = false;
                }
                Message::Step(step) => self.status = Some(step),
                Message::Done(version) => {
                    self.status = Some(format!(
                        "Updated to {version}. Restart waywall to pick up the new runtime."
                    ));
                    self.busy = false;
                    self.finished = true;
                    self.newer = false;
                }
                Message::Failed(why) => {
                    self.status = Some(why);
                    self.failed = true;
                    self.busy = false;
                }
            }
        }
    }

    pub fn check(&mut self) {
        self.start(|send| match update::latest() {
            Ok(latest) => {
                let newer = update::is_newer(&latest, update::current());
                let _ = send.send(Message::Found { latest, newer });
            }
            Err(err) => {
                let _ = send.send(Message::Failed(format!("Could not check: {err:#}")));
            }
        });
    }

    pub fn install(&mut self) {
        self.start(|send| {
            let mut say = |step: &str| {
                let _ = send.send(Message::Step(step.to_string()));
            };

            match update::run(&mut say) {
                Ok(version) => {
                    let _ = send.send(Message::Done(version));
                }
                Err(err) => {
                    let _ = send.send(Message::Failed(format!("{err:#}")));
                }
            }
        });
    }

    fn start(&mut self, work: impl FnOnce(Sender<Message>) + Send + 'static) {
        if self.busy {
            return;
        }

        let (send, receive) = std::sync::mpsc::channel();
        self.inbox = Some(receive);
        self.busy = true;
        self.failed = false;
        self.status = Some("Working...".into());

        std::thread::spawn(move || work(send));
    }
}

/// The section on the Theme tab.
pub fn show(ui: &mut egui::Ui, updates: &mut Updates) {
    updates.poll();

    ui.horizontal(|ui| {
        ui.label("Version");
        ui.strong(update::current());

        ui.add_enabled_ui(!updates.busy, |ui| {
            if ui.button("Check for updates").clicked() {
                updates.check();
            }
        });

        if updates.newer && !updates.finished {
            let install = egui::Button::new(egui::RichText::new("Update now").strong())
                .fill(ui.visuals().selection.bg_fill);

            if ui.add_enabled(!updates.busy, install).clicked() {
                updates.install();
            }
        }
    });

    if let Some(status) = &updates.status {
        ui.add_space(2.0);
        if updates.failed {
            ui.colored_label(egui::Color32::from_rgb(255, 140, 140), status);
        } else {
            ui.weak(status);
        }
    }

    if updates.busy {
        // Something has to move, or a two-minute build looks like a hang.
        ui.add_space(2.0);
        ui.spinner();
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }

    ui.add_space(2.0);
    ui.weak("Updating needs git and cargo. Your config is not touched.");
}
