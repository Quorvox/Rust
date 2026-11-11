use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

// год максимум, дальше уже бред
pub const MAX_HOURS: f64 = 24.0 * 365.0;
// лимит дискорда на текстовые поля
const MAX_TEXT: usize = 128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Profile {
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

pub type Profiles = BTreeMap<String, Profile>;

pub fn load(path: &Path) -> Result<Profiles> {
    let raw = fs::read_to_string(path).with_context(|| {
        format!("нет файла {} (создай его командой `quor-presence init`)", path.display())
    })?;
    toml::from_str(&raw).with_context(|| format!("{} не разобрался", path.display()))
}

pub fn save(path: &Path, profiles: &Profiles) -> Result<()> {
    let raw = toml::to_string_pretty(profiles).context("не смог собрать toml")?;
    fs::write(path, raw).with_context(|| format!("не смог записать {}", path.display()))
}

// старт = сейчас минус N часов. дискорд ждёт миллисекунды
pub fn start_ms(now_ms: i64, hours: f64) -> Result<i64> {
    if !hours.is_finite() || hours < 0.0 {
        bail!("hours должно быть числом от 0");
    }
    if hours > MAX_HOURS {
        bail!("hours слишком много (максимум {MAX_HOURS})");
    }
    Ok(now_ms - (hours * 3_600_000.0) as i64)
}

impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.app_id.is_empty() || !self.app_id.chars().all(|c| c.is_ascii_digit()) {
            bail!("app_id должен быть числом (Application ID с discord.com/developers)");
        }
        if self.app_id.chars().all(|c| c == '0') {
            bail!("в app_id остался плейсхолдер из примера, впиши свой Application ID");
        }
        let fields = [
            ("name", &self.name),
            ("details", &self.details),
            ("state", &self.state),
            ("large_text", &self.large_text),
            ("small_text", &self.small_text),
        ];
        for (n, v) in fields {
            if let Some(s) = v {
                let len = s.chars().count();
                if len > MAX_TEXT {
                    bail!("{n} длиннее {MAX_TEXT} символов");
                }
                if len < 2 && !s.is_empty() {
                    bail!("{n} короче 2 символов, дискорд такое не принимает");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_profile() -> Profile {
        Profile { app_id: "123456789012345678".into(), ..Default::default() }
    }

    #[test]
    fn start_is_now_minus_hours() {
        assert_eq!(start_ms(10_000_000, 1.0).unwrap(), 10_000_000 - 3_600_000);
        assert_eq!(start_ms(10_000_000, 0.0).unwrap(), 10_000_000);
        assert_eq!(start_ms(100_000_000, 0.5).unwrap(), 100_000_000 - 1_800_000);
    }

    #[test]
    fn bad_hours_rejected() {
        assert!(start_ms(0, -1.0).is_err());
        assert!(start_ms(0, f64::NAN).is_err());
        assert!(start_ms(0, f64::INFINITY).is_err());
        assert!(start_ms(0, 100_000.0).is_err());
    }

    #[test]
    fn placeholder_app_id_rejected() {
        let p = Profile { app_id: "000000000000000000".into(), ..Default::default() };
        assert!(p.validate().is_err());
    }

    #[test]
    fn non_numeric_app_id_rejected() {
        let p = Profile { app_id: "abc".into(), ..Default::default() };
        assert!(p.validate().is_err());
        let p = Profile { app_id: "".into(), ..Default::default() };
        assert!(p.validate().is_err());
    }

    #[test]
    fn text_limits() {
        let mut p = ok_profile();
        p.details = Some("x".repeat(129));
        assert!(p.validate().is_err());
        p.details = Some("x".into());
        assert!(p.validate().is_err());
        p.details = Some("ok text".into());
        assert!(p.validate().is_ok());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.toml");
        let mut all = Profiles::new();
        all.insert(
            "a".into(),
            Profile {
                app_id: "123456789012345678".into(),
                details: Some("hello".into()),
                hours: Some(2.5),
                ..Default::default()
            },
        );
        save(&path, &all).unwrap();
        assert_eq!(load(&path).unwrap(), all);
    }

    #[test]
    fn example_file_parses() {
        let raw = include_str!("../profiles.example.toml");
        let p: Profiles = toml::from_str(raw).unwrap();
        assert_eq!(p["vscode"].hours, Some(20.0));
        assert_eq!(p["vscode"].name.as_deref(), Some("Visual Studio Code"));
        assert!(p.contains_key("minecraft"));
    }
}
