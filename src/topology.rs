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

use serde::Deserialize;
use std::process::Command;

/// Result of a display scan. `known` is false when neither KScreen nor XRandR
/// could report the topology; callers must then treat the second screen as
/// absent (the safe direction: use the computer's own source).
#[derive(Clone, Debug, Default)]
pub struct Displays {
    pub known: bool,
    pub second_screen: bool,
}

impl Displays {
    pub fn has_second_screen(&self) -> bool {
        self.known && self.second_screen
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
    Displays {
        known: false,
        second_screen: false,
    }
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
    Some(Displays {
        known: true,
        second_screen,
    })
}

// ── XRandR ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct XOutput {
    name: String,
    connected: bool,
    enabled: bool,
    /// The exact `WxH+X+Y` token; identical tokens mean the outputs are mirrored.
    geometry: Option<String>,
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
    let mut outputs = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) || !line.contains(" connected") {
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
        });
    }
    outputs
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
    Some(Displays {
        known: true,
        second_screen,
    })
}

// ── connector names ──────────────────────────────────────────────────────

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
            {"id":1,"name":"DP-1","connected":true,"enabled":true,"clones":[]},
            {"id":2,"name":"eDP-1","connected":true,"enabled":true,"clones":[]}
        ]}"#;
        let root: KScreenRoot = serde_json::from_str(json).unwrap();
        let displays = from_kscreen(root, "dp-1").unwrap();
        assert!(displays.known);
        assert!(displays.second_screen);
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
}
