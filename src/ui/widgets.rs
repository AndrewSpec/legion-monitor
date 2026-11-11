use crate::ui::*;

pub(crate) fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::NONE
        .fill(CARD)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
        .stroke(Stroke::new(1.0_f32, BORDER))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Card with a header — the title belongs to the card, it does not float above it.
pub(crate) fn card_titled<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, |ui| {
        ui.label(RichText::new(title).size(12.5).strong().color(TEXT));
        ui.add_space(12.0);
        add(ui)
    })
}

pub(crate) fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(24.0).strong().color(TEXT));
    ui.add_space(2.0);
    ui.label(RichText::new(subtitle).size(12.5).color(DIM));
    ui.add_space(16.0);
}

/// "Label — value" row: label on the left, value aligned to the right.
pub(crate) fn info_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.0).color(TEXT));
        });
    });
}

pub(crate) fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.5).strong().color(FAINT));
    ui.add_space(6.0);
}

pub(crate) fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.0).color(FAINT));
        ui.add_space(2.0);
        ui.label(RichText::new(value).size(16.0).strong().color(TEXT));
    });
}

pub(crate) fn empty_state(ui: &mut Ui, text: &str) {
    card(ui, |ui| {
        ui.label(RichText::new(text).size(12.5).color(DIM));
    });
}

pub(crate) fn chip(ui: &mut Ui, text: &str, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.5), color);
    let size = Vec2::new(galley.size().x + 22.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), color.gamma_multiply(0.14));
    p.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0_f32, color.gamma_multiply(0.35)),
        StrokeKind::Inside,
    );
    p.galley(rect.center() - galley.size() / 2.0, galley, color);
}

pub(crate) fn segmented(ui: &mut Ui, current: usize, items: &[(&str, bool)]) -> Option<usize> {
    let mut chosen = None;
    Frame::NONE
        .fill(BG)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, (item, enabled)) in items.iter().enumerate() {
                    let selected = i == current;
                    let galley = ui.painter().layout_no_wrap(
                        item.to_string(),
                        FontId::proportional(12.5),
                        if selected { Color32::WHITE } else { DIM },
                    );
                    let size = Vec2::new(galley.size().x + 26.0, 30.0);
                    let (rect, resp) = ui
                        .add_enabled_ui(*enabled, |ui| ui.allocate_exact_size(size, Sense::click()))
                        .inner;
                    #[cfg(test)]
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new(("test-mode-rect", i)), rect));
                    let bg = if selected {
                        ACCENT
                    } else if resp.hovered() {
                        CARD
                    } else {
                        Color32::TRANSPARENT
                    };
                    if bg != Color32::TRANSPARENT {
                        ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
                    }
                    ui.painter().galley(
                        rect.center() - galley.size() / 2.0,
                        galley,
                        Color32::WHITE,
                    );
                    if *enabled && resp.clicked() && !selected {
                        chosen = Some(i);
                    }
                }
            });
        });
    chosen
}

pub(crate) fn pill(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.5),
        if selected { Color32::WHITE } else { DIM },
    );
    let size = Vec2::new(galley.size().x + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if selected {
        ACCENT
    } else if resp.hovered() {
        CARD_HI
    } else {
        CARD
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), bg);
    if !selected {
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0_f32, BORDER),
            StrokeKind::Inside,
        );
    }
    p.galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp
}

pub(crate) fn input_card(ui: &mut Ui, t: &Strings, name: &str, active: bool) -> egui::Response {
    let width = 132.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::click());
    let bg = if active {
        ACCENT.gamma_multiply(0.2)
    } else if resp.hovered() {
        CARD_HI
    } else {
        BG
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(11), bg);
    p.rect_stroke(
        rect,
        CornerRadius::same(11),
        Stroke::new(1.2_f32, if active { ACCENT } else { BORDER }),
        StrokeKind::Inside,
    );
    p.text(
        rect.left_top() + Vec2::new(12.0, 12.0),
        Align2::LEFT_TOP,
        name,
        FontId::proportional(14.0),
        if active { TEXT } else { DIM },
    );
    if active {
        p.text(
            rect.left_bottom() + Vec2::new(12.0, -11.0),
            Align2::LEFT_BOTTOM,
            t.t(Key::Active),
            FontId::proportional(10.5),
            ACCENT,
        );
    }
    resp
}

/// Tile for one of the two True Split layouts.
pub(crate) fn true_split_tile(
    ui: &mut Ui,
    split: catalog::TrueSplit,
    selected: bool,
    t: &Strings,
    panel_ratio: f32,
) -> egui::Response {
    let size = Vec2::new(208.0, 112.0);
    let (outer, resp) = ui.allocate_exact_size(size, Sense::click());
    let preview = Rect::from_min_size(outer.min, Vec2::new(outer.width(), 84.0));
    let inner = mini_monitor(ui, preview, selected, resp.hovered(), panel_ratio);

    // The left window is this computer's picture; a faint accent tint keeps
    // that reading without adding a badge to a tile this small.
    let divider = inner.left() + inner.width() * split.share();
    let gap = 2.0;
    let left = Rect::from_min_max(inner.min, Pos2::new(divider - gap / 2.0, inner.bottom()));
    let right = Rect::from_min_max(Pos2::new(divider + gap / 2.0, inner.top()), inner.max);
    ui.painter()
        .rect_filled(left, CornerRadius::same(2), ACCENT.gamma_multiply(0.12));
    ui.painter().rect_filled(
        right,
        CornerRadius::same(2),
        Color32::from_rgb(0x14, 0x19, 0x22),
    );
    for pane in [left, right] {
        ui.painter().rect_stroke(
            pane,
            CornerRadius::same(2),
            Stroke::new(
                1.0_f32,
                if selected {
                    ACCENT.gamma_multiply(0.6)
                } else {
                    BORDER
                },
            ),
            StrokeKind::Inside,
        );
    }
    ui.painter().line_segment(
        [
            Pos2::new(divider, inner.top()),
            Pos2::new(divider, inner.bottom()),
        ],
        Stroke::new(1.0_f32, BORDER),
    );

    caption(ui, outer, split.key(), selected, t);
    resp.on_hover_text(format!("{} · {}", t.t(Key::ModePbp), t.t(split.key())))
}

/// Tile for one PIP corner: the picture with a quarter-size inset.
pub(crate) fn pip_tile(
    ui: &mut Ui,
    corner: catalog::PipCorner,
    selected: bool,
    t: &Strings,
    panel_ratio: f32,
) -> egui::Response {
    let size = Vec2::new(150.0, 104.0);
    let (outer, resp) = ui.allocate_exact_size(size, Sense::click());
    let preview = Rect::from_min_size(outer.min, Vec2::new(outer.width(), 76.0));
    let inner = mini_monitor(ui, preview, selected, resp.hovered(), panel_ratio);
    ui.painter().rect_filled(
        inner,
        CornerRadius::same(2),
        Color32::from_rgb(0x14, 0x19, 0x22),
    );

    let [left, top, right, bottom] = corner.fractions();
    let inset = Rect::from_min_max(
        Pos2::new(
            inner.left() + inner.width() * left,
            inner.top() + inner.height() * top,
        ),
        Pos2::new(
            inner.left() + inner.width() * right,
            inner.top() + inner.height() * bottom,
        ),
    )
    .shrink(1.0);
    ui.painter()
        .rect_filled(inset, CornerRadius::same(2), ACCENT.gamma_multiply(0.2));
    ui.painter().rect_stroke(
        inset,
        CornerRadius::same(2),
        Stroke::new(
            1.0_f32,
            if selected {
                ACCENT
            } else {
                ACCENT.gamma_multiply(0.6)
            },
        ),
        StrokeKind::Inside,
    );

    caption(ui, outer, corner.key(), selected, t);
    resp.on_hover_text(format!("{} · {}", t.t(Key::ModePip), t.t(corner.key())))
}

/// Draws the tile background and the Overview-style monitor inside `preview`.
/// Returns the screen's inner rectangle.
fn mini_monitor(ui: &Ui, preview: Rect, selected: bool, hovered: bool, panel_ratio: f32) -> Rect {
    let bg = if selected {
        ACCENT.gamma_multiply(0.18)
    } else if hovered {
        CARD_HI
    } else {
        BG
    };
    let p = ui.painter();
    p.rect_filled(preview, CornerRadius::same(10), bg);
    p.rect_stroke(
        preview,
        CornerRadius::same(10),
        Stroke::new(1.2_f32, if selected { ACCENT } else { BORDER }),
        StrokeKind::Inside,
    );

    let ratio = panel_ratio.clamp(1.2, 4.0);
    let bezel = 4.0;
    let chin = 5.0;
    let max_height = preview.height() - 2.0 * bezel - chin - 14.0;
    let screen_width = (preview.width() - 32.0).min(max_height * ratio).max(40.0);
    let screen_height = screen_width / ratio;
    let frame = Rect::from_center_size(
        preview.center(),
        Vec2::new(screen_width + 2.0 * bezel, screen_height + bezel + chin),
    );
    p.rect_filled(
        frame,
        CornerRadius::same(4),
        Color32::from_rgb(0x07, 0x0a, 0x0f),
    );
    p.rect_stroke(
        frame,
        CornerRadius::same(4),
        Stroke::new(
            1.0_f32,
            if selected {
                ACCENT.gamma_multiply(0.65)
            } else {
                BORDER
            },
        ),
        StrokeKind::Inside,
    );
    let screen = Rect::from_min_max(
        frame.min + Vec2::new(bezel, bezel),
        frame.max - Vec2::new(bezel, chin),
    );
    p.rect_filled(
        screen,
        CornerRadius::same(2),
        Color32::from_rgb(0x0b, 0x0f, 0x15),
    );
    screen.shrink(2.0)
}

fn caption(ui: &Ui, outer: Rect, key: Key, selected: bool, t: &Strings) {
    ui.painter().text(
        Pos2::new(outer.center().x, outer.bottom() - 8.0),
        Align2::CENTER_BOTTOM,
        t.t(key),
        FontId::proportional(11.0),
        if selected { ACCENT } else { DIM },
    );
}

pub(crate) fn icon_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(15.0).color(DIM))
        .min_size(Vec2::new(34.0, 34.0))
}

pub(crate) fn accent_button(ui: &mut Ui, text: &str) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(12.5), Color32::WHITE);
    let size = Vec2::new(galley.size().x + 28.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if resp.hovered() {
        Color32::from_rgb(0x74, 0x9f, 0xff)
    } else {
        ACCENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp
}

pub(crate) fn small_button(ui: &mut Ui, text: &str) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(12.0), DIM);
    let size = Vec2::new(galley.size().x + 22.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if resp.hovered() { CARD_HI } else { CARD };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), bg);
    p.rect_stroke(
        rect,
        CornerRadius::same(9),
        Stroke::new(1.0_f32, BORDER),
        StrokeKind::Inside,
    );
    p.galley(rect.center() - galley.size() / 2.0, galley, DIM);
    resp
}

pub(crate) fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(12.0), Color32::WHITE);
    let size = Vec2::new(galley.size().x + 24.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if resp.hovered() {
        Color32::from_rgb(0xff, 0x76, 0x76)
    } else {
        ERR
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp
}

pub(crate) fn wide_button(ui: &mut Ui, text: &str, filled: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.5),
        if filled { TEXT } else { DIM },
    );
    let size = Vec2::new(galley.size().x + 30.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if resp.hovered() { CARD_HI } else { CARD };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), bg);
    p.rect_stroke(
        rect,
        CornerRadius::same(9),
        Stroke::new(
            1.0_f32,
            if filled {
                BORDER
            } else {
                BORDER.gamma_multiply(0.6)
            },
        ),
        StrokeKind::Inside,
    );
    p.galley(
        rect.center() - galley.size() / 2.0,
        galley,
        if filled { TEXT } else { DIM },
    );
    resp
}
