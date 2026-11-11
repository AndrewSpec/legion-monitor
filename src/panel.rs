//! Panel shape detection for the overview drawing.
//!
//! The monitor is drawn with the real proportions of its panel. A split can
//! change both the EDID timing and the desktop mode to the size of one half,
//! so a known model is the most stable source. EDID and the active desktop
//! mode are fallbacks for monitors that are not in the table.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelSource {
    Edid,
    Model,
    Desktop,
    Default,
}

/// The shape of the monitor's picture area.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Panel {
    /// Width divided by height of the picture area.
    pub ratio: f32,
    /// Native pixels, when the source reports them. A physical-size EDID
    /// measurement has a ratio but no pixel count.
    pub native: Option<(u32, u32)>,
    pub source: PanelSource,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            ratio: 16.0 / 9.0,
            native: None,
            source: PanelSource::Default,
        }
    }
}

impl Panel {
    fn from_native(width: u32, height: u32, source: PanelSource) -> Self {
        Self {
            ratio: width as f32 / height as f32,
            native: Some((width, height)),
            source,
        }
    }

    fn from_ratio(ratio: f32, source: PanelSource) -> Self {
        Self {
            ratio,
            native: None,
            source,
        }
    }

    /// Short label such as `16:9`, `21:9`, `32:9` or `2.42:1`.
    pub fn aspect_label(&self) -> String {
        aspect_label(self.ratio)
    }

    /// Picture size in pixels, used for the monitor's PIP rectangle.
    ///
    /// The rectangle registers 0x95..0x98 take pixels, not percentages. When
    /// only the ratio is known, 1440 lines is a safe assumption; the monitor
    /// clamps the rectangle to its real panel.
    pub fn pixel_size(&self) -> (u32, u32) {
        if let Some((width, height)) = self.native {
            return (width, height);
        }
        let height = 1440u32;
        let width = (height as f32 * self.ratio).round().max(1.0) as u32;
        (width, height)
    }
}

/// Panels this application knows by model name, independent of the mode the
/// monitor currently exposes over EDID. Matching is case-insensitive and
/// ignores spaces, hyphens and underscores.
const KNOWN_MODELS: &[(&str, (u32, u32))] = &[("r45w", (5120, 1440)), ("r34w", (3440, 1440))];

/// Detects the panel shape.
///
/// `desktop` is the active desktop mode; the caller must pass `None` during a
/// split, because the desktop then covers only one half of the panel.
pub fn detect(connector: &str, model: &str, desktop: Option<(u32, u32)>) -> Panel {
    if let Some((width, height)) = known_model(model) {
        return Panel::from_native(width, height, PanelSource::Model);
    }
    if let Some(panel) = edid_panel(connector) {
        return panel;
    }
    if let Some((width, height)) = desktop.filter(|(width, height)| *width > 0 && *height > 0) {
        return Panel::from_native(width, height, PanelSource::Desktop);
    }
    Panel::default()
}

fn known_model(model: &str) -> Option<(u32, u32)> {
    let normalized: String = model
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect();
    KNOWN_MODELS
        .iter()
        .find(|(needle, _)| normalized.contains(needle))
        .map(|(_, size)| *size)
}

fn aspect_label(ratio: f32) -> String {
    const TOLERANCE: f32 = 0.06;
    for (target, label) in [
        (16.0 / 9.0, "16:9"),
        (21.0 / 9.0, "21:9"),
        (32.0 / 9.0, "32:9"),
    ] {
        if (ratio - target).abs() <= target * TOLERANCE {
            return label.to_string();
        }
    }
    format!("{ratio:.2}:1")
}

/// Reads the connector's EDID from sysfs. The connector comes from `ddcutil
/// detect`, for example `card1-DP-1`.
fn edid_panel(connector: &str) -> Option<Panel> {
    let name = connector.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains("..")
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return None;
    }
    let bytes = std::fs::read(Path::new("/sys/class/drm").join(name).join("edid")).ok()?;
    parse_edid(&bytes)
}

/// Parses the base EDID block.
///
/// The physical image size is preferred over the detailed timing: a 5120 px
/// wide panel cannot encode its width in the 12-bit timing field, while the
/// physical size still describes the whole panel. Some monitors only fill in
/// the timing, so it remains a fallback.
pub(crate) fn parse_edid(bytes: &[u8]) -> Option<Panel> {
    const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
    if bytes.len() < 128 || bytes[..8] != HEADER {
        return None;
    }

    // Maximum image size in centimetres.
    let cm_width = f32::from(bytes[21]);
    let cm_height = f32::from(bytes[22]);
    if cm_width >= 10.0 && cm_height >= 10.0 {
        let ratio = cm_width / cm_height;
        if (1.0..=5.0).contains(&ratio) {
            return Some(Panel::from_ratio(ratio, PanelSource::Edid));
        }
    }

    // Four 18-byte detailed timing descriptors; the first non-zero clock wins.
    for index in 0..4 {
        let start = 54 + index * 18;
        let d = &bytes[start..start + 18];
        if d[0] == 0 && d[1] == 0 {
            continue;
        }
        let width = u32::from(d[2]) | (u32::from(d[4] & 0xf0) << 4);
        let height = u32::from(d[5]) | (u32::from(d[7] & 0xf0) << 4);
        if width == 0 || height == 0 {
            continue;
        }
        let ratio = width as f32 / height as f32;
        if (1.0..=5.0).contains(&ratio) {
            return Some(Panel::from_native(width, height, PanelSource::Edid));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edid(timing: Option<(u32, u32)>, centimetres: Option<(u8, u8)>) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes[..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
        if let Some((width, height)) = centimetres {
            bytes[21] = width;
            bytes[22] = height;
        }
        if let Some((width, height)) = timing {
            // One detailed timing descriptor at offset 54.
            bytes[54] = 0x01;
            bytes[55] = 0x01; // non-zero pixel clock
            bytes[56] = (width & 0xff) as u8;
            bytes[58] = (((width >> 8) & 0x0f) as u8) << 4;
            bytes[59] = (height & 0xff) as u8;
            bytes[61] = (((height >> 8) & 0x0f) as u8) << 4;
        }
        bytes
    }

    #[test]
    fn known_legion_models_have_their_panel_shape() {
        let ultrawide = detect("", "Legion R45w-30", None);
        assert_eq!(ultrawide.aspect_label(), "32:9");
        assert_eq!(ultrawide.native, Some((5120, 1440)));
        assert_eq!(ultrawide.source, PanelSource::Model);

        let wide = detect("", "r34w-30", None);
        assert_eq!(wide.aspect_label(), "21:9");
        assert_eq!(wide.native, Some((3440, 1440)));
    }

    #[test]
    fn physical_size_survives_a_split_edid() {
        // A 32:9 panel exposed as a 2560x1440 timing: the physical size still
        // describes the whole panel.
        let bytes = edid(Some((2560, 1440)), Some((105, 30)));
        let panel = parse_edid(&bytes).unwrap();
        assert_eq!(panel.aspect_label(), "32:9");
        assert_eq!(panel.native, None);
        assert_eq!(panel.source, PanelSource::Edid);
    }

    #[test]
    fn detailed_timing_is_a_fallback() {
        let bytes = edid(Some((3440, 1440)), None);
        let panel = parse_edid(&bytes).unwrap();
        assert_eq!(panel.aspect_label(), "21:9");
        assert_eq!(panel.native, Some((3440, 1440)));
    }

    #[test]
    fn desktop_mode_is_the_last_measured_fallback() {
        let panel = detect("", "", Some((2560, 1080)));
        assert_eq!(panel.aspect_label(), "21:9");
        assert_eq!(panel.source, PanelSource::Desktop);
    }

    #[test]
    fn unknown_inputs_fall_back_to_16_9() {
        let panel = detect("card1-DP-9", "", None);
        assert_eq!(panel.aspect_label(), "16:9");
        assert_eq!(panel.source, PanelSource::Default);
    }

    #[test]
    fn bad_edid_is_rejected() {
        assert!(parse_edid(&[]).is_none());
        assert!(parse_edid(&[0u8; 128]).is_none());
        let mut bytes = edid(None, None);
        bytes[0] = 0x01;
        assert!(parse_edid(&bytes).is_none());
    }

    #[test]
    fn unusual_ratio_gets_a_numeric_label() {
        let panel = Panel::from_native(2560, 1600, PanelSource::Desktop);
        assert_eq!(panel.aspect_label(), "1.60:1");
    }

    #[test]
    fn pixel_size_is_known_for_the_supported_panels() {
        assert_eq!(detect("", "R45w-30", None).pixel_size(), (5120, 1440));
        assert_eq!(detect("", "R34w-30", None).pixel_size(), (3440, 1440));
        assert_eq!(Panel::default().pixel_size(), (2560, 1440));
    }
}
