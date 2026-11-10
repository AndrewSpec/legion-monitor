//! User interface translations.
//!
//! Every language is a list of `(key, text)` pairs. A missing key falls back
//! to English, so adding a new string does not require touching every file
//! at once.
//!
//! Adding a language: a new `xx.rs` file with `pub const ENTRIES`, an entry in
//! `LANGS` and a variant in `Lang`. The rest (UI picker, saved setting) works
//! on its own.

mod cs;
mod en;
mod es;
mod fr;
mod it;
mod nl;
mod pl;
mod pt;

/// Languages available in the interface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pl,
    En,
    Es,
    Fr,
    It,
    Pt,
    Nl,
    Cs,
}

/// Language list shown in the picker — each name is written in its own language.
pub const LANGS: &[(Lang, &str)] = &[
    (Lang::Pl, "Polski"),
    (Lang::En, "English"),
    (Lang::Es, "Español"),
    (Lang::Fr, "Français"),
    (Lang::It, "Italiano"),
    (Lang::Pt, "Português"),
    (Lang::Nl, "Nederlands"),
    (Lang::Cs, "Čeština"),
];

impl Lang {
    /// Code used in the settings file.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Pt => "pt",
            Lang::Nl => "nl",
            Lang::Cs => "cs",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let code = code.trim().to_ascii_lowercase();
        let short = code.split(['_', '-', '.']).next().unwrap_or("");
        LANGS
            .iter()
            .map(|(l, _)| *l)
            .find(|l| l.code() == short || l.code() == code)
    }

    /// System language from `LC_ALL` / `LC_MESSAGES` / `LANG`.
    pub fn system() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(var) {
                if let Some(lang) = Lang::from_code(&value) {
                    return lang;
                }
            }
        }
        Lang::En
    }
}

/// Interface strings — all in one place, so translating is mechanical work
/// instead of searching through the code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// `InfoAppKey` ends with the enum name, but it mirrors the DDC/CI name of 0xC6.
#[allow(clippy::enum_variant_names)]
pub enum Key {
    // ── shared chrome ────────────────────────────────────────────────
    AppOffline,
    AppScanning,
    StatusConnecting,
    StatusConnected,
    StatusOffline,
    Refresh,
    ConfigWord,
    SlotEmpty,
    SlotSaved,
    Active,
    MonitorFallbackFmt,
    FooterControl,
    FooterShortcuts,
    Language,

    // ── navigation ───────────────────────────────────────────────────────
    NavSettings,
    NavOverview,
    NavImage,
    NavColor,
    NavInputs,
    NavMultiView,
    NavProfiles,
    NavAdvanced,

    // ── monitor-not-connected banner ──────────────────────────────────────
    BannerTitle,
    BannerBody,
    BannerRetry,
    Details,

    // ── errors ───────────────────────────────────────────────────────────
    ErrNoDisplay,
    ErrLaptop,
    ErrPermission,
    ErrGeneric,
    ErrNotConnected,
    ErrCreateDir,
    ErrSaveProfiles,
    ErrSaveSettings,

    // ── overview ────────────────────────────────────────────────────────
    OvTitle,
    OvDesc,
    OvMode,
    OvSource,
    OvBrightness,
    OvContrast,
    CardBrightness,
    CardConfigs,
    CardConfigsDesc,
    CardMode,
    CardWindows,
    SaveAs,
    NoSplit,
    ModeHint,

    // ── picture ───────────────────────────────────────────────────────────
    ImgTitle,
    ImgDesc,
    CardImageParams,

    // ── color ───────────────────────────────────────────────────────────
    ColTitle,
    ColDesc,
    CardColorTemp,
    CardColorPreset,
    CardRgb,

    // ── inputs ──────────────────────────────────────────────────────────
    InTitle,
    InDesc,
    CardInput,
    InSwitchHint,

    // ── multi-view ──────────────────────────────────────────────────────
    MvTitle,
    MvDesc,
    MvMode,
    CardLayouts,
    LayoutHint,
    CardWindowsShort,
    WindowsNote,
    SourcesPreview,
    SourceFmt,
    InputUnknownFmt,

    // ── profiles ─────────────────────────────────────────────────────────
    PrTitle,
    PrDesc,
    ConfigHeading,
    SlotEmptyTitle,
    SlotEmptyDesc,
    SlotSaveHint,
    SaveCurrent,
    SavedCount,
    NamedProfiles,
    NamePlaceholder,
    SaveAsProfile,
    ProfilesFileHint,
    NoProfiles,
    Apply,
    Overwrite,
    Delete,
    SaveSlotTip,
    Cancel,

    // ── advanced ────────────────────────────────────────────────────
    AdvTitle,
    AdvDesc,
    CardMonitor,
    VolLabel,
    MuteLabel,
    PowerLabel,
    OsdLangLabel,
    CardVendor,
    ShowVendor,
    VendorWarn,
    NoVendor,
    CardReset,
    ResetDesc,
    ResetBtn,
    ResetConfirm,
    ResetYes,
    CardLog,
    LogEmpty,

    // ── monitor feature names ──────────────────────────────────────────
    FnBrightness,
    FnContrast,
    FnSharpness,
    FnSaturation,
    FnHue,
    FnGamma,
    FnBlackLevel,
    FnColorTemp,
    FnColorPreset,
    FnRed,
    FnGreen,
    FnBlue,
    FnInput,
    FnKvm,
    FnDaisyChain,
    FnVolume,
    FnMute,
    FnPower,
    FnOsdLang,

    // ── hints under labels ──────────────────────────────────────
    HintBrightness,
    HintContrast,
    HintSharpness,
    HintSaturation,
    HintHue,
    HintGamma,
    HintBlack,
    HintColorTemp,
    HintColorPreset,
    HintRed,
    HintGreen,
    HintBlue,
    HintInput,
    HintVolume,
    HintMute,
    HintPower,
    HintOsdLang,
    HintAnalogStab,

    // ── window layouts ────────────────────────────────────────────────────
    L101,
    L102,
    L201,
    L202,
    L301,
    L302,
    L401,
    L402,

    // ── values and modes ────────────────────────────────────────────────
    PresetNative,
    PresetCustom,
    PowerOn,
    PowerStandby,
    PowerSuspend,
    MultiSingle,
    MultiPip,
    MultiPbp,
    MultiUnknown,
    InputUnknown,

    // ── notifications ──────────────────────────────────────────────────────
    ToastApplied,
    ToastAppliedErrors,
    ToastProfileDeleted,
    ToastSlotDeleted,
    ToastSaved,
    ToastProfileEmpty,

    // ── split windows and modes ───────────────────────────────────────────
    WindowLeft,
    WindowRight,
    ModeSingle,
    ModePip,
    ModePbp,
    // ── monitor feature labels and hints (catalog) ───────────────
    FnClockPhase,
    FnRedOffset,
    FnGreenOffset,
    FnBlueOffset,
    FnWindowSources,
    FnMultiMode,
    FnWindowLayout,
    FnWindowGeometry,
    FnFactoryReset,
    FnUnknown,
    HintOffset,
    PresetSrgb,
    Preset4000,
    Preset5000,
    Preset6500,
    Preset7500,
    Preset8200,
    Preset9300,
    Preset10000,
    TempWarm,
    TempNeutral,
    TempCool,
    InfoTitle,
    InfoNewValue,
    InfoColorTempStep,
    InfoActiveControl,
    InfoScaling,
    InfoWinPos,
    InfoHFreq,
    InfoVFreq,
    InfoSubpixel,
    InfoPanelType,
    InfoAppKey,
    InfoController,
    InfoFirmware,
    InfoOsdControl,
    InfoVcpVersion,

    // ── write confirmation and read limitations ───────────────────────
    WriteConfirmed,
    WriteMismatch,
    WriteUnconfirmed,
    WriteFailed,
    ExperimentalMulti,
    PartialRead,
    ProfileSkipped,
    WindowsUnknown,
    ProfileBusy,

    // ── worker log ──────────────────────────────────────────────────────
    LogDetected,
    LogFeatures,
    LogCapsFailed,
    LogWrite,

    // ── --dump report ───────────────────────────────────────────────────
    DumpMonitor,
    DumpBus,
    DumpSerial,
    DumpFeatures,
    DumpRange,
    DumpMode,
    DumpSource,
    DumpWindows,
    DumpCapsFailed,
    DumpNoMonitor,
    DumpConnect,
    DumpDetails,
}

/// String table for a single language.
pub struct Strings {
    entries: &'static [(Key, &'static str)],
}

impl Strings {
    /// Text for a key; a missing entry falls back to English, and when that is
    /// missing too, a placeholder is shown (visible while adding new text).
    pub fn t(&self, key: Key) -> &'static str {
        if let Some((_, text)) = self.entries.iter().find(|(k, _)| *k == key) {
            return text;
        }
        if let Some((_, text)) = en::ENTRIES.iter().find(|(k, _)| *k == key) {
            return text;
        }
        "<missing translation>"
    }

    /// Text for a key with every `{}` placeholder filled in order.
    pub fn fill(&self, key: Key, args: &[String]) -> String {
        let mut out = self.t(key).to_string();
        for arg in args {
            out = out.replacen("{}", arg, 1);
        }
        out
    }
}

/// String table for the selected language.
pub fn strings(lang: Lang) -> Strings {
    let entries = match lang {
        Lang::Pl => pl::ENTRIES,
        Lang::En => en::ENTRIES,
        Lang::Es => es::ENTRIES,
        Lang::Fr => fr::ENTRIES,
        Lang::It => it::ENTRIES,
        Lang::Pt => pt::ENTRIES,
        Lang::Nl => nl::ENTRIES,
        Lang::Cs => cs::ENTRIES,
    };
    Strings { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: &[(Lang, &[(Key, &str)])] = &[
        (Lang::Pl, pl::ENTRIES),
        (Lang::En, en::ENTRIES),
        (Lang::Es, es::ENTRIES),
        (Lang::Fr, fr::ENTRIES),
        (Lang::It, it::ENTRIES),
        (Lang::Pt, pt::ENTRIES),
        (Lang::Nl, nl::ENTRIES),
        (Lang::Cs, cs::ENTRIES),
    ];

    /// Every language must cover every key, with the same placeholder count
    /// as English — otherwise `fill` would silently drop a value.
    #[test]
    fn every_language_has_the_same_keys() {
        for (lang, table) in TABLES {
            for (key, english) in en::ENTRIES {
                let Some((_, text)) = table.iter().find(|(k, _)| k == key) else {
                    panic!("{} is missing {key:?}", lang.code());
                };
                assert_eq!(
                    english.matches("{}").count(),
                    text.matches("{}").count(),
                    "{}: placeholder count differs for {key:?}",
                    lang.code()
                );
            }
            for (key, _) in *table {
                assert!(
                    en::ENTRIES.iter().any(|(k, _)| k == key),
                    "{}: key {key:?} does not exist in English",
                    lang.code()
                );
            }
        }
    }

    /// A duplicated key would shadow the earlier entry and hide a typo.
    #[test]
    fn tables_have_no_duplicate_keys() {
        for (lang, table) in TABLES {
            for (i, (key, _)) in table.iter().enumerate() {
                assert!(
                    !table[..i].iter().any(|(k, _)| k == key),
                    "{}: duplicate key {key:?}",
                    lang.code()
                );
            }
        }
    }
}
