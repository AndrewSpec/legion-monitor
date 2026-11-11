//! Desktop display topology — is a second, independent screen available?
//!
//! Switching the monitor from PbP to a single picture is safe when the user
//! can still see another desktop: even if the monitor keeps the wrong input,
//! that other screen can be used to fix it. Without such a screen the
//! application has to put the computer's own source on screen before closing
//! the split.
//!
//! The monitor's DRM connector (from `ddcutil detect`) identifies which output
//! belongs to the monitor. Everything else that is connected, enabled and not
//! a clone of that output is an independent screen.
//!
//! The same scan reports the refresh rate of the monitor's active mode. PIP
//! needs a 60 Hz link and the monitor does not expose its rate over DDC/CI, so
//! the desktop topology is the only place to read it.

use serde::Deserialize;
use std::process::Command;

/// Highest refresh rate at which the monitor's PIP mode is known to work.
/// The documented rule is 60 Hz; the tolerance accepts a 59.98 Hz mode, which
/// is how a nominally 60 Hz signal is often reported.
pub const PIP_MAX_HZ: f32 = 60.5;

/// Result of a display scan. `known` is false when neither KScreen nor XRandR
/// could report the topology; callers must then treat the second screen as
/// absent (the safe direction: use the computer's own source).
#[derive(Clone, Debug, Default)]
pub struct Displays {
    pub known: bool,
    pub second_screen: bool,
    /// Active mode of the monitor's output, used only as a last-resort panel
    /// shape measurement while the monitor is in single mode.
    pub output_size: Option<(u32, u32)>,
    /// Refresh rate of the monitor's active mode, when the desktop reports it.
    pub refresh_hz: Option<f32>,
}

impl Displays {
    pub fn has_second_screen(&self) -> bool {
        self.known && self.second_screen
    }

    /// The monitor's refresh rate when it is too high for PIP.
    ///
    /// `None` means either the rate is within the documented limit or the
    /// desktop could not report it. Unknown topologies must not block a
    /// feature, because KScreen and XRandR are both unavailable on some
    /// compositors.
    pub fn refresh_too_high_for_pip(&self) -> Option<f32> {
        self.refresh_hz
            .filter(|hz| hz.is_finite() && *hz > PIP_MAX_HZ)
    }
}

/// Scans the desktop for a second independent screen.
pub fn scan(monitor_connector: &str) -> Displays {
    let monitor = short_name(monitor_connector);
    if monitor.is_empty() {
        return Displays::default();
    }

    if let Some(root) = kscreen() {
        if let Some(displays) = from_kscreen(root, &monitor) {
            return displays;
        }
    }
    if let Some(outputs) = xrandr() {
        if let Some(displays) = from_xrandr(outputs, &monitor) {
            return displays;
        }
    }
    Displays::default()
}

/// The monitor input family implied by the GPU connector.
///
/// USB-C displays through DisplayPort alt mode, so it shares the DP family
/// with a native DisplayPort connector. HDMI has its own family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Link {
    Hdmi,
    DisplayPort,
}

fn link_of_connector(connector: &str) -> Option<Link> {
    let c = connector.to_ascii_lowercase();
    if c.contains("hdmi") {
        Some(Link::Hdmi)
    } else if c.contains("dp") || c.contains("usb") {
        Some(Link::DisplayPort)
    } else {
        None
    }
}

fn link_of_source(source: u16) -> Option<Link> {
    match source {
        0x11 | 0x12 => Some(Link::Hdmi),
        0x0f | 0x31 => Some(Link::DisplayPort),
        _ => None,
    }
}

/// Infers which of the two PbP sources belongs to this computer.
///
/// Only a single unambiguous candidate is accepted. A pair such as HDMI 1 +
/// USB-C on a DisplayPort link is clear (USB-C is ours); HDMI 1 + HDMI 2 on an
/// HDMI link is not, and returns `None` so the interface can ask.
pub fn infer_self_source(pair: (u16, u16), connector: &str) -> Option<u16> {
    let link = link_of_connector(connector)?;
    let candidates: Vec<u16> = [pair.0, pair.1]
        .into_iter()
        .filter(|source| link_of_source(*source) == Some(link))
        .collect();
    let first = candidates.first()?;
    candidates
        .iter()
        .all(|source| source == first)
        .then_some(*first)
}

// ── KScreen ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct KScreenRoot {
    outputs: Vec<KScreenOutput>,
}

#[derive(Deserialize)]
struct KScreenOutput {
    id: u32,
    name: String,
    connected: bool,
    enabled: bool,
    #[serde(default)]
    clones: Vec<u32>,
    #[serde(rename = "currentModeId", default)]
    current_mode_id: String,
    #[serde(default)]
    modes: Vec<KScreenMode>,
}

#[derive(Deserialize)]
struct KScreenMode {
    id: String,
    size: KScreenSize,
    #[serde(rename = "refreshRate", default)]
    refresh_rate: Option<f64>,
}

#[derive(Deserialize)]
struct KScreenSize {
    width: u32,
    height: u32,
}

fn kscreen() -> Option<KScreenRoot> {
    let output = Command::new("kscreen-doctor")
        .env("LC_ALL", "C")
        .arg("-j")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn from_kscreen(root: KScreenRoot, monitor: &str) -> Option<Displays> {
    let ours = root
        .outputs
        .iter()
        .find(|output| same_connector(&output.name, monitor))?;
    let second_screen = root.outputs.iter().any(|output| {
        output.id != ours.id
            && output.connected
            && output.enabled
            && !output.clones.contains(&ours.id)
            && !ours.clones.contains(&output.id)
    });
    let current = ours
        .modes
        .iter()
        .find(|mode| mode.id == ours.current_mode_id);
    let output_size = current
        .map(|mode| (mode.size.width, mode.size.height))
        .filter(|(width, height)| *width > 0 && *height > 0);
    let refresh_hz = current
        .and_then(|mode| mode.refresh_rate)
        .map(|hz| hz as f32)
        .filter(|hz| hz.is_finite() && *hz > 0.0);
    Some(Displays {
        known: true,
        second_screen,
        output_size,
        refresh_hz,
    })
}

#[derive(Clone, Debug)]
struct XOutput {
    name: String,
    connected: bool,
    enabled: bool,
    /// The exact `WxH+X+Y` token; identical tokens mean the outputs are mirrored.
    geometry: Option<String>,
    /// Refresh rate of the starred mode, the one the output currently uses.
    refresh_hz: Option<f32>,
}

fn xrandr() -> Option<Vec<XOutput>> {
    let output = Command::new("xrandr")
        .env("LC_ALL", "C")
        .arg("--query")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_xrandr(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_xrandr(text: &str) -> Vec<XOutput> {
    let mut outputs: Vec<XOutput> = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            // Indented lines list the modes of the output above them.
            let Some(output) = outputs.last_mut() else {
                continue;
            };
            if output.connected && output.refresh_hz.is_none() {
                output.refresh_hz = active_refresh(line);
            }
            continue;
        }
        if !line.contains(" connected") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let rest: Vec<&str> = parts.collect();
        let connected = rest.first() == Some(&"connected");
        let geometry = rest
            .iter()
            .skip(1)
            .find(|token| is_geometry(token))
            .map(|token| (*token).to_string());
        let enabled = connected && geometry.is_some();
        outputs.push(XOutput {
            name: name.to_string(),
            connected,
            enabled,
            geometry,
            refresh_hz: None,
        });
    }
    outputs
}

/// `   5120x1440     59.98*+` — the starred mode is the active one.
fn active_refresh(line: &str) -> Option<f32> {
    let mut tokens = line.split_whitespace();
    tokens.next()?; // the mode name
    for token in tokens {
        let Some(rate) = token.trim_end_matches('+').strip_suffix('*') else {
            continue;
        };
        let hz = rate.parse::<f32>().ok()?;
        if hz.is_finite() && hz > 0.0 && hz < 1000.0 {
            return Some(hz);
        }
    }
    None
}

/// `2560x1440+0+0` — the geometry XRandR prints only for an active output.
fn is_geometry(token: &str) -> bool {
    let Some((width, rest)) = token.split_once('x') else {
        return false;
    };
    let Some((height, _)) = rest.split_once('+') else {
        return false;
    };
    width.parse::<u32>().is_ok() && height.parse::<u32>().is_ok()
}

fn from_xrandr(outputs: Vec<XOutput>, monitor: &str) -> Option<Displays> {
    let ours = outputs
        .iter()
        .find(|output| same_connector(&output.name, monitor))?;
    let second_screen = outputs.iter().any(|output| {
        output.name != ours.name
            && output.connected
            && output.enabled
            && output.geometry != ours.geometry
    });
    let output_size = ours.geometry.as_deref().and_then(geometry_size);
    Some(Displays {
        known: true,
        second_screen,
        output_size,
        refresh_hz: ours.refresh_hz,
    })
}

/// `2560x1440+0+0` → `(2560, 1440)`.
fn geometry_size(token: &str) -> Option<(u32, u32)> {
    let (size, _) = token.split_once('+')?;
    let (width, height) = size.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

/// `card1-DP-1` and `DP-1` are the same connector.
fn short_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    match lower.split_once('-') {
        Some((card, rest)) if card.starts_with("card") => rest.to_string(),
        _ => lower,
    }
}

fn same_connector(a: &str, b: &str) -> bool {
    !a.is_empty() && short_name(a) == short_name(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_the_source_from_the_gpu_link() {
        // A DisplayPort link with HDMI 1 + USB-C in PbP: the computer is USB-C.
        assert_eq!(infer_self_source((0x11, 0x31), "card1-DP-1"), Some(0x31));
        // An HDMI link with the same pair: the computer is HDMI 1.
        assert_eq!(
            infer_self_source((0x11, 0x31), "card1-HDMI-A-1"),
            Some(0x11)
        );
        // Two candidates of the same family are ambiguous.
        assert_eq!(infer_self_source((0x0f, 0x31), "card1-DP-1"), None);
        assert_eq!(infer_self_source((0x11, 0x12), "card1-HDMI-A-1"), None);
        assert_eq!(infer_self_source((0x11, 0x31), ""), None);
    }

    #[test]
    fn kscreen_counts_enabled_independent_outputs() {
        let json = r#"{"outputs":[
            {"id":1,"name":"DP-1","connected":true,"enabled":true,"clones":[],
             "currentModeId":"7","modes":[
                {"id":"6","size":{"width":2560,"height":1440}},
                {"id":"7","size":{"width":5120,"height":1440}}]},
            {"id":2,"name":"eDP-1","connected":true,"enabled":true,"clones":[]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        let displays = from_kscreen(root, "dp-1").unwrap();
        assert!(displays.known);
        assert!(displays.second_screen);
        assert_eq!(displays.output_size, Some((5120, 1440)));
    }

    #[test]
    fn kscreen_ignores_disabled_and_cloned_outputs() {
        let json = r#"{"outputs":[
            {"id":1,"name":"DP-1","connected":true,"enabled":true,"clones":[2]},
            {"id":2,"name":"eDP-1","connected":true,"enabled":true,"clones":[]},
            {"id":3,"name":"HDMI-A-1","connected":true,"enabled":false,"clones":[]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        let displays = from_kscreen(root, "card1-DP-1").unwrap();
        assert!(!displays.second_screen);
    }

    #[test]
    fn kscreen_requires_the_monitor_output() {
        let json = r#"{"outputs":[
            {"id":2,"name":"eDP-1","connected":true,"enabled":true,"clones":[]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        assert!(from_kscreen(root, "dp-1").is_none());
    }

    #[test]
    fn xrandr_counts_only_active_outputs() {
        let text = "\
eDP-1 connected primary 2560x1440+0+0 (normal left inverted right x axis y axis) 344mm x 194mm
DP-1 connected 2560x1440+2560+0 (normal left inverted right x axis y axis) 600mm x 340mm
HDMI-A-1 disconnected (normal left inverted right x axis y axis)
";
        let outputs = parse_xrandr(text);
        assert_eq!(outputs.len(), 2);
        let displays = from_xrandr(outputs, "card1-DP-1").unwrap();
        assert!(displays.second_screen);
        assert_eq!(displays.output_size, Some((2560, 1440)));
    }

    #[test]
    fn xrandr_ignores_mirrored_outputs() {
        let text = "\
DP-1 connected 2560x1440+0+0 (normal left inverted right x axis y axis) 600mm x 340mm
HDMI-A-1 connected 2560x1440+0+0 (normal left inverted right x axis y axis) 600mm x 340mm
";
        let outputs = parse_xrandr(text);
        let displays = from_xrandr(outputs, "card1-DP-1").unwrap();
        assert!(displays.known);
        assert!(!displays.second_screen);
    }

    #[test]
    fn kscreen_reads_the_active_mode_refresh_rate() {
        let json = r#"{"outputs":[
            {"id":1,"name":"DP-1","connected":true,"enabled":true,"clones":[],
             "currentModeId":"7","modes":[
                {"id":"6","name":"5120x1440@165","refreshRate":165,
                 "size":{"width":5120,"height":1440}},
                {"id":"7","name":"5120x1440@60","refreshRate":59.97999954223633,
                 "size":{"width":5120,"height":1440}}]},
            {"id":2,"name":"eDP-1","connected":false,"enabled":false,"clones":[]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        let displays = from_kscreen(root, "dp-1").unwrap();
        let hz = displays.refresh_hz.unwrap();
        assert!((hz - 59.98).abs() < 0.01, "unexpected refresh rate {hz}");
        assert_eq!(displays.refresh_too_high_for_pip(), None);
    }

    #[test]
    fn kscreen_flags_a_high_refresh_rate_for_pip() {
        let json = r#"{"outputs":[
            {"id":1,"name":"DP-1","connected":true,"enabled":true,"clones":[],
             "currentModeId":"6","modes":[
                {"id":"6","name":"5120x1440@165","refreshRate":165,
                 "size":{"width":5120,"height":1440}}]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        let displays = from_kscreen(root, "dp-1").unwrap();
        assert_eq!(displays.refresh_too_high_for_pip(), Some(165.0));
    }

    #[test]
    fn xrandr_reads_the_starred_mode_of_the_active_output() {
        let text = "\
eDP-1 connected primary 2560x1440+0+0 (normal left inverted right x axis y axis) 344mm x 194mm
   2560x1440     59.98*+
   2560x1440     165.00
DP-1 connected 5120x1440+2560+0 (normal left inverted right x axis y axis) 600mm x 340mm
   5120x1440     59.94*+
   5120x1440    165.00
HDMI-A-1 disconnected (normal left inverted right x axis y axis)
";
        let outputs = parse_xrandr(text);
        let displays = from_xrandr(outputs, "card1-DP-1").unwrap();
        let hz = displays.refresh_hz.unwrap();
        assert!((hz - 59.94).abs() < 0.01, "unexpected refresh rate {hz}");
        assert_eq!(displays.refresh_too_high_for_pip(), None);
    }

    #[test]
    fn unknown_topology_never_blocks_pip() {
        let displays = Displays::default();
        assert!(!displays.known);
        assert_eq!(displays.refresh_too_high_for_pip(), None);
    }
}
