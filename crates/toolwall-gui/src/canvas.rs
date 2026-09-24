//! A drag-and-resize canvas for things drawn over the game.
//!
//! waywall has no cursor position in Lua, so nothing can be dragged in the
//! scene itself. What it does have is a config that reloads live, so the drag
//! happens here on a scaled picture of the screen and the overlay follows
//! about a third of a second later.
//!
//! Nothing in here needs the waywall patch. Positions and sizes are ordinary
//! config fields; only the readout's panel and outline need patched waywall,
//! and those are a drawing detail rather than anything this touches.

use toolwall_core::schema::Rect;

/// How an item may be resized, which is not always by dragging a corner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Resize {
    /// Corners and edges, any aspect.
    Free,
    /// Corners only, aspect held. For anything where a stretch would lie,
    /// like a mirrored capture.
    Aspect,
    /// One integer scale rather than a width and a height. Scene text has no
    /// continuous size: the face is an 8x16 bitmap and waywall multiplies it
    /// by a whole number, so half a step does not exist.
    Steps { min: u32, max: u32, current: u32 },
    /// Position only. Nothing uses this yet; it is here for overlays whose
    /// size is not ours to set.
    #[allow(dead_code)]
    None,
}

pub struct Item {
    /// Stable across frames; identifies which document field this is.
    pub key: String,
    pub label: String,
    pub rect: Rect,
    pub resize: Resize,
    /// Drawn dimmer, for things that are configured but not in this mode.
    pub muted: bool,
}

/// What the canvas did, for the caller to write back.
pub enum Action {
    None,
    /// New rectangle for `key`.
    Moved { key: String, rect: Rect },
    /// New integer scale for `key`, from a Steps resize.
    Scaled { key: String, steps: u32 },
    Selected(Option<String>),
}

#[derive(Clone, Default)]
pub struct State {
    pub selected: Option<String>,
    drag: Option<Drag>,
}

#[derive(Clone)]
struct Drag {
    key: String,
    grip: Grip,
    /// Where the pointer went down, and what the rect was then. Everything is
    /// measured from these rather than accumulated per frame, because a slow
    /// drag rounds to nothing each frame and an item that never moves is a
    /// hard bug to see.
    from_pointer: egui::Pos2,
    from_rect: Rect,
    from_steps: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grip {
    Body,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Grip {
    fn corners() -> [Grip; 4] {
        [Grip::TopLeft, Grip::TopRight, Grip::BottomLeft, Grip::BottomRight]
    }

    fn edges() -> [Grip; 4] {
        [Grip::Left, Grip::Right, Grip::Top, Grip::Bottom]
    }

    fn cursor(self) -> egui::CursorIcon {
        match self {
            Grip::Body => egui::CursorIcon::Grab,
            Grip::Left | Grip::Right => egui::CursorIcon::ResizeHorizontal,
            Grip::Top | Grip::Bottom => egui::CursorIcon::ResizeVertical,
            Grip::TopLeft | Grip::BottomRight => egui::CursorIcon::ResizeNwSe,
            Grip::TopRight | Grip::BottomLeft => egui::CursorIcon::ResizeNeSw,
        }
    }

    /// Where on the rect this grip sits, as a 0..1 fraction of each side.
    fn anchor(self) -> (f32, f32) {
        match self {
            Grip::Body => (0.5, 0.5),
            Grip::Left => (0.0, 0.5),
            Grip::Right => (1.0, 0.5),
            Grip::Top => (0.5, 0.0),
            Grip::Bottom => (0.5, 1.0),
            Grip::TopLeft => (0.0, 0.0),
            Grip::TopRight => (1.0, 0.0),
            Grip::BottomLeft => (0.0, 1.0),
            Grip::BottomRight => (1.0, 1.0),
        }
    }
}

/// Snap distance, in canvas pixels rather than screen pixels, so it feels the
/// same however far the view is zoomed out.
const SNAP_PX: f32 = 6.0;
const HANDLE: f32 = 7.0;
const MIN_SIDE: i32 = 8;

fn lerp_rect(r: Rect, screen: (i32, i32), area: egui::Rect) -> egui::Rect {
    let sx = area.width() / screen.0.max(1) as f32;
    let sy = area.height() / screen.1.max(1) as f32;
    egui::Rect::from_min_size(
        egui::pos2(area.min.x + r.x as f32 * sx, area.min.y + r.y as f32 * sy),
        egui::vec2(r.w as f32 * sx, r.h as f32 * sy),
    )
}

/// Candidate lines to snap against: the screen's own edges and middle, plus
/// every edge and middle of every other item.
fn snap_targets(items: &[Item], skip: &str, screen: (i32, i32)) -> (Vec<i32>, Vec<i32>) {
    let mut xs = vec![0, screen.0 / 2, screen.0];
    let mut ys = vec![0, screen.1 / 2, screen.1];

    for item in items {
        if item.key == skip {
            continue;
        }
        xs.push(item.rect.x);
        xs.push(item.rect.x + item.rect.w as i32 / 2);
        xs.push(item.rect.x + item.rect.w as i32);
        ys.push(item.rect.y);
        ys.push(item.rect.y + item.rect.h as i32 / 2);
        ys.push(item.rect.y + item.rect.h as i32);
    }

    (xs, ys)
}

/// Nudge `value` onto the nearest target within `slack`, reporting the line it
/// landed on so a guide can be drawn there.
fn snap(value: i32, targets: &[i32], slack: i32) -> (i32, Option<i32>) {
    let mut best: Option<(i32, i32)> = None;
    for &t in targets {
        let d = (value - t).abs();
        if d <= slack && best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, t));
        }
    }
    match best {
        Some((_, t)) => (t, Some(t)),
        None => (value, None),
    }
}

pub struct Canvas<'a> {
    pub screen: (i32, i32),
    pub items: &'a [Item],
    /// Where Minecraft itself lands in this mode, if it is not the whole
    /// window. In Thin BT the game is 340 of 1920 pixels wide, and without
    /// this you cannot tell an overlay on the game from one in the black
    /// beside it.
    pub game: Option<Rect>,
    /// Held to move without snapping.
    pub snapping: bool,
    /// Drawn over the real overlays rather than over a picture of them.
    ///
    /// Same geometry either way; what changes is that the backdrop and the
    /// grid are left out, because the thing behind the window is the actual
    /// game and painting a fake screen on top of it would be silly.
    pub over_the_real_thing: bool,
}

/// How big to draw the picture of the screen.
///
/// Sized on width alone, a 16:10 screen came out taller than the panel, so the
/// bottom of your own screen sat behind the status bar and the numbers under
/// the canvas were never reachable. The default readout position is bottom
/// right, which is exactly the corner that went missing.
///
/// Shared with the drag tests, which have to click where the canvas actually
/// is rather than where they assume it would be.
pub(crate) fn canvas_size(
    available: egui::Vec2,
    screen: (i32, i32),
    over_the_real_thing: bool,
) -> egui::Vec2 {
    let aspect = screen.1.max(1) as f32 / screen.0.max(1) as f32;

    let room = if over_the_real_thing {
        available.y
    } else {
        // Leave the inspector below it on screen.
        (available.y - 120.0).max(180.0)
    };

    let width = available.x.max(200.0).min(room / aspect.max(0.01));
    egui::vec2(width, width * aspect)
}

impl Canvas<'_> {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut State) -> Action {
        let size = canvas_size(
            egui::vec2(ui.available_width(), ui.available_height()),
            self.screen,
            self.over_the_real_thing,
        );

        let (area, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(area);
        let visuals = ui.visuals().clone();

        let accent = visuals.selection.bg_fill;
        let faint = visuals.weak_text_color();

        if !self.over_the_real_thing {
            // a stand-in for the screen
            painter.rect_filled(area, 6.0, visuals.extreme_bg_color);
            self.grid(&painter, area, faint);

            if let Some(game) = self.game {
                let r = lerp_rect(game, self.screen, area);
                painter.rect_filled(r, 2.0, faint.gamma_multiply(0.10));
                painter.rect_stroke(r, 2.0, egui::Stroke::new(1.0, faint));
                painter.text(
                    egui::pos2(r.center().x, r.max.y - 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Minecraft",
                    egui::FontId::proportional(11.0),
                    faint,
                );
            }
            painter.rect_stroke(area, 6.0, egui::Stroke::new(1.0, faint));
        }

        let scale = (
            area.width() / self.screen.0.max(1) as f32,
            area.height() / self.screen.1.max(1) as f32,
        );
        // Floored, because a canvas drawn larger than the screen it stands
        // for makes six canvas pixels less than one real one, and an integer
        // slack of zero is snapping switched off.
        let slack = ((SNAP_PX / scale.0.max(0.0001)).round() as i32).max(2);

        let mut action = Action::None;
        let mut guides: (Option<i32>, Option<i32>) = (None, None);

        // Topmost first, so the item drawn in front also takes the click.
        for item in self.items.iter().rev() {
            let r = lerp_rect(item.rect, self.screen, area);
            let selected = state.selected.as_deref() == Some(item.key.as_str());

            if let Some(hit) = self.interact(ui, state, item, r, selected, scale) {
                match hit {
                    Hit::Select => action = Action::Selected(Some(item.key.clone())),
                    Hit::Drag(next, g) => {
                        let (snapped, gx, gy) = if self.snapping {
                            self.apply_snap(next, item, g, slack)
                        } else {
                            (next, None, None)
                        };
                        guides = (gx, gy);
                        action = match item.resize {
                            Resize::Steps { current, .. } if g != Grip::Body => {
                                Action::Scaled { key: item.key.clone(), steps: current }
                            }
                            _ => Action::Moved { key: item.key.clone(), rect: snapped },
                        };
                    }
                    Hit::Steps(steps) => {
                        action = Action::Scaled { key: item.key.clone(), steps };
                    }
                }
            }
        }

        // Painted after interaction so the selected item is on top of the rest.
        //
        // taken: where a name has already been drawn, so two overlays in the
        // same corner do not print their names over each other.
        let mut taken: Vec<egui::Pos2> = Vec::new();
        for item in self.items {
            let selected = state.selected.as_deref() == Some(item.key.as_str());
            self.paint_item(&painter, item, area, selected, &visuals, accent, &mut taken);
        }

        self.paint_guides(&painter, area, guides, accent);

        if response.clicked() && !matches!(action, Action::Selected(_)) {
            state.selected = None;
            action = Action::Selected(None);
        }

        if let Action::Selected(ref key) = action {
            state.selected = key.clone();
        }

        action
    }

    fn grid(&self, painter: &egui::Painter, area: egui::Rect, colour: egui::Color32) {
        let stroke = egui::Stroke::new(1.0, colour.gamma_multiply(0.18));
        for i in 1..4 {
            let x = area.min.x + area.width() * i as f32 / 4.0;
            let y = area.min.y + area.height() * i as f32 / 4.0;
            painter.line_segment([egui::pos2(x, area.min.y), egui::pos2(x, area.max.y)], stroke);
            painter.line_segment([egui::pos2(area.min.x, y), egui::pos2(area.max.x, y)], stroke);
        }
    }

    fn paint_item(
        &self,
        painter: &egui::Painter,
        item: &Item,
        area: egui::Rect,
        selected: bool,
        visuals: &egui::Visuals,
        accent: egui::Color32,
        taken: &mut Vec<egui::Pos2>,
    ) {
        let r = lerp_rect(item.rect, self.screen, area);

        // An overlay past the edge of waywall's window simply does not draw,
        // with no error anywhere, so say so here instead.
        let off = !area.expand(0.5).contains_rect(r);

        let base = if off {
            egui::Color32::from_rgb(220, 90, 90)
        } else if selected {
            accent
        } else {
            visuals.widgets.inactive.fg_stroke.color
        };

        let dim = if item.muted { 0.35 } else { 1.0 };

        if !self.over_the_real_thing {
            painter.rect_filled(r, 3.0, base.gamma_multiply(0.14 * dim));
        } else if selected {
            // just enough to show which one has the handles
            painter.rect_filled(r, 3.0, base.gamma_multiply(0.10));
        }
        painter.rect_stroke(
            r,
            3.0,
            egui::Stroke::new(if selected { 2.0 } else { 1.0 }, base.gamma_multiply(dim)),
        );

        // The name, cut off at the edge of its own box and sitting on a chip.
        //
        // Without the chip two overlays in the same place drew their names on
        // top of each other and neither could be read. Without the truncation
        // a long name ran out past the box it belongs to and looked like it
        // belonged to the one next door.
        if r.width() > 36.0 && r.height() > 14.0 {
            let mut job = egui::text::LayoutJob::simple_singleline(
                item.label.clone(),
                egui::FontId::proportional(11.0),
                base.gamma_multiply(dim),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 8.0);

            let galley = painter.layout_job(job);

            // Drop below anything already written here. Two overlays pinned to
            // the same corner is normal, and it used to print one name on top
            // of the other so neither could be read.
            let mut at = r.min + egui::vec2(4.0, 3.0);
            let step = galley.size().y + 2.0;
            while taken.iter().any(|p| (p.x - at.x).abs() < 40.0 && (p.y - at.y).abs() < step - 1.0)
                && at.y + step * 2.0 < r.max.y
            {
                at.y += step;
            }
            taken.push(at);

            painter.rect_filled(
                egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(3.0, 1.0)),
                2.0,
                visuals.panel_fill.gamma_multiply(0.85 * dim),
            );
            painter.galley(at, galley, base);
        }

        if selected {
            for grip in Grip::corners().into_iter().chain(Grip::edges()) {
                if !self.grip_live(item, grip) {
                    continue;
                }
                let p = grip_pos(r, grip);
                painter.rect_filled(
                    egui::Rect::from_center_size(p, egui::vec2(HANDLE, HANDLE)),
                    1.5,
                    accent,
                );
            }
        }
    }

    fn grip_live(&self, item: &Item, grip: Grip) -> bool {
        match item.resize {
            Resize::Free => true,
            Resize::Aspect | Resize::Steps { .. } => Grip::corners().contains(&grip),
            Resize::None => false,
        }
    }

    fn paint_guides(
        &self,
        painter: &egui::Painter,
        area: egui::Rect,
        guides: (Option<i32>, Option<i32>),
        accent: egui::Color32,
    ) {
        let stroke = egui::Stroke::new(1.0, accent);
        let sx = area.width() / self.screen.0.max(1) as f32;
        let sy = area.height() / self.screen.1.max(1) as f32;

        if let Some(x) = guides.0 {
            let px = area.min.x + x as f32 * sx;
            painter.line_segment([egui::pos2(px, area.min.y), egui::pos2(px, area.max.y)], stroke);
        }
        if let Some(y) = guides.1 {
            let py = area.min.y + y as f32 * sy;
            painter.line_segment([egui::pos2(area.min.x, py), egui::pos2(area.max.x, py)], stroke);
        }
    }

    fn interact(
        &self,
        ui: &mut egui::Ui,
        state: &mut State,
        item: &Item,
        r: egui::Rect,
        selected: bool,
        scale: (f32, f32),
    ) -> Option<Hit> {
        let base = ui.id().with(("canvas-item", &item.key));

        let handle_rect = |g: Grip| {
            egui::Rect::from_center_size(grip_pos(r, g), egui::vec2(HANDLE * 2.2, HANDLE * 2.2))
        };

        // Register the body first and the handles after, so that where they
        // overlap egui hands the pointer to the handle.
        //
        // Every live handle is registered every frame, whether or not the
        // pointer is near it. egui hit-tests against the widgets it saw last
        // frame, so a handle that only appears once the pointer is already on
        // it has never been seen and never becomes hoverable.
        let body = ui.interact(r, base.with("body"), egui::Sense::click_and_drag());

        let mut handles = Vec::new();
        if selected {
            for grip in Grip::corners().into_iter().chain(Grip::edges()) {
                if !self.grip_live(item, grip) {
                    continue;
                }
                let resp =
                    ui.interact(handle_rect(grip), base.with(grip as u8), egui::Sense::drag());
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(grip.cursor());
                }
                handles.push((grip, resp));
            }
        }

        // A drag in flight owns the pointer until the button comes up, so
        // dragging a corner across another handle cannot hand it over.
        if let Some(running) = state.drag.clone() {
            if running.key == item.key {
                let resp = match running.grip {
                    Grip::Body => Some(body),
                    g => handles.iter().find(|(h, _)| *h == g).map(|(_, r)| r.clone()),
                };
                return resp.and_then(|resp| {
                    self.drag_step(ui, state, item, resp, running.grip, scale)
                });
            }
        }

        for (grip, resp) in handles {
            if resp.drag_started() || resp.dragged() {
                return self.drag_step(ui, state, item, resp, grip, scale);
            }
        }

        if body.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if body.clicked() {
            return Some(Hit::Select);
        }
        self.drag_step(ui, state, item, body, Grip::Body, scale)
    }

    fn drag_step(
        &self,
        ui: &mut egui::Ui,
        state: &mut State,
        item: &Item,
        resp: egui::Response,
        grip: Grip,
        scale: (f32, f32),
    ) -> Option<Hit> {
        if resp.drag_started() {
            // Where the button went down, not where the pointer is now.
            // egui only calls it a drag once the pointer has moved, so those
            // are different positions on the very first frame and using the
            // later one throws the opening delta away.
            let origin = ui
                .input(|i| i.pointer.press_origin())
                .or_else(|| resp.interact_pointer_pos());
            if let Some(p) = origin {
                state.selected = Some(item.key.clone());
                state.drag = Some(Drag {
                    key: item.key.clone(),
                    grip,
                    from_pointer: p,
                    from_rect: item.rect,
                    from_steps: match item.resize {
                        Resize::Steps { current, .. } => current,
                        _ => 0,
                    },
                });
            }
        }

        if resp.drag_stopped() {
            state.drag = None;
        }

        if !resp.dragged() {
            return None;
        }

        let drag = state.drag.clone()?;
        if drag.key != item.key || drag.grip != grip {
            return None;
        }

        let pointer = resp.interact_pointer_pos()?;

        // Rounded, not truncated: a ten pixel drag that floating point makes
        // 9.9999 would otherwise land on nine.
        let dx = ((pointer.x - drag.from_pointer.x) / scale.0).round() as i32;
        let dy = ((pointer.y - drag.from_pointer.y) / scale.1).round() as i32;

        if let Resize::Steps { min, max, .. } = item.resize {
            if grip != Grip::Body {
                // Diagonal distance, so either axis grows the text.
                let step = ((dx + dy) as f32 / 40.0).round() as i32;
                let next = (drag.from_steps as i32 + step).clamp(min as i32, max as i32);
                return Some(Hit::Steps(next as u32));
            }
        }

        Some(Hit::Drag(resize_rect(drag.from_rect, grip, dx, dy, item.resize), grip))
    }

    fn apply_snap(
        &self,
        mut r: Rect,
        item: &Item,
        grip: Grip,
        slack: i32,
    ) -> (Rect, Option<i32>, Option<i32>) {
        let (xs, ys) = snap_targets(self.items, &item.key, self.screen);
        let (ax, ay) = grip.anchor();

        // Snap the edge being dragged. For a move that is every edge, so try
        // left/centre/right and keep whichever lands.
        let mut gx = None;
        let mut gy = None;

        if grip == Grip::Body {
            for (frac, _) in [(0.0, ()), (1.0, ()), (0.5, ())] {
                let probe = r.x + (r.w as f32 * frac) as i32;
                let (snapped, line) = snap(probe, &xs, slack);
                if line.is_some() {
                    r.x += snapped - probe;
                    gx = line;
                    break;
                }
            }
            for (frac, _) in [(0.0, ()), (1.0, ()), (0.5, ())] {
                let probe = r.y + (r.h as f32 * frac) as i32;
                let (snapped, line) = snap(probe, &ys, slack);
                if line.is_some() {
                    r.y += snapped - probe;
                    gy = line;
                    break;
                }
            }
            return (r, gx, gy);
        }

        if ax == 0.0 {
            let (s, line) = snap(r.x, &xs, slack);
            if line.is_some() {
                r.w = (r.w as i32 + (r.x - s)).max(MIN_SIDE) as u32;
                r.x = s;
                gx = line;
            }
        } else if ax == 1.0 {
            let right = r.x + r.w as i32;
            let (s, line) = snap(right, &xs, slack);
            if line.is_some() {
                r.w = (s - r.x).max(MIN_SIDE) as u32;
                gx = line;
            }
        }

        if ay == 0.0 {
            let (s, line) = snap(r.y, &ys, slack);
            if line.is_some() {
                r.h = (r.h as i32 + (r.y - s)).max(MIN_SIDE) as u32;
                r.y = s;
                gy = line;
            }
        } else if ay == 1.0 {
            let bottom = r.y + r.h as i32;
            let (s, line) = snap(bottom, &ys, slack);
            if line.is_some() {
                r.h = (s - r.y).max(MIN_SIDE) as u32;
                gy = line;
            }
        }

        (r, gx, gy)
    }
}

enum Hit {
    Select,
    Drag(Rect, Grip),
    Steps(u32),
}

fn grip_pos(r: egui::Rect, grip: Grip) -> egui::Pos2 {
    let (ax, ay) = grip.anchor();
    egui::pos2(r.min.x + r.width() * ax, r.min.y + r.height() * ay)
}

/// Apply a drag to a rectangle. Pure, so the awkward part is testable without
/// a window on screen.
pub fn resize_rect(from: Rect, grip: Grip, dx: i32, dy: i32, resize: Resize) -> Rect {
    let mut r = from;

    if grip == Grip::Body {
        r.x = from.x + dx;
        r.y = from.y + dy;
        return r;
    }

    let (ax, ay) = grip.anchor();

    if ax == 0.0 {
        let right = from.x + from.w as i32;
        r.x = (from.x + dx).min(right - MIN_SIDE);
        r.w = (right - r.x) as u32;
    } else if ax == 1.0 {
        r.w = (from.w as i32 + dx).max(MIN_SIDE) as u32;
    }

    if ay == 0.0 {
        let bottom = from.y + from.h as i32;
        r.y = (from.y + dy).min(bottom - MIN_SIDE);
        r.h = (bottom - r.y) as u32;
    } else if ay == 1.0 {
        r.h = (from.h as i32 + dy).max(MIN_SIDE) as u32;
    }

    if resize == Resize::Aspect && from.w > 0 && from.h > 0 {
        let ratio = from.h as f32 / from.w as f32;
        let h = (r.w as f32 * ratio).round().max(MIN_SIDE as f32) as u32;
        if ay == 0.0 {
            let bottom = from.y + from.h as i32;
            r.y = bottom - h as i32;
        }
        r.h = h;
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn dragging_the_body_moves_without_resizing() {
        let out = resize_rect(r(10, 20, 100, 50), Grip::Body, 15, -5, Resize::Free);
        assert_eq!(out, r(25, 15, 100, 50));
    }

    #[test]
    fn dragging_a_right_edge_only_changes_width() {
        let out = resize_rect(r(10, 20, 100, 50), Grip::Right, 20, 99, Resize::Free);
        assert_eq!(out, r(10, 20, 120, 50));
    }

    /// The left edge has to move the origin as well, or the box grows the
    /// wrong way and the thing you were dragging runs away from the cursor.
    #[test]
    fn dragging_a_left_edge_moves_the_origin() {
        let out = resize_rect(r(100, 20, 100, 50), Grip::Left, -30, 0, Resize::Free);
        assert_eq!(out, r(70, 20, 130, 50));
    }

    #[test]
    fn dragging_a_top_edge_moves_the_origin() {
        let out = resize_rect(r(10, 100, 100, 50), Grip::Top, 0, -20, Resize::Free);
        assert_eq!(out, r(10, 80, 100, 70));
    }

    /// Dragging an edge past its opposite would otherwise produce a negative
    /// width, which is a u32 underflow and a panic.
    #[test]
    fn an_edge_cannot_be_dragged_through_its_opposite() {
        let out = resize_rect(r(10, 20, 100, 50), Grip::Right, -500, 0, Resize::Free);
        assert!(out.w >= MIN_SIDE as u32);

        let out = resize_rect(r(10, 20, 100, 50), Grip::Left, 500, 0, Resize::Free);
        assert!(out.w >= MIN_SIDE as u32);
        assert!(out.x <= 10 + 100 - MIN_SIDE);

        let out = resize_rect(r(10, 20, 100, 50), Grip::Top, 0, 500, Resize::Free);
        assert!(out.h >= MIN_SIDE as u32);
    }

    #[test]
    fn aspect_resize_keeps_the_shape() {
        let from = r(0, 0, 200, 100);
        let out = resize_rect(from, Grip::BottomRight, 100, 0, Resize::Aspect);
        assert_eq!(out.w, 300);
        assert_eq!(out.h, 150, "height should follow width at 2:1");
    }

    #[test]
    fn snapping_lands_on_a_target_and_reports_it() {
        let targets = [0, 500, 1000];
        assert_eq!(snap(497, &targets, 6), (500, Some(500)));
        assert_eq!(snap(480, &targets, 6), (480, None));
    }

    #[test]
    fn snap_targets_include_the_screen_and_the_neighbours() {
        let items = vec![Item {
            key: "a".into(),
            label: "a".into(),
            rect: r(100, 200, 50, 60),
            resize: Resize::Free,
            muted: false,
        }];
        let (xs, ys) = snap_targets(&items, "other", (1000, 800));

        for want in [0, 500, 1000, 100, 125, 150] {
            assert!(xs.contains(&want), "missing x target {want}: {xs:?}");
        }
        for want in [0, 400, 800, 200, 230, 260] {
            assert!(ys.contains(&want), "missing y target {want}: {ys:?}");
        }
    }

    /// An item does not snap to itself, or it sticks where it started.
    #[test]
    fn an_item_is_not_a_snap_target_for_itself() {
        let items = vec![Item {
            key: "a".into(),
            label: "a".into(),
            rect: r(100, 200, 50, 60),
            resize: Resize::Free,
            muted: false,
        }];
        let (xs, _) = snap_targets(&items, "a", (1000, 800));
        assert_eq!(xs, vec![0, 500, 1000]);
    }
}

#[cfg(test)]
mod drag_tests {
    use super::*;

    /// Drive the canvas through real egui frames with synthetic pointer input.
    ///
    /// The geometry above is pure and easy to check; this is the part that is
    /// not, and it is the part that decides whether anything moves when you
    /// actually push the mouse.
    struct Harness {
        snapping: bool,
        /// The window the canvas is laid out in. Tall enough matters: the
        /// canvas is fitted to the panel height, so a short window makes one
        /// screen pixel smaller than one canvas pixel.
        viewport: egui::Vec2,
        ctx: egui::Context,
        items: Vec<Item>,
        state: State,
        screen: (i32, i32),
        /// Where the canvas landed last frame, for aiming the pointer.
        area: egui::Rect,
    }

    impl Harness {
        fn new(items: Vec<Item>) -> Self {
            let mut h = Harness {
                snapping: false,
                viewport: egui::vec2(900.0, 700.0),
                ctx: egui::Context::default(),
                items,
                state: State::default(),
                screen: (1000, 1000),
                area: egui::Rect::NOTHING,
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> Option<(String, Rect)> {
            let mut moved = None;
            // A window the size someone would actually have, so the canvas
            // scale in these tests resembles the real one.
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.viewport)),
                events,
                ..Default::default()
            };

            let items = std::mem::take(&mut self.items);
            let mut state = std::mem::take(&mut self.state);
            let screen = self.screen;
            let snapping = self.snapping;
            let mut area = self.area;

            self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let before = ui.next_widget_position();
                    let size = canvas_size(
                        egui::vec2(ui.available_width(), ui.available_height()),
                        screen,
                        false,
                    );
                    area = egui::Rect::from_min_size(before, size);

                    let action =
                        Canvas { screen, items: &items, game: None, snapping, over_the_real_thing: false }
                            .show(ui, &mut state);
                    if let Action::Moved { key, rect } = action {
                        moved = Some((key, rect));
                    }
                });
            });

            self.items = items;
            self.state = state;
            self.area = area;
            moved
        }

        /// Screen coordinates to a point on the canvas.
        fn at(&self, x: i32, y: i32) -> egui::Pos2 {
            let sx = self.area.width() / self.screen.0 as f32;
            let sy = self.area.height() / self.screen.1 as f32;
            egui::pos2(self.area.min.x + x as f32 * sx, self.area.min.y + y as f32 * sy)
        }

        fn press(&mut self, p: egui::Pos2) {
            self.frame(vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
        }

        fn drag_to(&mut self, p: egui::Pos2) -> Option<(String, Rect)> {
            self.frame(vec![egui::Event::PointerMoved(p)])
        }
    }

    fn one_item(resize: Resize) -> Vec<Item> {
        vec![Item {
            key: "a".into(),
            label: "A".into(),
            rect: Rect { x: 100, y: 100, w: 200, h: 200 },
            resize,
            muted: false,
        }]
    }

    #[test]
    fn dragging_the_body_actually_moves_the_item() {
        let mut h = Harness::new(one_item(Resize::Free));

        h.press(h.at(200, 200));
        let moved = h.drag_to(h.at(400, 300));

        let (key, rect) = moved.expect("nothing moved");
        assert_eq!(key, "a");
        assert_eq!(rect.x, 300, "dragged 200 right from x=100");
        assert_eq!(rect.y, 200, "dragged 100 down from y=100");
        assert_eq!((rect.w, rect.h), (200, 200), "a move must not resize");
    }

    /// Several small drags in a row must add up. Converting each frame's delta
    /// to whole pixels on its own rounds a slow drag to nothing.
    #[test]
    fn the_canvas_never_outgrows_the_panel_it_is_in() {
        // Sized on width alone it drew past the bottom of the window, so the
        // bottom of your own screen sat behind the status bar and the numbers
        // under the canvas could not be reached. The default readout position
        // is bottom right, which is the corner that went missing.
        let available = egui::vec2(900.0, 700.0);

        for screen in [(1920, 1080), (2560, 1600), (1080, 1920), (3440, 1440)] {
            let size = canvas_size(available, screen, false);
            assert!(
                size.y <= available.y - 120.0 + 0.5,
                "{screen:?} drew {size:?} into {available:?}, leaving nothing for the numbers"
            );
            assert!(size.x <= available.x + 0.5, "{screen:?} drew wider than the panel");
            assert!(size.x > 0.0 && size.y > 0.0, "{screen:?} drew nothing");
        }
    }

    #[test]
    fn a_slow_drag_does_not_get_lost_to_rounding() {
        let mut h = Harness::new(one_item(Resize::Free));

        // Tall enough that the canvas is not shrunk to fit the panel, so one
        // screen pixel is at least one canvas pixel and a one-pixel step is a
        // thing the mouse can express at all. That is the case this is about.
        h.viewport = egui::vec2(900.0, 1300.0);
        h.frame(vec![]);

        h.press(h.at(200, 200));

        let mut last = None;
        for step in 1..=10 {
            last = h.drag_to(h.at(200 + step, 200));
        }

        let (_, rect) = last.expect("nothing moved");
        assert_eq!(rect.x, 110, "ten one-pixel steps should be ten pixels");
    }

    #[test]
    fn clicking_an_item_selects_it_and_clicking_away_clears_it() {
        let mut h = Harness::new(one_item(Resize::Free));

        let p = h.at(200, 200);
        h.press(p);
        h.frame(vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(h.state.selected.as_deref(), Some("a"));

        // Empty canvas, and inside the window: the canvas is square while the
        // window is not, so the far corner is off screen and never clicked.
        let away = h.at(700, 200);
        h.press(away);
        h.frame(vec![egui::Event::PointerButton {
            pos: away,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(h.state.selected, None);
    }

    /// A corner drag has to resize rather than move, and only once the item is
    /// selected, since the handles are not there otherwise.
    #[test]
    fn dragging_a_corner_of_a_selected_item_resizes_it() {
        let mut h = Harness::new(one_item(Resize::Free));
        h.state.selected = Some("a".into());
        h.frame(vec![]);

        // bottom right of a 100,100 200x200 box
        h.press(h.at(300, 300));
        let moved = h.drag_to(h.at(400, 350));

        let (_, rect) = moved.expect("nothing resized");
        assert_eq!((rect.x, rect.y), (100, 100), "the origin should not move");
        assert_eq!((rect.w, rect.h), (300, 250));
    }

    #[test]
    fn a_muted_item_is_still_draggable() {
        let mut items = one_item(Resize::Free);
        items[0].muted = true;
        let mut h = Harness::new(items);

        h.press(h.at(200, 200));
        assert!(h.drag_to(h.at(250, 200)).is_some(), "faded should not mean frozen");
    }

    /// Snapping is the difference between lining two overlays up and nearly
    /// lining them up, so it has to actually bite.
    #[test]
    fn a_drag_snaps_onto_a_neighbour() {
        let items = vec![
            Item {
                key: "a".into(),
                label: "A".into(),
                rect: Rect { x: 100, y: 100, w: 200, h: 200 },
                resize: Resize::Free,
                muted: false,
            },
            Item {
                key: "b".into(),
                label: "B".into(),
                rect: Rect { x: 600, y: 400, w: 100, h: 100 },
                resize: Resize::Free,
                muted: false,
            },
        ];

        let mut h = Harness { snapping: true, ..Harness::new(items) };
        h.frame(vec![]);

        // Aim a few pixels short of b's left edge and let the snap finish it.
        h.press(h.at(200, 200));
        let moved = h.drag_to(h.at(697, 200));

        let (_, rect) = moved.expect("nothing moved");
        assert_eq!(rect.x, 600, "should have snapped onto b's left edge at 600");
    }

    #[test]
    fn snapping_off_leaves_the_drag_alone() {
        let items = vec![
            Item {
                key: "a".into(),
                label: "A".into(),
                rect: Rect { x: 100, y: 100, w: 200, h: 200 },
                resize: Resize::Free,
                muted: false,
            },
            Item {
                key: "b".into(),
                label: "B".into(),
                rect: Rect { x: 600, y: 400, w: 100, h: 100 },
                resize: Resize::Free,
                muted: false,
            },
        ];

        let mut h = Harness::new(items);
        h.frame(vec![]);
        h.press(h.at(200, 200));
        let (_, rect) = h.drag_to(h.at(697, 200)).expect("nothing moved");
        assert_eq!(rect.x, 597, "no snapping, so it lands exactly where dragged");
    }
}
