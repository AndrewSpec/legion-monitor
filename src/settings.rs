//! Application settings (language choice) — `~/.config/legion-monitor/settings.json`.
//!
//! No entry = language taken from the system (`LC_ALL` / `LC_MESSAGES` / `LANG`),
//! so the first launch already speaks the user's language.

use crate::i18n::Lang;
use crate::profiles::config_dir;
use serde::{Deserialize, Serialize};

fn path() -> std::path::PathBuf {
    config_dir().join("settings.json")
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The picture state the rescue mode returns to.
///
/// Only single-picture states are recorded: a half-written split is exactly
/// what the rescue tab is meant to undo. `source` is the active input from
/// VCP 0x60, `f2`/`f5` keep the mode around for the summary, and brightness
/// and contrast are restored when the monitor exposed them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RescueBase {
    pub source: u16,
    #[serde(default)]
    pub f2: u16,
    #[serde(default)]
    pub f5: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<u16>,
    /// Unix seconds; the interface only says when the state was captured.
    #[serde(default)]
    pub saved_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Language code, e.g. `"pl"`. `None` = take it from the system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Last known-good single picture, captured automatically on first run
    /// and refreshable from the rescue page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescue: Option<RescueBase>,
    /// Persisted arm switch: after a reconnect the app restores the rescued
    /// state without any interaction.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rescue_armed: bool,
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Interface language: chosen manually, otherwise taken from the system.
    pub fn lang(&self) -> Lang {
        self.language
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or_else(Lang::system)
    }

    /// Saves the language choice; returns an error message on failure.
    pub fn save_lang(&mut self, lang: Lang) -> Result<(), String> {
        self.language = Some(lang.code().to_string());
        self.save()
    }

    /// Writes the whole settings file; returns an error message on failure.
    pub fn save(&self) -> Result<(), String> {
        let file = path();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&file, text).map_err(|e| e.to_string())
    }
}

/// Seconds since the Unix epoch, used for the "saved at" note.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    /// The two tests below point `config_dir()` at temporary directories.
    static ENV: Mutex<()> = Mutex::new(());

    /// `save_lang` must write the language code and `load` + `lang()` must read
    /// it back — otherwise the UI choice would not survive a restart.
    #[test]
    fn language_roundtrip() {
        let _guard = ENV.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("legion-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // `config_dir()` reads XDG_CONFIG_HOME, so we point it at a temporary directory.
        // SAFETY: guarded by ENV, one test per process touching the variable.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };

        let mut s = Settings::default();
        assert_eq!(s.language, None, "system language by default");
        s.save_lang(Lang::Cs).expect("save settings");
        assert!(path().exists(), "settings.json should be created");

        let back = Settings::load();
        assert_eq!(back.language.as_deref(), Some("cs"));
        assert_eq!(back.lang(), Lang::Cs);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The rescue baseline must survive a restart together with the arm flag.
    #[test]
    fn rescue_roundtrip() {
        let _guard = ENV.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("legion-rescue-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: guarded by ENV; see the note above.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };

        let s = Settings {
            rescue: Some(RescueBase {
                source: 0x31,
                f2: 0x0100,
                f5: 0x0402,
                brightness: Some(80),
                contrast: Some(60),
                saved_at: 42,
            }),
            rescue_armed: true,
            ..Settings::default()
        };
        s.save().expect("save settings");

        let back = Settings::load();
        let base = back.rescue.expect("baseline should survive");
        assert_eq!(base.source, 0x31);
        assert_eq!(base.brightness, Some(80));
        assert!(back.rescue_armed);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
