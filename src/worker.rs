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

                        Cmd::DisableMultiView { source, pbp } => {
                            let Some(m) = mon.clone() else {
                                let _ =
                                    etx.send(Ev::Notice(Key::ErrNotConnected, String::new(), true));
                                continue;
                            };
                            let _ = etx.send(Ev::Busy(true));
                            let mut failure = None;
                            for &(code, value) in &single_plan(source, pbp) {
                                write_log(&etx, code, value);
                                if let Err(e) = ddc::set(m.display, code, value) {
                                    let _ = etx.send(Ev::Log(LogLine::Text(e.clone())));
                                    // A failed 0x60 write is only a warning:
                                    // the readback below decides whether the
                                    // desktop is on our source.
                                    if code == 0xa5 {
                                        failure = Some(e);
                                    }
                                }
                                settle();
                            }
                            let vals = read_codes(m.display, &caps);
                            let (key, argument, error) = single_feedback(source, &vals, failure);
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
fn single_plan(source: Option<u16>, pbp: bool) -> Vec<(u8, u16)> {
    match (source, pbp) {
        (Some(source), true) => vec![
            (0x60, catalog::window_pair(source, source)),
            (0xa5, 0),
            (0x60, source),
        ],
        (Some(source), false) => vec![(0xa5, 0), (0x60, source)],
        (None, _) => vec![(0xa5, 0)],
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
        0x87u8, 0x8a, 0x8b, 0x72, 0x6d, 0x0c, 0x14, 0x62, 0x8d, 0xd6, 0xcc,
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
    fn single_plan_puts_our_source_on_both_halves_before_closing() {
        assert_eq!(
            single_plan(Some(0x31), true),
            vec![(0x60, 0x3131), (0xa5, 0), (0x60, 0x31)]
        );
        assert_eq!(
            single_plan(Some(0x31), false),
            vec![(0xa5, 0), (0x60, 0x31)]
        );
        assert_eq!(single_plan(None, true), vec![(0xa5, 0)]);
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
