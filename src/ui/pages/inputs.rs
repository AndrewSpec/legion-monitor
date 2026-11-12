use crate::ui::*;

impl App {
    pub(crate) fn page_inputs(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavInputs), self.t(Key::InDesc));
        let current = self.value(0x60);
        // The monitor repeats its input list; `ddc::capabilities` already keeps
        // only the first entry per code. Every entry is named by us — the
        // monitor reports USB-C as "Unrecognized value".
        let reported = self.caps.inputs.clone();
        let mut inputs: Vec<(u16, String)> = reported
            .iter()
            .map(|(code, label)| (*code, input_card_name(&self.tr, *code, label)))
            .collect();
        if inputs.is_empty() {
            inputs = vec![
                (0x0f, "DisplayPort".into()),
                (0x11, "HDMI 1".into()),
                (0x12, "HDMI 2".into()),
                (0x31, "USB-C".into()),
            ];
        }

        let split = matches!(
            self.multi_mode(),
            MultiMode::PictureInPicture | MultiMode::PictureByPicture
        );
        card_titled(ui, self.t(Key::CardInput), |ui| {
            let mut chosen: Option<u16> = None;
            // In a split, 0x60 carries two sources. A single-byte write here
            // would change only one half (or be refused), so the split sources
            // are edited on the Multi-view page where both are written at once.
            ui.add_enabled_ui(!split, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (code, name) in &inputs {
                        let active = current == Some(*code);
                        if input_card(ui, &self.tr, name, active).clicked() {
                            chosen = Some(*code);
                        }
                        ui.add_space(6.0);
                    }
                });
            });
            if let Some(c) = chosen {
                self.send(Cmd::Apply(vec![(0x60, c)]));
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new(self.t(if split {
                    Key::WindowsNote
                } else {
                    Key::InSwitchHint
                }))
                .size(11.0)
                .color(FAINT),
            );
        });

        let features = catalog::features_in(Section::Inputs);
        if !features.is_empty() {
            ui.add_space(16.0);
            card_titled(ui, self.t(Key::CardExtraFeatures), |ui| {
                ui.label(
                    RichText::new(self.t(Key::ExtraFeaturesHint))
                        .size(11.0)
                        .color(FAINT),
                );
                ui.add_space(10.0);
                for (i, feature) in features.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(6.0);
                    }
                    self.feature_row(ui, feature);
                }
            });
        }
    }

    // ── page: multi-view ──────────────────────────────────────────────────
}
