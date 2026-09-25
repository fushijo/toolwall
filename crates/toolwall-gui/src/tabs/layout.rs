//! Layout: a keyboard you click, for building an XKB symbols file.
//!
//! The [xkbedit](https://xkbedit.github.io/) workflow, in the editor and
//! therefore reachable in game: click a key, say what it should type. Runners
//! want it for search crafting in another language, where the point is to get
//! at characters US QWERTY has no key for.
//!
//! Rebinds are on the Input tab and are a different thing. This changes what a
//! key *types*; a rebind changes which key it *is*. See `toolwall_core::xkb`.

use crate::widgets::scroll_body;
use toolwall_core::schema::CustomLayout;
use toolwall_core::{xkb, Document};

/// Which key is open for editing, if any.
#[derive(Clone, Default)]
pub struct LayoutEdit {
    pub selected: Option<String>,
    /// Which level the key caps are previewing.
    pub level: usize,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Document, state: &mut LayoutEdit) {
    scroll_body(ui, |ui| {
        ui.heading("Keyboard layout");
        ui.weak(
            "Changes what a key types without changing which key it is, so a \
             key can type something new in chat and the crafting book and \
             still do its job in game.",
        );

        // Turning it off must not throw the keys away. Someone unticked this
        // and ticked it again and lost every key they had mapped, because the
        // box used to delete the block rather than switch it off.
        let mut on = doc.input.custom_layout.as_ref().is_some_and(|l| l.enabled);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.checkbox(&mut on, "Use a custom layout").changed() {
                set_enabled(doc, on);
            }
        });

        if doc.input.custom_layout.as_ref().is_some_and(|l| !l.enabled) {
            ui.add_space(6.0);
            ui.weak(
                "Using your desktop's keyboard layout. The keys you set here \
                 are kept, so ticking the box puts them back.",
            );
            ui.add_space(4.0);
            ui.weak(
                "To use a layout your system already has instead of building \
                 one, see Keyboard language on the Input tab.",
            );
            return;
        }

        let Some(layout) = doc.input.custom_layout.as_mut() else {
            ui.add_space(6.0);
            ui.weak("Using your desktop's keyboard layout.");
            ui.add_space(4.0);
            ui.weak(
                "To use a layout your system already has instead of building \
                 one, turn on Advanced and look under Keyboard language on the \
                 Input tab.",
            );
            return;
        };

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Name").on_hover_text(
                "The filename under ~/.config/xkb/symbols, and what waywall is \
                 told to load.",
            );
            ui.text_edit_singleline(&mut layout.name);

            ui.add_space(12.0);
            ui.label("Built on").on_hover_text(
                "Keys you do not change come from this layout. Clearing it \
                 means every key you have not set stops typing, so leave it \
                 unless you know why you are changing it.",
            );
            ui.add(egui::TextEdit::singleline(&mut layout.base).desired_width(60.0));
        });

        if layout.base.trim().is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 60),
                "With no base layout, every key you have not set below will type nothing.",
            );
        }

        ui.add_space(8.0);
        ui.separator();

        // Which level the caps show. Editing is always all four at once, so
        // this only changes what you are looking at.
        ui.horizontal(|ui| {
            ui.label("Showing");
            for (index, name) in xkb::LEVEL_NAMES.iter().enumerate() {
                if ui.selectable_label(state.level == index, *name).clicked() {
                    state.level = index;
                }
            }
        });

        ui.add_space(6.0);
        keyboard(ui, layout, state);

        ui.add_space(8.0);
        if let Some(code) = state.selected.clone() {
            key_editor(ui, layout, &code, state);
        } else {
            ui.weak("Pick a key above to change what it types.");
        }

        ui.add_space(10.0);
        ui.separator();
        install(ui, doc);
    });
}

/// What this key produces at `level` right now: the edit if there is one, the
/// US default otherwise.
fn effective(layout: &CustomLayout, key: &xkb::Key, level: usize) -> String {
    layout
        .keys
        .get(key.code)
        .and_then(|levels| levels.get(level))
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| xkb::default_symbol(key, level).map(str::to_string))
        .unwrap_or_default()
}

fn is_changed(layout: &CustomLayout, key: &xkb::Key) -> bool {
    (0..xkb::LEVELS).any(|level| {
        effective(layout, key, level) != xkb::default_symbol(key, level).unwrap_or_default()
    })
}

/// A key cap label, short enough to fit.
///
/// Named keysyms like `BackSpace` and `ISO_Level3_Shift` are printed on the
/// cap, so they get the key's own short label instead.
fn cap_text(key: &xkb::Key, symbol: &str) -> String {
    if symbol.is_empty() {
        return String::new();
    }
    if symbol.chars().count() == 1 {
        return symbol.to_string();
    }
    if symbol == key.base.unwrap_or_default() || symbol == key.shift.unwrap_or_default() {
        return key.label.to_string();
    }
    symbol.chars().take(5).collect()
}

// a whole keyboard, for the low price of sixty buttons
fn keyboard(ui: &mut egui::Ui, layout: &CustomLayout, state: &mut LayoutEdit) {
    // Scroll rather than shrink: the editor is a small floating window and a
    // squeezed key cap stops being readable long before it stops fitting.
    egui::ScrollArea::horizontal().show(ui, |ui| {
        for row in xkb::ROWS {
            ui.horizontal(|ui| {
                for key in *row {
                    let symbol = effective(layout, key, state.level);
                    let changed = is_changed(layout, key);
                    let selected = state.selected.as_deref() == Some(key.code);

                    let label = cap_text(key, &symbol);
                    let text = if changed && !selected {
                        egui::RichText::new(label).strong()
                    } else {
                        egui::RichText::new(label)
                    };

                    let button = egui::Button::new(text)
                        .min_size(egui::vec2(34.0, 30.0))
                        .selected(selected);

                    let response = ui.add(button).on_hover_text(format!(
                        "{} ({})\n{}",
                        key.label,
                        key.code,
                        xkb::LEVEL_NAMES
                            .iter()
                            .enumerate()
                            .map(|(level, name)| {
                                let value = effective(layout, key, level);
                                let shown =
                                    if value.is_empty() { "-".to_string() } else { value };
                                format!("{name}: {shown}")
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ));

                    if response.clicked() {
                        state.selected = Some(key.code.to_string());
                    }
                }
            });
        }
    });
}

fn key_editor(ui: &mut egui::Ui, layout: &mut CustomLayout, code: &str, state: &mut LayoutEdit) {
    let Some(key) = xkb::key(code) else {
        state.selected = None;
        return;
    };

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.strong(format!("{}  ", key.label));
            ui.weak(key.code);

            if ui.small_button("Reset").on_hover_text("Back to US QWERTY").clicked() {
                layout.keys.remove(code);
            }
            if ui.small_button("×").on_hover_text("Close").clicked() {
                state.selected = None;
            }
        });

        ui.weak(
            "Type the character you want. A name works too (Escape, \
             adiaeresis); anything else is written as its Unicode code point.",
        );
        ui.add_space(4.0);

        let mut levels = layout
            .keys
            .get(code)
            .cloned()
            .unwrap_or_else(|| vec![String::new(); xkb::LEVELS]);
        levels.resize(xkb::LEVELS, String::new());

        let mut edited = false;

        egui::Grid::new(("layout-key", code)).num_columns(3).show(ui, |ui| {
            for (level, name) in xkb::LEVEL_NAMES.iter().enumerate() {
                ui.label(*name);

                let response =
                    ui.add(egui::TextEdit::singleline(&mut levels[level]).desired_width(90.0));
                edited |= response.changed();

                match xkb::default_symbol(key, level) {
                    Some(default) if levels[level].trim().is_empty() => {
                        ui.weak(format!("{default} (unchanged)"));
                    }
                    Some(_) | None if levels[level].trim().is_empty() => {
                        ui.weak("nothing");
                    }
                    _ => {
                        ui.weak(xkb::to_keysym(&levels[level]));
                    }
                }

                ui.end_row();
            }
        });

        if edited {
            if levels.iter().all(|v| v.trim().is_empty()) {
                layout.keys.remove(code);
            } else {
                layout.keys.insert(code.to_string(), levels);
            }
        }

        ui.add_space(4.0);
        ui.weak(
            "AltGr is the roomy one: putting the extra characters there leaves \
             normal typing exactly as it was.",
        );
    });
}

/// Switch the custom layout on or off without losing what is in it.
///
/// Turning it off used to delete the block, so unticking the box and ticking
/// it again threw away every key that had been mapped.
fn set_enabled(doc: &mut Document, on: bool) {
    let layout = doc.input.custom_layout.get_or_insert_with(CustomLayout::default);
    layout.enabled = on;
    let name = xkb::sanitise_name(&layout.name);

    if on {
        // Writing the file is what points waywall at it, and the name may
        // have been edited while it was off.
        if xkb::write_symbols(&layout.clone()).is_ok() {
            doc.input.layout = name;
            doc.input.variant = "basic".into();
        }
    } else if doc.input.layout == name {
        // Only ours to clear.
        doc.input.layout.clear();
        doc.input.variant.clear();
    }
}

/// Writing the symbols file, which is the step that makes any of this real.
fn install(ui: &mut egui::Ui, doc: &mut Document) {
    let Some(layout) = doc.input.custom_layout.clone() else {
        return;
    };

    let name = xkb::sanitise_name(&layout.name);
    let changed_keys = layout.keys.len();

    ui.horizontal(|ui| {
        if ui
            .button("Write layout file")
            .on_hover_text(
                "Writes ~/.config/xkb/symbols/<name> and points waywall at it. \
                 Needed once, and again after you change a key.",
            )
            .clicked()
        {
            state_after_write(ui, doc, &name);
        }

        ui.weak(format!(
            "{changed_keys} key{} changed",
            if changed_keys == 1 { "" } else { "s" }
        ));
    });

    if doc.input.layout != name {
        ui.add_space(4.0);
        ui.weak(format!(
            "waywall is currently set to load {:?}, not {name:?}. Writing the \
             file points it at this layout.",
            doc.input.layout
        ));
    }

    if let Some((ok, message)) = ui.data(|d| {
        d.get_temp::<Option<(bool, String)>>(egui::Id::new("layout-write-status")).flatten()
    }) {
        ui.add_space(4.0);
        if ok {
            ui.weak(message);
        } else {
            ui.colored_label(egui::Color32::from_rgb(220, 90, 90), message);
        }
    }

    ui.add_space(6.0);
    ui.collapsing("What gets written", |ui| {
        let body = xkb::symbols_file(&layout);
        ui.add(
            egui::TextEdit::multiline(&mut body.as_str())
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(8),
        );

    });
}

fn state_after_write(ui: &mut egui::Ui, doc: &mut Document, name: &str) {
    let layout = match &doc.input.custom_layout {
        Some(layout) => layout.clone(),
        None => return,
    };

    let status = match xkb::write_symbols(&layout) {
        Ok(path) => {
            // Point waywall at it. The variant is the section name inside the
            // file, which symbols_file always calls "basic".
            doc.input.layout = name.to_string();
            doc.input.variant = "basic".to_string();
            (true, format!("Wrote {}", path.display()))
        }
        Err(err) => (false, format!("{err:#}")),
    };

    ui.data_mut(|d| d.insert_temp(egui::Id::new("layout-write-status"), Some(status)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use toolwall_core::xkb;

    fn ctx_render(doc: &mut Document, state: &mut LayoutEdit) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show(ui, doc, state));
        });
    }

    #[test]
    fn switching_the_layout_off_and_on_keeps_every_key() {
        // The box used to delete the block, so unticking it and ticking it
        // again threw away every key that had been mapped. Someone hit this.
        let mut doc = Document::default();
        let mut state = LayoutEdit::default();

        let mut layout = CustomLayout::default();
        layout
            .keys
            .insert("AD01".into(), vec!["o".into(), "O".into(), String::new(), String::new()]);
        doc.input.custom_layout = Some(layout);

        set_enabled(&mut doc, false);
        ctx_render(&mut doc, &mut state);

        // And it has to survive the trip through the file, not just the frame.
        let json = serde_json::to_string(&doc).unwrap();
        let mut doc: Document = serde_json::from_str(&json).unwrap();

        set_enabled(&mut doc, true);
        ctx_render(&mut doc, &mut state);

        let layout = doc.input.custom_layout.expect("the layout is still there");
        assert_eq!(layout.keys.len(), 1, "and so are its keys");
        assert!(layout.enabled);
    }

    #[test]
    fn the_tab_renders_with_and_without_a_layout() {
        let mut doc = Document::default();
        let mut state = LayoutEdit::default();

        ctx_render(&mut doc, &mut state);

        doc.input.custom_layout = Some(CustomLayout::default());
        ctx_render(&mut doc, &mut state);

        // And with a key open for editing, at every level.
        state.selected = Some("AD01".into());
        for level in 0..xkb::LEVELS {
            state.level = level;
            ctx_render(&mut doc, &mut state);
        }
    }

    /// A key nobody changed reads as its US default, so the caps are right
    /// before any editing happens.
    #[test]
    fn an_untouched_key_shows_what_us_qwerty_types() {
        let layout = CustomLayout::default();
        let q = xkb::key("AD01").unwrap();

        assert_eq!(effective(&layout, q, 0), "q");
        assert_eq!(effective(&layout, q, 1), "Q");
        assert_eq!(effective(&layout, q, 2), "", "nothing sits on AltGr by default");
        assert!(!is_changed(&layout, q));
    }

    #[test]
    fn an_edited_key_reads_back_and_counts_as_changed() {
        let mut layout = CustomLayout::default();
        layout
            .keys
            .insert("AD01".into(), vec![String::new(), String::new(), "\u{f6}".into()]);

        let q = xkb::key("AD01").unwrap();

        // The levels left blank still fall through to the default.
        assert_eq!(effective(&layout, q, 0), "q");
        assert_eq!(effective(&layout, q, 2), "\u{f6}");
        assert!(is_changed(&layout, q));
    }

    /// Caps have to stay short or the keyboard stops being readable.
    #[test]
    fn key_caps_stay_short() {
        let layout = CustomLayout::default();

        for row in xkb::ROWS {
            for key in *row {
                for level in 0..xkb::LEVELS {
                    let cap = cap_text(key, &effective(&layout, key, level));
                    assert!(
                        cap.chars().count() <= key.label.chars().count().max(5),
                        "{} at level {level} renders as {cap:?}",
                        key.code,
                    );
                }
            }
        }
    }

    /// Named keysyms are unreadable on a cap, so the key's own label wins.
    #[test]
    fn a_named_keysym_shows_the_key_not_the_name() {
        let layout = CustomLayout::default();
        let space = xkb::key("SPCE").unwrap();
        let backspace = xkb::key("BKSP").unwrap();

        assert_eq!(cap_text(space, &effective(&layout, space, 0)), "Space");
        assert_eq!(cap_text(backspace, &effective(&layout, backspace, 0)), "Backspace");
    }
}
