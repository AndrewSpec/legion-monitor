use crate::ui::*;

impl App {
    pub(crate) fn page_inputs(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavInputs), self.t(Key::InDesc));
        let current = self.value(0x60);
        let mut inputs: Vec<(u16, String)> = self.caps.inputs.clone();
        if inputs.is_empty() {
            inputs = vec![
                (0x0f, "DisplayPort".into()),
                (0x11, "HDMI 1".into()),
                (0x12, "HDMI 2".into()),
                (0x31, "USB-C".into()),
            ];
        }

        card_titled(ui, self.t(Key::CardInput), |ui| {
            let mut chosen: Option<u16> = None;
            ui.horizontal_wrapped(|ui| {
                for (code, name) in &inputs {
                    let active = current == Some(*code);
                    if input_card(ui, &self.tr, name, active).clicked() {
                        chosen = Some(*code);
                    }
                    ui.add_space(6.0);
                }
            });
            if let Some(c) = chosen {
                self.send(Cmd::Apply(vec![(0x60, c)]));
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new(self.t(Key::InSwitchHint))
                    .size(11.0)
                    .color(FAINT),
            );
        });
    }

    // ── page: multi-view ──────────────────────────────────────────────────
}
