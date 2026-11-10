//! Worker thread — all monitor communication happens here, so the UI never freezes.

use crate::ddc::{self, Capabilities, Monitor, VcpValue};
use crate::i18n::Key;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

/// Codes we always want to know (in addition to what the monitor reports).
const ALWAYS: [u8; 7] = ddc::CORE;

pub enum Cmd {
    Init,
    Refresh,
    Set { code: u8, value: u16 },
    Apply(Vec<(u8, u16)>),
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
                                        let _ = etx.send(Ev::Busy(false));
                                        continue;
                                    }
                                }
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
                    }
                }
            })
            .expect("cannot start the DDC worker thread");

        Worker { tx: ctx, rx: erx }
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
        let monitor = ddc::detect().expect("detect monitor");
        assert_eq!(
            monitor.serial, "UPP03XLH",
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
