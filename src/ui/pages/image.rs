use crate::ui::*;

impl App {
    pub(crate) fn page_image(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavImage), self.t(Key::ImgDesc));
        let mut codes = self.codes_in(Section::Image);
        if codes.is_empty() {
            codes = vec![0x10, 0x12, 0x87, 0x8a, 0x8b, 0x72, 0x6d];
        }
        card_titled(ui, self.t(Key::CardImageParams), |ui| {
            for (i, code) in codes.iter().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                }
                self.auto_row(ui, *code);
            }
        });

        let features = catalog::features_in(Section::Image);
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

    // ── page: color ───────────────────────────────────────────────────────
}
