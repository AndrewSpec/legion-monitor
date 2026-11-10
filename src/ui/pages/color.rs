use crate::ui::*;

impl App {
    pub(crate) fn page_color(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavColor), self.t(Key::ColDesc));
        let mut codes = self.codes_in(Section::Color);
        if codes.is_empty() {
            codes = vec![0x0c, 0x14, 0x16, 0x18, 0x1a];
        }
        let presets: Vec<u8> = codes
            .iter()
            .copied()
            .filter(|c| *c == 0x14 || *c == 0x0c)
            .collect();
        if !presets.is_empty() {
            for code in presets {
                let owned: Vec<(u16, String)> = match self.caps.values_of(code) {
                    Some(v) if v.len() > 1 => v.to_vec(),
                    _ => catalog::color_presets(code)
                        .iter()
                        .map(|(v, k)| (*v, self.t(*k).to_string()))
                        .collect(),
                };
                let title = self.label_of(code);
                let current = self.value(code);
                let mut chosen: Option<u16> = None;
                card_titled(ui, &title, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (value, name) in &owned {
                            if pill(ui, name, current == Some(*value)).clicked() {
                                chosen = Some(*value);
                            }
                        }
                    });
                });
                if let Some(v) = chosen {
                    self.set_value(code, v);
                }
                ui.add_space(12.0);
            }
        }
        let rest: Vec<u8> = codes
            .iter()
            .copied()
            .filter(|c| *c != 0x14 && *c != 0x0c)
            .collect();
        if !rest.is_empty() {
            card_titled(ui, self.t(Key::CardRgb), |ui| {
                for (i, code) in rest.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                    }
                    self.auto_row(ui, *code);
                }
            });
        }
    }

    // ── page: inputs ──────────────────────────────────────────────────────
}
