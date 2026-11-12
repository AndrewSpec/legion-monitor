use crate::ui::*;

impl App {
    pub(crate) fn page_rescue(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavRescue), self.t(Key::RescueDesc));

        let base = self.settings.rescue.clone();
        let connected = self.connected();
        let busy = self.busy;
        card_titled(ui, self.t(Key::CardRescueBase), |ui| {
            match &base {
                Some(base) => {
                    ui.horizontal(|ui| {
                        stat(
                            ui,
                            self.t(Key::RescueSource),
                            &self.input_name_of(base.source),
                        );
                        ui.add_space(44.0);
                        stat(ui, self.t(Key::OvMode), self.t(Key::ModeSingle));
                        ui.add_space(44.0);
                        chip(ui, self.t(Key::RescueSaved), OK);
                    });
                }
                None => {
                    ui.label(
                        RichText::new(self.t(Key::RescueEmpty))
                            .size(12.0)
                            .color(DIM),
                    );
                }
            }
            ui.add_space(14.0);
            let save = ui
                .add_enabled_ui(connected && !busy, |ui| {
                    accent_button(ui, self.t(Key::RescueSave))
                })
                .inner
                .clicked();
            if save {
                self.save_rescue_baseline(true);
            }
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::CardRescueRestore), |ui| {
            ui.label(RichText::new(self.t(Key::RescueHint)).size(11.5).color(DIM));
            ui.add_space(12.0);

            let mut armed = self.settings.rescue_armed;
            let toggled = ui
                .add_enabled_ui(base.is_some(), |ui| {
                    ui.checkbox(&mut armed, self.t(Key::RescueArm))
                })
                .inner
                .changed();
            if toggled {
                self.settings.rescue_armed = armed;
                if let Err(error) = self.settings.save() {
                    let text = self.tf(Key::ErrSaveSettings, error);
                    self.toast = Some((text, true, Instant::now()));
                }
                if armed {
                    // Probe at once so a monitor that appears right after
                    // arming is noticed without waiting for the next tick.
                    // Do not restore while the picture is still here: arming
                    // must not close a working split. "Restore now" does that
                    // explicitly, and a reconnect triggers the automatic pass.
                    self.rescue_probe = Instant::now() - Duration::from_secs(3);
                }
            }
            if armed && !self.rescue_pending {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(self.t(Key::RescueArmedWait))
                        .size(11.0)
                        .color(ACCENT),
                );
            }

            ui.add_space(12.0);
            let restore = ui
                .add_enabled_ui(connected && !busy, |ui| {
                    accent_button(ui, self.t(Key::RescueRestoreNow))
                })
                .inner
                .clicked();
            if restore {
                self.restore_rescue();
            }

            if !connected {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(self.t(Key::RescueNoMonitor))
                        .size(11.0)
                        .color(WARN),
                );
            }
            if let Some((key, error)) = self.rescue_status {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(self.t(key))
                        .size(11.5)
                        .color(if error { ERR } else { OK }),
                );
            }
        });
    }
}
