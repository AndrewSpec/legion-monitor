use crate::ui::*;

impl App {
    pub(crate) fn page_overview(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavOverview), self.t(Key::OvDesc));

        let mode = self.multi_mode();
        let mode_label = if self.true_split_active() {
            Key::ModeTrueSplit
        } else {
            mode.key()
        };
        let bright = self.value(0x10);
        let contrast = self.value(0x12);

        card(ui, |ui| {
            ui.horizontal(|ui| {
                stat(ui, self.t(Key::OvMode), self.t(mode_label));
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

        // The source and split state are drawn inside the monitor instead of
        // listed as text, so the graphic is the primary source of truth.
        self.screen_card(ui);

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
    }

    pub(crate) fn mode_selector(&mut self, ui: &mut Ui) {
        let mode = self.multi_mode();
        let true_split = self.true_split_active();
        // True Split wins over the raw F4 reading: immediately after the
        // split F4 can still be the pre-split value while the two external
        // outputs already identify the mode.
        let current = if true_split {
            3
        } else {
            match mode {
                MultiMode::Off => 0,
                MultiMode::PictureInPicture => 1,
                MultiMode::PictureByPicture => 2,
                MultiMode::Unknown => 4,
            }
        };
        // True Split is a first-class option next to Single/PIP/PBP. It only
        // exists on a USB-C link, so keep the tile visible but disabled
        // otherwise. The worker raises the link to 60 Hz after the split.
        let usbc = matches!(self.current_input(), Some(0x1f | 0x31..=0x3f));
        let pip_blocked = self.pip_refresh_too_high().is_some();
        let items = [
            (self.t(Key::ModeSingle), true),
            (self.t(Key::ModePip), !pip_blocked),
            (self.t(Key::ModePbp), true),
            (self.t(Key::ModeTrueSplit), usbc || true_split),
        ];
        let selected = ui
            .add_enabled_ui(!self.busy && matches!(self.conn, Conn::Ok(_)), |ui| {
                segmented(ui, current, &items)
            })
            .inner;
        if let Some(i) = selected {
            match i {
                0 => {
                    self.win_sources = [0, 0];
                    self.mode_hint = Some(MultiMode::Off);
                    // Always name our input: closing True Split re-enumerates
                    // the link, and without 0x60 the monitor can come back on
                    // its default source instead of this computer. The plan
                    // clears F8/F7/F5 first so the monitor OSD also leaves
                    // True Split, not just the PIP/PBP window.
                    let source = self.current_input();
                    if source.is_none() {
                        self.toast = Some((
                            self.t(Key::ErrSelfSourceUnknown).to_string(),
                            true,
                            Instant::now(),
                        ));
                    } else {
                        let pbp = mode == MultiMode::PictureByPicture;
                        self.send(Cmd::DisableMultiView { source, pbp });
                    }
                }
                1 => {
                    // PIP is placed with the monitor's window rectangle, not
                    // with F5. The worker leaves True Split or PBP first and
                    // checks that it closed before writing the PIP selector.
                    let corner = self.pip_corner_from_registers();
                    self.enter_pip(corner);
                }
                2 => {
                    // PBP on this monitor is only 50/50; the two half sources
                    // are picked on the Multi-view page after it opens.
                    let layout = catalog::SplitLayout::Half.value();
                    self.win_sources = [0, 0];
                    self.mode_hint = Some(MultiMode::PictureByPicture);
                    self.send(Cmd::SetPbp {
                        layout: Some(layout),
                        source: self.pbp_entry_source(),
                    });
                }
                3 => {
                    if !true_split {
                        if !usbc {
                            self.toast = Some((
                                self.t(Key::ErrTrueSplitUsbC).to_string(),
                                true,
                                Instant::now(),
                            ));
                        } else {
                            // Close PIP/PBP before the split; the worker then
                            // activates True Split and lowers both screens.
                            if mode != MultiMode::Off {
                                let source = self.current_input();
                                self.send(Cmd::DisableMultiView {
                                    source,
                                    pbp: mode == MultiMode::PictureByPicture,
                                });
                            }
                            self.win_sources = [0, 0];
                            self.mode_hint = Some(MultiMode::PictureByPicture);
                            self.set_feature(catalog::TRUE_SPLIT_FEATURE, 1);
                        }
                    }
                }
                _ => {}
            }
        }
        // On the Multi-view page the PIP card carries the same explanation;
        // repeating it here would duplicate one sentence on a single screen.
        if self.section != catalog::Section::MultiView {
            if let Some(note) = self.refresh_note() {
                ui.add_space(8.0);
                ui.label(RichText::new(note).size(11.0).color(WARN));
            }
        }
        if matches!(
            self.multi_mode(),
            MultiMode::PictureInPicture | MultiMode::PictureByPicture
        ) {
            ui.add_space(10.0);
            self.self_source_row(ui);
            ui.add_space(4.0);
            let note = if !self.displays.known {
                Key::DisplaysUnknownNote
            } else if self.displays.has_second_screen() {
                Key::SecondScreenNote
            } else {
                Key::NoSecondScreenNote
            };
            ui.label(RichText::new(self.t(note)).size(11.0).color(FAINT));
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
