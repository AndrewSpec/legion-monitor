use crate::ui::*;

impl App {
    pub(crate) fn page_profiles(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavProfiles), self.t(Key::PrDesc));

        ui.columns(2, |cols| {
            for (i, slot) in ['A', 'B'].into_iter().enumerate() {
                let ui = &mut cols[i];
                let profile = if slot == 'A' {
                    self.store.slots.a.clone()
                } else {
                    self.store.slots.b.clone()
                };
                section_label(
                    ui,
                    &format!("{} {slot}", self.t(Key::ConfigWord).to_uppercase()),
                );
                let slot_title = format!("{} {slot}", self.t(Key::ConfigWord));
                card(ui, |ui| match &profile {
                    Some(p) => {
                        ui.horizontal(|ui| {
                            let badge = Rect::from_min_size(ui.cursor().min, Vec2::new(34.0, 34.0));
                            ui.allocate_exact_size(Vec2::new(34.0, 34.0), Sense::hover());
                            ui.painter().rect_filled(
                                badge,
                                CornerRadius::same(10),
                                ACCENT.gamma_multiply(0.18),
                            );
                            ui.painter().text(
                                badge.center(),
                                Align2::CENTER_CENTER,
                                slot.to_string(),
                                FontId::proportional(14.0),
                                ACCENT,
                            );
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&slot_title).size(13.5).strong().color(TEXT),
                                );
                                ui.label(
                                    RichText::new(self.tf(Key::SavedCount, p.values.len()))
                                        .size(11.0)
                                        .color(FAINT),
                                );
                            });
                        });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if accent_button(ui, self.t(Key::Apply)).clicked() {
                                self.apply_profile(p);
                            }
                            if small_button(ui, self.t(Key::Overwrite)).clicked() {
                                self.save_to_slot(slot);
                            }
                            if small_button(ui, self.t(Key::Delete)).clicked() {
                                if slot == 'A' {
                                    self.store.slots.a = None;
                                } else {
                                    self.store.slots.b = None;
                                }
                                self.persist(self.t(Key::ToastSlotDeleted).into());
                            }
                        });
                    }
                    None => {
                        ui.horizontal(|ui| {
                            let badge = Rect::from_min_size(ui.cursor().min, Vec2::new(34.0, 34.0));
                            ui.allocate_exact_size(Vec2::new(34.0, 34.0), Sense::hover());
                            ui.painter().rect_filled(badge, CornerRadius::same(10), BG);
                            ui.painter().text(
                                badge.center(),
                                Align2::CENTER_CENTER,
                                slot.to_string(),
                                FontId::proportional(14.0),
                                FAINT,
                            );
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(self.t(Key::SlotEmptyTitle))
                                        .size(13.5)
                                        .color(DIM),
                                );
                                ui.label(
                                    RichText::new(self.t(Key::SlotEmptyDesc))
                                        .size(11.0)
                                        .color(FAINT),
                                );
                            });
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(self.t(Key::SlotSaveHint))
                                .size(11.0)
                                .color(FAINT),
                        );
                        ui.add_space(12.0);
                        if accent_button(ui, self.t(Key::SaveCurrent)).clicked() {
                            self.save_to_slot(slot);
                        }
                    }
                });
            }
        });

        ui.add_space(20.0);
        card_titled(ui, self.t(Key::NamedProfiles), |ui| {
            let name_hint = self.t(Key::NamePlaceholder);
            ui.horizontal(|ui| {
                ui.add_sized(
                    [260.0, 30.0],
                    egui::TextEdit::singleline(&mut self.new_profile_name).hint_text(name_hint),
                );
                let can =
                    !self.new_profile_name.trim().is_empty() && !self.busy && !self.vals.is_empty();
                if accent_button(ui, self.t(Key::SaveAsProfile)).clicked() && can {
                    let p = Profile {
                        id: profiles::next_id(&self.store),
                        name: self.new_profile_name.trim().to_string(),
                        values: self.snapshot_values(),
                    };
                    let count = p.values.len();
                    self.store.profiles.push(p);
                    self.new_profile_name.clear();
                    self.persist(self.tf(Key::ToastSaved, count));
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(self.t(Key::ProfilesFileHint))
                    .size(10.5)
                    .color(FAINT),
            );
        });

        if self.store.profiles.is_empty() {
            ui.add_space(12.0);
            empty_state(ui, self.t(Key::NoProfiles));
        } else {
            ui.add_space(12.0);
            let list: Vec<Profile> = self.store.profiles.clone();
            for p in list {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&p.name).size(13.5).strong().color(TEXT));
                            ui.label(
                                RichText::new(self.tf(Key::SavedCount, p.values.len()))
                                    .size(11.0)
                                    .color(FAINT),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_button(ui, self.t(Key::Delete)).clicked() {
                                self.store.profiles.retain(|x| x.id != p.id);
                                self.persist(self.t(Key::ToastProfileDeleted).into());
                            }
                            if small_button(ui, self.t(Key::Overwrite)).clicked()
                                && !self.busy
                                && !self.vals.is_empty()
                            {
                                let snapshot = self.snapshot_values();
                                if let Some(item) =
                                    self.store.profiles.iter_mut().find(|x| x.id == p.id)
                                {
                                    item.values = snapshot;
                                }
                                self.persist(
                                    self.tf(Key::ToastSaved, self.snapshot_values().len()),
                                );
                            }
                            if accent_button(ui, self.t(Key::Apply)).clicked() {
                                self.apply_profile(&p);
                            }
                        });
                    });
                });
                ui.add_space(10.0);
            }
        }
    }

    // ── page: advanced ────────────────────────────────────────────────────
}
