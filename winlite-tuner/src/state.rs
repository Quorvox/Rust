use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// что было в реестре до нас. Missing = значения не было вообще
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Prev {
    Value(u32),
    Missing,
}

impl Prev {
    pub fn from_opt(v: Option<u32>) -> Self {
        v.map_or(Prev::Missing, Prev::Value)
    }
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct State {
    pub diagtrack_start: Option<Prev>,
    pub allow_telemetry: Option<Prev>,
}

impl State {
    pub fn load(dir: &Path) -> Result<Self> {
        let p = dir.join("state.json");
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&p).with_context(|| format!("читаю {}", p.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("{} битый", p.display()))
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let p = dir.join("state.json");
        fs::write(&p, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("пишу {}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let s = State {
            diagtrack_start: Some(Prev::Value(2)),
            allow_telemetry: Some(Prev::Missing),
        };
        s.save(dir.path()).unwrap();
        assert_eq!(State::load(dir.path()).unwrap(), s);
    }

    #[test]
    fn missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(dir.path()).unwrap(), State::default());
    }
}
