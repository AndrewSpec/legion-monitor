use crate::ui::*;

impl App {
    pub(crate) fn page_advanced(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavAdvanced), self.t(Key::AdvDesc));

        // The "Monitor features" card shows only features we know by name —
        // the rest (vendor and unknown codes) is hidden behind a toggle.
        let mut advanced: Vec<u8> = self
            .codes_in(Section::Advanced)
            .into_iter()
            .filter(|c| catalog::desc_for(*c).is_some() && *c != 0x04)
            .collect();
        if advanced.is_empty() {
            advanced = vec![0x62, 0x8d, 0xd6, 0xcc];
        }

        card_titled(ui, self.t(Key::CardMonitor), |ui| {
            for (i, code) in advanced.iter().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                }
                match *code {
                    0x62 => self.slider_row(ui, *code, self.t(Key::VolLabel), ""),
                    0x8d => self.toggle_row(ui, *code, self.t(Key::MuteLabel), ""),
                    0xd6 => {
                        self.choice_row(ui, *code, self.t(Key::PowerLabel), catalog::power_modes())
                    }
                    other => self.auto_row(ui, other),
                }
            }
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardVendor), |ui| {
            ui.horizontal(|ui| {
                let mut on = self.show_vendor;
                if ui.checkbox(&mut on, self.t(Key::ShowVendor)).changed() {
                    self.show_vendor = on;
                }
            });
            if self.show_vendor {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(self.t(Key::VendorWarn))
                        .size(11.0)
                        .color(FAINT),
                );
                ui.add_space(10.0);
                let vendor: Vec<u8> = self
                    .vals
                    .keys()
                    .copied()
                    .filter(|c| catalog::desc_for(*c).is_none())
                    .collect();
                if vendor.is_empty() {
                    ui.label(RichText::new(self.t(Key::NoVendor)).size(12.0).color(FAINT));
                }
                for code in vendor {
                    self.auto_row(ui, code);
                }
            }
        });

        // Technical information — raw diagnostic readings from the monitor.
        let mut info: Vec<(u8, u16)> = self
            .vals
            .iter()
            .filter(|(c, _)| catalog::is_info(**c))
            .map(|(c, v)| (*c, v.value))
            .collect();
        info.sort_by_key(|(c, _)| *c);
        if !info.is_empty() {
            ui.add_space(16.0);
            let title = RichText::new(self.t(Key::InfoTitle))
                .size(12.5)
                .strong()
                .color(TEXT);
            egui::CollapsingHeader::new(title)
                .id_salt("technical-info")
                .default_open(false)
                .show(ui, |ui| {
                    let win = win_pos(&info);
                    for (code, value) in &info {
                        // The four window-geometry codes are combined into one row.
                        if matches!(code, 0x95..=0x98) {
                            continue;
                        }
                        if let Some(key) = catalog::info_key(*code) {
                            info_row(ui, self.t(key), &info_value(*code, *value));
                        }
                    }
                    if let Some((x1, y1, x2, y2)) = win {
                        info_row(
                            ui,
                            self.t(Key::InfoWinPos),
                            &format!("{x1},{y1} – {x2},{y2}"),
                        );
                    }
                });
        }

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardReset), |ui| {
            ui.label(RichText::new(self.t(Key::ResetDesc)).size(12.0).color(DIM));
            ui.add_space(10.0);
            if !self.confirm_reset {
                if small_button(ui, self.t(Key::ResetBtn)).clicked() {
                    self.confirm_reset = true;
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.t(Key::ResetConfirm))
                            .size(12.5)
                            .color(WARN),
                    );
                    if danger_button(ui, self.t(Key::ResetYes)).clicked() {
                        self.confirm_reset = false;
                        self.send(Cmd::Apply(vec![(0x04, 1)]));
                    }
                    if small_button(ui, self.t(Key::Cancel)).clicked() {
                        self.confirm_reset = false;
                    }
                });
            }
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardLog), |ui| {
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.log.is_empty() {
                        ui.label(RichText::new(self.t(Key::LogEmpty)).size(11.5).color(FAINT));
                    }
                    for line in self.log.iter().rev().take(12).rev() {
                        ui.label(RichText::new(self.log_text(line)).size(11.0).color(FAINT));
                    }
                });
        });
    }

    // ── VCP code helpers ─────────────────────────────────────────────────
}

/// "Label — value" row in the technical information card.
fn info_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.0).color(TEXT));
        });
    });
}

/// Hardware/protocol versions are "major.minor" encoded in two bytes.
fn info_value(code: u8, value: u16) -> String {
    match code {
        0xdf | 0xc9 => {
            let major = value >> 8;
            let minor = value & 0xff;
            if minor == 0 {
                format!("{major}")
            } else {
                format!("{major}.{minor:02}")
            }
        }
        _ => value.to_string(),
    }
}

/// Split window geometry: TL_X, TL_Y, BR_X, BR_Y.
fn win_pos(info: &[(u8, u16)]) -> Option<(u16, u16, u16, u16)> {
    let get = |code: u8| info.iter().find(|(c, _)| *c == code).map(|(_, v)| *v);
    Some((get(0x95)?, get(0x96)?, get(0x97)?, get(0x98)?))
}
