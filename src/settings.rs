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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Language code, e.g. `"pl"`. `None` = take it from the system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
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
        let file = path();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&file, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `save_lang` must write the language code and `load` + `lang()` must read
    /// it back — otherwise the UI choice would not survive a restart.
    #[test]
    fn language_roundtrip() {
        let dir = std::env::temp_dir().join(format!("legion-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // `config_dir()` reads XDG_CONFIG_HOME, so we point it at a temporary directory.
        // SAFETY: one-shot test running in its own process.
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
}
