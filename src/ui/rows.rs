use crate::ui::*;

// ── widgets ──────────────────────────────────────────────────────────────

/// A ddcutil message is often technical and in English — we show a short
/// translated sentence and keep the original under "Technical details".
pub(crate) fn friendly_error(msg: &str) -> Key {
    let m = msg.to_lowercase();
    if m.contains("no displays implementing ddc/ci")
        || m.contains("no /dev/i2c devices")
        || m.contains("no monitor supporting ddc/ci")
    {
        Key::ErrNoDisplay
    } else if m.contains("laptop display") {
        Key::ErrLaptop
    } else if m.contains("permission denied") || m.contains("access denied") {
        Key::ErrPermission
    } else {
        Key::ErrGeneric
    }
}

impl App {
    pub(crate) fn codes_in(&self, section: Section) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for (code, v) in &self.vals {
            let ddc_name = self.caps.name_of(*code).unwrap_or("");
            if catalog::section_for(*code, ddc_name) != section {
                continue;
            }
            if catalog::kind_for(*code, v.max) == Kind::ReadOnly {
                continue;
            }
            out.push(*code);
        }
        out
    }

    pub(crate) fn label_of(&self, code: u8) -> String {
        if let Some(key) = catalog::fn_key(code) {
            return self.t(key).to_string();
        }
        let ddc_name = self.caps.name_of(code).unwrap_or("");
        catalog::unknown_label(&self.tr, code, ddc_name)
    }

    /// Hint under the label; empty when there is nothing to add.
    fn hint_of(&self, code: u8, hint: &str) -> String {
        if hint.is_empty() {
            catalog::hint_key(code)
                .map(|k| self.t(k).to_string())
                .unwrap_or_default()
        } else {
            hint.to_string()
        }
    }

    /// Slider with a label and value; saved when released.
    pub(crate) fn slider_row(&mut self, ui: &mut Ui, code: u8, label: &str, hint: &str) {
        let hint = self.hint_of(code, hint);
        let Some(v) = self.vals.get(&code).cloned() else {
            self.placeholder_row(ui, label, &hint);
            return;
        };
        let max = v.max.max(1) as f32;
        let mut cur = self.value(code).unwrap_or(v.value) as f32;

        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(13.0).color(TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{}", cur.round() as u16))
                        .size(13.0)
                        .strong()
                        .color(ACCENT),
                );
            });
        });
        if !hint.is_empty() {
            ui.label(RichText::new(&hint).size(11.0).color(FAINT));
        }

        let resp = ui.add(
            egui::Slider::new(&mut cur, 0.0..=max)
                .show_value(false)
                .trailing_fill(true),
        );
        if resp.changed() {
            self.pending.insert(code, cur.round() as u16);
            if !resp.dragged() {
                self.set_value(code, cur.round() as u16);
            }
        }
        if resp.drag_stopped() {
            self.set_value(code, cur.round() as u16);
        }
        ui.add_space(6.0);
    }

    pub(crate) fn toggle_row(&mut self, ui: &mut Ui, code: u8, label: &str, hint: &str) {
        let hint = self.hint_of(code, hint);
        let Some(v) = self.vals.get(&code).cloned() else {
            self.placeholder_row(ui, label, &hint);
            return;
        };
        let mut on = self.value(code).unwrap_or(v.value) != 0;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).size(13.0).color(TEXT));
                if !hint.is_empty() {
                    ui.label(RichText::new(&hint).size(11.0).color(FAINT));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Checkbox::without_text(&mut on)).changed() {
                    self.set_value(code, if on { 1 } else { 0 });
                }
            });
        });
        ui.add_space(6.0);
    }

    pub(crate) fn choice_row(
        &mut self,
        ui: &mut Ui,
        code: u8,
        label: &str,
        options: &'static [(u16, Key)],
    ) {
        let current = self.value(code).unwrap_or(0);
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let mut chosen: Option<u16> = None;
        ui.horizontal_wrapped(|ui| {
            for (value, key) in options {
                if pill(ui, self.t(*key), current == *value).clicked() {
                    chosen = Some(*value);
                }
            }
        });
        if let Some(v) = chosen {
            self.set_value(code, v);
        }
        ui.add_space(6.0);
    }

    /// Like `choice_row`, but takes a list built at run time
    /// (value names come from what the monitor reported).
    pub(crate) fn choice_row_owned(
        &mut self,
        ui: &mut Ui,
        code: u8,
        label: &str,
        options: &[(u16, String)],
    ) {
        let current = self.value(code).unwrap_or(0);
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let mut chosen: Option<u16> = None;
        ui.horizontal_wrapped(|ui| {
            for (value, name) in options {
                if pill(ui, name, current == *value).clicked() {
                    chosen = Some(*value);
                }
            }
        });
        if let Some(v) = chosen {
            self.set_value(code, v);
        }
        ui.add_space(6.0);
    }

    /// Picks a convenient control for the feature: value list → buttons,
    /// 0/1 value → toggle, otherwise → slider. Names are always human-readable.
    pub(crate) fn auto_row(&mut self, ui: &mut Ui, code: u8) {
        let label = self.label_of(code);
        let kind = catalog::kind_for(code, self.max(code));
        if kind == Kind::Choice {
            let opts = self.caps.values_of(code).map(|v| v.to_vec());
            if let Some(opts) = opts {
                if opts.len() > 1 {
                    self.choice_row_owned(ui, code, &label, &opts);
                    return;
                }
            }
        }
        if kind == Kind::Toggle || self.max(code) <= 1 {
            self.toggle_row(ui, code, &label, "");
        } else {
            self.slider_row(ui, code, &label, "");
        }
    }

    /// "No data" row — the page structure stays, only the values are empty.
    pub(crate) fn placeholder_row(&self, ui: &mut Ui, label: &str, hint: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(13.0).color(FAINT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("—").size(13.0).color(FAINT));
            });
        });
        if !hint.is_empty() {
            ui.label(
                RichText::new(hint)
                    .size(11.0)
                    .color(FAINT.gamma_multiply(0.75)),
            );
        }
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(4), BG);
        ui.add_space(8.0);
    }
}
