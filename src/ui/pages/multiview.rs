use crate::ui::*;

impl App {
    pub(crate) fn page_multiview(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavMultiView), self.t(Key::MvDesc));

        card_titled(ui, self.t(Key::CardMode), |ui| self.mode_selector(ui));

        ui.add_space(16.0);
        let active = catalog::layout_for(self.value(0xf5).unwrap_or(0));
        card_titled(ui, self.t(Key::CardLayouts), |ui| {
            ui.label(RichText::new(self.t(Key::LayoutHint)).size(11.5).color(DIM));
            ui.add_space(12.0);
            let mut chosen: Option<u16> = None;
            // Four tiles per row: eight tiles do not fit on one row at the
            // minimum window width and egui would clip the last ones.
            for row in catalog::LAYOUTS.chunks(4) {
                ui.horizontal(|ui| {
                    for layout in row {
                        let selected = active.map(|a| a.value == layout.value).unwrap_or(false);
                        if ui
                            .add_enabled_ui(!self.busy && matches!(self.conn, Conn::Ok(_)), |ui| {
                                layout_tile(ui, layout, selected, &self.tr)
                            })
                            .inner
                            .clicked()
                        {
                            chosen = Some(layout.value);
                        }
                        ui.add_space(8.0);
                    }
                });
                ui.add_space(8.0);
            }
            if let Some(v) = chosen {
                if self.multi_mode() == MultiMode::PictureByPicture {
                    self.send(Cmd::Apply(vec![(0xf5, v)]));
                } else {
                    // Choosing a PBP layout also has to enter PBP: F2=1 is
                    // an exchange, not an enable, and 0xA5=0x0200 is the
                    // register Lenovo uses to open the PBP window.
                    self.win_sources = [0, 0];
                    self.send(Cmd::SetPbp { layout: Some(v) });
                }
            }
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardWindowsShort), |ui| {
            self.window_sources_block(ui);
            ui.add_space(8.0);
            // Lenovo's own service picks the window selector from the current
            // mode: 0x0200 (the PBP window) for picture-by-picture, 0x0100 (the
            // PIP window) for picture-in-picture. Mixing them up froze the
            // monitor's OSD, so the interface follows the same rule.
            match self.multi_mode() {
                MultiMode::PictureByPicture => {
                    ui.label(
                        RichText::new(self.t(Key::WindowsNote))
                            .size(11.0)
                            .color(FAINT),
                    );
                    ui.add_space(10.0);
                    self.window_source_picker(ui, 0);
                    self.window_source_picker(ui, 1);
                }
                MultiMode::PictureInPicture => {
                    self.window_source_picker(ui, 1);
                }
                _ => {}
            }
        });
    }

    // ── page: profiles ────────────────────────────────────────────────────
}
