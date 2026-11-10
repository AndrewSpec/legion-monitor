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
    }

    // ── page: color ───────────────────────────────────────────────────────
}
