use crate::ui::*;

impl App {
    pub(crate) fn page_multiview(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavMultiView), self.t(Key::MvDesc));

        // One picture for mode, sources and "this computer", exactly like the
        // Overview. The mode selector inside it is the only place that turns
        // a split on or off.
        self.screen_card(ui);

        let mode = self.multi_mode();
        let pbp = mode == MultiMode::PictureByPicture;
        let panel_ratio = self.panel.ratio;
        let connected = !self.busy && matches!(self.conn, Conn::Ok(_));

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardPbp), |ui| {
            ui.label(RichText::new(self.t(Key::PbpHint)).size(11.5).color(DIM));
            ui.add_space(12.0);
            let clicked = ui
                .add_enabled_ui(connected, |ui| {
                    // PBP on this monitor applies 50/50; the wider layout
                    // belongs to the monitor's hardware True Split, which is
                    // not exposed because it forces a 120 Hz link.
                    split_tile(
                        ui,
                        catalog::SplitLayout::Half,
                        pbp,
                        Key::ModePbp,
                        &self.tr,
                        panel_ratio,
                    )
                })
                .inner
                .clicked();
            if clicked {
                self.mode_hint = Some(MultiMode::PictureByPicture);
                if pbp {
                    // Already in PBP: repair the layout without rewriting
                    // the sources.
                    self.send(Cmd::Apply(vec![(0xf5, catalog::SplitLayout::Half.value())]));
                } else {
                    // The old pair may belong to PIP or True Split; the
                    // readback fills in the real one.
                    self.win_sources = [0, 0];
                    self.send(Cmd::SetPbp {
                        layout: Some(catalog::SplitLayout::Half.value()),
                        source: self.pbp_entry_source(),
                    });
                }
            }

            if pbp {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(self.t(Key::WindowsNote))
                        .size(11.0)
                        .color(FAINT),
                );
                ui.add_space(10.0);
                self.window_source_picker(ui, 0);
                self.window_source_picker(ui, 1);
            }
        });

        ui.add_space(16.0);
        let pip = self.multi_mode() == MultiMode::PictureInPicture;
        let current_corner = self.pip_corner_from_registers();
        // PIP only works on a 60 Hz link; above that the monitor stretched the
        // second picture in testing. The worker guard is the real protection,
        // this only makes the refusal visible.
        let pip_blocked = self.pip_blocked_note();
        let pip_enabled = !self.busy && matches!(self.conn, Conn::Ok(_)) && pip_blocked.is_none();
        card_titled(ui, self.t(Key::CardPip), |ui| {
            if let Some(note) = &pip_blocked {
                ui.label(RichText::new(note).size(11.5).color(WARN));
            } else {
                ui.label(RichText::new(self.t(Key::PipHint)).size(11.5).color(DIM));
            }
            ui.add_space(12.0);
            let mut chosen: Option<catalog::PipCorner> = None;
            // Two tiles per row: four corner choices read as a 2×2 map of the
            // screen, which is how the monitor's OSD presents them.
            for row in catalog::PipCorner::ALL.chunks(2) {
                ui.horizontal(|ui| {
                    for corner in row {
                        if ui
                            .add_enabled_ui(pip_enabled, |ui| {
                                pip_tile(
                                    ui,
                                    *corner,
                                    pip && current_corner == *corner,
                                    &self.tr,
                                    panel_ratio,
                                )
                            })
                            .inner
                            .clicked()
                        {
                            chosen = Some(*corner);
                        }
                        ui.add_space(10.0);
                    }
                });
                ui.add_space(8.0);
            }
            if let Some(corner) = chosen {
                // Clicking a corner also opens PIP, so the choice is visible
                // immediately instead of waiting for the mode selector.
                self.enter_pip(corner);
            }

            ui.add_space(6.0);
            self.pip_source_picker(ui);
        });
    }
}
