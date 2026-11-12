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
    About,
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
            "about" | "o-aplikacji" => Some(Section::About),
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
        0xe0 => Key::FnOverdrive,
        0xea => Key::FnDcr,
        0xef => Key::FnHdr,
        0x86 => Key::FnScaling,
        0xf9 => Key::FnGameMode,
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
        0xf7 => Key::FnTrueSplitState,
        0xf8 => Key::FnFeatureIndex,
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
        0xe0 => Key::HintOverdrive,
        0xea => Key::HintDcr,
        0xef => Key::HintHdr,
        0x86 => Key::HintScaling,
        0xf9 => Key::HintGameMode,
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
    // Lenovo vendor picture controls (LADM 2.0.1.1).
    d(0xe0, Section::Image, Kind::Choice),
    d(0xea, Section::Image, Kind::Toggle),
    d(0xef, Section::Image, Kind::Choice),
    d(0xf9, Section::Image, Kind::Choice),
    d(0x86, Section::Image, Kind::Choice),
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

/// The two split proportions the R45w-30 actually applies.
///
/// Picture-by-picture uses 50/50. True Split uses either 50/50 or the wider
/// "about 70/30" layout. F5 carries the proportion for both modes: `0x0101`
/// (1:1) and `0x0401` (3:1). Other codes from the protocol notes are accepted
/// or rejected by the monitor without changing the picture, so they are not
/// offered. True Split itself is switched by the feature registers F8/F7.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitLayout {
    Half,
    Wide,
}

impl SplitLayout {
    pub const ALL: [SplitLayout; 2] = [SplitLayout::Half, SplitLayout::Wide];

    /// F5 value that selects the layout.
    pub fn value(self) -> u16 {
        match self {
            SplitLayout::Half => 0x0101,
            SplitLayout::Wide => 0x0401,
        }
    }

    /// Left window's share of the width, used by the drawing. The register
    /// asks for 3:1, but the OSD calls the layout 70/30 and that is how the
    /// picture reads, so the preview follows the label.
    pub fn share(self) -> f32 {
        match self {
            SplitLayout::Half => 0.5,
            SplitLayout::Wide => 0.70,
        }
    }

    /// Label of the layout, for example `50 / 50`.
    pub fn key(self) -> Key {
        match self {
            SplitLayout::Half => Key::Split5050,
            SplitLayout::Wide => Key::Split7030,
        }
    }
}

/// Decodes the F5 register into a layout the monitor supports.
pub fn split_layout_for(value: u16) -> Option<SplitLayout> {
    SplitLayout::ALL
        .into_iter()
        .find(|layout| layout.value() == value)
}

/// F8 selects which monitor feature the F7 register exposes.
pub const FEATURE_INDEX: u8 = 0xf8;
/// F7 carries the value of the feature selected through F8.
pub const FEATURE_VALUE: u8 = 0xf7;
/// Feature index 0x15 is the monitor's hardware True Split.
///
/// The app enables it from the Multi-view page while USB-C is active. The
/// monitor re-enumerates the link into two 2560x1440 screens, so the worker
/// waits and then lowers both to 60 Hz. Rescue and split cleanup write F7 = 0.
pub const TRUE_SPLIT_FEATURE: u16 = 0x15;

/// Value lists for controls the monitor accepts but ddcutil may not enumerate.
///
/// Names come from Lenovo's `VCPCodeDef.json` (version 1.7) and are technical
/// by nature, so they are not translated.
pub fn static_choices(code: u8) -> Option<&'static [(u16, &'static str)]> {
    Some(match code {
        0xe0 => &[
            (0x00, "Off"),
            (0x01, "Normal"),
            (0x02, "Extreme"),
            (0x03, "Level 1"),
            (0x04, "Level 2"),
            (0x05, "Level 3"),
            (0x06, "Level 4"),
            (0x07, "Level 5"),
        ],
        0xef => &[
            (0x00, "Off"),
            (0x01, "Auto"),
            (0x02, "HDR1000"),
            (0x03, "HLG"),
            (0x04, "Dolby Vision"),
            (0x05, "HDR"),
            (0x06, "HDR400"),
            (0x07, "HDR Game"),
            (0x08, "HDR Movie"),
            (0x09, "HDR Photo"),
            (0x0a, "Dolby Vision Bright"),
            (0x0b, "Dolby Vision Dark"),
            (0x0c, "HDR10"),
            (0x0d, "HDR sRGB"),
            (0x0e, "HDR Adobe RGB"),
            (0x0f, "HDR DCI-P3"),
            (0x10, "TB400"),
        ],
        0x86 => &[
            (0x01, "No scaling"),
            (0x02, "Full Screen"),
            (0x05, "Original AR"),
            (0x0b, "4:3"),
            (0x0f, "16:9"),
            (0x10, "16:10"),
            (0x0c, "5:4"),
        ],
        0xf9 => &[
            (0x00, "Off"),
            (0x01, "FPS1"),
            (0x02, "FPS2"),
            (0x03, "RCG"),
            (0x04, "RTS"),
            (0x05, "Game 1"),
            (0x06, "Game 2"),
            (0x07, "MOBA"),
            (0x08, "ACT"),
            (0x09, "RPG"),
            (0x0a, "Standard"),
        ],
        _ => return None,
    })
}

/// A monitor feature selected through F8 and written through F7.
///
/// These are not independent VCP codes: the monitor exposes one value register
/// and the F8 index chooses which setting it belongs to. Lenovo's service
/// always writes F8 first and F7 second.
#[derive(Clone, Copy)]
pub struct FeatureDef {
    pub index: u16,
    pub name: Key,
    pub hint: Key,
    pub section: Section,
    /// Value names from `VCPCodeDef.json`; empty when the value is a number.
    pub options: &'static [(u16, &'static str)],
}

pub const FEATURES: &[FeatureDef] = &[
    FeatureDef {
        index: 0x0b,
        name: Key::FnDpSelect,
        hint: Key::HintDpSelect,
        section: Section::Inputs,
        options: &[(0, "DP 1.2"), (1, "DP 1.4"), (2, "DP 1.1")],
    },
    FeatureDef {
        index: 0x0d,
        name: Key::FnUsbSpeed,
        hint: Key::HintUsbSpeed,
        section: Section::Inputs,
        options: &[(0, "USB 2.0"), (1, "USB 3.0"), (2, "USB 3.2")],
    },
    FeatureDef {
        index: 0x07,
        name: Key::FnKvm,
        hint: Key::HintKvm,
        section: Section::Inputs,
        options: &[(0, "Off"), (1, "On")],
    },
    FeatureDef {
        index: 0x1a,
        name: Key::FnOverclock,
        hint: Key::HintOverclock,
        section: Section::Image,
        options: &[(0, "Off"), (1, "On")],
    },
    FeatureDef {
        index: TRUE_SPLIT_FEATURE as u16,
        name: Key::FnTrueSplitState,
        hint: Key::TrueSplitHint,
        section: Section::MultiView,
        options: TRUE_SPLIT_MODES,
    },
];

/// True Split modes, in the order Lenovo's service accepts them.
pub const TRUE_SPLIT_MODES: &[(u16, &str)] =
    &[(0, "Off"), (1, "1:1"), (3, "3:1"), (2, "1:3"), (4, "1:1:1")];

pub fn features_in(section: Section) -> Vec<&'static FeatureDef> {
    FEATURES.iter().filter(|f| f.section == section).collect()
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

/// Multi-view mode derived from F2.
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

/// Decodes the window state from `F4`.
///
/// Lenovo's `GetPIPStatus` reads `0xF4` and maps `0x0100`/`0x0101` to PIP and
/// `0x0200`/`0x0201` to PBP; every other value means one picture. `0xF2` is
/// *not* the mode: it is the swap command, and it stays `1` in single, PIP and
/// PBP on the R45w-30. `0xF5` is the layout of a split and is used only as a
/// fallback when `0xF4` is missing.
pub fn multi_mode(f4: Option<u16>) -> MultiMode {
    match f4 {
        Some(0x0100 | 0x0101) => MultiMode::PictureInPicture,
        Some(0x0200 | 0x0201) => MultiMode::PictureByPicture,
        Some(_) => MultiMode::Off,
        None => MultiMode::Unknown,
    }
}

/// Mode detection that also accepts PBP when `F4` is unavailable.
///
/// On the R45w-30 PBP shrinks the DisplayPort EDID to one half, so the desktop
/// reports a half-width output. Only an active `F5` layout combined with that
/// half-width output is treated as PBP; a stale `F5` alone must not invent a
/// split.
pub fn multi_mode_detected(f4: Option<u16>, f5: Option<u16>, half_width_output: bool) -> MultiMode {
    let mode = multi_mode(f4);
    if mode == MultiMode::Off && half_width_output && f5.and_then(split_layout_for).is_some() {
        MultiMode::PictureByPicture
    } else {
        mode
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
    matches!(b, 0x01 | 0x03 | 0x0f | 0x10 | 0x11..=0x1f | 0x31..=0x3f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_mode_reads_the_status_register_f4() {
        assert_eq!(multi_mode(None), MultiMode::Unknown);
        assert_eq!(multi_mode(Some(0)), MultiMode::Off);
        assert_eq!(multi_mode(Some(0x0100)), MultiMode::PictureInPicture);
        assert_eq!(multi_mode(Some(0x0101)), MultiMode::PictureInPicture);
        assert_eq!(multi_mode(Some(0x0200)), MultiMode::PictureByPicture);
        assert_eq!(multi_mode(Some(0x0201)), MultiMode::PictureByPicture);
        // F2=1 (the swap command) must never be mistaken for a mode.
        assert_eq!(multi_mode(Some(1)), MultiMode::Off);
    }

    #[test]
    fn pbp_is_detected_without_f4_when_the_output_is_half_width() {
        assert_eq!(
            multi_mode_detected(Some(0), Some(0x0101), true),
            MultiMode::PictureByPicture
        );
        // Stale F5 alone, or a full-width output, stays single.
        assert_eq!(
            multi_mode_detected(Some(0), Some(0x0101), false),
            MultiMode::Off
        );
        assert_eq!(multi_mode_detected(Some(0), Some(0), true), MultiMode::Off);
        assert_eq!(
            multi_mode_detected(Some(0x0200), Some(0), false),
            MultiMode::PictureByPicture
        );
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
    fn split_layout_decodes_only_the_supported_codes() {
        assert_eq!(split_layout_for(0x0101), Some(SplitLayout::Half));
        assert_eq!(split_layout_for(0x0401), Some(SplitLayout::Wide));
        assert_eq!(SplitLayout::Half.share(), 0.5);
        assert_eq!(SplitLayout::Wide.share(), 0.70);
        for code in [0, 0x0102, 0x0201, 0x0202, 0x0301, 0x0302, 0x0402, 0x00ff] {
            assert_eq!(split_layout_for(code), None, "{code:04X}");
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

    #[test]
    fn vendor_choice_tables_match_vcp_code_def() {
        let overdrive = static_choices(0xe0).unwrap();
        assert_eq!(overdrive.len(), 8);
        assert_eq!(overdrive[0], (0, "Off"));
        assert_eq!(overdrive[7], (7, "Level 5"));
        let hdr = static_choices(0xef).unwrap();
        assert!(hdr.contains(&(4, "Dolby Vision")));
        assert!(hdr.contains(&(0x10, "TB400")));
        let scaling = static_choices(0x86).unwrap();
        assert!(scaling.contains(&(0x0b, "4:3")));
        assert!(scaling.contains(&(0x0f, "16:9")));
        let game = static_choices(0xf9).unwrap();
        assert_eq!(game.first(), Some(&(0, "Off")));
        assert_eq!(game.last(), Some(&(0x0a, "Standard")));
        assert_eq!(static_choices(0x10), None);
    }

    #[test]
    fn true_split_modes_follow_the_vendor_value_order() {
        let values: Vec<u16> = TRUE_SPLIT_MODES.iter().map(|(v, _)| *v).collect();
        assert_eq!(values, [0, 1, 3, 2, 4]);
        assert_eq!(TRUE_SPLIT_MODES[0].1, "Off");
        assert_eq!(TRUE_SPLIT_MODES[4].1, "1:1:1");
    }

    #[test]
    fn every_feature_has_options_and_a_pages() {
        for feature in FEATURES {
            assert!(!feature.options.is_empty(), "{}", feature.index);
            assert!(matches!(
                feature.section,
                Section::Image | Section::Inputs | Section::MultiView
            ));
            assert!(features_in(feature.section)
                .iter()
                .any(|f| f.index == feature.index));
        }
        assert!(features_in(Section::Image).iter().any(|f| f.index == 0x1a));
        assert!(features_in(Section::Inputs).iter().any(|f| f.index == 0x0b));
        assert!(features_in(Section::MultiView)
            .iter()
            .any(|f| f.index == 0x15));
    }
}
