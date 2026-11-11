//! Interface — a thin layer that ties the modules together:
//! `theme` (palette, style, icons), `widgets` (building blocks), `rows` (setting rows),
//! `chrome` (top bar, navigation, notifications), `pages` (one file per section).

use crate::catalog::Section;
use crate::ddc::{Capabilities, VcpValue};
use crate::i18n;
use crate::profiles::Store;
use crate::settings::Settings;
use crate::topology::{self, Displays};
use crate::worker::{Cmd, Conn, Ev, LogLine, Worker};
use std::collections::BTreeMap;

mod chrome;
mod pages;
mod rows;
mod theme;
mod widgets;

pub(crate) use crate::catalog::{self, Kind, MultiMode};
pub(crate) use crate::ddc::{input_card_name, input_name};
pub(crate) use crate::i18n::{Key, Lang, Strings, LANGS};
pub(crate) use crate::profiles::{self, Profile};
pub(crate) use eframe::egui;
pub(crate) use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};
pub(crate) use rows::friendly_error;
pub(crate) use std::time::{Duration, Instant};
pub(crate) use theme::{
    configure_style, draw_icon, nav_item, ACCENT, BG, BORDER, CARD, CARD_HI, DIM, ERR, FAINT, OK,
    SIDEBAR, TEXT, WARN,
};
pub(crate) use widgets::*;

pub struct App {
    pub(crate) worker: Worker,
    conn: Conn,
    caps: Capabilities,
    vals: BTreeMap<u8, VcpValue>,
    busy: bool,
    partial_read: bool,
    toast: Option<(String, bool, Instant)>,
    section: Section,
    store: Store,
    pending: BTreeMap<u8, u16>,
    new_profile_name: String,
    show_vendor: bool,
    confirm_reset: bool,
    log: Vec<LogLine>,
    started: bool,
    /// Source of the left and the right window of a split (VCP F0/F0).
    win_sources: [u16; 2],
    /// Desktop topology: is there another screen to fall back to?
    displays: Displays,
    /// Monitor input this computer is connected to. Needed to close a split
    /// safely when no other screen is available.
    self_source: Option<u16>,
    pub(crate) settings: Settings,
    pub(crate) lang: Lang,
    pub(crate) tr: Strings,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::starting_at(cc, Section::Overview)
    }

    /// Same as `new`, but opens directly on the given page (`--page`).
    pub fn starting_at(cc: &eframe::CreationContext<'_>, section: Section) -> Self {
        configure_style(&cc.egui_ctx);
        let worker = Worker::spawn();
        let settings = Settings::load();
        let lang = settings.lang();
        Self {
            worker,
            conn: Conn::Probing,
            caps: Capabilities::default(),
            vals: BTreeMap::new(),
            busy: true,
            partial_read: false,
            toast: None,
            section,
            store: Store::load(),
            pending: BTreeMap::new(),
            new_profile_name: String::new(),
            show_vendor: false,
            confirm_reset: false,
            log: Vec::new(),
            started: false,
            win_sources: [0, 0],
            displays: Displays::default(),
            self_source: None,
            settings,
            lang,
            tr: i18n::strings(lang),
        }
    }

    // ── communication ─────────────────────────────────────────────────────
    fn send(&mut self, cmd: Cmd) {
        if self.worker.tx.send(cmd).is_ok() {
            self.busy = true;
        }
    }

    fn can_edit(&self) -> bool {
        !self.busy && matches!(self.conn, Conn::Ok(_))
    }

    fn set_value(&mut self, code: u8, value: u16) {
        if !self.can_edit() {
            return;
        }
        self.pending.insert(code, value);
        self.send(Cmd::Set { code, value });
    }

    /// Translation of a key in the selected language.
    pub(crate) fn t(&self, key: Key) -> &'static str {
        self.tr.t(key)
    }

    /// Translation with a single `{}` replaced by the value.
    pub(crate) fn tf(&self, key: Key, value: impl std::fmt::Display) -> String {
        self.t(key).replacen("{}", &value.to_string(), 1)
    }

    /// Text of a worker log entry in the selected language.
    pub(crate) fn log_text(&self, line: &LogLine) -> String {
        match line {
            LogLine::Text(text) => text.clone(),
            LogLine::Key(key, args) => self.tr.fill(*key, args),
        }
    }

    /// Language change: save the setting and rebuild the string table.
    pub(crate) fn set_lang(&mut self, lang: Lang) {
        if lang == self.lang {
            return;
        }
        self.lang = lang;
        self.tr = i18n::strings(lang);
        if let Err(e) = self.settings.save_lang(lang) {
            self.toast = Some((self.tf(Key::ErrSaveSettings, e), true, Instant::now()));
        }
    }

    fn apply_profile(&mut self, p: &Profile) {
        if self.busy || !matches!(self.conn, Conn::Ok(_)) {
            return;
        }
        let list = profiles::replay_plan(&p.values);
        let skipped = p.values.len().saturating_sub(list.len());
        if skipped > 0 {
            self.log
                .push(LogLine::Key(Key::ProfileSkipped, vec![skipped.to_string()]));
        }
        if list.is_empty() {
            self.toast = Some((
                self.t(Key::ToastProfileEmpty).to_string(),
                true,
                Instant::now(),
            ));
            return;
        }
        self.send(Cmd::Apply(list));
    }

    fn snapshot_values(&self) -> BTreeMap<String, u16> {
        let mut out = BTreeMap::new();
        for (code, v) in &self.vals {
            if !profiles::replay_safe(*code) {
                continue;
            }
            out.insert(profiles::key(*code), v.value);
        }
        out
    }

    fn save_to_slot(&mut self, slot: char) {
        if self.busy || self.vals.is_empty() {
            self.toast = Some((self.t(Key::ProfileBusy).into(), true, Instant::now()));
            return;
        }
        let name = format!("{} {}", self.t(Key::ConfigWord), slot);
        let values = self.snapshot_values();
        let count = values.len();
        let p = Profile {
            id: format!("slot{}", slot.to_lowercase()),
            name,
            values,
        };
        match slot {
            'A' => self.store.slots.a = Some(p),
            _ => self.store.slots.b = Some(p),
        }
        let saved = self.tf(Key::ToastSaved, count);
        self.persist(saved);
    }

    fn persist(&mut self, ok_msg: String) {
        let tr = i18n::strings(self.lang);
        match self.store.save(&tr) {
            Ok(()) => self.toast = Some((ok_msg, false, Instant::now())),
            Err(e) => self.toast = Some((e, true, Instant::now())),
        }
    }

    fn value(&self, code: u8) -> Option<u16> {
        if let Some(v) = self.pending.get(&code) {
            return Some(*v);
        }
        self.vals.get(&code).map(|v| v.value)
    }

    fn max(&self, code: u8) -> u16 {
        self.vals.get(&code).map(|v| v.max).unwrap_or(100)
    }

    fn multi_mode(&self) -> MultiMode {
        catalog::multi_mode(self.value(0xf2), self.value(0xf4))
    }

    fn connector(&self) -> &str {
        match &self.conn {
            Conn::Ok(monitor) => monitor.connector.as_str(),
            _ => "",
        }
    }

    /// Keeps `self_source` in sync with the monitor.
    ///
    /// In single mode 0x60 is the input in use, so it is authoritative. In a
    /// split the application remembers the source from before the split; if it
    /// started while a split was already active, the GPU connector can resolve
    /// an unambiguous pair (for example HDMI 1 + USB-C on a DisplayPort link).
    fn refresh_self_source(&mut self) {
        let mode = self.multi_mode();
        if mode == MultiMode::Off {
            if let Some(value) = self.value(0x60) {
                if value <= 0xff && catalog::known_source(value as u8) {
                    self.self_source = Some(value);
                }
            }
            return;
        }
        if self.self_source.is_some() {
            return;
        }
        if matches!(
            mode,
            MultiMode::PictureInPicture | MultiMode::PictureByPicture
        ) {
            if let Some(pair) = self.split_pair() {
                let connector = self.connector().to_string();
                if let Some(source) = topology::infer_self_source(pair, &connector) {
                    self.self_source = Some(source);
                }
            }
        }
    }

    /// Picker for the monitor input this computer is connected to.
    ///
    /// It is shown during a split because that value decides what happens when
    /// the split is closed without another screen to fall back to.
    fn self_source_row(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(self.t(Key::SelfSource))
                .size(12.5)
                .color(TEXT),
        );
        if !self.displays.has_second_screen() {
            ui.label(
                RichText::new(self.t(Key::SelfSourceHint))
                    .size(11.0)
                    .color(DIM),
            );
        }
        ui.add_space(6.0);
        let sources = self.input_sources();
        let current = self.self_source.unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            for (code, name) in &sources {
                if pill(ui, name, current == *code).clicked() {
                    self.self_source = Some(*code);
                }
            }
        });
        if self.self_source.is_none() {
            ui.add_space(4.0);
            ui.label(
                RichText::new(self.t(Key::SelfSourceUnknown))
                    .size(11.0)
                    .color(WARN),
            );
        }
    }

    fn input_name_of(&self, code: u16) -> String {
        if let Some((_, label)) = self.caps.inputs.iter().find(|(c, _)| *c == code) {
            // ddcutil fills the `Values:` list with its own placeholders ("Unrecognized
            // value", "Unknown") when it does not know a code — we prefer our own name.
            let lower = label.trim().to_lowercase();
            let placeholder =
                lower.is_empty() || lower.contains("unrecognized") || lower == "unknown";
            if !placeholder {
                return label.clone();
            }
        }
        input_name(&self.tr, code)
    }

    /// Sources the monitor offers, with our own names.
    pub(crate) fn input_sources(&self) -> Vec<(u16, String)> {
        let reported = self.caps.inputs.clone();
        let mut out: Vec<(u16, String)> = reported
            .iter()
            .map(|(code, label)| (*code, input_card_name(&self.tr, *code, label)))
            .collect();
        if out.is_empty() {
            out = vec![
                (0x0f, "DisplayPort".into()),
                (0x11, "HDMI 1".into()),
                (0x12, "HDMI 2".into()),
                (0x31, "USB-C".into()),
            ];
        }
        out
    }

    /// The source pair of the split, from 0xF0 or from a combined 0x60 value.
    ///
    /// Both registers use the same encoding: low byte = left/primary, high
    /// byte = right/secondary. 0xF0 is preferred because it is the register
    /// that describes the windows; 0x60 is the fallback when F0 is unreadable.
    fn split_pair(&self) -> Option<(u16, u16)> {
        // Prefer the pair the application last wrote: F0 can momentarily
        // report the window that was written last instead of the whole pair.
        let [left, right] = self.win_sources;
        if left != 0 && right != 0 && left != 0xff && right != 0xff {
            return Some((left, right));
        }
        catalog::window_sources(self.value(0xf0))
            .or_else(|| catalog::window_sources(self.value(0x60)))
    }

    /// Source picker for one window of a split.
    ///
    /// Picking a source writes VCP 0x60 as `(right << 8) | left` — the same
    /// pair Lenovo's service writes. No selector and no swap are involved,
    /// so the right half is reached directly and the monitor's OSD stays alive.
    fn window_source_picker(&mut self, ui: &mut Ui, window: usize) {
        let label = if window == 0 {
            self.t(Key::WindowLeft)
        } else {
            self.t(Key::WindowRight)
        };
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let sources = self.input_sources();
        let pair = self.split_pair();
        let current = pair
            .map(|p| if window == 0 { p.0 } else { p.1 })
            .unwrap_or(0);
        let enabled = pair.is_some() && !self.busy && matches!(self.conn, Conn::Ok(_));
        let mut chosen: Option<u16> = None;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (code, name) in &sources {
                    if pill(ui, name, current == *code).clicked() {
                        chosen = Some(*code);
                    }
                }
            });
        });
        if let Some(code) = chosen {
            self.apply_window_source(window, code);
        }
        if pair.is_none() {
            ui.add_space(4.0);
            ui.label(
                RichText::new(self.t(Key::WindowsUnknown))
                    .size(11.0)
                    .color(DIM),
            );
        }
        ui.add_space(6.0);
    }

    /// Writes the source of one window of a split.
    ///
    /// In PBP both sources are written together to 0x60; in PIP the monitor
    /// keeps its separate window selector `0xA5 = 0x0100`.
    fn apply_window_source(&mut self, window: usize, source: u16) {
        if self.busy || !matches!(self.conn, Conn::Ok(_)) {
            return;
        }
        let Some((left, right)) = self.split_pair() else {
            self.toast = Some((
                self.t(Key::WindowsUnknown).to_string(),
                true,
                Instant::now(),
            ));
            return;
        };
        match self.multi_mode() {
            MultiMode::PictureByPicture => {
                let (left, right) = if window == 0 {
                    (source, right)
                } else {
                    (left, source)
                };
                // A valid layout is left untouched: changing only a source
                // must not blank the picture with a redundant F5 write.
                let layout = if self.value(0xf5).and_then(catalog::layout_for).is_some() {
                    None
                } else {
                    Some(0x0101)
                };
                self.win_sources = [left, right];
                self.send(Cmd::ApplySplit {
                    left,
                    right,
                    layout,
                    enable_pbp: false,
                });
            }
            MultiMode::PictureInPicture => {
                self.win_sources[1] = source;
                self.send(Cmd::ApplyWindow {
                    selector: 0x0100,
                    source,
                });
            }
            _ => {}
        }
    }

    // ── event handling ────────────────────────────────────────────────────
    fn pump(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.worker.rx.try_recv() {
            match ev {
                Ev::Conn(c) => {
                    self.vals.clear();
                    self.pending.clear();
                    if matches!(c, Conn::Err(_)) {
                        self.self_source = None;
                        self.win_sources = [0, 0];
                        self.displays = Displays::default();
                    }
                    self.conn = c;
                }
                Ev::Caps(c) => self.caps = c,
                Ev::Displays(d) => self.displays = d,
                Ev::Values(v) => {
                    self.partial_read = crate::ddc::CORE.iter().any(|code| !v.contains_key(code))
                        || self.vals.keys().any(|code| !v.contains_key(code));
                    self.vals = v;
                    self.pending.clear();
                    // The monitor reports the source of each window in one
                    // register; when it reports a usable pair, that is the
                    // truth — a pick the monitor refused is not kept.
                    if let Some((left, right)) = catalog::window_sources(
                        self.vals.get(&0xf0).map(|v| v.value),
                    )
                    .or_else(|| catalog::window_sources(self.vals.get(&0x60).map(|v| v.value)))
                    {
                        self.win_sources = [left, right];
                    }
                    self.refresh_self_source();
                }
                Ev::Busy(b) => self.busy = b,
                Ev::Notice(key, argument, err) => {
                    let msg = self.tf(key, &argument);
                    self.log.push(LogLine::Key(key, vec![argument]));
                    self.toast = Some((msg, err, Instant::now()));
                }
                Ev::Log(line) => {
                    self.log.push(line);
                    if self.log.len() > 200 {
                        self.log.remove(0);
                    }
                }
            }
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        if let Some((_, _, at)) = &self.toast {
            if at.elapsed() < Duration::from_millis(3600) {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }
    }

    fn hotkeys(&mut self, ctx: &egui::Context) {
        let (f1, f2, r) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::F1),
                i.key_pressed(egui::Key::F2),
                i.key_pressed(egui::Key::F5),
            )
        });
        if f1 {
            if let Some(p) = self.store.slots.a.clone() {
                self.apply_profile(&p);
            }
        }
        if f2 {
            if let Some(p) = self.store.slots.b.clone() {
                self.apply_profile(&p);
            }
        }
        if r && !self.busy {
            self.send(Cmd::Refresh);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.started {
            self.started = true;
            self.send(Cmd::Init);
        }
        self.pump(ctx);
        self.hotkeys(ctx);

        egui::TopBottomPanel::top("bar")
            .exact_height(72.0)
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(22, 12)))
            .show(ctx, |ui| self.top_bar(ui));

        egui::SidePanel::left("nav")
            .exact_width(228.0)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(SIDEBAR)
                    .inner_margin(Margin::symmetric(14, 16)),
            )
            .show(ctx, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(24, 10)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.partial_read && !self.busy {
                            ui.label(RichText::new(self.t(Key::PartialRead)).color(WARN));
                            ui.add_space(10.0);
                        }
                        self.content(ui);
                        ui.add_space(24.0);
                    });
            });

        self.toast_overlay(ctx);
    }
}
