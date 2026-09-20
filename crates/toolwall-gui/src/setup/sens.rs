//! The boat eye calculator, in the setup window.
//!
//! Same maths as <https://arjuncgore.github.io/waywall-boat-eye-calc/>, which
//! is where this came from. Built in and not just linked, because the two
//! numbers it spits out have to land in the config, and typing them across by
//! hand is a step people get wrong and then never notice.

use toolwall_core::minecraft::{self, Instance};
use toolwall_core::sens::{self, Pointer, Sens};

use crate::widgets::settings_grid;

/// What was scaling the mouse before. One radio, three fields would be worse.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PointerKind {
    Flat,
    Windows,
    Linux,
}

pub struct SensState {
    instances: Vec<Instance>,
    scanned: bool,
    /// Which instance the sensitivity was read out of, for saying so.
    read_from: Option<String>,

    pub mc_sens: String,
    pub kind: PointerKind,
    pub windows_step: u8,
    pub linux_speed: f64,

    pub normal_height: String,
    pub tall_height: String,

    pub result: Option<Sens>,
    pub error: Option<String>,
}

impl Default for SensState {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            scanned: false,
            read_from: None,
            mc_sens: String::new(),
            kind: PointerKind::Flat,
            windows_step: 10,
            linux_speed: 0.0,
            normal_height: sens::DEFAULT_NORMAL_HEIGHT.to_string(),
            tall_height: sens::DEFAULT_TALL_HEIGHT.to_string(),
            result: None,
            error: None,
        }
    }
}

impl SensState {
    /// Fill the heights in from the config, so the numbers match this setup
    /// and not the calculator's defaults.
    pub fn seed(&mut self, screen_height: u32, tall_height: Option<u32>) {
        if screen_height > 0 {
            self.normal_height = screen_height.to_string();
        }
        if let Some(tall) = tall_height {
            self.tall_height = tall.to_string();
        }
    }

    fn pointer(&self) -> Pointer {
        match self.kind {
            PointerKind::Flat => Pointer::Flat,
            PointerKind::Windows => Pointer::Windows(self.windows_step),
            PointerKind::Linux => Pointer::Linux(self.linux_speed),
        }
    }

    fn calculate(&mut self) {
        self.result = None;

        let Ok(mc) = self.mc_sens.trim().parse::<f64>() else {
            self.error = Some("Minecraft sensitivity has to be a number between 0 and 1".into());
            return;
        };
        let normal = self.normal_height.trim().parse::<u32>().unwrap_or(0);
        let tall = self.tall_height.trim().parse::<u32>().unwrap_or(0);

        if normal == 0 {
            self.error = Some("the screen height has to be a whole number of pixels".into());
            return;
        }

        match sens::calculate(mc, self.pointer(), normal, tall, sens::DEFAULT_VFOV) {
            Ok(result) => {
                self.error = None;
                self.result = Some(result);
            }
            Err(message) => self.error = Some(message),
        }
    }
}

/// Draws the step. Returns the sensitivities if the user accepted them.
pub fn show(ui: &mut egui::Ui, state: &mut SensState) -> Option<Sens> {
    let mut accepted = None;

    ui.heading("Sensitivity");
    ui.label(
        "Boat eye works by everyone measuring at the same Minecraft sensitivity. \
         This works out the waywall multipliers that make your mouse turn the \
         same amount it does now once you are on it.",
    );
    ui.add_space(6.0);
    // Credit where it is due, and a second opinion where it is wanted.
    ui.hyperlink_to(
        "Same maths as gore's calculator",
        "https://arjuncgore.github.io/waywall-boat-eye-calc/",
    );
    ui.separator();

    // ---- where the current sensitivity comes from ----
    if !state.scanned {
        state.instances = minecraft::instances();
        state.scanned = true;

        // One instance and nothing typed yet is the common case. fill it in,
        // do not make them click a button with one option on it.
        if state.mc_sens.is_empty() {
            if let Some(only) = state.instances.first() {
                if state.instances.len() == 1 {
                    if let Some(value) = minecraft::read_sensitivity(&only.options) {
                        state.mc_sens = format!("{value}");
                        state.read_from = Some(only.name.clone());
                    }
                }
            }
        }
    }

    if state.instances.is_empty() {
        ui.weak("No Minecraft instances found, so type your sensitivity in below.");
    } else {
        ui.label("Read it from an instance:");
        ui.horizontal_wrapped(|ui| {
            for instance in &state.instances {
                if ui
                    .button(&instance.name)
                    .on_hover_text(instance.options.display().to_string())
                    .clicked()
                {
                    match minecraft::read_sensitivity(&instance.options) {
                        Some(value) => {
                            state.mc_sens = format!("{value}");
                            state.read_from = Some(instance.name.clone());
                            state.error = None;
                            state.result = None;
                        }
                        None => {
                            state.error = Some(format!(
                                "no usable mouseSensitivity in {}",
                                instance.options.display()
                            ));
                        }
                    }
                }
            }
        });
    }

    if let Some(name) = &state.read_from {
        ui.weak(format!("read from {name}"));
    }

    ui.add_space(8.0);

    settings_grid(ui, "sens-inputs", |ui| {
        ui.label("Minecraft sensitivity").on_hover_text(
            "The mouseSensitivity line in options.txt, which is 0 to 1 rather \
             than the percentage the slider shows you.",
        );
        if ui.text_edit_singleline(&mut state.mc_sens).changed() {
            state.result = None;
            state.read_from = None;
        }
        ui.end_row();

        ui.label("Mouse was scaled by").on_hover_text(
            "Your sensitivity means something different once the pointer \
             acceleration you had is gone, so it gets converted first.",
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.kind, PointerKind::Flat, "Nothing");
            ui.selectable_value(&mut state.kind, PointerKind::Windows, "Windows");
            ui.selectable_value(&mut state.kind, PointerKind::Linux, "Linux");
        });
        ui.end_row();

        match state.kind {
            PointerKind::Flat => {}
            PointerKind::Windows => {
                ui.label("Windows pointer speed").on_hover_text(
                    "The slider in Mouse Properties, 1 to 20. The middle notch is 10.",
                );
                ui.add(egui::Slider::new(&mut state.windows_step, 1..=20));
                ui.end_row();
            }
            PointerKind::Linux => {
                ui.label("Linux pointer speed").on_hover_text(
                    "libinput's acceleration speed, -1 to 1. Your desktop's mouse \
                     slider is this number. 0 is unscaled.",
                );
                ui.add(egui::Slider::new(&mut state.linux_speed, -1.0..=1.0));
                ui.end_row();
            }
        }

    });

    ui.add_space(4.0);
    ui.collapsing("Resolutions this is worked out for", |ui| {
        ui.label(
            "The tall multiplier comes from how much of the vertical FOV a taller \
             framebuffer squeezes into the same window, so both heights matter.",
        );
        ui.add_space(4.0);
        ui.weak(format!(
            "Worked out at {} FOV, the minimum, because that is what you drop to \
             for an eye measurement and put back afterwards.",
            sens::DEFAULT_VFOV
        ));
        ui.add_space(4.0);
        settings_grid(ui, "sens-res", |ui| {
            ui.label("Screen height");
            ui.text_edit_singleline(&mut state.normal_height);
            ui.end_row();

            ui.label("Tall height");
            ui.text_edit_singleline(&mut state.tall_height);
            ui.end_row();
        });
    });

    ui.add_space(8.0);
    if ui.button("Calculate").clicked() {
        state.calculate();
    }

    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::from_rgb(255, 120, 120), error);
    }

    if let Some(result) = state.result {
        ui.add_space(8.0);
        ui.separator();
        settings_grid(ui, "sens-results", |ui| {
            ui.label("Set Minecraft's sensitivity to");
            ui.horizontal(|ui| {
                ui.strong(format!("{}", result.mc));
                if ui.small_button("copy").clicked() {
                    ui.ctx().copy_text(format!("{}", result.mc));
                }
            });
            ui.end_row();

            ui.label("waywall sensitivity");
            ui.strong(format!("{:.8}", result.normal));
            ui.end_row();

            ui.label("waywall tall sensitivity");
            ui.strong(format!("{:.8}", result.tall));
            ui.end_row();
        });

        ui.add_space(4.0);
        ui.label(
            "Minecraft's own number is the one thing this cannot set for you. \
             Change it in the game's settings, or it will not match.",
        );

        ui.add_space(6.0);
        if ui.button("Use these").clicked() {
            accepted = Some(result);
        }
    }

    accepted
}
