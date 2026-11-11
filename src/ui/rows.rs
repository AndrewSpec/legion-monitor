use crate::ui::*;

// ── widgets ──────────────────────────────────────────────────────────────

/// A ddcutil message is often technical and in English — we show a short
/// translated sentence and keep the original under "Technical details".
pub(crate) fn friendly_error(msg: &str) -> Key {
    let m = msg.to_lowercase();
    if m.contains("no displays implementing ddc/ci")
        || m.contains("no /dev/i2c devices")
        || m.contains("no monitor supporting ddc/ci")
    {
        Key::ErrNoDisplay
    } else if m.contains("laptop display") {
        Key::ErrLaptop
    } else if m.contains("permission denied") || m.contains("access denied") {
        Key::ErrPermission
    } else {
        Key::ErrGeneric
    }
}

impl App {
    pub(crate) fn codes_in(&self, section: Section) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for (code, v) in &self.vals {
            let ddc_name = self.caps.name_of(*code).unwrap_or("");
            if catalog::section_for(*code, ddc_name) != section {
                continue;
            }
            if catalog::kind_for(*code, v.max) == Kind::ReadOnly {
                continue;
            }
            out.push(*code);
        }
        out
    }

    pub(crate) fn label_of(&self, code: u8) -> String {
        if let Some(key) = catalog::fn_key(code) {
            return self.t(key).to_string();
        }
        let ddc_name = self.caps.name_of(code).unwrap_or("");
        catalog::unknown_label(&self.tr, code, ddc_name)
    }

    /// Hint under the label; empty when there is nothing to add.
    fn hint_of(&self, code: u8, hint: &str) -> String {
        if hint.is_empty() {
            catalog::hint_key(code)
                .map(|k| self.t(k).to_string())
                .unwrap_or_default()
        } else {
            hint.to_string()
        }
    }

    /// Slider with a label and value; saved when released.
    pub(crate) fn slider_row(&mut self, ui: &mut Ui, code: u8, label: &str, hint: &str) {
        let hint = self.hint_of(code, hint);
        let Some(v) = self.vals.get(&code).cloned() else {
            self.placeholder_row(ui, label, &hint);
            return;
        };
        let max = v.max.max(1) as f32;
        let mut cur = self.value(code).unwrap_or(v.value) as f32;

        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(13.0).color(TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{}", cur.round() as u16))
                        .size(13.0)
                        .strong()
                        .color(ACCENT),
                );
            });
        });
        if !hint.is_empty() {
            ui.label(RichText::new(&hint).size(11.0).color(FAINT));
        }

        let editable = self.can_edit();
        let resp = ui.add_enabled(
            editable,
            egui::Slider::new(&mut cur, 0.0..=max)
                .show_value(false)
                .trailing_fill(true),
        );
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("test-slider-rect"), resp.rect));
        if editable && resp.changed() {
            self.pending.insert(code, cur.round() as u16);
            if !resp.dragged() {
                self.set_value(code, cur.round() as u16);
            }
        }
        if editable && resp.drag_stopped() {
            self.set_value(code, cur.round() as u16);
        }
        ui.add_space(6.0);
    }

    pub(crate) fn toggle_row(&mut self, ui: &mut Ui, code: u8, label: &str, hint: &str) {
        let hint = self.hint_of(code, hint);
        let Some(v) = self.vals.get(&code).cloned() else {
            self.placeholder_row(ui, label, &hint);
            return;
        };
        let mut on = self.value(code).unwrap_or(v.value) != 0;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).size(13.0).color(TEXT));
                if !hint.is_empty() {
                    ui.label(RichText::new(&hint).size(11.0).color(FAINT));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(self.can_edit(), egui::Checkbox::without_text(&mut on))
                    .changed()
                {
                    self.set_value(code, if on { 1 } else { 0 });
                }
            });
        });
        ui.add_space(6.0);
    }

    pub(crate) fn choice_row(
        &mut self,
        ui: &mut Ui,
        code: u8,
        label: &str,
        options: &'static [(u16, Key)],
    ) {
        let current = self.value(code).unwrap_or(0);
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let mut chosen: Option<u16> = None;
        ui.add_enabled_ui(self.can_edit(), |ui| {
            ui.horizontal_wrapped(|ui| {
                for (value, key) in options {
                    if pill(ui, self.t(*key), current == *value).clicked() {
                        chosen = Some(*value);
                    }
                }
            })
        });
        if let Some(v) = chosen {
            self.set_value(code, v);
        }
        ui.add_space(6.0);
    }

    /// Like `choice_row`, but takes a list built at run time
    /// (value names come from what the monitor reported).
    pub(crate) fn choice_row_owned(
        &mut self,
        ui: &mut Ui,
        code: u8,
        label: &str,
        options: &[(u16, String)],
    ) {
        let current = self.value(code).unwrap_or(0);
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let mut chosen: Option<u16> = None;
        ui.add_enabled_ui(self.can_edit(), |ui| {
            ui.horizontal_wrapped(|ui| {
                for (value, name) in options {
                    if pill(ui, name, current == *value).clicked() {
                        chosen = Some(*value);
                    }
                }
            })
        });
        if let Some(v) = chosen {
            self.set_value(code, v);
        }
        ui.add_space(6.0);
    }

    /// Picks a convenient control for the feature: value list → buttons,
    /// 0/1 value → toggle, otherwise → slider. Names are always human-readable.
    pub(crate) fn auto_row(&mut self, ui: &mut Ui, code: u8) {
        let label = self.label_of(code);
        let kind = catalog::kind_for(code, self.max(code));
        if kind == Kind::Choice {
            let opts = self.caps.values_of(code).map(|v| v.to_vec());
            if let Some(opts) = opts {
                if opts.len() > 1 {
                    self.choice_row_owned(ui, code, &label, &opts);
                    return;
                }
            }
        }
        if kind == Kind::Toggle || self.max(code) <= 1 {
            self.toggle_row(ui, code, &label, "");
        } else {
            self.slider_row(ui, code, &label, "");
        }
    }

    /// "No data" row — the page structure stays, only the values are empty.
    pub(crate) fn placeholder_row(&self, ui: &mut Ui, label: &str, hint: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(13.0).color(FAINT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("—").size(13.0).color(FAINT));
            });
        });
        if !hint.is_empty() {
            ui.label(
                RichText::new(hint)
                    .size(11.0)
                    .color(FAINT.gamma_multiply(0.75)),
            );
        }
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(4), BG);
        ui.add_space(8.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_for_test() -> (App, std::sync::mpsc::Receiver<Cmd>) {
        let (tx, commands) = std::sync::mpsc::channel();
        let (_, rx) = std::sync::mpsc::channel();
        let app = App {
            worker: Worker { tx, rx },
            conn: Conn::Ok(crate::ddc::Monitor::default()),
            caps: Capabilities::default(),
            vals: [(
                0x10,
                VcpValue {
                    value: 80,
                    max: 100,
                },
            )]
            .into_iter()
            .collect(),
            busy: false,
            partial_read: false,
            toast: None,
            section: Section::Overview,
            store: Store::default(),
            pending: BTreeMap::new(),
            new_profile_name: String::new(),
            show_vendor: false,
            confirm_reset: false,
            log: Vec::new(),
            started: true,
            win_sources: [0, 0],
            pip_corner: catalog::PipCorner::default(),
            mode_hint: None,
            displays: crate::topology::Displays::default(),
            panel: crate::panel::Panel::default(),
            self_source: None,
            settings: Settings::default(),
            lang: Lang::En,
            tr: i18n::strings(Lang::En),
            rescue_pending: false,
            rescue_probe: std::time::Instant::now(),
            rescue_after: None,
            rescue_status: None,
        };
        (app, commands)
    }

    fn slider_frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> Pos2 {
        let mut target = Pos2::ZERO;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 200.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.slider_row(ui, 0x10, "Brightness", "");
                    target = ctx
                        .data(|d| d.get_temp::<Rect>(egui::Id::new("test-slider-rect")))
                        .unwrap()
                        .left_center()
                        + Vec2::new(30.0, 0.0);
                });
            },
        );
        target
    }

    fn mode_frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 350.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mode_selector(ui));
            },
        );
    }

    fn click_mode_target(app: &mut App, ctx: &egui::Context, target: usize) {
        mode_frame(ctx, app, vec![]);
        let pos = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new(("test-mode-rect", target))))
            .unwrap()
            .center();
        mode_frame(
            ctx,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        mode_frame(
            ctx,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    #[test]
    fn pbp_single_switch_uses_the_detected_or_chosen_source() {
        for (second_screen, expected) in [(false, Some(0x31)), (true, None)] {
            let (mut app, commands) = app_for_test();
            app.vals.insert(
                0xf2,
                VcpValue {
                    value: 0x0103,
                    max: 0xffff,
                },
            );
            app.vals.insert(
                0xf4,
                VcpValue {
                    value: 0x0100,
                    max: 0xffff,
                },
            );
            app.vals.insert(
                0xf5,
                VcpValue {
                    value: 0x0101,
                    max: 0xffff,
                },
            );
            app.displays = crate::topology::Displays {
                known: true,
                second_screen,
                output_size: None,
                refresh_hz: None,
            };
            app.self_source = Some(0x31);
            let ctx = egui::Context::default();
            click_mode_target(&mut app, &ctx, 0);
            match commands.try_recv().unwrap() {
                Cmd::DisableMultiView { source, pbp } => {
                    assert_eq!(source, expected);
                    assert!(pbp);
                }
                other => panic!("unexpected command: {:?}", std::mem::discriminant(&other)),
            }
        }
    }

    #[test]
    fn pip_from_single_uses_the_displayed_input_and_does_not_close_again() {
        let (mut app, commands) = app_for_test();
        app.displays = crate::topology::Displays {
            known: true,
            second_screen: false,
            output_size: None,
            refresh_hz: None,
        };
        app.self_source = Some(0x31);
        // The desktop is on HDMI 1 even though the saved self-source says USB-C.
        app.vals.insert(
            0x60,
            VcpValue {
                value: 0x11,
                max: 0xffff,
            },
        );
        app.vals.insert(
            0xf2,
            VcpValue {
                value: 0x0000,
                max: 0xffff,
            },
        );
        app.enter_pip(catalog::PipCorner::TopRight);
        match commands.try_recv().unwrap() {
            Cmd::ApplyPip {
                corner,
                main,
                source,
                leave_pbp,
            } => {
                assert_eq!(corner, (1280, 0, 2560, 720));
                assert_eq!(main, Some(0x11));
                assert_eq!(source, Some(0x11));
                assert!(!leave_pbp);
            }
            other => panic!("unexpected command: {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn pip_is_refused_above_sixty_hertz() {
        let (mut app, commands) = app_for_test();
        app.displays = crate::topology::Displays {
            known: true,
            second_screen: false,
            output_size: None,
            refresh_hz: Some(165.0),
        };
        app.self_source = Some(0x31);
        app.enter_pip(catalog::PipCorner::TopRight);
        assert!(
            commands.try_recv().is_err(),
            "a 165 Hz link must not start PIP"
        );
        let (note, error, _) = app.toast.clone().expect("the refusal is explained");
        assert!(error);
        assert!(note.contains("165"), "unexpected note: {note}");
    }

    #[test]
    fn pip_allows_a_sixty_hertz_link() {
        let (mut app, commands) = app_for_test();
        app.displays = crate::topology::Displays {
            known: true,
            second_screen: false,
            output_size: None,
            // Display servers report 59.98 Hz for a 60 Hz mode.
            refresh_hz: Some(59.98),
        };
        app.self_source = Some(0x31);
        app.enter_pip(catalog::PipCorner::TopRight);
        assert!(matches!(commands.try_recv().unwrap(), Cmd::ApplyPip { .. }));
    }

    #[test]
    fn mode_clicks_send_the_expected_command_and_respect_busy_offline() {
        for (busy, connected) in [(false, true), (true, true), (false, false)] {
            for target in 0..3 {
                let (mut app, commands) = app_for_test();
                app.busy = busy;
                if !connected {
                    app.conn = Conn::Err("offline".into());
                }
                // A corrupt F5 must not be echoed back when enabling PbP.
                app.vals.insert(
                    0xf5,
                    VcpValue {
                        value: 0xffff,
                        max: 0xffff,
                    },
                );
                // The single-mode path needs a known topology and source.
                app.displays = crate::topology::Displays {
                    known: true,
                    second_screen: false,
                    output_size: None,
                    refresh_hz: None,
                };
                app.self_source = Some(0x31);
                let ctx = egui::Context::default();
                mode_frame(&ctx, &mut app, vec![]);
                let pos = ctx
                    .data(|d| d.get_temp::<Rect>(egui::Id::new(("test-mode-rect", target))))
                    .unwrap()
                    .center();
                mode_frame(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                mode_frame(
                    &ctx,
                    &mut app,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                if busy || !connected {
                    assert!(commands.try_recv().is_err());
                    assert_eq!(app.busy, busy);
                } else {
                    match (target, commands.try_recv().unwrap()) {
                        (0, Cmd::DisableMultiView { source, pbp }) => {
                            assert_eq!(source, Some(0x31));
                            assert!(!pbp);
                        }
                        (
                            1,
                            Cmd::ApplyPip {
                                corner,
                                main,
                                source,
                                leave_pbp,
                            },
                        ) => {
                            assert_eq!(corner, (1280, 0, 2560, 720));
                            assert_eq!(main, Some(0x31));
                            // No second source was picked, so PIP mirrors the
                            // current picture until the user chooses one; the
                            // old default picked an unconnected DisplayPort.
                            assert_eq!(source, Some(0x31));
                            // No F2/F4 readback in this fixture: the mode is
                            // unknown, so the worker verifies that PBP closed
                            // before writing the PIP selector.
                            assert!(leave_pbp);
                            assert_eq!(app.multi_mode(), MultiMode::PictureInPicture);
                        }
                        (2, Cmd::SetPbp { layout }) => {
                            assert_eq!(layout, Some(0x0101));
                        }
                        (_, other) => {
                            panic!("unexpected command: {:?}", std::mem::discriminant(&other))
                        }
                    }
                    assert!(app.busy);
                }
            }
        }
    }

    #[test]
    fn single_switch_without_second_screen_requires_a_source() {
        let (mut app, commands) = app_for_test();
        app.vals.insert(0xf2, VcpValue { value: 1, max: 1 });
        app.vals.insert(
            0xf4,
            VcpValue {
                value: 0x0100,
                max: 0xffff,
            },
        );
        app.displays = crate::topology::Displays {
            known: true,
            second_screen: false,
            output_size: None,
            refresh_hz: None,
        };
        app.self_source = None;

        let ctx = egui::Context::default();
        mode_frame(&ctx, &mut app, vec![]);
        let pos = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new(("test-mode-rect", 0))))
            .unwrap()
            .center();
        mode_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        mode_frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );

        assert!(commands.try_recv().is_err(), "the split must stay on");
        assert!(app.toast.is_some(), "the user must be told why");
    }

    /// The primary window of 0x60 is the low byte. Using the high byte put
    /// HDMI in the main picture and left USB-C in the PIP quarter.
    #[test]
    fn pip_uses_the_low_byte_of_a_pair_as_the_main_picture() {
        let (mut app, commands) = app_for_test();
        app.self_source = Some(0x11);
        app.vals.insert(
            0x60,
            VcpValue {
                value: 0x1131,
                max: 0xffff,
            },
        );
        app.vals.insert(
            0xf4,
            VcpValue {
                value: 0x0100,
                max: 0xffff,
            },
        );
        app.enter_pip_with(catalog::PipCorner::TopRight, Some(0x12));
        match commands.try_recv().expect("PIP command") {
            Cmd::ApplyPip {
                main,
                source,
                leave_pbp,
                ..
            } => {
                assert_eq!(main, Some(0x31));
                assert_eq!(source, Some(0x12));
                assert!(leave_pbp, "F4 reports an open split");
            }
            _ => panic!("expected ApplyPip"),
        }
    }

    #[test]
    fn true_split_50_50_is_not_reported_as_pip() {
        let (mut app, _commands) = app_for_test();
        for (code, value) in [
            (0x60, 0x0031),
            (0xf2, 0x0103),
            (0xf4, 0x0100),
            (0xf5, 0x0402),
            (0x95, 2560),
            (0x96, 0),
            (0x97, 5120),
            (0x98, 720),
        ] {
            app.vals.insert(
                code,
                VcpValue {
                    value,
                    max: u16::MAX,
                },
            );
        }
        assert_eq!(app.multi_mode(), MultiMode::PictureByPicture);
    }

    #[test]
    fn pip_reports_one_in_f4_low_byte() {
        let (mut app, _commands) = app_for_test();
        for (code, value) in [(0x60, 0x0031), (0xf2, 0x0101), (0xf4, 0x0101)] {
            app.vals.insert(
                code,
                VcpValue {
                    value,
                    max: u16::MAX,
                },
            );
        }
        assert_eq!(app.multi_mode(), MultiMode::PictureInPicture);
    }

    #[test]
    fn rescue_baseline_refuses_a_split() {
        let (mut app, _commands) = app_for_test();
        app.vals.insert(
            0x60,
            VcpValue {
                value: 0x1131,
                max: 0xffff,
            },
        );
        app.vals.insert(
            0xf4,
            VcpValue {
                value: 0x0100,
                max: 0xffff,
            },
        );
        app.save_rescue_baseline(true);
        assert!(
            app.settings.rescue.is_none(),
            "a split is not a rescue base"
        );
        assert_eq!(app.rescue_status, Some((Key::RescueNeedSingle, true)));
    }

    #[test]
    fn rescue_restore_sends_the_saved_source() {
        let (mut app, commands) = app_for_test();
        app.settings.rescue = Some(crate::settings::RescueBase {
            source: 0x31,
            f2: 0x0100,
            f5: 0x0402,
            brightness: Some(80),
            contrast: Some(60),
            saved_at: 1,
        });
        app.restore_rescue();
        match commands.try_recv().expect("rescue command") {
            Cmd::Rescue {
                source,
                brightness,
                contrast,
            } => {
                assert_eq!(source, 0x31);
                assert_eq!(brightness, Some(80));
                assert_eq!(contrast, Some(60));
            }
            _ => panic!("expected Rescue"),
        }
    }

    #[test]
    fn slider_does_not_stage_unsent_edits_while_busy_or_offline() {
        for (busy, connected) in [(true, true), (false, false), (false, true)] {
            let (mut app, commands) = app_for_test();
            app.busy = busy;
            if !connected {
                app.conn = Conn::Err("offline".into());
            }
            let ctx = egui::Context::default();
            let pos = slider_frame(&ctx, &mut app, vec![]);
            slider_frame(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            slider_frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if busy || !connected {
                assert!(app.pending.is_empty());
                assert_eq!(app.value(0x10), Some(80));
                assert!(commands.try_recv().is_err());
            } else {
                assert!(
                    !app.pending.is_empty(),
                    "idle control must accept the same click"
                );
                assert!(matches!(
                    commands.try_recv(),
                    Ok(Cmd::Set { code: 0x10, .. })
                ));
            }
        }
    }
}
