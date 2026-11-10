use crate::ui::*;

impl App {
    pub(crate) fn page_overview(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavOverview), self.t(Key::OvDesc));

        let mode = self.multi_mode();
        let input = self.value(0x60);
        let bright = self.value(0x10);
        let contrast = self.value(0x12);

        card(ui, |ui| {
            ui.horizontal(|ui| {
                stat(ui, self.t(Key::OvMode), self.t(mode.key()));
                ui.add_space(30.0);
                stat(
                    ui,
                    self.t(Key::OvSource),
                    &input
                        .map(|c| self.input_name_of(c))
                        .unwrap_or_else(|| "—".into()),
                );
                ui.add_space(30.0);
                stat(
                    ui,
                    self.t(Key::OvBrightness),
                    &bright.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                );
                ui.add_space(30.0);
                stat(
                    ui,
                    self.t(Key::OvContrast),
                    &contrast
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".into()),
                );
            });
        });

        ui.add_space(16.0);

        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            card_titled(ui, self.t(Key::CardBrightness), |ui| {
                self.slider_row(
                    ui,
                    0x10,
                    self.t(Key::OvBrightness),
                    self.t(Key::HintBrightness),
                );
                self.slider_row(ui, 0x12, self.t(Key::OvContrast), self.t(Key::HintContrast));
            });

            let ui = &mut cols[1];
            card_titled(ui, self.t(Key::CardConfigs), |ui| {
                ui.label(
                    RichText::new(self.t(Key::CardConfigsDesc))
                        .size(12.0)
                        .color(DIM),
                );
                ui.add_space(12.0);
                let save_a = self.tf(Key::SaveAs, "A");
                let save_b = self.tf(Key::SaveAs, "B");
                let a_filled = self.store.slots.a.is_some();
                let b_filled = self.store.slots.b.is_some();
                ui.horizontal(|ui| {
                    if wide_button(ui, &save_a, a_filled).clicked() {
                        self.save_to_slot('A');
                    }
                    ui.add_space(6.0);
                    if wide_button(ui, &save_b, b_filled).clicked() {
                        self.save_to_slot('B');
                    }
                });
            });
        });

        ui.add_space(16.0);

        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            card_titled(ui, self.t(Key::OvMode), |ui| {
                self.mode_selector(ui);
            });

            let ui = &mut cols[1];
            card_titled(ui, self.t(Key::CardWindows), |ui| {
                self.window_sources_block(ui);
            });
        });
    }

    pub(crate) fn window_sources_block(&mut self, ui: &mut Ui) {
        // Register F0 is unstable and can be non-zero even with multi-view off,
        // so windows are shown only when a split is actually active.
        let split = matches!(
            self.multi_mode(),
            MultiMode::PictureInPicture | MultiMode::PictureByPicture
        );
        match catalog::window_sources(self.value(0xf0)).filter(|_| split) {
            Some((left, right)) => {
                ui.horizontal(|ui| {
                    stat(ui, self.t(Key::WindowLeft), &self.input_name_of(left));
                    ui.add_space(24.0);
                    stat(ui, self.t(Key::WindowRight), &self.input_name_of(right));
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(self.t(Key::WindowsUnknown))
                        .size(11.0)
                        .color(FAINT),
                );
            }
            None => {
                ui.label(
                    RichText::new(self.t(if self.multi_mode() == MultiMode::Off {
                        Key::NoSplit
                    } else {
                        Key::WindowsUnknown
                    }))
                    .size(12.5)
                    .color(DIM),
                );
            }
        }
    }

    pub(crate) fn mode_selector(&mut self, ui: &mut Ui) {
        let current = match self.multi_mode() {
            MultiMode::Off => 0,
            MultiMode::PictureInPicture => 1,
            MultiMode::PictureByPicture => 2,
            MultiMode::Unknown => 3,
        };
        let items = [
            self.t(Key::ModeSingle),
            self.t(Key::ModePip),
            self.t(Key::ModePbp),
        ];
        let selected = ui
            .add_enabled_ui(!self.busy && matches!(self.conn, Conn::Ok(_)), |ui| {
                segmented(ui, current, &items)
            })
            .inner;
        if let Some(i) = selected {
            match i {
                0 => {
                    self.send(Cmd::Apply(vec![(0xf2, 0)]));
                }
                1 => {
                    // PiP: enable multi-view; the monitor keeps its layout.
                    self.send(Cmd::Apply(vec![(0xf2, 1)]));
                }
                _ => {
                    let layout = self.value(0xf5).filter(|v| *v != 0).unwrap_or(0x0101);
                    self.send(Cmd::Apply(vec![(0xf5, layout), (0xf2, 1)]));
                }
            }
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(self.t(Key::ExperimentalMulti))
                .size(11.0)
                .color(FAINT),
        );
    }

    // ── page: picture ─────────────────────────────────────────────────────
}
