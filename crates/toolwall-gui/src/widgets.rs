//! Shared editors used by more than one tab.

use std::path::{Path, PathBuf};

use toolwall_core::schema::{Anchor, Rect};
use toolwall_core::{Problem, Scope};

/// A two column settings grid: the name of a thing on the left, the control
/// for it on the right.
///
/// Every tab is built out of these, and they have to match each other, so the
/// spacing lives here rather than being written out again at each one.
pub fn settings_grid<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    body: impl FnOnce(&mut egui::Ui) -> R,
) {
    egui::Grid::new(id).num_columns(2).spacing([12.0, 6.0]).show(ui, body);
}

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
        let mut chosen = depth.is_some();
        if ui.checkbox(&mut chosen, "Choose the layer myself").changed() {
            *depth = chosen.then_some(1);
        }
        if let Some(value) = depth {
            ui.add(egui::DragValue::new(value).speed(1.0).range(-100..=100));
            ui.weak("higher draws in front");
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
        let swatch = ui.color_edit_button_srgba_unmultiplied(&mut rgba);
        if swatch.changed() {
            *hex = format!("#{:02x}{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2], rgba[3]);
        }

        // A border, because the usual value here is black and a black square
        // on a black panel is indistinguishable from nothing at all. A
        // chequer behind it so a transparent colour reads as transparent.
        let rect = swatch.rect;
        let painter = ui.painter_at(rect);
        if rgba[3] < 255 {
            let step = rect.height() / 2.0;
            for row in 0..2 {
                for column in 0..((rect.width() / step).ceil() as usize) {
                    if (row + column) % 2 == 0 {
                        continue;
                    }
                    let at = rect.min + egui::vec2(column as f32 * step, row as f32 * step);
                    painter.rect_filled(
                        egui::Rect::from_min_size(at, egui::vec2(step, step))
                            .intersect(rect),
                        0.0,
                        egui::Color32::from_gray(90),
                    );
                }
            }
            painter.rect_filled(
                rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]),
            );
        }
        painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_gray(150)));

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
/// Which edge a rectangle is measured from, or none at all.
///
/// The same widget for both ends of a mirror. `src_anchor` is measured from a
/// corner of the game and `dst_anchor` from a corner of the screen, but the
/// choice is the same five and so is the meaning of the offsets.
pub fn anchor_picker(ui: &mut egui::Ui, salt: &str, value: &mut Option<Anchor>) {
    const CHOICES: &[(Option<Anchor>, &str)] = &[
        (None, "Not pinned"),
        (Some(Anchor::TopLeft), "Top left"),
        (Some(Anchor::TopRight), "Top right"),
        (Some(Anchor::BottomLeft), "Bottom left"),
        (Some(Anchor::BottomRight), "Bottom right"),
        (Some(Anchor::Center), "Centre"),
    ];

    let label = CHOICES
        .iter()
        .find(|(a, _)| a == value)
        .map(|(_, name)| *name)
        .unwrap_or("Not pinned");

    egui::ComboBox::from_id_salt(salt)
        .selected_text(label)
        .width(150.0)
        .show_ui(ui, |ui| {
            for (choice, name) in CHOICES {
                ui.selectable_value(value, *choice, *name);
            }
        });
}

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
            // Take the rest of the row. At a fixed width a path clipped mid
            // word while half the row sat empty, and the half that survived
            // was the directory rather than the file name.
            let room = (ui.available_width() - 100.0).max(160.0);
            ui.add(egui::TextEdit::singleline(value).desired_width(room));

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

/// A tab's scrolling body.
///
/// One place, so every tab leaves the same room under its last row and the
/// status bar never sits flush against a control.
///
/// It used to fade the bottom few pixels to say "there is more below". The
/// fade painted over whatever was there, which was usually the Add button, so
/// the one action on the screen came out at half contrast and looked
/// disabled. The scrollbar is solid and always visible; that is the signal.
pub fn scroll_body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .show(ui, |ui| {
            let value = add(ui);
            ui.add_space(24.0);
            value
        })
        .inner
}

/// One option of a segmented control, with chrome whether it is picked or not.
///
/// egui's `selectable_label` paints nothing at all when it is neither selected
/// nor hovered, so "Light" next to a filled "Dark" read as a caption sitting
/// beside a button rather than as the other half of a switch. This gives the
/// unpicked side a fill and a border, and the picked side the accent plus a
/// tick, so the state is not carried by colour alone.
pub fn segment(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let visuals = ui.visuals();

    let button = if selected {
        egui::Button::new(egui::RichText::new(format!("• {text}")).strong())
            .fill(visuals.selection.bg_fill)
            .stroke(egui::Stroke::new(1.0, visuals.selection.bg_fill))
    } else {
        egui::Button::new(text)
            .fill(visuals.widgets.inactive.weak_bg_fill)
            .stroke(visuals.widgets.inactive.bg_stroke)
    };

    ui.add(button)
}

/// `segment`, for the common case of picking one value out of several.
pub fn segment_value<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    text: &str,
) -> egui::Response {
    let response = segment(ui, *current == value, text);
    if response.clicked() {
        *current = value;
    }
    response
}

/// `segment` as a widget, for the call sites that want the Response chained.
pub fn segment_button(ui: &egui::Ui, selected: bool, text: &str) -> egui::Button<'static> {
    let visuals = ui.visuals();

    if selected {
        egui::Button::new(egui::RichText::new(format!("• {text}")).strong())
            .fill(visuals.selection.bg_fill)
            .stroke(egui::Stroke::new(1.0, visuals.selection.bg_fill))
    } else {
        egui::Button::new(text.to_string())
            .fill(visuals.widgets.inactive.weak_bg_fill)
            .stroke(visuals.widgets.inactive.bg_stroke)
    }
}
