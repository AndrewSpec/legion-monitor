//! The overview's visual monitor: bezel, stand and the source layout drawn
//! inside the panel. The shape comes from `panel`, so a 16:9, 21:9 and 32:9
//! monitor each look different instead of sharing one generic rectangle.

use crate::panel::{Panel, PanelSource};
use crate::ui::*;

/// Left window's share of the width and whether the monitor confirms that
/// geometry. Only the two layouts verified on the R45w-30 claim a real split;
/// every other code is drawn as an even, explicitly approximate split.
fn split_share(layout: Option<u16>) -> (f32, bool) {
    match layout {
        Some(0x0101) => (0.5, true),
        Some(0x0401) => (0.75, true),
        _ => (0.5, false),
    }
}

#[derive(Clone, Copy)]
struct Scene {
    mode: MultiMode,
    pair: Option<(u16, u16)>,
    single: Option<u16>,
    self_source: Option<u16>,
    share: f32,
    connected: bool,
}

impl App {
    /// Replaces the old textual source card on the overview.
    pub(crate) fn screen_card(&mut self, ui: &mut Ui) {
        let panel = self.panel;
        let mode = self.multi_mode();
        let split = matches!(
            mode,
            MultiMode::PictureInPicture | MultiMode::PictureByPicture
        );
        let pair = self.split_pair().filter(|_| split);
        let single = self.value(0x60).filter(|value| *value <= 0xff);
        let (share, exact) = split_share(self.value(0xf5));
        let connected = matches!(self.conn, Conn::Ok(_));
        let scene = Scene {
            mode,
            pair,
            single,
            self_source: self.self_source.filter(|_| split),
            share,
            connected,
        };

        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t(Key::CardScreen))
                        .size(12.5)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let shape = match panel.native {
                        Some((width, height)) => {
                            format!("{} · {width} × {height}", panel.aspect_label())
                        }
                        None => panel.aspect_label(),
                    };
                    let color = if panel.source == PanelSource::Default {
                        FAINT
                    } else {
                        ACCENT
                    };
                    chip(ui, &shape, color);
                });
            });
            ui.add_space(2.0);
            ui.label(RichText::new(self.panel_note()).size(10.5).color(FAINT));
            ui.add_space(10.0);

            self.monitor_scene(ui, panel, scene);

            if split && !exact {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(self.t(Key::ScreenApproxSplit))
                        .size(11.0)
                        .color(WARN),
                );
            }
            if split && pair.is_none() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(self.t(Key::WindowsUnknown))
                        .size(11.0)
                        .color(DIM),
                );
            }

            ui.add_space(14.0);
            self.mode_selector(ui);
        });
    }

    fn monitor_scene(&self, ui: &mut Ui, panel: Panel, scene: Scene) {
        let available = ui.available_width().max(260.0);
        let ratio = panel.ratio.clamp(1.2, 4.0);
        let max_height = 270.0;
        // Keep a normal monitor from becoming taller than the card while an
        // ultrawide still uses the whole width.
        let screen_width = (available - 40.0).max(180.0).min(max_height * ratio);
        let screen_height = screen_width / ratio;
        let outer_width = screen_width + 20.0;
        let outer_height = screen_height + 26.0;
        let neck_height = 24.0;
        let base_height = 9.0;
        let stand_gap = 6.0;
        let total_height = outer_height + stand_gap + neck_height + base_height + 2.0;

        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), total_height),
            Sense::hover(),
        );
        let painter = ui.painter_at(canvas);
        let center_x = canvas.center().x;
        let top = canvas.top() + 1.0;

        // Stand, drawn first so the bezel can overlap its neck. A trapezoid
        // reads as a monitor stand; a plain rectangle looks like a speaker.
        let neck_top = top + outer_height + stand_gap;
        let neck_bottom = neck_top + neck_height;
        let neck = egui::Shape::convex_polygon(
            vec![
                Pos2::new(center_x - 18.0, neck_top),
                Pos2::new(center_x + 18.0, neck_top),
                Pos2::new(center_x + 28.0, neck_bottom),
                Pos2::new(center_x - 28.0, neck_bottom),
            ],
            Color32::from_rgb(0x0b, 0x0f, 0x13),
            Stroke::new(1.0_f32, BORDER),
        );
        let base_width = (screen_width * 0.28).clamp(120.0, 240.0);
        let base = Rect::from_center_size(
            Pos2::new(center_x, canvas.bottom() - base_height / 2.0 - 1.0),
            Vec2::new(base_width, base_height),
        );
        let stand_color = Color32::from_rgb(0x0b, 0x0f, 0x13);
        painter.add(neck);
        painter.rect_filled(base, CornerRadius::same(4), stand_color);
        painter.rect_stroke(
            base,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, BORDER),
            StrokeKind::Inside,
        );

        // Bezel and screen.
        let outer = Rect::from_center_size(
            Pos2::new(center_x, top + outer_height / 2.0),
            Vec2::new(outer_width, outer_height),
        );
        painter.rect_filled(
            outer,
            CornerRadius::same(14),
            Color32::from_rgb(0x07, 0x0a, 0x0f),
        );
        painter.rect_stroke(
            outer,
            CornerRadius::same(14),
            Stroke::new(1.0_f32, BORDER),
            StrokeKind::Inside,
        );
        let screen = Rect::from_min_max(
            outer.min + Vec2::new(10.0, 10.0),
            outer.max - Vec2::new(10.0, 16.0),
        );
        painter.rect_filled(
            screen,
            CornerRadius::same(7),
            Color32::from_rgb(0x0b, 0x0f, 0x15),
        );
        painter.rect_stroke(
            screen,
            CornerRadius::same(7),
            Stroke::new(1.0_f32, BORDER),
            StrokeKind::Inside,
        );
        painter.circle_filled(
            Pos2::new(center_x, outer.bottom() - 8.0),
            2.0,
            ACCENT.gamma_multiply(0.9),
        );

        let inner = screen.shrink(5.0);
        let unknown_note = if scene.connected {
            Key::WindowsUnknown
        } else {
            Key::AppOffline
        };

        match scene.mode {
            MultiMode::Off => {
                self.draw_pane(
                    &painter,
                    inner,
                    scene.single,
                    false,
                    Some(self.t(Key::ScreenFull)),
                );
            }
            MultiMode::PictureByPicture => match scene.pair {
                Some((left, right)) => {
                    let gap = 4.0;
                    let divider = inner.left() + inner.width() * scene.share;
                    let left_rect = Rect::from_min_max(
                        inner.min,
                        Pos2::new(divider - gap / 2.0, inner.bottom()),
                    );
                    let right_rect =
                        Rect::from_min_max(Pos2::new(divider + gap / 2.0, inner.top()), inner.max);
                    self.draw_pane(
                        &painter,
                        left_rect,
                        Some(left),
                        scene.self_source == Some(left),
                        Some(self.t(Key::WindowLeft)),
                    );
                    self.draw_pane(
                        &painter,
                        right_rect,
                        Some(right),
                        scene.self_source == Some(right),
                        Some(self.t(Key::WindowRight)),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(divider, inner.top()),
                            Pos2::new(divider, inner.bottom()),
                        ],
                        Stroke::new(1.0_f32, BORDER),
                    );
                }
                None => self.draw_unknown(&painter, inner, unknown_note),
            },
            MultiMode::PictureInPicture => match scene.pair {
                Some((main, pip)) => {
                    self.draw_pane(&painter, inner, Some(main), false, None);
                    let inset_width = (inner.width() * 0.32).clamp(110.0, 260.0);
                    let inset_height = inset_width * 9.0 / 16.0;
                    let inset = Rect::from_min_size(
                        Pos2::new(inner.right() - inset_width - 12.0, inner.top() + 12.0),
                        Vec2::new(inset_width, inset_height),
                    );
                    painter.rect_filled(
                        inset.expand(2.0),
                        CornerRadius::same(7),
                        Color32::from_rgb(0x05, 0x07, 0x0a),
                    );
                    self.draw_pane(
                        &painter,
                        inset,
                        Some(pip),
                        scene.self_source == Some(pip),
                        Some(self.t(Key::ModePip)),
                    );
                }
                None => self.draw_unknown(&painter, inner, unknown_note),
            },
            MultiMode::Unknown => self.draw_unknown(&painter, inner, unknown_note),
        }
    }

    fn draw_pane(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        source: Option<u16>,
        self_here: bool,
        sublabel: Option<&str>,
    ) {
        let p = painter.with_clip_rect(rect);
        let background = if self_here {
            ACCENT.gamma_multiply(0.14)
        } else {
            Color32::from_rgb(0x14, 0x19, 0x22)
        };
        p.rect_filled(rect, CornerRadius::same(5), background);
        if self_here {
            p.rect_stroke(
                rect,
                CornerRadius::same(5),
                Stroke::new(1.2_f32, ACCENT.gamma_multiply(0.7)),
                StrokeKind::Inside,
            );
        }

        if let Some(code) = source {
            let name = self.input_name_of(code);
            let name_size = if rect.width() < 150.0 || rect.height() < 90.0 {
                12.0
            } else {
                16.0
            };
            let name_galley = p.layout(
                name,
                FontId::proportional(name_size),
                TEXT,
                (rect.width() - 24.0).max(40.0),
            );
            let sub_height = if sublabel.is_some() { 18.0 } else { 0.0 };
            let top = rect.center().y - (name_galley.size().y + sub_height) / 2.0;
            p.galley(
                Pos2::new(rect.center().x - name_galley.size().x / 2.0, top),
                name_galley.clone(),
                TEXT,
            );
            if let Some(sub) = sublabel {
                p.text(
                    Pos2::new(rect.center().x, top + name_galley.size().y + 4.0),
                    Align2::CENTER_TOP,
                    sub,
                    FontId::proportional(10.5),
                    FAINT,
                );
            }
        } else {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "—",
                FontId::proportional(20.0),
                FAINT,
            );
        }

        if self_here {
            self_badge(
                &p,
                rect.left_top() + Vec2::new(8.0, 8.0),
                self.t(Key::ScreenThisPc),
            );
        }
    }

    fn draw_unknown(&self, painter: &egui::Painter, rect: Rect, note: Key) {
        let p = painter.with_clip_rect(rect);
        p.text(
            Pos2::new(rect.center().x, rect.center().y - 14.0),
            Align2::CENTER_CENTER,
            "—",
            FontId::proportional(24.0),
            FAINT,
        );
        let galley = p.layout(
            self.t(note).to_string(),
            FontId::proportional(11.5),
            DIM,
            (rect.width() - 40.0).max(60.0),
        );
        p.galley(
            Pos2::new(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y + 8.0,
            ),
            galley,
            DIM,
        );
    }
}

fn self_badge(painter: &egui::Painter, pos: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(9.5), ACCENT);
    let rect = Rect::from_min_size(pos, galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(rect, CornerRadius::same(6), ACCENT.gamma_multiply(0.18));
    painter.galley(pos + Vec2::new(6.0, 3.0), galley, ACCENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_layouts_have_their_shares() {
        assert_eq!(split_share(Some(0x0101)), (0.5, true));
        assert_eq!(split_share(Some(0x0401)), (0.75, true));
    }

    #[test]
    fn unknown_layouts_are_drawn_even_and_flagged() {
        assert_eq!(split_share(None), (0.5, false));
        assert_eq!(split_share(Some(0x0201)), (0.5, false));
        assert_eq!(split_share(Some(0x0302)), (0.5, false));
    }
}
