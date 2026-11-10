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
            ui.horizontal_wrapped(|ui| {
                for layout in catalog::LAYOUTS {
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
            if let Some(v) = chosen {
                self.send(Cmd::Apply(vec![(0xf5, v), (0xf2, 1)]));
            }
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardWindowsShort), |ui| {
            self.window_sources_block(ui);
            ui.add_space(8.0);
            ui.label(
                RichText::new(self.t(Key::WindowsNote))
                    .size(11.0)
                    .color(FAINT),
            );
        });
    }

    // ── page: profiles ────────────────────────────────────────────────────
}
