//! Interface — a thin layer that ties the modules together:
//! `theme` (palette, style, icons), `widgets` (building blocks), `rows` (setting rows),
//! `chrome` (top bar, navigation, notifications), `pages` (one file per section).

use crate::catalog::Section;
use crate::ddc::{Capabilities, VcpValue};
use crate::i18n;
use crate::panel::{self, Panel, PanelSource};
use crate::profiles::Store;
use crate::settings::Settings;
use crate::topology::{self, Displays};
use crate::worker::{Cmd, Conn, Ev, LogLine, Worker};
use std::collections::BTreeMap;

mod chrome;
mod pages;
mod rows;
mod screen;
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
    /// Last PIP corner chosen in the interface. The monitor's 0x95..0x98
    /// rectangle is the source of truth when it can be read.
    pip_corner: catalog::PipCorner,
    /// Mode the user just asked for. It only bridges the gap until the next
    /// readback: F4 carries the real window bits, so a fresh read wins.
    mode_hint: Option<MultiMode>,
    /// Desktop topology: is there another screen to fall back to?
    displays: Displays,
    /// Detected shape of the monitor's picture area, used by the overview.
    panel: Panel,
    /// Monitor input this computer is connected to. Needed to close a split
    /// safely when no other screen is available.
    self_source: Option<u16>,
    pub(crate) settings: Settings,
    pub(crate) lang: Lang,
    pub(crate) tr: Strings,
    /// Rescue restore is in flight; the next notice decides the result.
    rescue_pending: bool,
    /// Last connection probe from the rescue page.
    rescue_probe: Instant,
    /// One-shot restore scheduled after the monitor reconnects.
    rescue_after: Option<Instant>,
    /// Last rescue result, shown on the rescue page.
    rescue_status: Option<(Key, bool)>,
    /// Values read back from the F8/F7 feature registers, by feature index.
    feature_values: BTreeMap<u16, u16>,
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
            pip_corner: catalog::PipCorner::default(),
            mode_hint: None,
            displays: Displays::default(),
            panel: Panel::default(),
            self_source: None,
            settings,
            lang,
            tr: i18n::strings(lang),
            rescue_pending: false,
            rescue_probe: Instant::now(),
            rescue_after: None,
            rescue_status: None,
            feature_values: BTreeMap::new(),
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

    /// Re-reads the desktop topology without taking the busy flag.
    ///
    /// The rescan only inspects the compositor, so the worker answers with
    /// `Ev::Displays` and no `Busy(false)`; going through `send` would leave
    /// the interface disabled forever.
    fn scan_displays(&mut self) {
        let _ = self.worker.tx.send(Cmd::ScanDisplays);
    }

    /// The desktop refresh rate when PIP is not allowed at it.
    pub(crate) fn pip_refresh_too_high(&self) -> Option<f32> {
        self.displays.refresh_too_high_for_pip()
    }

    /// Warning shown when PIP/True Split will have to lower the refresh rate.
    pub(crate) fn refresh_note(&self) -> Option<String> {
        self.pip_refresh_too_high()
            .map(|hz| self.tf(Key::RefreshAuto60Fmt, format!("{hz:.0}")))
    }

    fn set_value(&mut self, code: u8, value: u16) {
        if !self.can_edit() {
            return;
        }
        self.pending.insert(code, value);
        self.send(Cmd::Set { code, value });
    }

    /// Writes a feature selected through F8/F7 and remembers the requested
    /// value until the monitor answers with the real one.
    pub(crate) fn set_feature(&mut self, index: u16, value: u16) {
        if !self.can_edit() {
            return;
        }
        // Do not stage the value optimistically: the monitor may refuse it on
        // the 0xFA lock or on an unsupported index, and then the highlight
        // would claim a change that never happened.
        self.send(Cmd::SetFeature { index, value });
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

    pub(crate) fn multi_mode(&self) -> MultiMode {
        self.mode_hint.unwrap_or_else(|| self.derive_multi_mode())
    }

    /// True Split also reports PBP in F4. The distinguishing signal is the
    /// desktop: True Split exposes two external outputs (DP-3 + DP-4), while
    /// PBP and single mode expose one.
    pub(crate) fn true_split_active(&self) -> bool {
        self.displays.external_screens >= 2
            && (self.multi_mode() == MultiMode::PictureByPicture
                || self.feature_values.get(&catalog::TRUE_SPLIT_FEATURE) == Some(&1))
    }

    /// Reads the mode from the registers when the user has not just chosen one.
    ///
    /// F4 is the state register on the R45w-30: `0x0100`/`0x0101` is PIP and
    /// `0x0200`/`0x0201` is PBP; zero (or anything else) is one picture. F2 is
    /// the swap command and stays non-zero in every state, so it must not be
    /// used here. When F4 is unavailable, an active F5 layout plus a
    /// half-width desktop output also means PBP.
    fn derive_multi_mode(&self) -> MultiMode {
        let (panel_width, _) = self.panel.pixel_size();
        let half_width = self
            .displays
            .output_size
            .is_some_and(|(width, _)| panel_width > 0 && width.saturating_mul(2) <= panel_width);
        catalog::multi_mode_detected(self.value(0xf4), self.value(0xf5), half_width)
    }

    fn connector(&self) -> &str {
        match &self.conn {
            Conn::Ok(monitor) => monitor.connector.as_str(),
            _ => "",
        }
    }

    /// Source of this computer, as far as the app can tell.
    ///
    /// In single mode the low byte of 0x60 is the active input. In PIP/PBP
    /// only an explicit pick or the GPU connector inference is trusted; the
    /// pair bytes can belong to either half.
    pub(crate) fn current_input(&self) -> Option<u16> {
        if let Some(source) = self.self_source {
            return Some(source);
        }
        if matches!(
            self.multi_mode(),
            MultiMode::Off | MultiMode::PictureByPicture
        ) {
            // In PBP the low byte still names the primary (left) input, which
            // is the half carried by this DisplayPort link.
            return self
                .value(0x60)
                .map(|value| value & 0xff)
                .filter(|value| *value > 0);
        }
        None
    }

    /// The source to seed both PBP halves with when entering the mode.
    ///
    /// In single mode this is the live input; when a split is already open it
    /// is the remembered source or the low byte of the 0x60 pair.
    pub(crate) fn pbp_entry_source(&self) -> Option<u16> {
        self.self_source
            .or_else(|| {
                self.value(0x60)
                    .map(|value| value & 0xff)
                    .filter(|value| *value > 0)
            })
            .or_else(|| self.current_input())
    }

    /// Keeps `self_source` in sync with the monitor.
    ///
    /// In single mode, 0x60 is the input in use, so it is authoritative. In a
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

    /// Recomputes the drawn panel shape from the model, EDID and desktop mode.
    ///
    /// A split changes the exposed timing to one half, so the desktop mode is
    /// only used in single mode. A shape measured from the desktop is kept
    /// while the split is active instead of downgrading to the fallback.
    fn refresh_panel(&mut self) {
        let mode = self.multi_mode();
        if mode != MultiMode::Off && self.panel.source == PanelSource::Desktop {
            return;
        }
        let connector = self.connector().to_string();
        let model = match &self.conn {
            Conn::Ok(monitor) => monitor.model.clone(),
            _ => String::new(),
        };
        let desktop = if mode == MultiMode::Off {
            self.displays.output_size
        } else {
            None
        };
        self.panel = panel::detect(&connector, &model, desktop);
    }

    /// Sentence under the screen title that says where the shape came from.
    fn panel_note(&self) -> &'static str {
        match self.panel.source {
            PanelSource::Edid => self.t(Key::ScreenAspectEdid),
            PanelSource::Model => self.t(Key::ScreenAspectModel),
            PanelSource::Desktop => self.t(Key::ScreenAspectDesktop),
            PanelSource::Default => self.t(Key::ScreenAspectDefault),
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

    /// Pair usable for editing PBP, even when the monitor reports one half.
    ///
    /// In PBP the unit often answers 0x60 with only the primary byte (for
    /// example 0x0031). Treat that as the left source and mirror it into the
    /// missing right half so a picker can still write a complete pair; the
    /// user can then replace either side.
    fn pbp_pair_for_edit(&self) -> Option<(u16, u16)> {
        self.split_pair().or_else(|| {
            let left = self
                .value(0x60)
                .map(|value| value & 0xff)
                .filter(|value| *value > 0)?;
            let right = Some(self.win_sources[1])
                .filter(|value| *value != 0)
                .unwrap_or(left);
            Some((left, right))
        })
    }

    /// PIP rectangle reported by the monitor, as fractions of the picture.
    ///
    /// 0x95..0x98 take panel pixels, not percentages: on the R45w-30 the
    /// maxima are 4960/1320/5120/1440. An empty, inverted or out-of-range
    /// rectangle is ignored rather than drawn.
    fn pip_rect_from_registers(&self) -> Option<[f32; 4]> {
        let left = self.value(0x95)?;
        let top = self.value(0x96)?;
        let right = self.value(0x97)?;
        let bottom = self.value(0x98)?;
        let (width, height) = self.panel.pixel_size();
        if right > left && bottom > top && right as u32 <= width && bottom as u32 <= height {
            Some([
                left as f32 / width as f32,
                top as f32 / height as f32,
                right as f32 / width as f32,
                bottom as f32 / height as f32,
            ])
        } else {
            None
        }
    }

    /// Corner of the PIP window: the reported rectangle, then the last choice.
    pub(crate) fn pip_corner_from_registers(&self) -> catalog::PipCorner {
        let (Some(left), Some(top), Some(right), Some(bottom)) = (
            self.value(0x95),
            self.value(0x96),
            self.value(0x97),
            self.value(0x98),
        ) else {
            return self.pip_corner;
        };
        let (width, height) = self.panel.pixel_size();
        catalog::PipCorner::from_pixels(left, top, right, bottom, width, height)
            .unwrap_or(self.pip_corner)
    }

    /// Source this computer drives the monitor from.
    ///
    /// 0x60 is the register the monitor actually displays, so it wins over
    /// the configured "this computer" source. In a split the low byte is the
    /// left/primary window and the high byte the right/secondary one; using
    /// the high byte here is what made HDMI the main picture and USB-C the
    /// PIP window.
    fn main_source(&self) -> Option<u16> {
        match self.value(0x60) {
            Some(raw) if raw > 0xff => Some(raw & 0xff),
            Some(raw) if raw != 0 => Some(raw),
            _ => self
                .self_source
                .or_else(|| self.split_pair().map(|pair| pair.0)),
        }
    }

    /// Saves the current single picture as the rescue baseline.
    ///
    /// A split is deliberately refused: the rescue tab exists to undo a
    /// half-written split, so the saved state must be a plain picture.
    pub(crate) fn save_rescue_baseline(&mut self, manual: bool) {
        let result = if !self.connected() {
            Err(Key::RescueNoMonitor)
        } else if catalog::multi_mode(self.value(0xf4)) != MultiMode::Off {
            Err(Key::RescueNeedSingle)
        } else {
            self.value(0x60)
                .filter(|value| *value > 0 && *value <= 0xff)
                .map(|source| crate::settings::RescueBase {
                    source,
                    f2: self.value(0xf2).unwrap_or(0),
                    f5: self.value(0xf5).unwrap_or(0),
                    brightness: self.value(0x10),
                    contrast: self.value(0x12),
                    saved_at: crate::settings::unix_now(),
                })
                .ok_or(Key::RescueNeedSingle)
        };
        match result {
            Ok(base) => {
                self.settings.rescue = Some(base);
                if let Err(error) = self.settings.save() {
                    if manual {
                        let text = self.tf(Key::ErrSaveSettings, error);
                        self.toast = Some((text, true, Instant::now()));
                    }
                } else if manual {
                    self.rescue_status = Some((Key::RescueSaved, false));
                }
            }
            Err(key) => {
                if manual {
                    self.rescue_status = Some((key, true));
                }
            }
        }
    }

    /// Sends a hard single-picture recovery to the worker.
    ///
    /// A saved baseline is optional: when there is none, the current input is
    /// used and only the mode reset is applied. This is deliberate — the rescue
    /// tab has to work after a split that left the monitor unusable, even if
    /// the user never saved a baseline.
    pub(crate) fn restore_rescue(&mut self) {
        if !self.connected() || self.busy {
            self.rescue_status = Some((Key::RescueNoMonitor, true));
            return;
        }
        let base = self.settings.rescue.clone();
        let source = base
            .as_ref()
            .map(|base| base.source)
            .or(self.self_source)
            .or_else(|| {
                self.value(0x60)
                    .map(|value| value & 0xff)
                    .filter(|value| *value > 0)
            })
            .unwrap_or(0x31);
        self.rescue_after = None;
        self.rescue_pending = true;
        self.send(Cmd::Rescue {
            source,
            brightness: base.as_ref().and_then(|base| base.brightness),
            contrast: base.as_ref().and_then(|base| base.contrast),
        });
    }

    /// Re-checks the connection while armed and fires the one-shot restore
    /// after the monitor comes back.
    fn maybe_rescue(&mut self, ctx: &egui::Context) {
        // Poll even while connected: after a black screen the monitor can
        // still answer DDC until it is unplugged, and only a later probe
        // notices that it went away and came back.
        if self.settings.rescue_armed
            && !self.busy
            && !self.rescue_pending
            && self.rescue_probe.elapsed() >= Duration::from_secs(3)
        {
            self.rescue_probe = Instant::now();
            self.send(Cmd::Init);
        }
        if let Some(at) = self.rescue_after {
            if Instant::now() >= at && !self.busy && !self.rescue_pending {
                self.rescue_after = None;
                if self.connected() {
                    self.restore_rescue();
                }
            }
        }
        if self.settings.rescue_armed || self.rescue_after.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    /// The source currently sitting in the PIP window, if known.
    fn pip_source(&self) -> Option<u16> {
        self.split_pair()
            .map(|pair| pair.1)
            .or_else(|| Some(self.win_sources[1]).filter(|code| *code != 0))
    }

    /// Fallback source when PIP is opened without one. The monitor may keep a
    /// stale PIP source (often an unconnected DisplayPort) in 0xF0, so do not
    /// trust it: mirror the current picture until the user picks a second
    /// source in the PIP card.
    fn default_pip_source(&self) -> Option<u16> {
        self.main_source()
    }

    /// Opens PIP, leaving a True Split first.
    ///
    /// The worker closes PBP with the full single-picture plan and checks
    /// 0xF4 before it writes the PIP selector. Writing only `0xA5 = 0` first
    /// left the PBP window open; the following `0xA5 = 0x0100` froze the
    /// monitor's OSD and the source write turned the main picture black.
    pub(crate) fn enter_pip(&mut self, corner: catalog::PipCorner) {
        // Only a source the user picked in this session is a safe default;
        // 0xF0/split_pair can still hold an unconnected input from an old PBP.
        let main = self.main_source();
        let chosen = Some(self.win_sources[1]).filter(|code| *code != 0 && Some(*code) != main);
        let source = chosen.or_else(|| self.default_pip_source());
        self.enter_pip_with(corner, source);
    }

    fn enter_pip_with(&mut self, corner: catalog::PipCorner, source: Option<u16>) {
        if !self.can_edit() {
            return;
        }
        let main = self.main_source();
        // Trust the monitor's F4 status register over the cached mode: after a
        // failed or partial attempt the UI hint can disagree with the panel.
        let f4_mode = catalog::multi_mode(self.value(0xf4));
        let leave_pbp = f4_mode == MultiMode::PictureByPicture
            || (f4_mode == MultiMode::Unknown
                && matches!(
                    self.multi_mode(),
                    MultiMode::PictureByPicture | MultiMode::Unknown
                ));
        let (width, height) = self.panel.pixel_size();
        self.pip_corner = corner;
        self.mode_hint = Some(MultiMode::PictureInPicture);
        self.send(Cmd::ApplyPip {
            corner: corner.rect(width, height),
            main,
            source,
            leave_pbp,
        });
    }

    /// Source picker for one window of a split.
    ///
    /// Picking a source writes VCP 0x60 as `(right << 8) | left` — the same
    /// pair Lenovo's service writes. No selector and no swap are involved,
    /// so the right half is reached directly and the monitor's OSD stays alive.
    fn window_source_picker(&mut self, ui: &mut Ui, window: usize) {
        let pip = self.multi_mode() == MultiMode::PictureInPicture;
        // In PIP there is no "right half": the second picture is the floating
        // PIP window, so the picker uses that name instead of a side.
        let label = if pip {
            self.t(Key::ModePip)
        } else if window == 0 {
            self.t(Key::WindowLeft)
        } else {
            self.t(Key::WindowRight)
        };
        ui.label(RichText::new(label).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let sources = self.input_sources();
        let pair = self.pbp_pair_for_edit();
        let current = if pip {
            self.pip_source().unwrap_or(0)
        } else {
            pair.map(|p| if window == 0 { p.0 } else { p.1 })
                .or_else(|| Some(self.win_sources[window]).filter(|code| *code != 0))
                .or_else(|| (window == 0).then(|| self.current_input()).flatten())
                .unwrap_or(0)
        };
        // A PIP source is independent of the F0 pair: the monitor can accept
        // it even when the pair register is unreadable. In PBP a half that
        // the monitor has not reported yet can still be written: the other
        // half starts from this computer's input.
        let enabled = (pip
            || pair.is_some()
            || self.current_input().is_some()
            || self.win_sources[window] != 0)
            && !self.busy
            && matches!(self.conn, Conn::Ok(_));
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
        if !pip && pair.is_none() && !enabled {
            ui.add_space(4.0);
            ui.label(
                RichText::new(self.t(Key::WindowsUnknown))
                    .size(11.0)
                    .color(DIM),
            );
        }
        ui.add_space(6.0);
    }

    /// Source picker for the floating PIP window.
    ///
    /// Picking a source also opens PIP when it is off: the monitor keeps the
    /// window source independently of the rectangle, so this is the shortest
    /// path from "single" to a visible second picture.
    fn pip_source_picker(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(self.t(Key::PipSource)).size(13.0).color(TEXT));
        ui.add_space(6.0);
        let sources = self.input_sources();
        let current = Some(self.win_sources[1])
            .filter(|code| *code != 0)
            .or_else(|| self.pip_source())
            .unwrap_or(0);
        let mut chosen: Option<u16> = None;
        ui.add_enabled_ui(!self.busy && matches!(self.conn, Conn::Ok(_)), |ui| {
            ui.horizontal_wrapped(|ui| {
                for (code, name) in &sources {
                    if pill(ui, name, current == *code).clicked() {
                        chosen = Some(*code);
                    }
                }
            });
        });
        if let Some(code) = chosen {
            if self.pip_source() == Some(code) {
                return;
            }
            // Resolve the previous mode before the hint is set: a source pick
            // while PIP is off must also place the window, not just write 0xA5.
            let was_pip = self.multi_mode() == MultiMode::PictureInPicture;
            let corner = if was_pip {
                self.pip_corner
            } else {
                self.pip_corner_from_registers()
            };
            self.win_sources[1] = code;
            if was_pip {
                self.pip_corner = corner;
                self.mode_hint = Some(MultiMode::PictureInPicture);
                self.send(Cmd::ApplyWindow {
                    selector: 0x0100,
                    source: code,
                    swap: true,
                });
            } else {
                self.enter_pip_with(corner, Some(code));
            }
        }
        ui.add_space(6.0);
    }

    /// Writes the source of one window of a split.
    ///
    /// In PBP both halves are written together to 0x60; in PIP the monitor
    /// keeps its separate window selector `0xA5 = 0x0100`, so the source can
    /// be set even when the F0 pair is unreadable. True Split has no second
    /// source and never reaches this function.
    fn apply_window_source(&mut self, window: usize, source: u16) {
        if self.busy || !matches!(self.conn, Conn::Ok(_)) {
            return;
        }
        match self.multi_mode() {
            MultiMode::PictureByPicture => {
                // If the monitor has not reported the pair yet, start the
                // unknown half from this computer's input and set the chosen
                // one; writing a zero byte would select an invalid input.
                let pair = self.pbp_pair_for_edit();
                let Some((left, right)) = pair else {
                    self.toast = Some((
                        self.t(Key::WindowsUnknown).to_string(),
                        true,
                        Instant::now(),
                    ));
                    return;
                };
                let (left, right) = if window == 0 {
                    (source, right)
                } else {
                    (left, source)
                };
                // A valid layout is left untouched: changing only a source
                // must not blank the picture with a redundant F5 write.
                let layout = if self
                    .value(0xf5)
                    .and_then(catalog::split_layout_for)
                    .is_some()
                {
                    None
                } else {
                    Some(catalog::SplitLayout::Half.value())
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
                if self.pip_source() == Some(source) {
                    return;
                }
                self.win_sources[1] = source;
                self.mode_hint = Some(MultiMode::PictureInPicture);
                self.send(Cmd::ApplyWindow {
                    selector: 0x0100,
                    source,
                    swap: true,
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
                    let connected = matches!(c, Conn::Ok(_));
                    let was_connected = matches!(self.conn, Conn::Ok(_));
                    self.vals.clear();
                    self.pending.clear();
                    self.mode_hint = None;
                    if connected {
                        self.rescue_probe = Instant::now();
                        if connected
                            && !was_connected
                            && self.settings.rescue_armed
                            && self.settings.rescue.is_some()
                            && !self.rescue_pending
                        {
                            self.rescue_after = Some(Instant::now() + Duration::from_secs(1));
                        }
                    } else {
                        self.self_source = None;
                        self.win_sources = [0, 0];
                        self.displays = Displays::default();
                        self.feature_values.clear();
                        self.rescue_after = None;
                    }
                    self.conn = c;
                    self.panel = Panel::default();
                    self.refresh_panel();
                }
                Ev::Caps(c) => self.caps = c,
                Ev::Displays(d) => {
                    self.displays = d;
                    self.refresh_panel();
                }
                Ev::Values(v) => {
                    self.partial_read = crate::ddc::CORE.iter().any(|code| !v.contains_key(code))
                        || self.vals.keys().any(|code| !v.contains_key(code));
                    self.vals = v;
                    self.pending.clear();
                    // Readback is the truth. An optimistic mode hint must not
                    // outlive it, or a PIP window opened from the OSD stays
                    // invisible until the monitor disconnects.
                    self.mode_hint = None;
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
                    self.refresh_panel();
                    if self.settings.rescue.is_none() {
                        self.save_rescue_baseline(false);
                    }
                }
                Ev::Busy(b) => self.busy = b,
                Ev::Feature { index, value } => {
                    if let Some(value) = value {
                        self.feature_values.insert(index, value);
                    }
                }
                Ev::Notice(key, argument, err) => {
                    if matches!(
                        key,
                        Key::WriteFailed | Key::WriteMismatch | Key::WriteUnconfirmed
                    ) {
                        self.mode_hint = None;
                    }
                    let msg = self.tf(key, &argument);
                    self.log.push(LogLine::Key(key, vec![argument]));
                    self.toast = Some((msg, err, Instant::now()));
                    if self.rescue_pending {
                        self.rescue_pending = false;
                        match key {
                            Key::WriteConfirmed => {
                                self.settings.rescue_armed = false;
                                if let Err(error) = self.settings.save() {
                                    self.log.push(LogLine::Text(error));
                                }
                                self.rescue_status = Some((Key::RescueRestored, false));
                                self.toast = Some((
                                    self.t(Key::RescueRestored).into(),
                                    false,
                                    Instant::now(),
                                ));
                            }
                            _ => {
                                // The worker can also answer ErrNotConnected when
                                // the monitor vanishes between probes. Keep the
                                // baseline armed, but show that this attempt
                                // failed instead of silently waiting forever.
                                self.rescue_status = Some((Key::RescueFailed, true));
                                self.toast =
                                    Some((self.t(Key::RescueFailed).into(), true, Instant::now()));
                            }
                        }
                    }
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
        self.maybe_rescue(ctx);

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
