//! Feature catalog: human-readable names, section assignment, presentation.
//! VCP codes never reach the interface — they are used internally only,
//! and every name comes from `i18n`, so it is translated.

use crate::i18n::Key;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Overview,
    Image,
    Color,
    Inputs,
    MultiView,
    Profiles,
    Rescue,
    Advanced,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Overview,
        Section::Image,
        Section::Color,
        Section::Inputs,
        Section::MultiView,
        Section::Profiles,
        Section::Rescue,
    ];

    /// Maps a command-line page name (`--page multi-view`) to a section.
    pub fn by_slug(slug: &str) -> Option<Section> {
        match slug.to_lowercase().as_str() {
            "overview" | "przeglad" => Some(Section::Overview),
            "image" | "obraz" => Some(Section::Image),
            "color" | "kolor" => Some(Section::Color),
            "inputs" | "zrodla" => Some(Section::Inputs),
            "multi-view" | "multiview" | "pip" | "pbp" => Some(Section::MultiView),
            "profiles" | "profile" => Some(Section::Profiles),
            "rescue" | "ratunek" => Some(Section::Rescue),
            "advanced" | "zaawansowane" => Some(Section::Advanced),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Slider 0..max.
    Continuous,
    /// Toggle (max = 1).
    Toggle,
    /// Choice list with the values reported by the monitor.
    Choice,
    /// Read-only — shown as information, without editing.
    ReadOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct Desc {
    pub code: u8,
    pub section: Section,
    pub kind: Kind,
}

const fn d(code: u8, section: Section, kind: Kind) -> Desc {
    Desc {
        code,
        section,
        kind,
    }
}

/// Feature label in the interface language. `None` means "code outside the
/// catalog" — the name reported by the monitor is shown instead.
pub fn fn_key(code: u8) -> Option<Key> {
    Some(match code {
        0x10 => Key::FnBrightness,
        0x12 => Key::FnContrast,
        0x87 => Key::FnSharpness,
        0x8a => Key::FnSaturation,
        0x8b => Key::FnHue,
        0x72 => Key::FnGamma,
        0x6d => Key::FnBlackLevel,
        0x0e | 0x70 => Key::FnClockPhase,
        0x0c => Key::FnColorTemp,
        0x14 => Key::FnColorPreset,
        0x16 => Key::FnRed,
        0x18 => Key::FnGreen,
        0x1a => Key::FnBlue,
        0x17 => Key::FnRedOffset,
        0x19 => Key::FnGreenOffset,
        0x1b => Key::FnBlueOffset,
        0x60 => Key::FnInput,
        0xf0 => Key::FnWindowSources,
        0xf2 => Key::FnMultiMode,
        0xf4 => Key::FnWindowLayout,
        0xf5 => Key::FnWindowGeometry,
        0xf7 => Key::FnKvm,
        0xf8 => Key::FnDaisyChain,
        0x62 => Key::FnVolume,
        0x8d => Key::FnMute,
        0xd6 => Key::FnPower,
        0xcc => Key::FnOsdLang,
        0x04 => Key::FnFactoryReset,
        _ => return None,
    })
}

/// One short sentence under the label — no jargon.
pub fn hint_key(code: u8) -> Option<Key> {
    Some(match code {
        0x10 => Key::HintBrightness,
        0x12 => Key::HintContrast,
        0x87 => Key::HintSharpness,
        0x8a => Key::HintSaturation,
        0x8b => Key::HintHue,
        0x72 => Key::HintGamma,
        0x6d => Key::HintBlack,
        0x0e | 0x70 => Key::HintAnalogStab,
        0x0c => Key::HintColorTemp,
        0x14 => Key::HintColorPreset,
        0x16 => Key::HintRed,
        0x18 => Key::HintGreen,
        0x1a => Key::HintBlue,
        0x17 | 0x19 | 0x1b => Key::HintOffset,
        0x62 => Key::HintVolume,
        0x8d => Key::HintMute,
        0xd6 => Key::HintPower,
        0xcc => Key::HintOsdLang,
        0x60 => Key::HintInput,
        _ => return None,
    })
}

/// Features we know by name and want to show in the interface.
pub const KNOWN: &[Desc] = &[
    // Picture
    d(0x10, Section::Image, Kind::Continuous),
    d(0x12, Section::Image, Kind::Continuous),
    d(0x87, Section::Image, Kind::Continuous),
    d(0x8a, Section::Image, Kind::Continuous),
    d(0x8b, Section::Image, Kind::Continuous),
    d(0x72, Section::Image, Kind::Continuous),
    d(0x6d, Section::Image, Kind::Continuous),
    d(0x0e, Section::Image, Kind::Continuous),
    d(0x70, Section::Image, Kind::Continuous),
    // Color
    d(0x0c, Section::Color, Kind::Choice),
    d(0x14, Section::Color, Kind::Choice),
    d(0x16, Section::Color, Kind::Continuous),
    d(0x18, Section::Color, Kind::Continuous),
    d(0x1a, Section::Color, Kind::Continuous),
    d(0x17, Section::Color, Kind::Continuous),
    d(0x19, Section::Color, Kind::Continuous),
    d(0x1b, Section::Color, Kind::Continuous),
    // Inputs
    d(0x60, Section::Inputs, Kind::Choice),
    // Multi-view
    d(0xf0, Section::MultiView, Kind::ReadOnly),
    d(0xf2, Section::MultiView, Kind::Toggle),
    d(0xf4, Section::MultiView, Kind::ReadOnly),
    d(0xf5, Section::MultiView, Kind::Choice),
    d(0xf7, Section::MultiView, Kind::ReadOnly),
    d(0xf8, Section::MultiView, Kind::ReadOnly),
    // Advanced
    d(0x62, Section::Advanced, Kind::Continuous),
    d(0x8d, Section::Advanced, Kind::Toggle),
    d(0xd6, Section::Advanced, Kind::Choice),
    d(0xcc, Section::Advanced, Kind::Choice),
    d(0x04, Section::Advanced, Kind::Toggle),
    // Read-only codes — they make no sense as sliders, so they go into the
    // "Technical information" card in the Advanced section.
    d(0x02, Section::Advanced, Kind::ReadOnly),
    d(0x0b, Section::Advanced, Kind::ReadOnly),
    d(0x52, Section::Advanced, Kind::ReadOnly),
    d(0x86, Section::Advanced, Kind::ReadOnly),
    d(0x95, Section::Advanced, Kind::ReadOnly),
    d(0x96, Section::Advanced, Kind::ReadOnly),
    d(0x97, Section::Advanced, Kind::ReadOnly),
    d(0x98, Section::Advanced, Kind::ReadOnly),
    d(0xac, Section::Advanced, Kind::ReadOnly),
    d(0xae, Section::Advanced, Kind::ReadOnly),
    d(0xb2, Section::Advanced, Kind::ReadOnly),
    d(0xb6, Section::Advanced, Kind::ReadOnly),
    d(0xc6, Section::Advanced, Kind::ReadOnly),
    d(0xc8, Section::Advanced, Kind::ReadOnly),
    d(0xc9, Section::Advanced, Kind::ReadOnly),
    d(0xca, Section::Advanced, Kind::ReadOnly),
    d(0xdf, Section::Advanced, Kind::ReadOnly),
];

/// Diagnostic codes shown as text rather than as controls.
pub fn info_key(code: u8) -> Option<Key> {
    Some(match code {
        0x02 => Key::InfoNewValue,
        0x0b => Key::InfoColorTempStep,
        0x52 => Key::InfoActiveControl,
        0x86 => Key::InfoScaling,
        0x95..=0x98 => Key::InfoWinPos,
        0xac => Key::InfoHFreq,
        0xae => Key::InfoVFreq,
        0xb2 => Key::InfoSubpixel,
        0xb6 => Key::InfoPanelType,
        0xc6 => Key::InfoAppKey,
        0xc8 => Key::InfoController,
        0xc9 => Key::InfoFirmware,
        0xca => Key::InfoOsdControl,
        0xdf => Key::InfoVcpVersion,
        _ => return None,
    })
}

/// Whether the code is technical information only (not a control).
pub fn is_info(code: u8) -> bool {
    info_key(code).is_some()
}

pub fn desc_for(code: u8) -> Option<&'static Desc> {
    KNOWN.iter().find(|d| d.code == code)
}

/// Label for a code outside the catalog: the name from the monitor, or the
/// feature number as a last resort — we never pretend to know the meaning.
pub fn unknown_label(t: &crate::i18n::Strings, code: u8, ddc_name: &str) -> String {
    let name = ddc_name.trim();
    // ddcutil calls every vendor code "Manufacturer Specific", which says
    // nothing — a name with the feature number is better for those.
    let generic = name.is_empty() || name.eq_ignore_ascii_case("manufacturer specific");
    if generic || code >= 0xe0 {
        t.fill(Key::FnUnknown, &[format!("{code:02X}")])
    } else {
        name.to_string()
    }
}

pub fn section_for(code: u8, ddc_name: &str) -> Section {
    if let Some(d) = desc_for(code) {
        return d.section;
    }
    let n = ddc_name.to_lowercase();
    if n.contains("brightness")
        || n.contains("contrast")
        || n.contains("sharpness")
        || n.contains("gamma")
        || n.contains("saturation")
        || n.contains("hue")
        || n.contains("clock")
        || n.contains("phase")
    {
        return Section::Image;
    }
    if n.contains("color") || n.contains("gain") || n.contains("temperature") {
        return Section::Color;
    }
    if n.contains("input") {
        return Section::Inputs;
    }
    Section::Advanced
}

pub fn kind_for(code: u8, max: u16) -> Kind {
    if let Some(d) = desc_for(code) {
        return d.kind;
    }
    if max == 1 {
        Kind::Toggle
    } else {
        Kind::Continuous
    }
}

/// Color presets (MCCS 0x14 / 0x0C).
pub fn color_presets(code: u8) -> &'static [(u16, Key)] {
    match code {
        0x14 => &[
            (0x01, Key::PresetSrgb),
            (0x02, Key::PresetNative),
            (0x03, Key::Preset4000),
            (0x04, Key::Preset5000),
            (0x05, Key::Preset6500),
            (0x06, Key::Preset7500),
            (0x07, Key::Preset8200),
            (0x08, Key::Preset9300),
            (0x09, Key::Preset10000),
            (0x0b, Key::PresetCustom),
        ],
        0x0c => &[
            (0x01, Key::TempWarm),
            (0x02, Key::TempNeutral),
            (0x03, Key::TempCool),
            (0x04, Key::PresetCustom),
        ],
        _ => &[],
    }
}

/// Power modes (VCP 0xD6).
pub fn power_modes() -> &'static [(u16, Key)] {
    &[
        (0x01, Key::PowerOn),
        (0x02, Key::PowerStandby),
        (0x03, Key::PowerSuspend),
    ]
}

/// The two True Split layouts the R45w-30 actually applies.
///
/// The OSD offers 50/50 and "about 70/30"; F5 uses `0x0101` and `0x0401`.
/// Other codes from the protocol notes are accepted or rejected by the
/// monitor without changing the picture, so they are not offered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrueSplit {
    Half,
    Wide,
}

impl TrueSplit {
    pub const ALL: [TrueSplit; 2] = [TrueSplit::Half, TrueSplit::Wide];

    /// F5 value that selects the layout.
    pub fn value(self) -> u16 {
        match self {
            TrueSplit::Half => 0x0101,
            TrueSplit::Wide => 0x0401,
        }
    }

    /// Left window's share of the width, used by the drawing. The register
    /// asks for 3:1, but the OSD calls the layout 70/30 and that is how the
    /// picture reads, so the preview follows the label.
    pub fn share(self) -> f32 {
        match self {
            TrueSplit::Half => 0.5,
            TrueSplit::Wide => 0.70,
        }
    }

    /// Label of the layout, for example `50 / 50`.
    pub fn key(self) -> Key {
        match self {
            TrueSplit::Half => Key::Split5050,
            TrueSplit::Wide => Key::Split7030,
        }
    }
}

/// Decodes the F5 register into a layout the monitor supports.
pub fn true_split_for(value: u16) -> Option<TrueSplit> {
    TrueSplit::ALL
        .into_iter()
        .find(|layout| layout.value() == value)
}

/// Corner of the PIP window. The monitor's OSD offers exactly these four.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PipCorner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl PipCorner {
    pub const ALL: [PipCorner; 4] = [
        PipCorner::TopLeft,
        PipCorner::TopRight,
        PipCorner::BottomLeft,
        PipCorner::BottomRight,
    ];

    /// VCP 0x95..0x98 rectangle, in panel pixels.
    ///
    /// The monitor reports these registers in pixels (0x95 max 4960, 0x96 max
    /// 1320 on the R45w-30), not in percentages. Writing 0..100 produced a
    /// 100-pixel window in the top-left corner and was the PIP bug.
    pub fn rect(self, width: u32, height: u32) -> (u16, u16, u16, u16) {
        let width = width.clamp(2, u16::MAX as u32) as u16;
        let height = height.clamp(2, u16::MAX as u32) as u16;
        let half_width = width / 2;
        let half_height = height / 2;
        match self {
            PipCorner::TopLeft => (0, 0, half_width, half_height),
            PipCorner::TopRight => (half_width, 0, width, half_height),
            PipCorner::BottomLeft => (0, half_height, half_width, height),
            PipCorner::BottomRight => (half_width, half_height, width, height),
        }
    }

    /// The same rectangle as fractions of the screen, for the drawing.
    pub fn fractions(self) -> [f32; 4] {
        match self {
            PipCorner::TopLeft => [0.0, 0.0, 0.5, 0.5],
            PipCorner::TopRight => [0.5, 0.0, 1.0, 0.5],
            PipCorner::BottomLeft => [0.0, 0.5, 0.5, 1.0],
            PipCorner::BottomRight => [0.5, 0.5, 1.0, 1.0],
        }
    }

    /// Recovers the corner from a rectangle reported by the monitor.
    /// Returns `None` for an empty, inverted or out-of-range rectangle.
    pub fn from_pixels(
        left: u16,
        top: u16,
        right: u16,
        bottom: u16,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        if width == 0
            || height == 0
            || left >= right
            || top >= bottom
            || right as u32 > width
            || bottom as u32 > height
        {
            return None;
        }
        Self::from_fractions([
            left as f32 / width as f32,
            top as f32 / height as f32,
            right as f32 / width as f32,
            bottom as f32 / height as f32,
        ])
    }

    /// Recovers the corner from a 0..1 rectangle.
    pub fn from_fractions([left, top, right, bottom]: [f32; 4]) -> Option<Self> {
        if !(0.0..=1.0).contains(&left)
            || !(0.0..=1.0).contains(&top)
            || !(0.0..=1.0).contains(&right)
            || !(0.0..=1.0).contains(&bottom)
            || left >= right
            || top >= bottom
        {
            return None;
        }
        let horizontal_left = (left + right) / 2.0 < 0.5;
        let vertical_top = (top + bottom) / 2.0 < 0.5;
        Some(match (horizontal_left, vertical_top) {
            (true, true) => PipCorner::TopLeft,
            (false, true) => PipCorner::TopRight,
            (true, false) => PipCorner::BottomLeft,
            (false, false) => PipCorner::BottomRight,
        })
    }

    pub fn key(self) -> Key {
        match self {
            PipCorner::TopLeft => Key::PipCornerTopLeft,
            PipCorner::TopRight => Key::PipCornerTopRight,
            PipCorner::BottomLeft => Key::PipCornerBottomLeft,
            PipCorner::BottomRight => Key::PipCornerBottomRight,
        }
    }
}

/// Multi-view mode derived from F2 + F4.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MultiMode {
    Off,
    PictureInPicture,
    PictureByPicture,
    Unknown,
}

impl MultiMode {
    pub fn key(self) -> Key {
        match self {
            MultiMode::Off => Key::ModeSingle,
            MultiMode::PictureInPicture => Key::ModePip,
            MultiMode::PictureByPicture => Key::ModePbp,
            MultiMode::Unknown => Key::MultiUnknown,
        }
    }
}

/// Decodes the state from the registers that were read.
///
/// `F2` is the enable/swap register and is authoritative when it is zero:
/// the monitor then has no split. Its non-zero values are not stable — after
/// an input switch it can read back `0x5D` — so in that case the split type
/// comes from the high byte of `F4` (`0x01` = PIP, `0x02` = PBP). Requiring
/// `F2 == 1` hid the window pickers exactly when the monitor was in PBP.
///
/// A zero `F4` is the other reliable "no split" signal. The R45w-30 reports
/// `F2 = 1` and `F4 = 0` in single mode (observed 2026-11-11), so treating a
/// non-zero `F2` as proof of a split left the Overview on "unknown" while the
/// monitor was plainly showing one input.
pub fn multi_mode(f2: Option<u16>, f4: Option<u16>) -> MultiMode {
    if f2 == Some(0) || f4 == Some(0) {
        return MultiMode::Off;
    }
    match f4.map(|v| v >> 8) {
        Some(0x01) => MultiMode::PictureInPicture,
        Some(0x02) => MultiMode::PictureByPicture,
        _ => MultiMode::Unknown,
    }
}

/// Sources visible in the windows — from VCP F0 (two bytes).
///
/// The monitor returns this register inconsistently (observed 0x0031, 0x0202,
/// 0x0001, 0xFFFF), so only values that make sense as picture sources are
/// accepted — otherwise we show "unknown" instead of an invented input.
///
/// Encoding is low byte = primary/left window, high byte = secondary/right
/// window. Two independent observations agree: with the left half on HDMI 1
/// and the right on USB-C the register read `0x3111`, and Lenovo's own service
/// writes VCP 0x60 as `(secondary << 8) | primary`. After the OSD set the right
/// window to HDMI 2 it read `0x3112`; after a swap the picture followed
/// `0x1231`, which again puts the right window in the high byte.
///
/// The register is also unreliable in another way: it usually reports the
/// source of the window that was written last, not a stable left/right pair.
pub fn window_sources(f0: Option<u16>) -> Option<(u16, u16)> {
    let v = f0?;
    let left = v & 0xff;
    let right = (v >> 8) & 0xff;
    if !known_source(left as u8) || !known_source(right as u8) {
        return None;
    }
    // A split always has two pictures; a zero or 0xFF byte means the monitor
    // did not report that half, not that the input is "none".
    if left == 0 || right == 0 {
        return None;
    }
    Some((left, right))
}

/// Combines the two window sources into the 16-bit value of VCP 0x60.
///
/// Lenovo's service writes `(secondary << 8) | primary`; primary is the
/// left/top window and secondary the right/bottom one.
pub fn window_pair(left: u16, right: u16) -> u16 {
    ((right & 0xff) << 8) | (left & 0xff)
}

/// Whether an F0 byte looks like a picture-source code (0 = no window, 0xff = unknown).
pub fn known_source(b: u8) -> bool {
    matches!(
        b,
        0x01 | 0x03 | 0x0f | 0x10 | 0x11 | 0x12 | 0x13 | 0x1f | 0x31
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_mode_uses_f4_when_f2_is_stale() {
        for f2 in [None, Some(2), Some(93), Some(u16::MAX)] {
            assert_eq!(multi_mode(f2, None), MultiMode::Unknown);
            assert_eq!(multi_mode(f2, Some(0)), MultiMode::Off);
        }
        assert_eq!(multi_mode(Some(0), Some(0x0201)), MultiMode::Off);
        assert_eq!(
            multi_mode(Some(1), Some(0x0101)),
            MultiMode::PictureInPicture
        );
        assert_eq!(
            multi_mode(Some(1), Some(0x0201)),
            MultiMode::PictureByPicture
        );
        // After an input switch the monitor can report a stale F2; the split
        // type from F4 is still usable.
        assert_eq!(
            multi_mode(Some(0x5d), Some(0x0201)),
            MultiMode::PictureByPicture
        );
        // Live R45w-30 single mode: F2 = 1, F4 = 0. The old code called this
        // "unknown" and the Overview showed no picture mode at all.
        assert_eq!(multi_mode(Some(1), Some(0)), MultiMode::Off);
    }

    #[test]
    fn window_sources_are_low_left_high_right() {
        assert_eq!(window_sources(Some(0x3111)), Some((0x11, 0x31)));
        assert_eq!(window_sources(Some(0x1231)), Some((0x31, 0x12)));
        assert_eq!(window_sources(Some(0xffff)), None);
        assert_eq!(window_sources(Some(0)), None);
        assert_eq!(window_pair(0x11, 0x31), 0x3111);
        assert_eq!(window_pair(0x31, 0x12), 0x1231);
    }

    #[test]
    fn true_split_decodes_only_the_supported_codes() {
        assert_eq!(true_split_for(0x0101), Some(TrueSplit::Half));
        assert_eq!(true_split_for(0x0401), Some(TrueSplit::Wide));
        assert_eq!(TrueSplit::Half.share(), 0.5);
        assert_eq!(TrueSplit::Wide.share(), 0.70);
        for code in [0, 0x0102, 0x0201, 0x0202, 0x0301, 0x0302, 0x0402, 0x00ff] {
            assert_eq!(true_split_for(code), None, "{code:04X}");
        }
    }

    #[test]
    fn pip_corners_are_quarter_pixel_rectangles() {
        assert_eq!(PipCorner::TopLeft.rect(5120, 1440), (0, 0, 2560, 720));
        assert_eq!(PipCorner::TopRight.rect(5120, 1440), (2560, 0, 5120, 720));
        assert_eq!(PipCorner::BottomLeft.rect(5120, 1440), (0, 720, 2560, 1440));
        assert_eq!(
            PipCorner::BottomRight.rect(5120, 1440),
            (2560, 720, 5120, 1440)
        );
        for corner in PipCorner::ALL {
            let (left, top, right, bottom) = corner.rect(5120, 1440);
            assert_eq!(
                PipCorner::from_pixels(left, top, right, bottom, 5120, 1440),
                Some(corner)
            );
        }
        assert_eq!(PipCorner::from_pixels(50, 50, 50, 50, 5120, 1440), None);
        assert_eq!(PipCorner::from_pixels(0, 0, 6000, 50, 5120, 1440), None);
        assert_eq!(
            PipCorner::from_fractions([0.5, 0.5, 1.0, 1.0]),
            Some(PipCorner::BottomRight)
        );
    }
}
