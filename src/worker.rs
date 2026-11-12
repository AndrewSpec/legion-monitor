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
    /// Re-reads only the desktop topology, without touching the monitor.
    ///
    /// The refresh rate of the desktop output decides whether PIP is allowed,
    /// so it is rescanned when the Multi-view page opens; it can change while
    /// the application is running.
    ScanDisplays,
    Set {
        code: u8,
        value: u16,
    },
    /// Writes a Lenovo feature selected through `0xF8` and valued through
    /// `0xF7`. The monitor exposes one shared value register, so the index has
    /// to be written first every time.
    SetFeature {
        index: u16,
        value: u16,
    },
    Apply(Vec<(u8, u16)>),
    /// Source of one window of a PIP split: `0xA5 = 0x0100` selects the PIP
    /// window and `0x60` sets its source.
    ApplyWindow {
        selector: u16,
        source: u16,
        swap: bool,
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
    ///
    /// `source` seeds both halves in 0x60; without it the monitor can keep an
    /// old pair that is unreadable and cannot be edited.
    SetPbp {
        layout: Option<u16>,
        source: Option<u16>,
    },
    /// Places the PIP window and stages its source.
    ///
    /// The monitor exposes the PIP rectangle as VCP 0x95..0x98 in panel
    /// pixels; 0xA4 masks the window while it moves. `main` and `source` are
    /// staged together in 0x60 before the PIP selector is written, so a
    /// selector the monitor ignores cannot change the main picture. `leave_pbp`
    /// closes an active PBP window first; writing 0xA5 = 0x0100 while PBP was
    /// open froze the monitor's OSD, so PBP uses its own close sequence.
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
    /// Rescue mode: restores a single picture even when the monitor currently
    /// shows nothing usable. It always turns the hardware True Split off
    /// first, because an older build could leave it on.
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
    /// Value read back from `0xF7` after a feature write. `None` means the
    /// monitor did not answer (a True Split can take the link down).
    Feature {
        index: u16,
        value: Option<u16>,
    },
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

                        Cmd::ScanDisplays => {
                            let displays = mon
                                .as_ref()
                                .map(|m| topology::scan(&m.connector))
                                .unwrap_or_default();
                            let _ = etx.send(Ev::Displays(displays));
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

                        Cmd::SetFeature { index, value } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            // True Split is different from PIP/PBP: lowering the
                            // refresh before F8/F7 makes the monitor re-enumerate
                            // at the wrong moment, so the mode is activated first
                            // and the two new screens are switched to 60 Hz below.
                            // Lenovo's 0xFA mutex uses feature << 8; F8 features
                            // therefore use 0xF700 | index (True Split 0xF715).
                            if ddc::locked(m.display, 0xf700 | (index & 0xff)) {
                                let _ = etx.send(Ev::Notice(Key::ErrLocked, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }
                            let mut failure = None;
                            let mut index_failed = None;
                            for (code, val) in [
                                (catalog::FEATURE_INDEX, index),
                                (catalog::FEATURE_VALUE, value),
                            ] {
                                write_log(&etx, code, val);
                                if let Err(e) = ddc::set(m.display, code, val) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    if code == catalog::FEATURE_INDEX {
                                        // Without the selector the value write
                                        // would land on whichever feature the
                                        // monitor selected last, so stop here.
                                        index_failed = Some(e);
                                        break;
                                    }
                                    if !ddc::is_transient(&e) {
                                        failure = Some(e);
                                    }
                                }
                                // The index needs a moment to latch before the
                                // value write; the value write itself lands
                                // slower on this monitor.
                                thread::sleep(Duration::from_millis(if code
                                    == catalog::FEATURE_INDEX
                                {
                                    150
                                } else {
                                    500
                                }));
                            }
                            if index_failed.is_some() {
                                let _ = etx.send(Ev::Notice(
                                    Key::WriteFailed,
                                    String::new(),
                                    true,
                                ));
                                let _ = etx.send(Ev::Feature { index, value: None });
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }
                            if index == catalog::TRUE_SPLIT_FEATURE && value != 0 {
                                // The monitor re-enumerates into two screens;
                                // wait for both to appear before touching their
                                // refresh rates.
                                thread::sleep(Duration::from_millis(6000));
                                let mut applied = Vec::new();
                                let mut last_error = None;
                                let mut settled = false;
                                for attempt in 0..3 {
                                    match topology::set_refresh_60_all() {
                                        Ok(list) => {
                                            // An empty list is a success: both
                                            // split outputs are already enabled
                                            // and at 60 Hz, so there is nothing
                                            // to change and no reason to retry.
                                            applied = list;
                                            settled = true;
                                            break;
                                        }
                                        Err(error) => last_error = Some(error),
                                    }
                                    if attempt < 2 {
                                        // The second screen may still be
                                        // appearing; refresh the topology and
                                        // retry before reporting a failure.
                                        thread::sleep(Duration::from_millis(1500));
                                        let _ = etx
                                            .send(Ev::Displays(topology::scan(&m.connector)));
                                    }
                                }
                                if !settled {
                                    if let Some(error) = last_error {
                                        let _ = etx.send(Ev::Notice(
                                            Key::ErrRefresh60,
                                            error,
                                            true,
                                        ));
                                    }
                                }
                                if !applied.is_empty() {
                                    let summary = applied
                                        .iter()
                                        .map(|(name, hz)| format!("{name}={hz:.2}Hz"))
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    let _ = etx.send(Ev::Log(LogLine::Text(format!(
                                        "True Split: lowered refresh on {summary}"
                                    ))));
                                } else if settled {
                                    let _ = etx.send(Ev::Log(LogLine::Text(
                                        "True Split: both external screens are enabled and already at 60 Hz"
                                            .to_string(),
                                    )));
                                }
                                thread::sleep(Duration::from_millis(2000));
                                let _ = etx.send(Ev::Displays(topology::scan(&m.connector)));
                            }
                            let mut got = None;
                            let attempts = if index == catalog::TRUE_SPLIT_FEATURE && value != 0 {
                                5
                            } else {
                                3
                            };
                            for attempt in 0..attempts {
                                got = ddc::read(m.display, &[catalog::FEATURE_VALUE])
                                    .get(&catalog::FEATURE_VALUE)
                                    .map(|v| v.value);
                                if got == Some(value) {
                                    break;
                                }
                                if attempt + 1 < attempts {
                                    thread::sleep(Duration::from_millis(600));
                                }
                            }
                            let (key, error) = if failure.is_some() {
                                (Key::WriteFailed, true)
                            } else if got == Some(value) {
                                (Key::WriteConfirmed, false)
                            } else {
                                // True Split re-enumerates the link; a missing
                                // readback is expected and not a failed write.
                                (Key::WriteUnconfirmed, true)
                            };
                            let _ = etx.send(Ev::Notice(key, String::new(), error));
                            let _ = etx.send(Ev::Feature { index, value: got });
                            if index == catalog::TRUE_SPLIT_FEATURE {
                                thread::sleep(Duration::from_millis(1500));
                                let _ =
                                    etx.send(Ev::Displays(topology::scan(&m.connector)));
                                // A True Split change re-enumerates the link
                                // (DP-2 becomes half-width DP-3 and the
                                // internal panel appears); reading every VCP
                                // code here can block for minutes on the new
                                // topology. Keep the last known values and let
                                // the user/refresh read them again.
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }
                            let vals = read_codes(m.display, &caps);
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
                        Cmd::ApplyWindow {
                            selector,
                            source,
                            swap,
                        } => {
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
                            if failure.is_none() && swap {
                                // Selecting the background and writing the new
                                // source is what actually changes the full
                                // picture.  One swap then moves the old PIP
                                // picture to the background and the new source
                                // into the quarter.
                                thread::sleep(Duration::from_millis(200));
                                write_log(&etx, 0xf2, 1);
                                failure = ddc::set(m.display, 0xf2, 1).err();
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
                                    if !ddc::is_transient(&e) {
                                        failure = Some(e);
                                    }
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

                        Cmd::SetPbp { layout, source } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            if !ensure_refresh_60(&etx, &m) {
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }
                            let mut failure = None;
                            // Lenovo's SetPIPPBPState maps PBP to 0xA5 = 0x0200;
                            // 0x0100 in PBP froze the monitor's OSD.
                            for &(code, value) in &pbp_plan(layout, source) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    if !ddc::is_transient(&e) {
                                        failure = Some(e);
                                    }
                                }
                                settle();
                            }
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) =
                                mode_feedback(MultiMode::PictureByPicture, &vals, failure);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            // The DP EDID shrinks to one half; give the
                            // compositor time to apply it, then rescan so PBP
                            // detection sees the half-width output.
                            thread::sleep(Duration::from_millis(1200));
                            let _ = etx.send(Ev::Displays(topology::scan(&m.connector)));
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
                            if !ensure_refresh_60(&etx, &m) {
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }
                            let mut failure = None;
                            let mut stopped = None;

                            // Lenovo's SetPIPWndRect checks the 0xA5 mutex before
                            // touching the window; a locked feature means another
                            // setting owns PIP right now.
                            if ddc::locked(m.display, 0xa500) {
                                let _ = etx.send(Ev::Notice(Key::ErrLocked, String::new(), true));
                                let _ = etx.send(Ev::Busy(false));
                                continue;
                            }

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
                                        // F4 is the state register. Retry because
                                        // a read during the mode change can fail;
                                        // refuse to open PIP while a split remains.
                                        if failure.is_none() {
                                            let mut probe = None;
                                            for attempt in 0..3 {
                                                if let Some(value) = ddc::read(m.display, &[0xf4])
                                                    .get(&0xf4)
                                                    .map(|v| v.value)
                                                {
                                                    probe = Some(value);
                                                    if catalog::multi_mode(Some(value))
                                                        == MultiMode::Off
                                                    {
                                                        break;
                                                    }
                                                }
                                                if attempt < 2 {
                                                    thread::sleep(Duration::from_millis(300));
                                                }
                                            }
                                            match probe {
                                                Some(value)
                                                    if catalog::multi_mode(Some(value))
                                                        != MultiMode::Off =>
                                                {
                                                    stopped = Some(Key::WriteMismatch)
                                                }
                                                None => stopped = Some(Key::WriteUnconfirmed),
                                                Some(_) => {}
                                            }
                                        }
                                    }
                                }
                            }

                            // Hardware order (verified on the R45w-30): the
                            // rectangle has to be written before 0xA5 selects
                            // the window. 0x60 is the write that actually
                            // creates the PIP window and it re-enumerates the
                            // scaler, so it comes last and DDC is expected to
                            // disappear for a few seconds afterwards.
                            if failure.is_none() && stopped.is_none() {
                                // Lenovo hides the window while it moves:
                                // 0xA4 = 0, write 0x95..0x98, 0xA4 = 0xFFFF.
                                // Preflight the show value: some firmware rejects
                                // it, and hiding first would then leave the whole
                                // picture masked with no way to bring it back.
                                write_log(&etx, 0xa4, 0xffff);
                                let mask_ok = match ddc::set(m.display, 0xa4, 0xffff) {
                                    Ok(()) => true,
                                    Err(e) => {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                        false
                                    }
                                };
                                if mask_ok {
                                    write_log(&etx, 0xa4, 0x0000);
                                    if let Err(e) = ddc::set(m.display, 0xa4, 0) {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                    }
                                    thread::sleep(Duration::from_millis(120));
                                }
                                for &(code, value) in &pip_rect_plan(corner) {
                                    write_log(&etx, code, value);
                                    if let Err(e) = ddc::set(m.display, code, value) {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                        failure = Some(e);
                                        break;
                                    }
                                    thread::sleep(Duration::from_millis(120));
                                }
                                if failure.is_none() && mask_ok {
                                    // Never leave the mask closed: retry the
                                    // show write even when the first one is
                                    // rejected (the monitor can answer EIO and
                                    // still apply it).
                                    write_log(&etx, 0xa4, 0xffff);
                                    let mut shown = false;
                                    for attempt in 0..5 {
                                        match ddc::set(m.display, 0xa4, 0xffff) {
                                            Ok(()) => {
                                                shown = true;
                                                break;
                                            }
                                            Err(e) => {
                                                if attempt == 0 {
                                                    let _ = etx
                                                        .send(Ev::Log(LogLine::Text(e)));
                                                }
                                                thread::sleep(Duration::from_millis(250));
                                            }
                                        }
                                    }
                                    if !shown {
                                        let _ = etx.send(Ev::Log(LogLine::Text(
                                            "0xA4 show mask was rejected".to_string(),
                                        )));
                                        stopped = Some(Key::WriteUnconfirmed);
                                    }
                                }
                            }

                            // Select the PIP window after the rectangle is in
                            // place. 0xF2 is the swap command, not a mode, and
                            // A5 is the only vendor state switch.
                            if failure.is_none() && stopped.is_none() {
                                write_log(&etx, 0xa5, 0x0100);
                                if let Err(e) = ddc::set(m.display, 0xa5, 0x0100) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                }
                                thread::sleep(Duration::from_millis(400));
                            }

                            // 0x60 = (secondary << 8) | primary. This write is
                            // what opens PIP on the R45w-30; it also makes the
                            // monitor re-enumerate, so tolerate EIO and give it
                            // time to come back before probing F4/rect.
                            let mut pip_pair = None;
                            if failure.is_none() && stopped.is_none() {
                                if let (Some(main_source), Some(second)) = (main, source) {
                                    let pair = catalog::window_pair(main_source, second);
                                    write_log(&etx, 0x60, pair);
                                    if let Err(e) = ddc::set(m.display, 0x60, pair) {
                                        let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                    }
                                    pip_pair = Some(pair);
                                    thread::sleep(Duration::from_millis(5000));
                                }
                            }

                            // The scaler may still be re-enumerating. Probe F4
                            // and, when the window did not open, retry the pair
                            // once after DDC comes back.
                            if failure.is_none() && stopped.is_none() {
                                if let Some(pair) = pip_pair {
                                    let mut open = false;
                                    for _ in 0..3 {
                                        if let Some(value) = ddc::read(m.display, &[0xf4])
                                            .get(&0xf4)
                                            .map(|v| v.value)
                                        {
                                            if matches!(
                                                catalog::multi_mode(Some(value)),
                                                MultiMode::PictureInPicture
                                            ) {
                                                open = true;
                                                break;
                                            }
                                        }
                                        thread::sleep(Duration::from_millis(700));
                                    }
                                    if !open {
                                        write_log(&etx, 0x60, pair);
                                        if let Err(e) = ddc::set(m.display, 0x60, pair) {
                                            let _ = etx.send(Ev::Log(LogLine::Text(e)));
                                        }
                                        thread::sleep(Duration::from_millis(5000));
                                    }
                                }
                            }

                            // In single mode this same write changes the main
                            // picture, so only trust the rectangle after the
                            // 0x60 handshake above has had time to settle.
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
                                    0xf8 => thread::sleep(Duration::from_millis(150)),
                                    0xf7 => thread::sleep(Duration::from_millis(150)),
                                    0xf5 => thread::sleep(Duration::from_millis(250)),
                                    _ => thread::sleep(Duration::from_millis(120)),
                                }
                            }
                            settle();
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = single_feedback(Some(source), &vals, None);
                            let _ = etx.send(Ev::Notice(key, argument, error));
                            let _ = etx.send(Ev::Values(vals));
                            thread::sleep(Duration::from_millis(900));
                            let _ = etx.send(Ev::Displays(topology::scan(&m.connector)));
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
                            // The single path always clears F7. Tell the UI so
                            // it stops labelling the remaining PBP-shaped F4
                            // readback (during re-enumeration) as True Split.
                            let _ = etx.send(Ev::Feature {
                                index: catalog::TRUE_SPLIT_FEATURE,
                                value: Some(0),
                            });
                            let _ = etx.send(Ev::Values(vals));
                            thread::sleep(Duration::from_millis(900));
                            let _ = etx.send(Ev::Displays(topology::scan(&m.connector)));
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
/// Ensures the desktop link is at 60 Hz before a mode that the monitor only
/// supports at that rate (PIP and True Split). Returns false (after telling the
/// interface why) when the desktop reported a higher rate and the switch failed.
fn ensure_refresh_60(etx: &Sender<Ev>, monitor: &Monitor) -> bool {
    if topology::scan(&monitor.connector)
        .refresh_too_high_for_pip()
        .is_none()
    {
        return true;
    }
    match topology::set_refresh_60(&monitor.connector) {
        Ok(hz) => {
            let _ = etx.send(Ev::Log(LogLine::Text(format!(
                "Lowered the desktop refresh rate to {hz:.2} Hz for PIP/PBP"
            ))));
            // The compositor needs a moment to apply the mode, and the
            // monitor may drop DDC while it re-locks the link.
            thread::sleep(Duration::from_millis(2500));
            let _ = etx.send(Ev::Displays(topology::scan(&monitor.connector)));
            true
        }
        Err(error) => {
            let _ = etx.send(Ev::Notice(Key::ErrRefresh60, error, true));
            false
        }
    }
}

fn pbp_plan(layout: Option<u16>, source: Option<u16>) -> Vec<(u8, u16)> {
    // A hardware True Split left on by the OSD or an older build keeps both
    // halves on one source. Clear it before opening a real PBP split.
    let mut plan = vec![
        (catalog::FEATURE_INDEX, catalog::TRUE_SPLIT_FEATURE),
        (catalog::FEATURE_VALUE, 0),
        (0xa5, 0x0200),
    ];
    if let Some(value) = layout {
        plan.push((0xf5, value));
    }
    // PBP needs both half-source bytes. Starting from a known pair makes the
    // state readable and lets the user change either half; leaving 0x60 alone
    // showed the monitor's old pair and the pickers could not recover it.
    if let Some(source) = source {
        plan.push((0x60, catalog::window_pair(source, source)));
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
/// PBP is closed without `0xA5 = 0x0100`: that selector froze the monitor's
/// OSD while a PBP split was open. Both halves are first pointed at our source,
/// then the split closes, then 0x60 becomes a plain single input again.
///
/// PIP closes with `0xA5 = 0` followed by the single source; writing the
/// source is what makes the monitor leave the PIP timing.
fn single_plan(source: Option<u16>, _pbp: bool) -> Vec<(u8, u16)> {
    let mut plan = vec![
        // Clear a hardware True Split left on by the OSD or an older build.
        // The monitor keeps reporting True Split in its OSD until F7 (and the
        // F5 layout) are explicitly cleared, even when A5 closes the window.
        (catalog::FEATURE_INDEX, catalog::TRUE_SPLIT_FEATURE),
        (catalog::FEATURE_VALUE, 0),
        // Without clearing F5 the link can stay at the half-width PBP timing.
        (0xf5, 0),
        // F2=0 is the swap-off/no-op write; A5=0 closes the PIP/PBP window.
        (0xf2, 0),
        (0xa5, 0),
    ];
    if let Some(source) = source {
        plan.push((0x60, source));
    }
    plan
}

/// Restores one picture and, when the monitor exposes them, the saved
/// brightness and contrast.
///
/// The hardware True Split switch (F8/F7) is always turned off first: an older
/// build or the OSD could leave it on, and no PIP/PBP write can clear it. The
/// monitor exposes this feature as index 0x15; F7 = 0 is the safe state. The
/// remaining writes close PIP/PBP and select the single input.
fn rescue_plan(source: u16, brightness: Option<u16>, contrast: Option<u16>) -> Vec<(u8, u16)> {
    let mut plan = vec![
        (catalog::FEATURE_INDEX, catalog::TRUE_SPLIT_FEATURE),
        (catalog::FEATURE_VALUE, 0),
        (0xf5, 0),
        (0xf2, 0),
        (0xa5, 0),
        (0x60, source),
    ];
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
/// pixels. The mask (0xA4) is handled by the command arm around this plan:
/// hardware needs the rectangle before 0xA5 selects the window. The input
/// source is written afterwards; that 0x60 write is what opens PIP.
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
    let have = catalog::multi_mode(observed.get(&0xf4).map(|v| v.value));
    if have != MultiMode::PictureInPicture {
        return (Key::WriteMismatch, "0xF4".to_string(), true);
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
    let have = catalog::multi_mode(observed.get(&0xf4).map(|v| v.value));
    if have != MultiMode::Off {
        return (Key::WriteMismatch, "0xF4".to_string(), true);
    }
    if let Some(source) = source {
        if observed.get(&0x60).map(|v| v.value & 0xff) != Some(source & 0xff) {
            return (Key::WriteMismatch, "1".to_string(), true);
        }
    }
    (Key::WriteConfirmed, "1".to_string(), false)
}

/// Checks the window state reported by F4.
fn mode_feedback(
    want: MultiMode,
    observed: &BTreeMap<u8, VcpValue>,
    failure: Option<String>,
) -> (Key, String, bool) {
    if let Some(error) = failure {
        return (Key::WriteFailed, error, true);
    }
    let have = catalog::multi_mode(observed.get(&0xf4).map(|value| value.value));
    // F4 is the authoritative status register. If it was not read, accept an
    // active F5 layout as the only fallback for PBP.
    let split_by_layout = want == MultiMode::PictureByPicture
        && have == MultiMode::Unknown
        && observed
            .get(&0xf5)
            .and_then(|value| catalog::split_layout_for(value.value))
            .is_some();
    if have == want || split_by_layout {
        (Key::WriteConfirmed, "1".to_string(), false)
    } else if have == MultiMode::Unknown {
        (Key::WriteUnconfirmed, String::new(), true)
    } else {
        (Key::WriteMismatch, "0xF4".to_string(), true)
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
    // Lenovo vendor controls the monitor accepts even when ddcutil does not
    // enumerate them in its capability string.
    for extra in [
        0x87u8, 0x8a, 0x8b, 0x72, 0x6d, 0x0c, 0x14, 0x62, 0x8d, 0xd6, 0xcc, 0x86, 0xe0, 0xea, 0xef,
        0xf9, 0x95, 0x96, 0x97, 0x98,
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

    /// Explicit opt-in: end-to-end PIP acceptance on the attached R45w-30.
    #[test]
    #[ignore = "requires attached LEN R45w-30 and explicit device permission"]
    fn live_pip_corner_roundtrip() {
        assert_eq!(std::env::var("LEGION_LIVE_TEST").as_deref(), Ok("1"));
        let expected = std::env::var("LEGION_LIVE_SERIAL")
            .expect("set LEGION_LIVE_SERIAL to the test monitor serial");
        let monitor = ddc::detect().expect("detect monitor");
        assert_eq!(
            monitor.serial, expected,
            "never write an unidentified monitor"
        );
        let worker = Worker::spawn();
        let finish = |worker: &Worker| {
            let mut values = BTreeMap::new();
            let mut notice = None;
            loop {
                match worker
                    .rx
                    .recv_timeout(Duration::from_secs(180))
                    .expect("worker deadline")
                {
                    Ev::Values(v) => values = v,
                    Ev::Notice(key, arg, error) => {
                        eprintln!("NOTICE {key:?}: {arg}, error={error}");
                        notice = Some((key, error));
                    }
                    Ev::Conn(Conn::Err(error)) => panic!("connection failed: {error}"),
                    Ev::Busy(false) => break,
                    _ => {}
                }
            }
            (values, notice)
        };

        worker.tx.send(Cmd::Init).expect("queue init");
        let _ = finish(&worker);

        let source = 0x31;
        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(source),
                pbp: false,
            })
            .expect("queue disable");
        let _ = finish(&worker);
        std::thread::sleep(Duration::from_secs(3));

        let corner_name = std::env::var("LEGION_LIVE_CORNER").unwrap_or_else(|_| "TR".to_string());
        let corner = match corner_name.as_str() {
            "TL" => catalog::PipCorner::TopLeft,
            "BL" => catalog::PipCorner::BottomLeft,
            "BR" => catalog::PipCorner::BottomRight,
            _ => catalog::PipCorner::TopRight,
        }
        .rect(5120, 1440);
        worker
            .tx
            .send(Cmd::ApplyPip {
                corner,
                main: Some(source),
                source: Some(source),
                leave_pbp: false,
            })
            .expect("queue pip");
        let (values, notice) = finish(&worker);
        let f4 = values.get(&0xf4).map(|v| v.value);
        let rect = (
            values.get(&0x95).map(|v| v.value),
            values.get(&0x96).map(|v| v.value),
            values.get(&0x97).map(|v| v.value),
            values.get(&0x98).map(|v| v.value),
        );
        eprintln!("PIP notice={notice:?} f4={f4:?} rect={rect:?}");
        assert_eq!(notice, Some((Key::WriteConfirmed, false)));
        assert_eq!(
            catalog::multi_mode(f4),
            catalog::MultiMode::PictureInPicture,
            "F4 must report PIP after a successful write"
        );
        assert_eq!(
            rect,
            (
                Some(corner.0),
                Some(corner.1),
                Some(corner.2),
                Some(corner.3)
            ),
            "PIP rectangle must read back exactly"
        );

        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(source),
                pbp: false,
            })
            .expect("queue restore");
        let _ = finish(&worker);
    }

    #[test]
    #[ignore]
    fn live_true_split_roundtrip() {
        assert_eq!(std::env::var("LEGION_LIVE_TEST").as_deref(), Ok("1"));
        let expected = std::env::var("LEGION_LIVE_SERIAL")
            .expect("set LEGION_LIVE_SERIAL to the test monitor serial");
        let monitor = ddc::detect().expect("detect monitor");
        assert_eq!(
            monitor.serial, expected,
            "never write an unidentified monitor"
        );
        let worker = Worker::spawn();
        let finish = |worker: &Worker| {
            let mut values = BTreeMap::new();
            let mut notice = None;
            loop {
                match worker
                    .rx
                    .recv_timeout(Duration::from_secs(240))
                    .expect("worker deadline")
                {
                    Ev::Values(v) => values = v,
                    Ev::Notice(key, arg, error) => {
                        eprintln!("NOTICE {key:?}: {arg}, error={error}");
                        notice = Some((key, error));
                    }
                    Ev::Conn(Conn::Err(error)) => panic!("connection failed: {error}"),
                    Ev::Busy(false) => break,
                    _ => {}
                }
            }
            (values, notice)
        };

        worker.tx.send(Cmd::Init).expect("queue init");
        let _ = finish(&worker);
        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(0x31),
                pbp: false,
            })
            .expect("queue disable");
        let _ = finish(&worker);
        std::thread::sleep(Duration::from_secs(3));

        eprintln!("TRUE SPLIT: enabling 1:1");
        worker
            .tx
            .send(Cmd::SetFeature {
                index: catalog::TRUE_SPLIT_FEATURE,
                value: 1,
            })
            .expect("queue true split on");
        let (_values, notice) = finish(&worker);
        eprintln!("TRUE SPLIT: notice={notice:?}");

        // Restore single mode before asserting, so a failed expectation does
        // not leave the user's desktop split.
        worker
            .tx
            .send(Cmd::SetFeature {
                index: catalog::TRUE_SPLIT_FEATURE,
                value: 0,
            })
            .expect("queue true split off");
        let _ = finish(&worker);
        std::thread::sleep(Duration::from_secs(3));
        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(0x31),
                pbp: false,
            })
            .expect("queue restore");
        let _ = finish(&worker);

        assert!(
            matches!(
                notice,
                Some((Key::WriteConfirmed, false)) | Some((Key::WriteUnconfirmed, true))
            ),
            "unexpected True Split result: {notice:?}"
        );
    }

    #[test]
    #[ignore]
    fn live_pip_all_four_corners() {
        assert_eq!(std::env::var("LEGION_LIVE_TEST").as_deref(), Ok("1"));
        let expected = std::env::var("LEGION_LIVE_SERIAL")
            .expect("set LEGION_LIVE_SERIAL to the test monitor serial");
        let monitor = ddc::detect().expect("detect monitor");
        assert_eq!(
            monitor.serial, expected,
            "never write an unidentified monitor"
        );
        let worker = Worker::spawn();
        let finish = |worker: &Worker| {
            let mut values = BTreeMap::new();
            let mut notice = None;
            loop {
                match worker
                    .rx
                    .recv_timeout(Duration::from_secs(180))
                    .expect("worker deadline")
                {
                    Ev::Values(v) => values = v,
                    Ev::Notice(key, arg, error) => {
                        eprintln!("NOTICE {key:?}: {arg}, error={error}");
                        notice = Some((key, error));
                    }
                    Ev::Conn(Conn::Err(error)) => panic!("connection failed: {error}"),
                    Ev::Busy(false) => break,
                    _ => {}
                }
            }
            (values, notice)
        };

        worker.tx.send(Cmd::Init).expect("queue init");
        let _ = finish(&worker);

        let source = 0x31;
        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(source),
                pbp: false,
            })
            .expect("queue disable");
        let _ = finish(&worker);
        std::thread::sleep(Duration::from_secs(3));

        for corner in catalog::PipCorner::ALL {
            let rect = corner.rect(5120, 1440);
            worker
                .tx
                .send(Cmd::ApplyPip {
                    corner: rect,
                    main: Some(source),
                    source: Some(source),
                    leave_pbp: false,
                })
                .expect("queue pip");
            let (values, notice) = finish(&worker);
            let f4 = values.get(&0xf4).map(|v| v.value);
            let seen = (
                values.get(&0x95).map(|v| v.value),
                values.get(&0x96).map(|v| v.value),
                values.get(&0x97).map(|v| v.value),
                values.get(&0x98).map(|v| v.value),
            );
            eprintln!("corner {corner:?} notice={notice:?} f4={f4:?} rect={seen:?}");
            assert_eq!(notice, Some((Key::WriteConfirmed, false)));
            assert_eq!(
                catalog::multi_mode(f4),
                catalog::MultiMode::PictureInPicture
            );
            assert_eq!(
                seen,
                (Some(rect.0), Some(rect.1), Some(rect.2), Some(rect.3)),
                "rectangle must read back exactly for {corner:?}"
            );
            std::thread::sleep(Duration::from_secs(2));
        }

        worker
            .tx
            .send(Cmd::DisableMultiView {
                source: Some(source),
                pbp: false,
            })
            .expect("queue restore");
        let _ = finish(&worker);
    }

    #[test]
    fn split_plan_sets_both_halves_without_a_swap() {
        assert_eq!(split_plan(0x11, 0x31, None, false), vec![(0x60, 0x3111)]);
        assert_eq!(
            split_plan(0x11, 0x31, Some(0x0101), true),
            vec![(0xa5, 0x0200), (0xf5, 0x0101), (0x60, 0x3111)]
        );
        assert_eq!(
            pbp_plan(None, None),
            vec![(0xf8, 0x15), (0xf7, 0), (0xa5, 0x0200)]
        );
        assert_eq!(
            pbp_plan(Some(0x0101), Some(0x31)),
            vec![
                (0xf8, 0x15),
                (0xf7, 0),
                (0xa5, 0x0200),
                (0xf5, 0x0101),
                (0x60, 0x3131),
            ]
        );
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
        observed.insert(0xf4, value(0x0100));
        for (code, v) in [(0x95, 2560), (0x96, 0), (0x97, 5120), (0x98, 720)] {
            observed.insert(code, value(v));
        }
        assert!(!pip_feedback((2560, 0, 5120, 720), &observed, None).2);
        assert!(pip_feedback((0, 0, 2560, 720), &observed, None).2);
        assert!(pip_feedback((2560, 0, 5120, 720), &BTreeMap::new(), None).2);
    }

    #[test]
    fn rescue_plan_turns_true_split_off_before_picture_settings() {
        let plan = rescue_plan(0x31, Some(80), Some(60));
        assert_eq!(
            plan,
            vec![
                (0xf8, 0x15),
                (0xf7, 0),
                (0xf5, 0),
                (0xf2, 0),
                (0xa5, 0),
                (0x60, 0x31),
                (0x10, 80),
                (0x12, 60),
            ]
        );
    }

    #[test]
    fn pbp_feedback_uses_f4_and_falls_back_to_the_layout() {
        let value = |v: u16| VcpValue {
            value: v,
            max: 0xffff,
        };
        let mut observed = BTreeMap::new();
        observed.insert(0xf4, value(0x0200));
        assert!(!mode_feedback(MultiMode::PictureByPicture, &observed, None).2);
        // F4 missing: an active F5 layout is the only fallback.
        observed.remove(&0xf4);
        observed.insert(0xf5, value(0x0101));
        assert!(!mode_feedback(MultiMode::PictureByPicture, &observed, None).2);
        // Neither register: unconfirmed, not a mismatch.
        observed.remove(&0xf5);
        assert!(mode_feedback(MultiMode::PictureByPicture, &observed, None).2);
        assert!(mode_feedback(MultiMode::PictureByPicture, &BTreeMap::new(), None).2);
    }

    #[test]
    fn single_plan_closes_pbp_without_the_pip_selector() {
        assert_eq!(
            single_plan(Some(0x31), true),
            vec![
                (0xf8, 0x15),
                (0xf7, 0),
                (0xf5, 0),
                (0xf2, 0),
                (0xa5, 0),
                (0x60, 0x31),
            ]
        );
        // Any single-mode switch clears a hardware True Split first: the
        // monitor OSD otherwise keeps showing True Split after the window is
        // closed. The PIP path gets the same safety writes.
        let clear = [(0xf8, 0x15), (0xf7, 0), (0xf5, 0), (0xf2, 0), (0xa5, 0)];
        let mut pip = clear.to_vec();
        pip.push((0x60, 0x31));
        assert_eq!(single_plan(Some(0x31), false), pip);
        assert_eq!(single_plan(None, true), clear.to_vec());
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
        vals.insert(0xf4, value(0x0100));
        vals.insert(0x60, value(0x3111));
        assert!(single_feedback(Some(0x31), &vals, None).2);

        // F4 is the mode register; F2=1 is the swap command and proves nothing.
        vals.insert(0xf4, value(0));
        vals.insert(0x60, value(0x3131));
        assert!(!single_feedback(Some(0x31), &vals, None).2);

        // Single mode but on the other input.
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
