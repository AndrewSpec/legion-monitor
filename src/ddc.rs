//! DDC/CI layer — the only place that knows about `ddcutil` and VCP codes.

use crate::i18n::{Key, Strings};
use std::collections::BTreeMap;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct VcpValue {
    pub value: u16,
    pub max: u16,
}

#[derive(Clone, Debug, Default)]
pub struct Monitor {
    pub display: u32,
    pub bus: String,
    pub connector: String,
    pub model: String,
    pub mfg: String,
    pub serial: String,
    pub vcp: String,
}

impl Monitor {
    /// Pretty name for the header, e.g. "Lenovo Legion R45w-30".
    pub fn title(&self, t: &Strings) -> String {
        let model = self.model.trim();
        if model.is_empty() {
            t.fill(Key::MonitorFallbackFmt, &[self.display.to_string()])
        } else if model.to_lowercase().contains("lenovo") {
            model.to_string()
        } else {
            format!("Lenovo {model}")
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Capabilities {
    /// All features reported by the monitor: (code, name from ddcutil).
    pub features: Vec<(u8, String)>,
    /// Inputs from the value list of the "Input Source" feature.
    pub inputs: Vec<(u16, String)>,
    /// Other enumerated features: code → list of (value, name).
    pub values: Vec<(u8, Vec<(u16, String)>)>,
}

impl Capabilities {
    pub fn name_of(&self, code: u8) -> Option<&str> {
        self.features
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, n)| n.as_str())
    }

    /// Allowed values of a feature (if the monitor reported them).
    pub fn values_of(&self, code: u8) -> Option<&[(u16, String)]> {
        self.values
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.as_slice())
    }
}

#[derive(Debug)]
struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn run(args: &[String]) -> Result<Output, String> {
    let out = Command::new("ddcutil")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|e| format!("cannot start ddcutil: {e}"))?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

fn tail(text: &str) -> String {
    let interesting: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("Checking") && !l.starts_with("Verifying"))
        .collect();
    interesting
        .last()
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Finds the first working monitor (skips the laptop panel).
pub fn detect() -> Result<Monitor, String> {
    let out = run(&["detect".to_string()])?;
    let mut best: Option<Monitor> = None;
    let mut cur: Option<Monitor> = None;

    let flush = |cur: &mut Option<Monitor>, best: &mut Option<Monitor>| {
        if let Some(m) = cur.take() {
            if !m.vcp.is_empty() {
                let is_laptop = m.connector.to_lowercase().contains("edp")
                    || m.model.to_lowercase().contains("laptop");
                let better = match best {
                    None => true,
                    Some(b) => {
                        let b_laptop = b.connector.to_lowercase().contains("edp")
                            || b.model.to_lowercase().contains("laptop");
                        b_laptop && !is_laptop
                    }
                };
                if better {
                    *best = Some(m);
                }
            }
        }
    };

    for line in out.stdout.lines() {
        let t = line.trim();
        if t.starts_with("Display ") || t.starts_with("Invalid display") {
            flush(&mut cur, &mut best);
            let display = t
                .strip_prefix("Display ")
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            cur = Some(Monitor {
                display,
                ..Default::default()
            });
            continue;
        }
        let Some(m) = cur.as_mut() else { continue };
        if let Some(v) = t.strip_prefix("I2C bus:") {
            m.bus = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("DRM_connector:") {
            m.connector = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Mfg id:") {
            // "LEN - Lenovo"
            m.mfg = v.split('-').next().unwrap_or("").trim().to_string();
        } else if let Some(v) = t.strip_prefix("Model:") {
            m.model = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Serial number:") {
            m.serial = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("VCP version:") {
            m.vcp = v.trim().to_string();
        }
    }
    flush(&mut cur, &mut best);

    best.ok_or_else(|| {
        let msg = tail(&out.stdout);
        if msg.is_empty() {
            "no monitor supporting DDC/CI was found".to_string()
        } else {
            msg
        }
    })
}

/// Reads the list of features supported by the monitor.
pub fn capabilities(display: u32) -> Result<Capabilities, String> {
    let out = run(&[
        "--display".to_string(),
        display.to_string(),
        "capabilities".to_string(),
    ])?;
    if !out.ok && out.stdout.trim().is_empty() {
        return Err(with_reason(&out));
    }

    let caps = parse_capabilities(&out.stdout);
    if caps.features.is_empty() {
        return Err("the monitor reported no features".to_string());
    }
    Ok(caps)
}

/// Parses the `ddcutil capabilities` output.
///
/// Some monitors repeat their value lists — the R45w-30 reports every input
/// source twice (`60(0F 11 12 31 FF 0F 11 12 31)`) — so every list keeps only
/// the first entry per code.
fn parse_capabilities(text: &str) -> Capabilities {
    let mut caps = Capabilities::default();
    let mut current: Option<u8> = None;
    let mut in_values = false;
    let mut extra: std::collections::BTreeMap<u8, Vec<(u16, String)>> =
        std::collections::BTreeMap::new();

    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("Feature:") {
            in_values = false;
            let rest = rest.trim();
            let mut it = rest.splitn(2, char::is_whitespace);
            let code_txt = it.next().unwrap_or("");
            let code = u8::from_str_radix(code_txt.trim_start_matches("0x"), 16).ok();
            let name = it
                .next()
                .map(|s| s.trim())
                .unwrap_or("")
                .trim_start_matches('(')
                .trim_end_matches(')')
                .to_string();
            if let Some(code) = code {
                caps.features.push((code, name));
                current = Some(code);
            }
        } else if t.starts_with("Values:") {
            in_values = true;
        } else if in_values {
            let vals = t.trim();
            let parsed = vals
                .split_once(':')
                .filter(|(k, _)| {
                    let k = k.trim();
                    k.len() <= 4 && u8::from_str_radix(k.trim_start_matches("0x"), 16).is_ok()
                })
                .map(|(k, v)| {
                    let code =
                        u8::from_str_radix(k.trim().trim_start_matches("0x"), 16).unwrap_or(0);
                    (code, v.trim().to_string())
                });
            match parsed {
                Some((code, label)) if current == Some(0x60) => {
                    if !caps.inputs.iter().any(|(c, _)| *c == code as u16) {
                        caps.inputs.push((code as u16, human_input(&label)));
                    }
                }
                Some((code, label)) => {
                    if let Some(owner) = current {
                        let list = extra.entry(owner).or_default();
                        if !list.iter().any(|(v, _)| *v == code as u16) {
                            list.push((code as u16, label));
                        }
                    }
                }
                None => in_values = false,
            }
        }
    }

    caps.values = extra.into_iter().collect();
    caps
}

fn with_reason(out: &Output) -> String {
    let e = out.stderr.trim();
    let s = tail(&out.stdout);
    if !e.is_empty() {
        e.lines().next().unwrap_or(e).to_string()
    } else if !s.is_empty() {
        s
    } else {
        "ddcutil returned an error".to_string()
    }
}

/// How many codes are passed in a single `getvcp` call.
///
/// ddcutil rejects a list of codes that is too long with "Too many arguments"
/// (verified: 53 codes at once = error, ~20 works), so we read in batches.
const READ_CHUNK: usize = 10;
/// How many attempts are made when the monitor drops the link during a read.
const READ_ATTEMPTS: usize = 3;

/// Codes the whole interface relies on (picture mode, source, windows).
///
/// This monitor (RealTek) can return wrong values when they are queried in a
/// large combined `getvcp`, so these codes are read in a separate, short call —
/// verified that seven codes in one command give correct results and longer
/// lists no longer do.
pub const CORE: [u8; 7] = [0x10, 0x12, 0x60, 0xf0, 0xf2, 0xf4, 0xf5];

/// Reads many features at once — in batches, because ddcutil limits arguments.
pub fn read(display: u32, codes: &[u8]) -> BTreeMap<u8, VcpValue> {
    let mut map = BTreeMap::new();
    if codes.is_empty() {
        return map;
    }
    let core: Vec<u8> = CORE.iter().copied().filter(|c| codes.contains(c)).collect();
    if !core.is_empty() {
        read_chunk_into(display, &core, &mut map);
    }
    let rest: Vec<u8> = codes
        .iter()
        .copied()
        .filter(|c| !CORE.contains(c))
        .collect();
    for chunk in rest.chunks(READ_CHUNK) {
        read_chunk_into(display, chunk, &mut map);
    }
    map
}

fn read_chunk_into(display: u32, codes: &[u8], map: &mut BTreeMap<u8, VcpValue>) {
    for attempt in 0..READ_ATTEMPTS {
        let mut args = vec![
            "--display".to_string(),
            display.to_string(),
            // Without --terse ddcutil prints "VCP code 0x10 (Brightness): current value = ...",
            // a format that cannot be parsed with one simple line split.
            "--terse".to_string(),
            "getvcp".to_string(),
        ];
        for c in codes {
            args.push(format!("{c:02X}"));
        }
        let Ok(out) = run(&args) else { return };
        let debug = std::env::var_os("LEGION_DEBUG").is_some();
        if debug {
            eprintln!(
                "[ddc] getvcp {} codes (attempt {}), ok={}",
                codes.len(),
                attempt + 1,
                out.ok
            );
            eprintln!("[ddc] stderr: {}", out.stderr.trim());
        }
        let before = map.len();
        for line in out.stdout.lines() {
            if let Some((code, v)) = parse_vcp_line(line).or_else(|| parse_verbose_line(line)) {
                map.insert(code, v);
            }
        }
        if debug {
            eprintln!("[ddc] parsed {} new values", map.len() - before);
        }
        // Whatever arrived is better than nothing — ddcutil can return rc≠0
        // only because some codes are "not readable".
        if map.len() > before {
            return;
        }
    }
}

/// Fallback parser for the format without `--terse`:
/// `VCP code 0x10 (Brightness  ): current value =   100, max value =   100`
/// `VCP code 0xf4 (Manufacturer Specific ): mh=0x02, ml=0x01, sh=0x00, sl=0x31`
fn parse_verbose_line(line: &str) -> Option<(u8, VcpValue)> {
    let rest = line.trim().strip_prefix("VCP code 0x")?;
    let (code_txt, rest) = rest.split_once(' ')?;
    let code = u8::from_str_radix(code_txt.trim(), 16).ok()?;
    let body = rest.rsplit_once("):").map(|(_, b)| b).unwrap_or(rest);

    if let Some(after) = body.split_once("current value").map(|(_, b)| b) {
        // "=   100, max value =   100"
        let nums: Vec<&str> = after
            .split(|c: char| !(c.is_ascii_digit() || c == 'x' || c.is_ascii_hexdigit()))
            .filter(|s| !s.is_empty())
            .collect();
        let mut it = nums.iter().filter_map(|s| {
            let s = s.trim();
            match s.strip_prefix('x') {
                Some(hex) => u16::from_str_radix(hex, 16).ok(),
                None => s.parse::<u16>().ok(),
            }
        });
        let value = it.next()?;
        let max = it.next().unwrap_or(value);
        return Some((
            code,
            VcpValue {
                value,
                max: max.max(value),
            },
        ));
    }

    let field = |name: &str| -> Option<u16> {
        let idx = body.find(name)?;
        let tail = &body[idx + name.len()..];
        let tail = tail.trim_start_matches(['=', ' ']);
        hex_token(tail.split([',', ' ']).next()?)
    };
    let (mh, ml, sh, sl) = (field("mh")?, field("ml")?, field("sh")?, field("sl")?);
    Some((
        code,
        VcpValue {
            max: (mh << 8) | ml,
            value: (sh << 8) | sl,
        },
    ))
}

fn hex_token(tok: &str) -> Option<u16> {
    let t = tok.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix('x'))
        .unwrap_or(t);
    u16::from_str_radix(t, 16).ok()
}

/// "VCP 10 C 100 100" / "VCP 60 SNC x12" / "VCP F5 CNC x04 x02 x04 x01"
fn parse_vcp_line(line: &str) -> Option<(u8, VcpValue)> {
    let t: Vec<&str> = line.split_whitespace().collect();
    if t.len() < 4 || t[0] != "VCP" {
        return None;
    }
    let code = u8::from_str_radix(t[1], 16).ok()?;
    match t[2] {
        "C" | "NC" => {
            let value = if t[2] == "C" {
                t[3].parse::<u16>().ok()?
            } else {
                hex_token(t[3])?
            };
            let max = t
                .get(4)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(value);
            Some((
                code,
                VcpValue {
                    value,
                    max: max.max(value),
                },
            ))
        }
        "SNC" => {
            let value = hex_token(t[3])?;
            Some((code, VcpValue { value, max: value }))
        }
        "CNC" => {
            if t.len() != 7 {
                return None;
            }
            let mut bytes = [0u16; 4];
            for (byte, token) in bytes.iter_mut().zip(&t[3..]) {
                let digits = token
                    .strip_prefix("0x")
                    .or_else(|| token.strip_prefix('x'))
                    .unwrap_or(token);
                if !digits.bytes().all(|digit| digit.is_ascii_hexdigit()) {
                    return None;
                }
                *byte = u8::from_str_radix(digits, 16).ok()? as u16;
            }
            Some((
                code,
                VcpValue {
                    max: (bytes[0] << 8) | bytes[1],
                    value: (bytes[2] << 8) | bytes[3],
                },
            ))
        }
        _ => None,
    }
}

/// Write. The monitor accepts the write, but an immediate read is often stale,
/// so we always call with `--noverify` and read back separately after a pause.
pub fn set(display: u32, code: u8, value: u16) -> Result<(), String> {
    let mut args = vec!["--display".to_string(), display.to_string()];
    if code >= 0xE0 {
        args.push("--permit-unknown-feature".to_string());
    }
    args.push("--noverify".to_string());
    args.push("setvcp".to_string());
    args.push(format!("{code:02X}"));
    args.push(format!("0x{value:04x}"));
    let out = run(&args)?;
    if out.ok {
        Ok(())
    } else {
        Err(with_reason(&out))
    }
}

/// Whether a failed `setvcp` can still have applied.
///
/// The R45w-30 answers EIO (or "Invalid value") while it switches PIP/PBP
/// modes even though the write lands. Callers must continue and decide by
/// reading the register back instead of treating this as a hard failure.
pub fn is_transient(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("eio")
        || error.contains("-5")
        || error.contains("invalid value")
        || error.contains("unsupported")
        // The link renegotiates when PBP opens or closes: ddcutil then reports
        // a lost bus for a few seconds even though the write landed.
        || error.contains("ddc communication failed")
        || error.contains("no monitor detected")
        || error.contains("retries")
}

/// Turns an input name from ddcutil into a human-readable one.
pub fn human_input(raw: &str) -> String {
    let r = raw.trim();
    let lower = r.to_lowercase();
    if lower.starts_with("displayport") || lower.starts_with("dp") {
        return "DisplayPort".to_string();
    }
    if let Some(rest) = lower.strip_prefix("hdmi-") {
        return format!("HDMI {rest}");
    }
    if lower == "hdmi" {
        return "HDMI".to_string();
    }
    if lower.starts_with("usb-c") || lower.starts_with("usbc") {
        return "USB-C".to_string();
    }
    if let Some(rest) = lower.strip_prefix("dvi-") {
        return format!("DVI {rest}");
    }
    if lower.starts_with("analog") {
        return "VGA".to_string();
    }
    // Fallback: "hdmi-2" -> "HDMI 2"
    let cleaned = r.replace('-', " ");
    let mut chars = cleaned.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => r.to_string(),
    }
}

/// Connector name for a VCP 0x60 input code.
///
/// `0x31` is USB-C on this monitor, but ddcutil does not know that code and
/// reports it as "Unrecognized value".
pub fn known_input(code: u16) -> Option<&'static str> {
    Some(match code {
        0x01 => "VGA",
        0x03 => "DVI",
        0x0f => "DisplayPort",
        0x10 => "DisplayPort 2",
        0x11 => "HDMI 1",
        0x12 => "HDMI 2",
        0x13 => "HDMI 3",
        0x1f | 0x31 => "USB-C",
        _ => return None,
    })
}

/// Label of an input in the input list.
///
/// Our own connector names win, because ddcutil fills the value list with
/// placeholders ("Unrecognized value") for codes it does not know. A real name
/// reported by the monitor is the fallback; a code we cannot name is shown as
/// a code, because a word like "unknown" among the connectors helps nobody.
pub fn input_card_name(t: &Strings, code: u16, reported: &str) -> String {
    if let Some(name) = known_input(code) {
        return name.to_string();
    }
    let lower = reported.trim().to_lowercase();
    if !lower.is_empty() && !lower.contains("unrecognized") && lower != "unknown" {
        return human_input(reported);
    }
    t.fill(Key::InputUnknownFmt, &[format!("{code:02x}")])
}

/// Input name from the VCP code (when the monitor did not provide a list).
///
/// Connector names (VGA, DVI, DisplayPort, HDMI, USB-C) are proper nouns and
/// stay untranslated; only the unknown fallbacks come from `i18n`.
pub fn input_name(t: &Strings, code: u16) -> String {
    match code {
        // 0x00 in the window register means "this window is not used".
        0x00 => "—".into(),
        0xff => t.t(Key::InputUnknown).to_string(),
        other => known_input(other)
            .map(|name| name.to_string())
            .unwrap_or_else(|| t.fill(Key::InputUnknownFmt, &[format!("{other:02x}")])),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        input_card_name, is_transient, parse_capabilities, parse_vcp_line, parse_verbose_line,
        VcpValue,
    };

    fn assert_value(parsed: Option<(u8, VcpValue)>, expected: (u8, u16, u16)) {
        let (code, value) = parsed.expect("expected a valid VCP line");
        assert_eq!((code, value.value, value.max), expected);
    }

    #[test]
    fn link_renegotiation_is_treated_as_transient() {
        for error in [
            "DDC communication failed for monitor on bus /dev/i2c-8",
            "No monitor detected on bus /dev/i2c-8",
            "maximum retries exceeded",
            "EIO in ddc_write_read_with_retry",
        ] {
            assert!(is_transient(error), "{error}");
        }
        assert!(!is_transient("permission denied"));
    }

    #[test]
    fn terse_continuous_values_preserve_decimal_and_max_fallbacks() {
        for (line, expected) in [
            ("VCP 10 C 42 100", (0x10, 42, 100)),
            ("  VCP\t12 C 100 100  ", (0x12, 100, 100)),
            ("VCP 10 C 42", (0x10, 42, 42)),
            ("VCP 10 C 42 20", (0x10, 42, 42)),
            ("VCP 10 C 65535 65535", (0x10, 65535, 65535)),
            ("VCP 60 NC x12", (0x60, 0x12, 0x12)),
        ] {
            assert_value(parse_vcp_line(line), expected);
        }
    }

    #[test]
    fn terse_simple_non_continuous_values_accept_existing_hex_formats() {
        for line in ["VCP 60 SNC x12", "VCP 60 SNC 0x12", "VCP 60 SNC 12"] {
            assert_value(parse_vcp_line(line), (0x60, 0x12, 0x12));
        }
        assert_value(parse_vcp_line("VCP f0 SNC xAb"), (0xf0, 0xab, 0xab));
    }

    #[test]
    fn terse_complex_non_continuous_values_combine_exactly_four_bytes() {
        for line in [
            "VCP F5 CNC x04 x02 x04 x01",
            "VCP f5 CNC 0x04 0x02 0x04 0x01",
            " VCP\tF5 CNC 04 02 04 01 ",
        ] {
            assert_value(parse_vcp_line(line), (0xf5, 0x0401, 0x0402));
        }
        assert_value(
            parse_vcp_line("VCP F5 CNC x00 x00 xFF xff"),
            (0xf5, 0xffff, 0),
        );
        assert_value(
            parse_vcp_line("VCP F5 CNC xFF xFF x00 x00"),
            (0xf5, 0, 0xffff),
        );
    }

    #[test]
    fn terse_complex_non_continuous_values_reject_wrong_token_counts() {
        for line in [
            "VCP F5 CNC",
            "VCP F5 CNC x04",
            "VCP F5 CNC x04 x02 x04",
            "VCP F5 CNC x04 x02 x04 x01 x00",
            "VCP F5 CNC invalid x04 x02 x04 x01",
            "VCP F5 CNC x04 x02 x04 x01 invalid",
        ] {
            assert!(parse_vcp_line(line).is_none(), "accepted {line:?}");
        }
    }

    #[test]
    fn terse_complex_non_continuous_values_reject_invalid_bytes_in_every_position() {
        for invalid in [
            "invalid", "xGG", "0x", "x", "-1", "+01", "x+01", "0x+01", "x100", "100", "0xFFFF",
            "x10000",
        ] {
            for position in 0..4 {
                let mut tokens = ["x04", "x02", "x04", "x01"];
                tokens[position] = invalid;
                let line = format!("VCP F5 CNC {}", tokens.join(" "));
                assert!(parse_vcp_line(&line).is_none(), "accepted {line:?}");
            }
        }
    }

    #[test]
    fn terse_values_reject_invalid_headers_types_and_values() {
        for line in [
            "",
            "VCP 10 C",
            "not VCP 10 C 42 100",
            "VCP GG C 42 100",
            "VCP 100 C 42 100",
            "VCP 10 unknown 42 100",
            "VCP 10 C invalid 100",
            "VCP 10 C 65536 100",
            "VCP 60 SNC xGG",
            "VCP 60 SNC x10000",
        ] {
            assert!(parse_vcp_line(line).is_none(), "accepted {line:?}");
        }
    }

    #[test]
    fn verbose_continuous_values_preserve_decimal_and_max_fallbacks() {
        for (line, expected) in [
            (
                "VCP code 0x10 (Brightness  ): current value =   42, max value =   100",
                (0x10, 42, 100),
            ),
            (
                "  VCP code 0x12 (Contrast): current value = 100, max value = 100  ",
                (0x12, 100, 100),
            ),
            (
                "VCP code 0x10 (Brightness): current value = 42",
                (0x10, 42, 42),
            ),
            (
                "VCP code 0x10 (Brightness): current value = 42, max value = 20",
                (0x10, 42, 42),
            ),
        ] {
            assert_value(parse_verbose_line(line), expected);
        }
    }

    #[test]
    fn verbose_raw_fields_preserve_byte_order_and_existing_hex_formats() {
        for line in [
            "VCP code 0xf4 (Manufacturer Specific ): mh=0x02, ml=0x01, sh=0x00, sl=0x31",
            " VCP code 0xF4 (Manufacturer Specific ): sl=x31, sh=x00, ml=x01, mh=x02 ",
            "VCP code 0xf4 (Manufacturer Specific ): mh=02, ml=01, sh=00, sl=31",
        ] {
            assert_value(parse_verbose_line(line), (0xf4, 0x0031, 0x0201));
        }
    }

    #[test]
    fn verbose_values_reject_invalid_headers_and_missing_or_invalid_fields() {
        for line in [
            "",
            "VCP 10 C 42 100",
            "VCP code 0xGG (Brightness): current value = 42, max value = 100",
            "VCP code 0x10 (Brightness): current value = unavailable",
            "VCP code 0xf4 (Manufacturer Specific ): mh=0x02, ml=0x01, sh=0x00",
            "VCP code 0xf4 (Manufacturer Specific ): mh=0x02, ml=0x01, sh=0x00, sl=0xGG",
        ] {
            assert!(parse_verbose_line(line).is_none(), "accepted {line:?}");
        }
    }

    /// The R45w-30 reports every input source twice and calls USB-C
    /// "Unrecognized value".
    const CAPABILITIES: &str = "\
Model: Legion R45w-30
VCP Features:
   Feature: 10 (Brightness)
   Feature: 60 (Input Source)
      Values:
         0f: DisplayPort-1
         11: HDMI-1
         12: HDMI-2
         31: Unrecognized value
         ff: Unrecognized value
         0f: DisplayPort-1
         11: HDMI-1
         12: HDMI-2
         31: Unrecognized value
";

    #[test]
    fn capabilities_parsing_keeps_the_first_entry_per_input_code() {
        let caps = parse_capabilities(CAPABILITIES);
        let codes: Vec<u16> = caps.inputs.iter().map(|(code, _)| *code).collect();
        assert_eq!(codes, vec![0x0f, 0x11, 0x12, 0x31, 0xff]);
        assert_eq!(caps.inputs[0].1, "DisplayPort");
        assert_eq!(caps.inputs[1].1, "HDMI 1");
        assert_eq!(caps.inputs[2].1, "HDMI 2");
    }

    #[test]
    fn input_cards_use_our_own_connector_names() {
        let t = crate::i18n::strings(crate::i18n::Lang::En);
        assert_eq!(input_card_name(&t, 0x31, "Unrecognized value"), "USB-C");
        assert_eq!(input_card_name(&t, 0x0f, "DisplayPort-1"), "DisplayPort");
        assert_eq!(input_card_name(&t, 0x11, "HDMI-1"), "HDMI 1");
        assert_eq!(input_card_name(&t, 0x12, "HDMI-2"), "HDMI 2");
    }

    #[test]
    fn input_cards_show_the_code_when_neither_we_nor_the_monitor_know_the_name() {
        let t = crate::i18n::strings(crate::i18n::Lang::En);
        assert_eq!(
            input_card_name(&t, 0xff, "Unrecognized value"),
            "Input 0xff"
        );
        // A real name from the monitor is kept.
        assert_eq!(input_card_name(&t, 0x77, "HDMI-3"), "HDMI 3");
    }
}
