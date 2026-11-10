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
    Advanced,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Overview,
        Section::Image,
        Section::Color,
        Section::Inputs,
        Section::MultiView,
        Section::Profiles,
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

/// Experimental candidates for F5 values. `panes` are UI illustrations,
/// not a reconstructed or confirmed physical monitor geometry.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub value: u16,
    pub panes: &'static [(f32, f32, f32, f32)],
}

const fn p(x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    (x, y, w, h)
}

pub const LAYOUTS: &[Layout] = &[
    Layout {
        value: 0x0101,
        panes: &[p(0.0, 0.0, 0.5, 1.0), p(0.5, 0.0, 0.5, 1.0)],
    },
    Layout {
        value: 0x0102,
        panes: &[p(0.0, 0.0, 0.45, 1.0), p(0.45, 0.0, 0.55, 1.0)],
    },
    Layout {
        value: 0x0201,
        panes: &[p(0.0, 0.0, 1.0, 0.5), p(0.0, 0.5, 1.0, 0.5)],
    },
    Layout {
        value: 0x0202,
        panes: &[p(0.0, 0.0, 1.0, 0.45), p(0.0, 0.45, 1.0, 0.55)],
    },
    Layout {
        value: 0x0301,
        panes: &[
            p(0.0, 0.0, 0.5, 1.0),
            p(0.5, 0.0, 0.5, 0.5),
            p(0.5, 0.5, 0.5, 0.5),
        ],
    },
    Layout {
        value: 0x0302,
        panes: &[
            p(0.5, 0.0, 0.5, 1.0),
            p(0.0, 0.0, 0.5, 0.5),
            p(0.0, 0.5, 0.5, 0.5),
        ],
    },
    Layout {
        value: 0x0401,
        panes: &[
            p(0.0, 0.0, 0.5, 0.5),
            p(0.5, 0.0, 0.5, 0.5),
            p(0.0, 0.5, 0.5, 0.5),
            p(0.5, 0.5, 0.5, 0.5),
        ],
    },
    Layout {
        value: 0x0402,
        panes: &[
            p(0.0, 0.0, 0.6, 1.0),
            p(0.6, 0.0, 0.4, 0.333),
            p(0.6, 0.333, 0.4, 0.333),
            p(0.6, 0.666, 0.4, 0.334),
        ],
    },
];

pub fn layout_for(value: u16) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|l| l.value == value)
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
pub fn multi_mode(f2: Option<u16>, f4: Option<u16>) -> MultiMode {
    match f2 {
        Some(0) => return MultiMode::Off,
        None => return MultiMode::Unknown,
        Some(_) => {}
    }
    match f4.map(|v| v >> 8) {
        Some(0x01) => MultiMode::PictureInPicture,
        Some(0x02) => MultiMode::PictureByPicture,
        _ => MultiMode::Unknown,
    }
}

/// Sources visible in the windows — from VCP F0 (two bytes: left, right).
///
/// The monitor returns this register inconsistently (observed 0x0031, 0x0202,
/// 0x0001), so only values that make sense as picture sources are accepted —
/// otherwise we show "unknown" instead of an invented input.
pub fn window_sources(f0: Option<u16>) -> Option<(u16, u16)> {
    let v = f0?;
    let left = (v >> 8) & 0xff;
    let right = v & 0xff;
    if !known_source(left as u8) || !known_source(right as u8) {
        return None;
    }
    if left == 0 && right == 0 {
        return None;
    }
    Some((left, right))
}

/// Whether an F0 byte looks like a picture-source code (0 = no window, 0xff = unknown).
fn known_source(b: u8) -> bool {
    matches!(
        b,
        0x00 | 0x01 | 0x03 | 0x0f | 0x10 | 0x11 | 0x12 | 0x13 | 0x1f | 0x31 | 0xff
    )
}
