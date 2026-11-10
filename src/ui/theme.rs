use crate::ui::*;

pub(crate) const BG: Color32 = Color32::from_rgb(0x0d, 0x10, 0x15);
pub(crate) const SIDEBAR: Color32 = Color32::from_rgb(0x13, 0x17, 0x1e);
pub(crate) const CARD: Color32 = Color32::from_rgb(0x1a, 0x1f, 0x29);
pub(crate) const CARD_HI: Color32 = Color32::from_rgb(0x23, 0x2a, 0x36);
pub(crate) const BORDER: Color32 = Color32::from_rgb(0x2c, 0x34, 0x43);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xe9, 0xec, 0xf2);
pub(crate) const DIM: Color32 = Color32::from_rgb(0x92, 0x9b, 0xab);
pub(crate) const FAINT: Color32 = Color32::from_rgb(0x67, 0x70, 0x80);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x5b, 0x8c, 0xff);
pub(crate) const OK: Color32 = Color32::from_rgb(0x3d, 0xd6, 0x8c);
pub(crate) const WARN: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
pub(crate) const ERR: Color32 = Color32::from_rgb(0xf2, 0x5d, 0x5d);

pub(crate) fn nav_item(ui: &mut Ui, section: Section, active: bool, t: &Strings) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let bg = if active {
        ACCENT.gamma_multiply(0.16)
    } else if resp.hovered() {
        CARD
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    if bg != Color32::TRANSPARENT {
        p.rect_filled(rect, CornerRadius::same(9), bg);
    }
    if active {
        let bar = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 10.0), Vec2::new(3.0, 18.0));
        p.rect_filled(bar, CornerRadius::same(2), ACCENT);
    }
    draw_icon(
        p,
        rect.left_center() + Vec2::new(22.0, 0.0),
        9.0,
        section,
        if active { ACCENT } else { FAINT },
    );
    p.text(
        rect.left_center() + Vec2::new(40.0, 0.0),
        Align2::LEFT_CENTER,
        section_title(t, section),
        FontId::proportional(13.5),
        if active { TEXT } else { DIM },
    );
    resp.clicked()
}

/// Section name in the selected language.
#[allow(dead_code)]
pub(crate) fn section_title(t: &Strings, section: Section) -> &'static str {
    t.t(match section {
        Section::Overview => Key::NavOverview,
        Section::Image => Key::NavImage,
        Section::Color => Key::NavColor,
        Section::Inputs => Key::NavInputs,
        Section::MultiView => Key::NavMultiView,
        Section::Profiles => Key::NavProfiles,
        Section::Advanced => Key::NavAdvanced,
    })
}

/// Icons drawn with vectors — no glyphs that might be missing from the font.
pub(crate) fn draw_icon(p: &egui::Painter, center: Pos2, r: f32, section: Section, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    match section {
        Section::Overview => {
            let s = r * 0.66;
            for (dx, dy) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let c = center + Vec2::new(dx * r * 0.46, dy * r * 0.46);
                p.rect_filled(
                    Rect::from_center_size(c, Vec2::splat(s)),
                    CornerRadius::same(2),
                    color,
                );
            }
        }
        Section::Image => {
            let rect = Rect::from_center_size(center, Vec2::new(r * 1.7, r * 1.4));
            p.rect_stroke(rect, CornerRadius::same(2), stroke, StrokeKind::Inside);
            p.circle_filled(center + Vec2::new(-r * 0.32, -r * 0.26), r * 0.16, color);
            p.line_segment(
                [
                    center + Vec2::new(-r * 0.62, r * 0.42),
                    center + Vec2::new(-r * 0.05, -r * 0.14),
                ],
                stroke,
            );
            p.line_segment(
                [
                    center + Vec2::new(-r * 0.05, -r * 0.14),
                    center + Vec2::new(r * 0.42, r * 0.3),
                ],
                stroke,
            );
        }
        Section::Color => {
            let mut right = Vec::new();
            for i in 0..=16 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                right.push(center + Vec2::new(a.cos() * r * 0.72, a.sin() * r * 0.72));
            }
            p.add(egui::Shape::convex_polygon(
                right,
                color.gamma_multiply(0.85),
                Stroke::NONE,
            ));
            p.circle_stroke(center, r * 0.72, stroke);
        }
        Section::Inputs => {
            let a = center + Vec2::new(-r * 0.7, -r * 0.32);
            let b = center + Vec2::new(r * 0.7, -r * 0.32);
            p.line_segment([a, b], stroke);
            p.line_segment([b, b + Vec2::new(-r * 0.3, -r * 0.26)], stroke);
            p.line_segment([b, b + Vec2::new(-r * 0.3, r * 0.26)], stroke);
            let c = center + Vec2::new(r * 0.7, r * 0.36);
            let d = center + Vec2::new(-r * 0.7, r * 0.36);
            p.line_segment([c, d], stroke);
            p.line_segment([d, d + Vec2::new(r * 0.3, -r * 0.26)], stroke);
            p.line_segment([d, d + Vec2::new(r * 0.3, r * 0.26)], stroke);
        }
        Section::MultiView => {
            let rect = Rect::from_center_size(center, Vec2::new(r * 1.8, r * 1.35));
            p.rect_stroke(rect, CornerRadius::same(2), stroke, StrokeKind::Inside);
            p.line_segment(
                [
                    Pos2::new(center.x, rect.top() + 1.0),
                    Pos2::new(center.x, rect.bottom() - 1.0),
                ],
                stroke,
            );
        }
        Section::Profiles => {
            let w = r * 0.72;
            let pts = vec![
                center + Vec2::new(-w, -r * 0.78),
                center + Vec2::new(w, -r * 0.78),
                center + Vec2::new(w, r * 0.8),
                center + Vec2::new(0.0, r * 0.34),
                center + Vec2::new(-w, r * 0.8),
            ];
            p.add(egui::Shape::convex_polygon(
                pts,
                color.gamma_multiply(0.55),
                stroke,
            ));
        }
        Section::Advanced => {
            for (i, dy) in [-0.5_f32, 0.5].iter().enumerate() {
                let y = center.y + dy * r * 0.95;
                p.line_segment(
                    [
                        Pos2::new(center.x - r * 0.8, y),
                        Pos2::new(center.x + r * 0.8, y),
                    ],
                    stroke,
                );
                let kx = if i == 0 { -r * 0.32 } else { r * 0.28 };
                p.circle_filled(Pos2::new(center.x + kx, y), r * 0.22, color);
            }
        }
    }
}

pub(crate) fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SIDEBAR;
    style.visuals.extreme_bg_color = Color32::from_rgb(0x0d, 0x10, 0x14);
    style.visuals.faint_bg_color = CARD;
    style.visuals.code_bg_color = BG;
    style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, DIM);
    style.visuals.widgets.hovered.bg_fill = CARD_HI;
    style.visuals.widgets.hovered.weak_bg_fill = CARD_HI;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.open.bg_fill = CARD_HI;
    style.visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals.menu_corner_radius = CornerRadius::same(10);
    style.spacing.item_spacing = Vec2::new(10.0, 9.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.slider_width = 200.0;

    use egui::{FontFamily, FontId as FId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Button, FId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FId::new(20.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FId::new(12.5, FontFamily::Monospace)),
    ]
    .into();

    ctx.set_style(style);
}
