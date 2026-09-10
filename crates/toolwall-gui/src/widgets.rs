//! Shared editors used by more than one tab.

use std::path::{Path, PathBuf};

use toolwall_core::schema::Rect;
use toolwall_core::{Problem, Scope};

/// A number with -/+ buttons either side.
///
/// Dragging a DragValue is fine with a mouse you can see; nudging a rectangle
/// a few pixels while the change applies live is what this is actually for.
pub fn stepper_i32(ui: &mut egui::Ui, value: &mut i32, step: i32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.small_button("−").clicked() {
            *value -= step;
            changed = true;
        }
        changed |= ui.add(egui::DragValue::new(value).speed(1.0)).changed();
        if ui.small_button("+").clicked() {
            *value += step;
            changed = true;
        }
    });
    changed
}

pub fn stepper_u32(ui: &mut egui::Ui, value: &mut u32, step: u32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.small_button("−").clicked() {
            *value = value.saturating_sub(step);
            changed = true;
        }
        changed |= ui.add(egui::DragValue::new(value).speed(1.0)).changed();
        if ui.small_button("+").clicked() {
            *value += step;
            changed = true;
        }
    });
    changed
}

/// Position and size, as two labelled rows of steppers.
pub fn rect_editor(ui: &mut egui::Ui, _salt: &str, rect: &mut Rect) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label("X");
            stepper_i32(ui, &mut rect.x, 10);
            ui.label("Y");
            stepper_i32(ui, &mut rect.y, 10);
        });
        ui.horizontal(|ui| {
            ui.label("W");
            stepper_u32(ui, &mut rect.w, 10);
            ui.label("H");
            stepper_u32(ui, &mut rect.h, 10);
        });
    });
}

/// Depth is optional, and "unset" is meaningfully different from 0 - it
/// selects a different layer in waywall's ordering.
pub fn depth_editor(ui: &mut egui::Ui, depth: &mut Option<i32>) {
    ui.horizontal(|ui| {
        let mut set = depth.is_some();
        if ui.checkbox(&mut set, "set").changed() {
            *depth = set.then_some(1);
        }
        if let Some(value) = depth {
            ui.add(egui::DragValue::new(value).speed(1.0));
        } else {
            ui.weak("auto");
        }
    });
}

/// Shader names come from `doc.shaders`, so you can only pick one that exists.
pub fn shader_picker(
    ui: &mut egui::Ui,
    salt: &str,
    shader: &mut Option<String>,
    available: &[String],
) {
    if available.is_empty() {
        ui.weak("no shaders defined");
        return;
    }

    let current = shader.clone().unwrap_or_else(|| "none".into());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current)
        .show_ui(ui, |ui| {
            if ui.selectable_label(shader.is_none(), "none").clicked() {
                *shader = None;
            }
            for name in available {
                let picked = shader.as_deref() == Some(name.as_str());
                if ui.selectable_label(picked, name).clicked() {
                    *shader = Some(name.clone());
                }
            }
        });
}

/// A colour swatch you can click, backed by a #rrggbb / #rrggbbaa string.
///
/// waywall wants hex, but nobody should have to type hex to pick a colour.
pub fn color_field(ui: &mut egui::Ui, hex: &mut String) {
    let mut rgba = parse_hex(hex).unwrap_or([0, 0, 0, 255]);

    ui.horizontal(|ui| {
        if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
            *hex = format!("#{:02x}{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2], rgba[3]);
        }
        ui.add(egui::TextEdit::singleline(hex).desired_width(110.0));
    });
}

fn parse_hex(hex: &str) -> Option<[u8; 4]> {
    let body = hex.trim().strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(body.get(i..i + 2)?, 16).ok();

    match body.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

/// An optional free-text field, where empty means absent.
pub fn optional_text(ui: &mut egui::Ui, value: &mut Option<String>) {
    let mut text = value.clone().unwrap_or_default();
    if ui.text_edit_singleline(&mut text).changed() {
        *value = (!text.is_empty()).then_some(text);
    }
}

/// Draw any validation problems for this item, in place.
pub fn problems_for(ui: &mut egui::Ui, problems: &[Problem], scope: &Scope) {
    for problem in problems.iter().filter(|p| &p.scope == scope) {
        ui.colored_label(egui::Color32::from_rgb(255, 120, 120), format!("⚠ {}", problem.message));
    }
}

/// A minimal directory browser.
///
/// Deliberately not a native file dialog: `rfd` would pull in GTK or the
/// desktop portal, and this binary launches inside a nested compositor next to
/// a running game. Browsing with `std::fs` costs nothing.
/// What a pick should be written back to.
#[derive(Clone, PartialEq, Eq)]
pub enum PickTarget {
    Image(usize),
    Background,
    NinbJar,
    Font,
}

#[derive(Default)]
pub struct FileBrowser {
    pub target: Option<PickTarget>,
    dir: PathBuf,
    extension: &'static str,
}

impl FileBrowser {
    pub fn open(&mut self, target: PickTarget, start_from: &str, extension: &'static str) {
        self.target = Some(target);
        self.extension = extension;

        let expanded = expand_tilde(start_from);
        let start = Path::new(&expanded);
        self.dir = start
            .parent()
            .filter(|p| p.is_dir())
            .map(Path::to_path_buf)
            .or_else(|| dirs_home().map(|h| h.join(".config").join("waywall")))
            .unwrap_or_else(|| PathBuf::from("."));
    }

    /// Returns what was picked, and where it should go, if anything.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<(PickTarget, String)> {
        let target = self.target.clone()?;

        let mut picked = None;
        let mut keep_open = true;

        egui::Window::new("Choose a file")
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 380.0])
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("⬆ up").clicked() {
                        if let Some(parent) = self.dir.parent() {
                            self.dir = parent.to_path_buf();
                        }
                    }
                    ui.weak(self.dir.display().to_string());
                });

                ui.separator();

                let mut entries: Vec<_> = std::fs::read_dir(&self.dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_dir()
                            || p.extension().is_some_and(|e| {
                                e.eq_ignore_ascii_case(self.extension)
                            })
                    })
                    .collect();
                entries.sort_by_key(|p| (!p.is_dir(), p.to_string_lossy().to_lowercase()));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for path in entries {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();

                        if path.is_dir() {
                            if ui.button(format!("🗀 {name}")).clicked() {
                                self.dir = path;
                            }
                        } else if ui.button(format!("• {name}")).clicked() {
                            picked = Some(path.display().to_string());
                        }
                    }
                });
            });

        if picked.is_some() || !keep_open {
            self.target = None;
        }
        picked.map(|path| (target, path))
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn expand_tilde(path: &str) -> String {
    match path.strip_prefix("~") {
        Some(rest) => match dirs_home() {
            Some(home) => format!("{}{}", home.display(), rest),
            None => path.to_string(),
        },
        None => path.to_string(),
    }
}

/// A path field with a Browse button, so nobody has to type one.
pub fn path_field(
    ui: &mut egui::Ui,
    value: &mut String,
    browser: &mut FileBrowser,
    target: PickTarget,
    extension: &'static str,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(value);
            if ui.button("Browse…").clicked() {
                browser.open(target, value, extension);
            }
        });

        if !value.is_empty() && !Path::new(&expand_tilde(value)).is_file() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 170, 80),
                "⚠ no file at this path",
            );
        }
    });
}
