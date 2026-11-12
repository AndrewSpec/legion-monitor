//! Settings profiles — stored in ~/.config/legion-monitor/profiles.json

use crate::i18n::{Key, Strings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// VCP code (hex, e.g. "F5") -> value
    pub values: BTreeMap<String, u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Slots {
    pub a: Option<Profile>,
    pub b: Option<Profile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    pub version: u32,
    pub slots: Slots,
    pub profiles: Vec<Profile>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: 1,
            slots: Slots::default(),
            profiles: Vec::new(),
        }
    }
}

/// Application settings directory (`~/.config/legion-monitor`).
pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("legion-monitor")
}

pub fn config_path() -> PathBuf {
    config_dir().join("profiles.json")
}

impl Store {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self, t: &Strings) -> Result<(), String> {
        let path = config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| {
                t.fill(
                    Key::ErrCreateDir,
                    &[dir.display().to_string(), e.to_string()],
                )
            })?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| t.fill(Key::ErrSaveProfiles, &[e.to_string()]))
    }
}

pub fn key(code: u8) -> String {
    format!("{code:02X}")
}

pub fn parse_key(k: &str) -> Option<u8> {
    u8::from_str_radix(k.trim_start_matches("0x"), 16).ok()
}

/// Registers safe for automatic replay. Reading a register does not mean it is
/// a setting; status values, reset, power and unknown vendor codes never end up
/// in a profile or in the replay of an old JSON file.
pub fn replay_safe(code: u8) -> bool {
    matches!(
        code,
        0x0c | 0x10
            | 0x12
            | 0x14
            | 0x16
            | 0x18
            | 0x1a
            | 0x60
            | 0x62
            | 0x87
            | 0x8a
            | 0x8b
            | 0x8d
            | 0xcc
            | 0xf2
            | 0xf5
    )
}

/// Single owner of filtering and ordering, also for existing profiles.
/// The source switch comes last because it can cut off the control channel.
pub fn replay_plan(values: &BTreeMap<String, u16>) -> Vec<(u8, u16)> {
    let mut list: Vec<_> = values
        .iter()
        .filter_map(|(k, v)| {
            let code = parse_key(k)?;
            if !replay_safe(code)
                || (code == 0xf2 && *v > 1)
                || (code == 0xf5 && *v != 0 && crate::catalog::split_layout_for(*v).is_none())
            {
                return None;
            }
            Some((code, *v))
        })
        .collect();
    list.sort_by_key(|(c, _)| match c {
        // Temperature/RGB writes can change the preset as a side effect.
        // Restore the requested preset last among picture controls; readback
        // must still reject tuples that the monitor cannot represent together.
        0x14 => 0x100,
        0xf5 => 0x101,
        0xf2 => 0x102,
        0x60 => 0x103,
        _ => *c as u16,
    });
    list
}

/// A slot is active only when its non-empty replay plan matches fresh readback.
/// Missing registers must not turn a partially read profile into a match.
pub fn matches_observed(
    values: &BTreeMap<String, u16>,
    observed: &BTreeMap<u8, crate::ddc::VcpValue>,
) -> bool {
    let plan = replay_plan(values);
    !plan.is_empty()
        && plan
            .iter()
            .all(|(code, value)| observed.get(code).is_some_and(|v| v.value == *value))
}

/// Next profile id.
pub fn next_id(store: &Store) -> String {
    let mut n = 1;
    loop {
        let id = format!("p{n}");
        if !store.profiles.iter().any(|p| p.id == id)
            && store.slots.a.as_ref().map(|p| p.id != id).unwrap_or(true)
            && store.slots.b.as_ref().map(|p| p.id != id).unwrap_or(true)
        {
            return id;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_profile_cannot_replay_status_reset_power_or_vendor() {
        let values = [
            ("04", 1),
            ("F0", 49),
            ("F4", 513),
            ("FD", 4419),
            ("D6", 5),
            ("10", 80),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        assert_eq!(replay_plan(&values), vec![(0x10, 80)]);
    }

    #[test]
    fn replay_orders_transitions_after_picture_and_source_last() {
        let values = [("60", 49), ("F2", 1), ("F5", 257), ("12", 87)]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        assert_eq!(
            replay_plan(&values),
            vec![(0x12, 87), (0xf5, 257), (0xf2, 1), (0x60, 49)]
        );
    }

    #[test]
    fn replay_restores_color_preset_after_temperature_and_rgb() {
        let values = [
            ("14", 8),
            ("0C", 63),
            ("16", 100),
            ("18", 90),
            ("1A", 87),
            ("F5", 257),
            ("F2", 1),
            ("60", 49),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        assert_eq!(
            replay_plan(&values),
            vec![
                (0x0c, 63),
                (0x16, 100),
                (0x18, 90),
                (0x1a, 87),
                (0x14, 8),
                (0xf5, 257),
                (0xf2, 1),
                (0x60, 49)
            ]
        );
    }

    #[test]
    fn active_profile_requires_complete_matching_readback() {
        let values = [("10".into(), 95), ("12".into(), 84)].into_iter().collect();
        let mut observed = BTreeMap::new();
        assert!(!matches_observed(&values, &observed));
        observed.insert(
            0x10,
            crate::ddc::VcpValue {
                value: 95,
                max: 100,
            },
        );
        assert!(!matches_observed(&values, &observed));
        observed.insert(
            0x12,
            crate::ddc::VcpValue {
                value: 87,
                max: 100,
            },
        );
        assert!(!matches_observed(&values, &observed));
        observed.get_mut(&0x12).unwrap().value = 84;
        assert!(matches_observed(&values, &observed));
        assert!(!matches_observed(&BTreeMap::new(), &observed));
        let status_only = [("F4".into(), 513)].into_iter().collect();
        assert!(!matches_observed(&status_only, &observed));
    }

    #[test]
    fn invalid_multi_values_are_not_replayed() {
        let values = [("F2", 42), ("F5", 65535)]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        assert!(replay_plan(&values).is_empty());
    }
}
