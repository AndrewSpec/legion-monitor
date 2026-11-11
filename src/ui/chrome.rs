use crate::ui::*;

impl App {
    pub(crate) fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let (logo, _) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(logo, CornerRadius::same(11), ACCENT.gamma_multiply(0.16));
            p.rect_stroke(
                logo,
                CornerRadius::same(11),
                Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.45)),
                StrokeKind::Inside,
            );
            draw_icon(p, logo.center(), 9.0, Section::MultiView, ACCENT);

            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(crate::about::NAME)
                        .size(15.5)
                        .strong()
                        .color(TEXT),
                );
                let sub = match &self.conn {
                    Conn::Ok(m) => {
                        let mut parts = vec![m.title(&self.tr)];
                        if !m.connector.is_empty() {
                            parts.push(m.connector.clone());
                        }
                        if !m.serial.is_empty() {
                            parts.push(m.serial.clone());
                        }
                        parts.join("  ·  ")
                    }
                    Conn::Probing => self.t(Key::AppScanning).to_string(),
                    Conn::Err(_) => self.t(Key::AppOffline).to_string(),
                };
                ui.label(RichText::new(sub).size(11.0).color(FAINT));
            });
            ui.add_space(16.0);
            self.connection_chip(ui);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let refresh = ui.add(icon_button("⟳"));
                if refresh.on_hover_text(self.t(Key::Refresh)).clicked() && !self.busy {
                    self.send(Cmd::Refresh);
                }
                ui.add_space(10.0);
                self.slot_button(ui, 'B');
                ui.add_space(8.0);
                self.slot_button(ui, 'A');
                ui.add_space(14.0);
                self.language_picker(ui);
            });
        });
    }

    pub(crate) fn connection_chip(&mut self, ui: &mut Ui) {
        let (text, color) = match &self.conn {
            Conn::Probing => (self.t(Key::StatusConnecting).to_string(), WARN),
            Conn::Ok(_) => (self.t(Key::StatusConnected).to_string(), OK),
            Conn::Err(_) => (self.t(Key::StatusOffline).to_string(), FAINT),
        };
        chip(ui, &text, color);
        if self.busy {
            ui.add_space(8.0);
            ui.add(egui::Spinner::new().size(13.0));
        }
    }

    pub(crate) fn slot_button(&mut self, ui: &mut Ui, slot: char) {
        let profile = if slot == 'A' {
            self.store.slots.a.clone()
        } else {
            self.store.slots.b.clone()
        };
        let filled = profile.is_some();
        let active = !self.busy
            && matches!(self.conn, Conn::Ok(_))
            && profile
                .as_ref()
                .is_some_and(|p| profiles::matches_observed(&p.values, &self.vals));
        let sub = profile
            .as_ref()
            .map(|p| {
                // Slot profiles are named in the current language, not from the
                // saved JSON, so a language switch updates the label too.
                if p.id == format!("slot{}", slot.to_ascii_lowercase()) {
                    self.t(Key::SlotSaved).to_string()
                } else {
                    p.name.clone()
                }
            })
            .unwrap_or_else(|| self.t(Key::SlotEmpty).to_string());

        let (rect, resp) = ui.allocate_exact_size(Vec2::new(140.0, 40.0), Sense::click());
        let bg = if active {
            ACCENT.gamma_multiply(0.2)
        } else if resp.hovered() {
            CARD_HI
        } else {
            CARD
        };
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(10), bg);
        p.rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(
                1.0_f32,
                if active {
                    ACCENT
                } else if filled {
                    BORDER
                } else {
                    BORDER.gamma_multiply(0.6)
                },
            ),
            StrokeKind::Inside,
        );
        // A/B letter in a circle
        let badge =
            Rect::from_center_size(rect.left_center() + Vec2::new(20.0, 0.0), Vec2::splat(22.0));
        p.rect_filled(
            badge,
            CornerRadius::same(7),
            if filled {
                ACCENT.gamma_multiply(0.25)
            } else {
                BG
            },
        );
        p.text(
            badge.center(),
            Align2::CENTER_CENTER,
            slot.to_string(),
            FontId::proportional(12.0),
            if filled { ACCENT } else { FAINT },
        );
        p.text(
            rect.left_center() + Vec2::new(38.0, -7.0),
            Align2::LEFT_CENTER,
            format!("{} {slot}", self.t(Key::ConfigWord)),
            FontId::proportional(12.0),
            if filled { TEXT } else { DIM },
        );
        p.text(
            rect.left_center() + Vec2::new(38.0, 8.0),
            Align2::LEFT_CENTER,
            sub,
            FontId::proportional(10.5),
            FAINT,
        );
        if resp.clicked() {
            if let Some(pr) = profile.clone() {
                self.apply_profile(&pr);
            } else {
                self.save_to_slot(slot);
            }
        }
        let key = if slot == 'A' { 1 } else { 2 };
        resp.on_hover_text(if filled {
            format!("{} {} (F{key})", self.t(Key::Apply), slot)
        } else {
            self.tf(Key::SaveSlotTip, slot)
        });
    }

    // ── navigation ────────────────────────────────────────────────────────
    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(
            RichText::new(self.t(Key::NavSettings))
                .size(10.5)
                .color(FAINT)
                .strong(),
        );
        ui.add_space(10.0);

        for section in Section::ALL {
            if nav_item(ui, section, self.section == section, &self.tr) {
                self.section = section;
            }
            ui.add_space(2.0);
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "{}\n{}",
                    self.t(Key::FooterControl),
                    self.t(Key::FooterShortcuts)
                ))
                .size(10.5)
                .color(FAINT),
            );
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            if nav_item(ui, Section::About, self.section == Section::About, &self.tr) {
                self.section = Section::About;
            }
            ui.add_space(2.0);
            if nav_item(
                ui,
                Section::Advanced,
                self.section == Section::Advanced,
                &self.tr,
            ) {
                self.section = Section::Advanced;
            }
        });
    }

    /// Language picker — each language is named in its own language, so it can
    /// be found without knowing the current one. It lives in the top bar because
    /// the list opens downwards there and has room for every entry.
    pub(crate) fn language_picker(&mut self, ui: &mut Ui) {
        let current = self.lang.code();
        let chosen = LANGS
            .iter()
            .find(|(l, _)| l.code() == current)
            .map(|(_, name)| *name)
            .unwrap_or("English");
        egui::ComboBox::from_id_salt("lang")
            .selected_text(RichText::new(chosen).size(12.0).color(TEXT))
            .width(150.0)
            .show_ui(ui, |ui| {
                for (lang, name) in LANGS {
                    let selected = *lang == self.lang;
                    if ui
                        .selectable_label(selected, RichText::new(*name).size(12.5))
                        .clicked()
                    {
                        self.set_lang(*lang);
                    }
                }
            });
        ui.add_space(2.0);
    }

    pub(crate) fn connected(&self) -> bool {
        matches!(self.conn, Conn::Ok(_))
    }

    pub(crate) fn disconnected_banner(&mut self, ui: &mut Ui, msg: &str) {
        Frame::NONE
            .fill(WARN.gamma_multiply(0.08))
            .corner_radius(CornerRadius::same(14))
            .stroke(Stroke::new(1.0_f32, WARN.gamma_multiply(0.35)))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (icon, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                    let p = ui.painter();
                    p.circle_stroke(icon.center(), 8.0, Stroke::new(1.6_f32, WARN));
                    p.text(
                        icon.center(),
                        Align2::CENTER_CENTER,
                        "!",
                        FontId::proportional(11.0),
                        WARN,
                    );
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(self.t(Key::BannerTitle))
                                .size(13.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(RichText::new(self.t(Key::BannerBody)).size(11.5).color(DIM));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, self.t(Key::BannerRetry)).clicked() {
                            self.conn = Conn::Probing;
                            self.send(Cmd::Init);
                        }
                    });
                });
                if !msg.trim().is_empty() {
                    ui.add_space(6.0);
                    let short = self.t(friendly_error(msg));
                    ui.label(RichText::new(short).size(10.5).color(FAINT));
                    egui::CollapsingHeader::new(
                        RichText::new(self.t(Key::Details)).size(10.0).color(FAINT),
                    )
                    .id_salt("conn-details")
                    .show(ui, |ui| {
                        ui.label(RichText::new(msg.trim()).size(10.0).color(FAINT));
                    });
                }
            });
        ui.add_space(16.0);
    }

    // ── page: overview ────────────────────────────────────────────────────

    pub(crate) fn toast_overlay(&mut self, ctx: &egui::Context) {
        let Some((msg, is_err, at)) = self.toast.clone() else {
            return;
        };
        if at.elapsed() > Duration::from_millis(3600) {
            self.toast = None;
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -22.0))
            .interactable(false)
            .show(ctx, |ui| {
                let color = if is_err { ERR } else { OK };
                Frame::NONE
                    .fill(CARD)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(14, 9))
                    .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.6)))
                    .show(ui, |ui| {
                        ui.label(RichText::new(msg).size(12.5).color(color));
                    });
            });
        let _ = screen;
    }
}
