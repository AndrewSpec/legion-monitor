//! About page — application version, author and project links.
//!
//! Everything shown here comes from `crate::about`, which reads Cargo
//! metadata, so the page cannot drift from the release that was built.
//! The version is also the value a future update check compares with the
//! latest GitHub release tag.

use crate::about;
use crate::ui::*;

impl App {
    pub(crate) fn page_about(&mut self, ui: &mut Ui) {
        page_header(ui, self.t(Key::NavAbout), self.t(Key::AbtDesc));

        card_titled(ui, self.t(Key::AbtCardApp), |ui| {
            ui.horizontal(|ui| {
                let (mark, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(mark, CornerRadius::same(12), ACCENT.gamma_multiply(0.16));
                p.rect_stroke(
                    mark,
                    CornerRadius::same(12),
                    Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.45)),
                    StrokeKind::Inside,
                );
                draw_icon(p, mark.center(), 11.0, Section::About, ACCENT);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(about::NAME).size(16.0).strong().color(TEXT));
                    ui.label(
                        RichText::new(self.t(Key::AbtTagline))
                            .size(11.5)
                            .color(FAINT),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    chip(ui, &format!("v{}", about::VERSION), ACCENT);
                });
            });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            info_row(ui, self.t(Key::AbtAuthor), &about::display_authors());
            info_row(ui, self.t(Key::AbtLicense), about::LICENSE);
            link_row(
                ui,
                self.t(Key::AbtSource),
                about::display_repository(),
                about::REPOSITORY,
            );
        });

        ui.add_space(16.0);
        card_titled(ui, self.t(Key::AbtCardUpdates), |ui| {
            ui.label(
                RichText::new(self.t(Key::AbtUpdatesBody))
                    .size(12.0)
                    .color(DIM),
            );
            ui.add_space(10.0);
            ui.add(egui::Hyperlink::from_label_and_url(
                RichText::new(self.t(Key::AbtReleases)).size(12.0),
                about::RELEASES_URL,
            ));
        });
    }
}

/// Row whose value is an external link — used for the repository address.
fn link_row(ui: &mut Ui, label: &str, text: &str, url: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::Hyperlink::from_label_and_url(
                RichText::new(text).size(12.0),
                url,
            ));
        });
    });
}
