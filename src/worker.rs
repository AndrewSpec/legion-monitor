//! Worker thread — all monitor communication happens here, so the UI never freezes.

use crate::catalog::{self, MultiMode};
use crate::ddc::{self, Capabilities, Monitor, VcpValue};
use crate::i18n::Key;
use crate::topology::{self, Displays};
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

/// Codes we always want to know (in addition to what the monitor reports).
const ALWAYS: [u8; 7] = ddc::CORE;

pub enum Cmd {
    Init,
    Refresh,
    Set {
        code: u8,
        value: u16,
    },
    Apply(Vec<(u8, u16)>),
    /// Source of one window of a PIP split: `0xA5 = 0x0100` selects the PIP
    /// window and `0x60` sets its source.
    ApplyWindow {
        selector: u16,
        source: u16,
    },
    /// Sets both halves of a picture-by-picture split in one step.
    ///
    /// Lenovo's service writes VCP 0x60 as `(secondary << 8) | primary`, so
    /// the monitor can be given both sources directly. No `0xF2` swap is
    /// involved. `layout` is written only when it has to be repaired; a valid
    /// layout is left untouched so changing a source does not blank the picture.
    ApplySplit {
        left: u16,
        right: u16,
        layout: Option<u16>,
        enable_pbp: bool,
    },
    /// Enters picture-by-picture, optionally choosing the layout first.
    SetPbp {
        layout: Option<u16>,
    },
    /// Places the PIP window and stages its source.
    ///
    /// The monitor exposes the PIP rectangle as VCP 0x95..0x98 in panel
    /// pixels; 0xA4 masks the window while it moves. `main` and `source` are
    /// staged together in 0x60 before the PIP selector is written, so a
    /// selector the monitor ignores cannot change the main picture. The UI
    /// closes a True Split first; writing 0xA5 = 0x0100 while PBP was open
    /// froze the monitor's OSD.
    ApplyPip {
        corner: (u16, u16, u16, u16),
        main: Option<u16>,
        source: Option<u16>,
        leave_pbp: bool,
    },
    /// Closes the PIP/PBP window and returns to a single picture.
    ///
    /// `source` is the monitor input this computer runs on. When it is given,
    /// both halves are first pointed at that source and the single picture is
    /// confirmed on it, so closing the split cannot leave the desktop black.
    DisableMultiView {
        source: Option<u16>,
        /// PbP needs both halves pointed at our source before closing; in PiP
        /// only the main input matters.
        pbp: bool,
    },
    /// Rescue mode: restores the saved single picture even when the monitor
    /// currently shows nothing usable.
    Rescue {
        source: u16,
        brightness: Option<u16>,
        contrast: Option<u16>,
    },
}

#[derive(Clone, Debug)]
pub enum Conn {
    Probing,
    Ok(Monitor),
    Err(String),
}

/// A log entry: either raw technical text or a translation key with its
/// placeholder arguments, formatted when the entry is rendered.
pub enum LogLine {
    /// Text passed through unchanged (ddcutil output, OS errors).
    Text(String),
    /// Translation key plus the arguments for its `{}` placeholders.
    Key(Key, Vec<String>),
}

pub enum Ev {
    Conn(Conn),
    Caps(Capabilities),
    Values(BTreeMap<u8, VcpValue>),
    Displays(Displays),
    Busy(bool),
    Notice(Key, String, bool),
    Log(LogLine),
}

pub struct Worker {
    pub tx: Sender<Cmd>,
    pub rx: Receiver<Ev>,
}

impl Worker {
    pub fn spawn() -> Self {
        let (ctx, crx) = std::sync::mpsc::channel::<Cmd>();
        let (etx, erx) = std::sync::mpsc::channel::<Ev>();

        thread::Builder::new()
            .name("ddc-worker".into())
            .spawn(move || {
                let mut mon: Option<Monitor> = None;
                let mut caps = Capabilities::default();

                while let Ok(cmd) = crx.recv() {
                    match cmd {
                        Cmd::Init | Cmd::Refresh => {
                            let _ = etx.send(Ev::Busy(true));
                            // Refresh must rediscover the display: its number/EDID can
                            // change after a mode switch or reconnection.
                            {
                                caps = Capabilities::default();
                                let _ = etx.send(Ev::Caps(caps.clone()));
                                match ddc::detect() {
                                    Ok(m) => {
                                        let _ = etx.send(Ev::Log(LogLine::Key(
                                            Key::LogDetected,
                                            vec![m.display.to_string(), m.bus.clone()],
                                        )));
                                        let _ = etx.send(Ev::Conn(Conn::Ok(m.clone())));
                                        mon = Some(m);
                                    }
                                    Err(e) => {
                                        let _ = etx.send(Ev::Conn(Conn::Err(e.clone())));
                                        let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                        mon = None;
                                        caps = Capabilities::default();
                                        let _ = etx.send(Ev::Caps(caps.clone()));
                                        let _ = etx.send(Ev::Displays(Displays::default()));
                                        let _ = etx.send(Ev::Busy(false));
                                        continue;
                                    }
                                }
                                let displays = mon
                                    .as_ref()
                                    .map(|m| topology::scan(&m.connector))
                                    .unwrap_or_default();
                                let _ = etx.send(Ev::Displays(displays));
                                if let Some(m) = &mon {
                                    match ddc::capabilities(m.display) {
                                        Ok(c) => {
                                            let _ = etx.send(Ev::Log(LogLine::Key(
                                                Key::LogFeatures,
                                                vec![c.features.len().to_string()],
                                            )));
                                            caps = c.clone();
                                            let _ = etx.send(Ev::Caps(c));
                                        }
                                        Err(e) => {
                                            let _ = etx.send(Ev::Log(LogLine::Key(
                                                Key::LogCapsFailed,
                                                vec![e],
                                            )));
                                        }
                                    }
                                }
                            }
                            if let Some(m) = &mon {
                                let vals = read_codes(m.display, &caps);
                                let _ = etx.send(Ev::Values(vals));
                            }
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::Set { code, value } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            let _ = etx.send(Ev::Log(LogLine::Key(
                                Key::LogWrite,
                                vec![format!("{code:02X}"), value.to_string()],
                            )));
                            let result = ddc::set(m.display, code, value);
                            settle();
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) =
                                write_feedback(&[(code, value)], &vals, result.err());
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::Apply(list) => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            let mut failure = None;
                            for &(code, value) in &list {
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    failure = Some(e);
                                    break;
                                }
                                if matches!(code, 0x60 | 0xf2 | 0xf5) {
                                    settle();
                                } else {
                                    thread::sleep(Duration::from_millis(120));
                                }
                            }
                            settle();
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = write_feedback(&list, &vals, failure);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }
                        Cmd::ApplyWindow { selector, source } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            write_log(&etx, 0xa5, selector);
                            let mut failure = ddc::set(m.display, 0xa5, selector).err();
                            if failure.is_none() {
                                // The monitor needs a moment before the selected
                                // window accepts a new source.
                                thread::sleep(Duration::from_millis(300));
                                write_log(&etx, 0x60, source);
                                failure = ddc::set(m.display, 0x60, source).err();
                            }
                            settle();
                            let vals = read_codes(m.display, &caps);
                            // Only 0x60 can be read back; 0xA5 always reads 0.
                            let (key, argument, error) =
                                write_feedback(&[(0x60, source)], &vals, failure);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::ApplySplit {
                            left,
                            right,
                            layout,
                            enable_pbp,
                        } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            let mut failure = None;
                            for &(code, value) in &split_plan(left, right, layout, enable_pbp) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    failure = Some(e);
                                    break;
                                }
                                settle();
                            }
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) =
                                split_feedback(left, right, &vals, failure);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::SetPbp { layout } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            // Lenovo's SetPIPPBPState maps PBP to 0xA5 = 0x0200;
                            // 0x0100 in PBP froze the monitor's OSD.
                            let mut failure = None;
                            for &(code, value) in &pbp_plan(layout) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    failure = Some(e);
                                    break;
                                }
                                settle();
                            }
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) =
                                mode_feedback(MultiMode::PictureByPicture, &vals, failure);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::ApplyPip {
                            corner,
                            main,
                            source,
                            leave_pbp,
                        } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            let mut failure = None;
                            let mut stopped = None;

                            if leave_pbp {
                                match main {
                                    None => stopped = Some(Key::ErrSelfSourceUnknown),
                                    Some(main_source) => {
                                        for &(code, value) in &single_plan(Some(main_source), true)
                                        {
                                            write_log(&etx, code, value);
                                            if let Err(e) = ddc::set(m.display, code, value) {
                                                // The monitor answers EIO while it
                                                // switches modes but applies the
                                                // write; the state probe below
                                                // decides whether it worked.
                                                let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                            }
                                            if code == 0xa5 {
                                                thread::sleep(Duration::from_millis(400));
                                            } else if code == 0x60 {
                                                settle();
                                            } else {
                                                thread::sleep(Duration::from_millis(120));
                                            }
                                        }
                                        settle();
                                        thread::sleep(Duration::from_millis(300));
                                        // 0xF4 reads 0x0201 while the PBP window
                                        // is still open and 0x0200 after it closes.
                                        // Retry because a read during the mode
                                        // change can fail; refuse to write the PIP
                                        // selector while True Split is reported.
                                        if failure.is_none() {
                                            let mut probe = None;
                                            for attempt in 0..3 {
                                                if let Some(value) = ddc::read(m.display, &[0xf4])
                                                    .get(&0xf4)
                                                    .map(|v| v.value)
                                                {
                                                    probe = Some(value);
                                                    if value != 0x0201 {
                                                        break;
                                                    }
                                                }
                                                if attempt < 2 {
                                                    thread::sleep(Duration::from_millis(300));
                                                }
                                            }
                                            match probe {
                                                Some(0x0201) => stopped = Some(Key::WriteMismatch),
                                                None => stopped = Some(Key::WriteUnconfirmed),
                                                Some(_) => {}
                                            }
                                        }
                                    }
                                }
                            }

                            // Place the rectangle before selecting the PIP
                            // window. The monitor accepts and reports 0x95..0x98
                            // in single mode, so a failed selector cannot leave
                            // a split with a zero rectangle.
                            if failure.is_none() && stopped.is_none() {
                                for &(code, value) in &pip_rect_plan(corner) {
                                    write_log(&etx, code, value);
                                    if let Err(e) = ddc::set(m.display, code, value) {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                        failure = Some(e);
                                        break;
                                    }
                                    thread::sleep(Duration::from_millis(120));
                                }
                            }

                            // The source may only be written once the rectangle
                            // reads back and a window is actually open. In single
                            // mode the same write changes the main picture, which
                            // is how the first PIP attempt went black.
                            if failure.is_none() && stopped.is_none() {
                                let mut rect_seen = None;
                                for attempt in 0..3 {
                                    let vals = ddc::read(
                                        m.display,
                                        &[0x95, 0x96, 0x97, 0x98],
                                    );
                                    let got = |code: u8| {
                                        vals.get(&code).map(|value| value.value)
                                    };
                                    match (
                                        got(0x95),
                                        got(0x96),
                                        got(0x97),
                                        got(0x98),
                                    ) {
                                        (Some(left), Some(top), Some(right), Some(bottom))
                                            if (left, top, right, bottom) == corner =>
                                        {
                                            rect_seen = Some(true);
                                            break;
                                        }
                                        (Some(left), Some(top), Some(right), Some(bottom)) => {
                                            rect_seen = Some(false);
                                            let _ = etx.send(Ev::Log(LogLine::Text(format!(
                                                "PIP rectangle readback {left},{top},{right},{bottom}"
                                            ))));
                                            break;
                                        }
                                        _ => {}
                                    }
                                    if attempt < 2 {
                                        thread::sleep(Duration::from_millis(300));
                                    }
                                }
                                match rect_seen {
                                    Some(true) => {}
                                    Some(false) => stopped = Some(Key::WriteMismatch),
                                    None => stopped = Some(Key::WriteUnconfirmed),
                                }
                            }
                            // Only select the PIP window after the rectangle is
                            // already valid. A5 answers EIO even when it takes
                            // effect, so the F4 probe below decides.
                            if failure.is_none() && stopped.is_none() {
                                write_log(&etx, 0xa5, 0x0100);
                                if let Err(e) = ddc::set(m.display, 0xa5, 0x0100) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                }
                                thread::sleep(Duration::from_millis(400));
                            }
                            if failure.is_none() && stopped.is_none() {
                                let mut active = None;
                                for attempt in 0..3 {
                                    if let Some(value) = ddc::read(m.display, &[0xf4])
                                        .get(&0xf4)
                                        .map(|v| v.value)
                                    {
                                        active = Some(value);
                                        if value == 0x0201 {
                                            break;
                                        }
                                    }
                                    if attempt < 2 {
                                        thread::sleep(Duration::from_millis(300));
                                    }
                                }
                                match active {
                                    Some(0x0201) => {}
                                    Some(_) => stopped = Some(Key::WriteMismatch),
                                    None => stopped = Some(Key::WriteUnconfirmed),
                                }
                            }
                            if failure.is_none() && stopped.is_none() {
                                if let Some(source) = source {
                                    // The selector is already confirmed above;
                                    // this write only changes the PIP window.
                                    write_log(&etx, 0x60, source);
                                    if let Err(e) = ddc::set(m.display, 0x60, source) {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                        failure = Some(e);
                                    }
                                    settle();
                                }
                            }
                            settle();
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = match stopped {
                                Some(key) => (key, String::new(), true),
                                None => pip_feedback(corner, &vals, failure),
                            };
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::Rescue {
                            source,
                            brightness,
                            contrast,
                        } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            for &(code, value) in &rescue_plan(source, brightness, contrast) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    // Mode writes answer EIO while they land;
                                    // the readback below decides the outcome.
                                    let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                }
                                match code {
                                    0xa5 => thread::sleep(Duration::from_millis(400)),
                                    0x60 => settle(),
                                    _ => thread::sleep(Duration::from_millis(120)),
                                }
                            }
                            settle();
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = single_feedback(Some(source), &vals, None);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }

                        Cmd::DisableMultiView { source, pbp } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            for &(code, value) in &single_plan(source, pbp) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    // A5 and 0x60 both answer EIO during a mode
                                    // change even when they land; the readback
                                    // below decides whether the desktop is on
                                    // our source.
                                    let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                }
                                settle();
                            }
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = single_feedback(source, &vals, None);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            let _ = etx.send(Ev::Busy(false));
                        }
                    }
                }
            })
            .expect("cannot start the DDC worker thread");

        Worker { tx: ctx, rx: erx }
    }
}

/// Writes that enter PBP and set its layout, in the order Lenovo uses.
fn pbp_plan(layout: Option<u16>) -> Vec<(u8, u16)> {
    let mut plan = vec![(0xa5, 0x0200)];
    if let Some(value) = layout {
        plan.push((0xf5, value));
    }
    plan
}

/// Writes that set both halves of a PBP split. A valid layout is omitted, so
/// changing a source never blanks the picture with a redundant F5 write.
fn split_plan(left: u16, right: u16, layout: Option<u16>, enable_pbp: bool) -> Vec<(u8, u16)> {
    let mut plan = Vec::new();
    if enable_pbp {
        plan.push((0xa5, 0x0200));
    }
    if let Some(value) = layout {
        plan.push((0xf5, value));
    }
    plan.push((0x60, catalog::window_pair(left, right)));
    plan
}

/// Writes that close a split without losing the desktop.
///
/// Both halves are pointed at the computer's source before the split closes,
/// so whichever half the monitor keeps already shows the desktop. The final
/// single-source write makes 0x60 a plain input code again.
/// Leaves True Split or PIP for one picture.
///
/// The pair and `0xA5 = 0` close a True Split, but an active PIP window also
/// needs `0xF2 = 0`: on the test unit the pair/selector/source writes alone
/// left `0xF2 = 0x0101` and the PIP window on screen. `0xF2 = 0` cleared it.
fn single_plan(source: Option<u16>, pbp: bool) -> Vec<(u8, u16)> {
    match (source, pbp) {
        (Some(source), true) => vec![
            (0x60, catalog::window_pair(source, source)),
            (0xa5, 0),
            (0x60, source),
            (0xf2, 0),
        ],
        (Some(source), false) => vec![(0xa5, 0), (0x60, source), (0xf2, 0)],
        (None, _) => vec![(0xa5, 0), (0xf2, 0)],
    }
}

/// Restores the saved picture and, when the monitor exposes them, the saved
/// brightness and contrast. The single-picture writes come first so the
/// picture is usable even if one of the picture settings fails.
fn rescue_plan(source: u16, brightness: Option<u16>, contrast: Option<u16>) -> Vec<(u8, u16)> {
    let mut plan = single_plan(Some(source), true);
    if let Some(value) = brightness {
        plan.push((0x10, value));
    }
    if let Some(value) = contrast {
        plan.push((0x12, value));
    }
    plan
}

/// Writes that enter PIP and place its window.
///
/// The rectangle is 0x95 left, 0x96 top, 0x97 right, 0x98 bottom in panel
/// pixels. 0xA4 is deliberately not written: the monitor rejects the show
/// value in this path and can leave the whole picture masked. The input source
/// is not part of this plan either; the command arm writes it only after the
/// rectangle reads back and 0xF4 reports an open window.
fn pip_rect_plan(corner: (u16, u16, u16, u16)) -> Vec<(u8, u16)> {
    let (left, top, right, bottom) = corner;
    vec![(0x95, left), (0x96, top), (0x97, right), (0x98, bottom)]
}

/// Checks the PIP rectangle back. 0xA5 and 0xA4 are not readable state
/// (they read as zero), so the geometry is the only useful confirmation.
fn pip_feedback(
    corner: (u16, u16, u16, u16),
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let read = |code: u8| observed.get(&code).map(|value| value.value);
    match (read(0x95), read(0x96), read(0x97), read(0x98)) {
        (Some(left), Some(top), Some(right), Some(bottom)) => {
            if (left, top, right, bottom) == corner {
                (Key::WriteConfirmed, "1".to_string(), false)
            } else {
                (Key::WriteMismatch, "1".to_string(), true)
            }
        }
        _ => (Key::WriteUnconfirmed, String::new(), true),
    }
}

fn write_log(etx: &Sender<Ev>, code: u8, value: u16) {
    let _ = etx.send(Ev::Log(LogLine::Key(
        Key::LogWrite,
        vec![format!("0x{code:02X}"), format!("0x{value:04X}")],
    )));
}

/// Checks a two-source PBP write. The monitor sometimes reports the pair in
/// 0x60 and sometimes only in 0xF0, so either register is accepted.
fn split_feedback(
    left: u16,
    right: u16,
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let pair = catalog::window_pair(left, right);
    let in_input = observed.get(&0x60).is_some_and(|v| v.value == pair);
    let in_windows =
        catalog::window_sources(observed.get(&0xf0).map(|v| v.value)) == Some((left, right));
    if in_input || in_windows {
        (Key::WriteConfirmed, "1".to_string(), false)
    } else {
        (Key::WriteMismatch, "1".to_string(), true)
    }
}

/// Checks that closing the split ended in single mode on our source.
///
/// Only the low byte of 0x60 matters after the split closes: it is the active
/// input. The high byte may still carry a stale half from the best-effort
/// pre-write, which is harmless.
fn single_feedback(
    source: Option<u16>,
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let have = catalog::multi_mode(
        observed.get(&0xf2).map(|v| v.value),
        observed.get(&0xf4).map(|v| v.value),
    );
    // A closed split reports a single-byte 0x60. Accept that even when F2/F4
    // still carry a stale split value; a live PBP reports the full pair.
    let single = observed
        .get(&0x60)
        .map(|v| v.value)
        .filter(|value| *value <= 0xff);
    if have != MultiMode::Off && !(source.is_some() && single.is_some()) {
        return (Key::WriteMismatch, "1".to_string(), true);
    }
    if let Some(source) = source {
        if observed.get(&0x60).map(|v| v.value & 0xff) != Some(source & 0xff) {
            return (Key::WriteMismatch, "1".to_string(), true);
        }
    }
    (Key::WriteConfirmed, "1".to_string(), false)
}

/// Checks that the mode reported by F2/F4 is the one that was requested.
fn mode_feedback(
    want: MultiMode,
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let have = catalog::multi_mode(
        observed.get(&0xf2).map(|v| v.value),
        observed.get(&0xf4).map(|v| v.value),
    );
    if have == want {
        (Key::WriteConfirmed, "1".to_string(), false)
    } else {
        (Key::WriteMismatch, "1".to_string(), true)
    }
}

/// A successful transport (--noverify) is not proof that the monitor changed.
fn write_feedback(
    requested: &[(u8, u16)],
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let mismatched = requested
        .iter()
        .filter(|(c, v)| observed.get(c).is_some_and(|o| o.value != *v))
        .count();
    let missing = requested
        .iter()
        .filter(|(c, _)| !observed.contains_key(c))
        .count();
    if mismatched > 0 {
        (Key::WriteMismatch, mismatched.to_string(), true)
    } else if missing > 0 || requested.is_empty() {
        (Key::WriteUnconfirmed, String::new(), true)
    } else {
        (Key::WriteConfirmed, requested.len().to_string(), false)
    }
}

/// Waits for the monitor to come back after a picture switch.
fn settle() {
    thread::sleep(Duration::from_millis(1600));
}

fn codes_to_read(caps: &Capabilities) -> Vec<u8> {
    let mut codes: Vec<u8> = ALWAYS.to_vec();
    for (code, _) in &caps.features {
        if !codes.contains(code) {
            codes.push(*code);
        }
    }
    // Layout F5 and window sources F0 are sometimes missing from capabilities.
    for extra in [
        0x87u8, 0x8a, 0x8b, 0x72, 0x6d, 0x0c, 0x14, 0x62, 0x8d, 0xd6, 0xcc, 0x95, 0x96, 0x97, 0x98,
    ] {
        if !codes.contains(&extra) {
            codes.push(extra);
        }
    }
    codes
}

fn read_codes(display: u32, caps: &Capabilities) -> BTreeMap<u8, VcpValue> {
    let codes = codes_to_read(caps);
    for attempt in 0..4 {
        let vals = ddc::read(display, &codes);
        if !vals.is_empty() {
            return vals;
        }
        thread::sleep(Duration::from_millis(900 + attempt * 600));
    }
    BTreeMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_advertised_code_and_always_layout() {
        let caps = Capabilities {
            features: (0..=255).map(|c| (c, String::new())).collect(),
            ..Default::default()
        };
        let codes = codes_to_read(&caps);
        assert_eq!(codes.len(), 256);
        assert!(codes.contains(&0xfd));
        assert!(codes_to_read(&Capabilities::default()).contains(&0xf5));
    }

    #[test]
    fn rescue_without_monitor_releases_busy() {
        let worker = Worker::spawn();
        worker
            .tx
            .send(Cmd::Rescue {
                source: 0x31,
                brightness: Some(80),
                contrast: Some(60),
            })
            .expect("queue rescue");
        let mut noticed = false;
        let mut released = false;
        for _ in 0..10 {
            match worker.rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ev::Notice(Key::ErrNotConnected, _, _)) => noticed = true,
                Ok(Ev::Busy(false)) => released = true,
                Ok(_) => {}
                Err(_) => break,
            }
            if noticed && released {
                break;
            }
        }
        assert!(noticed, "the missing monitor is reported");
        assert!(released, "busy must not stay latched after a no-op command");
    }

    /// Explicit opt-in: reversible attached-monitor acceptance for the real worker.
    #[test]
    #[ignore = "requires attached LEN R45w-30 and explicit device permission"]
    fn live_reversible_picture_and_profile_roundtrip() {
        assert_eq!(std::env::var("LEGION_LIVE_TEST").as_deref(), Ok("1"));
        let expected = std::env::var("LEGION_LIVE_SERIAL")
            .expect("set LEGION_LIVE_SERIAL to the test monitor serial");
        let monitor = ddc::detect().expect("detect monitor");
        assert_eq!(
            monitor.serial, expected,
            "never write an unidentified monitor"
        );
        let original = ddc::read(monitor.display, &[0x10, 0x12]);
        let brightness = original.get(&0x10).expect("brightness baseline").value;
        let contrast = original.get(&0x12).expect("contrast baseline").value;
        struct Restore(u32, u16, u16);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = ddc::set(self.0, 0x10, self.1);
                let _ = ddc::set(self.0, 0x12, self.2);
            }
        }
        let restore = Restore(monitor.display, brightness, contrast);
        let worker = Worker::spawn();
        let finish = |worker: &Worker| {
            let mut result = BTreeMap::new();
            let mut notice = None;
            loop {
                match worker
                    .rx
                    .recv_timeout(Duration::from_secs(120))
                    .expect("worker deadline")
                {
                    Ev::Values(values) => result = values,
                    Ev::Notice(key, arg, error) => {
                        eprintln!("NOTICE {key:?}: {arg}, error={error}");
                        notice = Some((key, error));
                    }
                    Ev::Conn(Conn::Err(error)) => panic!("connection failed: {error}"),
                    Ev::Busy(false) => break,
                    _ => {}
                }
            }
            (result, notice)
        };
        worker.tx.send(Cmd::Init).unwrap();
        let (initial, _) = finish(&worker);
        assert_eq!(initial.get(&0x10).unwrap().value, brightness);
        assert!(initial.contains_key(&0xf5), "layout cannot be truncated");
        let changed = brightness.saturating_sub(5).max(1);
        worker
            .tx
            .send(Cmd::Set {
                code: 0x10,
                value: changed,
            })
            .unwrap();
        let (observed, notice) = finish(&worker);
        assert_eq!(observed.get(&0x10).unwrap().value, changed);
        assert_eq!(notice, Some((Key::WriteConfirmed, false)));
        let values = [
            ("10".into(), changed),
            ("12".into(), contrast.saturating_sub(3).max(1)),
        ]
        .into_iter()
        .collect();
        worker
            .tx
            .send(Cmd::Apply(crate::profiles::replay_plan(&values)))
            .unwrap();
        let (observed, notice) = finish(&worker);
        assert_eq!(
            observed.get(&0x12).unwrap().value,
            contrast.saturating_sub(3).max(1)
        );
        assert_eq!(notice, Some((Key::WriteConfirmed, false)));
        let baseline = [("10".into(), brightness), ("12".into(), contrast)]
            .into_iter()
            .collect();
        worker
            .tx
            .send(Cmd::Apply(crate::profiles::replay_plan(&baseline)))
            .unwrap();
        let (observed, notice) = finish(&worker);
        assert_eq!(observed.get(&0x10).unwrap().value, brightness);
        assert_eq!(observed.get(&0x12).unwrap().value, contrast);
        assert_eq!(notice, Some((Key::WriteConfirmed, false)));
        drop(restore);
        eprintln!("RESTORED brightness={brightness}, contrast={contrast}");
    }

    #[test]
    fn split_plan_sets_both_halves_without_a_swap() {
        assert_eq!(split_plan(0x11, 0x31, None, false), vec![(0x60, 0x3111)]);
        assert_eq!(
            split_plan(0x11, 0x31, Some(0x0101), true),
            vec![(0xa5, 0x0200), (0xf5, 0x0101), (0x60, 0x3111)]
        );
        assert_eq!(pbp_plan(None), vec![(0xa5, 0x0200)]);
        assert!(!split_plan(0x11, 0x31, None, false)
            .iter()
            .any(|(code, _)| *code == 0xf2));
    }

    #[test]
    fn split_feedback_accepts_either_register() {
        let value = |v: u16| VcpValue { value: v, max: v };
        let mut input = BTreeMap::new();
        input.insert(0x60, value(0x3111));
        assert!(!split_feedback(0x11, 0x31, &input, None).2);

        let mut windows = BTreeMap::new();
        windows.insert(0xf0, value(0x3111));
        assert!(!split_feedback(0x11, 0x31, &windows, None).2);

        let mut wrong = BTreeMap::new();
        wrong.insert(0x60, value(0x1211));
        assert!(split_feedback(0x11, 0x31, &wrong, None).2);
    }

    #[test]
    fn pip_rect_is_placed_before_the_window_is_selected() {
        // The rectangle is written while the monitor is still single, then
        // 0xA5 selects the PIP window. The input source is written only after
        // the rectangle and 0xF4 confirm the window.
        assert_eq!(
            pip_rect_plan((2560, 0, 5120, 720)),
            vec![(0x95, 2560), (0x96, 0), (0x97, 5120), (0x98, 720),]
        );
    }

    #[test]
    fn pip_feedback_checks_the_reported_rectangle() {
        let value = |v: u16| VcpValue {
            value: v,
            max: 5120,
        };
        let mut observed = BTreeMap::new();
        for (code, v) in [(0x95, 2560), (0x96, 0), (0x97, 5120), (0x98, 720)] {
            observed.insert(code, value(v));
        }
        assert!(!pip_feedback((2560, 0, 5120, 720), &observed, None).2);
        assert!(pip_feedback((0, 0, 2560, 720), &observed, None).2);
        assert!(pip_feedback((2560, 0, 5120, 720), &BTreeMap::new(), None).2);
    }

    #[test]
    fn rescue_plan_closes_the_split_before_picture_settings() {
        let plan = rescue_plan(0x31, Some(80), Some(60));
        assert_eq!(
            plan,
            vec![
                (0x60, 0x3131),
                (0xa5, 0),
                (0x60, 0x31),
                (0xf2, 0),
                (0x10, 80),
                (0x12, 60),
            ]
        );
    }

    #[test]
    fn single_plan_puts_our_source_on_both_halves_before_closing() {
        assert_eq!(
            single_plan(Some(0x31), true),
            vec![(0x60, 0x3131), (0xa5, 0), (0x60, 0x31), (0xf2, 0)]
        );
        assert_eq!(
            single_plan(Some(0x31), false),
            vec![(0xa5, 0), (0x60, 0x31), (0xf2, 0)]
        );
        assert_eq!(single_plan(None, true), vec![(0xa5, 0), (0xf2, 0)]);
    }

    #[test]
    fn single_feedback_requires_single_mode_and_our_source() {
        let value = |v: u16| VcpValue { value: v, max: v };
        let mut vals = BTreeMap::new();
        vals.insert(0xf2, value(0));
        vals.insert(0xf4, value(0));
        vals.insert(0x60, value(0x31));
        assert!(!single_feedback(Some(0x31), &vals, None).2);

        // A stale high byte from the pre-write is harmless.
        vals.insert(0x60, value(0x3131));
        assert!(!single_feedback(Some(0x31), &vals, None).2);

        // Still split: nothing was closed.
        vals.insert(0xf2, value(1));
        vals.insert(0xf4, value(0x0201));
        vals.insert(0x60, value(0x3111));
        assert!(single_feedback(Some(0x31), &vals, None).2);

        // F2/F4 can be stale after closing; a single-byte 0x60 proves the mode.
        vals.insert(0xf2, value(0x5d));
        vals.insert(0xf4, value(0x0201));
        vals.insert(0x60, value(0x31));
        assert!(!single_feedback(Some(0x31), &vals, None).2);

        // Single mode but on the other input.
        vals.insert(0xf2, value(0));
        vals.insert(0xf4, value(0));
        vals.insert(0x60, value(0x11));
        assert!(single_feedback(Some(0x31), &vals, None).2);

        assert!(single_feedback(Some(0x31), &vals, Some("transport error".into())).2);
    }

    #[test]
    fn success_requires_matching_readback() {
        let mut vals = BTreeMap::new();
        assert!(write_feedback(&[(0x10, 80)], &vals, None).2);
        vals.insert(
            0x10,
            VcpValue {
                value: 100,
                max: 100,
            },
        );
        assert!(write_feedback(&[(0x10, 80)], &vals, None).2);
        vals.get_mut(&0x10).unwrap().value = 80;
        assert!(!write_feedback(&[(0x10, 80)], &vals, None).2);
        assert!(write_feedback(&[(0x10, 80)], &vals, Some("transport error".into())).2);
    }
}
