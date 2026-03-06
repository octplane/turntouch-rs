use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::protocol::{ButtonEvent, Direction};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub north: DirectionConfig,
    #[serde(default)]
    pub east: DirectionConfig,
    #[serde(default)]
    pub west: DirectionConfig,
    #[serde(default)]
    pub south: DirectionConfig,
    #[serde(default)]
    pub multi: MultiConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct MultiConfig {
    /// Command for any multi-button press (all combos)
    pub press: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DirectionConfig {
    pub press: Option<String>,
    pub double: Option<String>,
    pub hold: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => {
                    tracing::info!("Loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!("Failed to parse config: {e}");
                    Self::default()
                }
            },
            Err(_) => {
                tracing::info!(
                    "No config found at {}, using defaults (print-only mode)",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("turntouch")
            .join("config.toml")
    }

    fn direction_config(&self, dir: Direction) -> &DirectionConfig {
        match dir {
            Direction::North => &self.north,
            Direction::East => &self.east,
            Direction::West => &self.west,
            Direction::South => &self.south,
        }
    }

    pub fn command_for(&self, event: &ButtonEvent) -> Option<&str> {
        match event {
            ButtonEvent::Multi(_) => self.multi.press.as_deref(),
            ButtonEvent::Press(d) => self.direction_config(*d).press.as_deref(),
            ButtonEvent::DoubleTap(d) => self.direction_config(*d).double.as_deref(),
            ButtonEvent::Hold(d) => self.direction_config(*d).hold.as_deref(),
        }
    }

    pub fn to_env_map(event: &ButtonEvent) -> HashMap<String, String> {
        let mut env = HashMap::new();
        match event {
            ButtonEvent::Multi(dirs) => {
                let names: Vec<_> = dirs.iter().map(|d| d.to_string()).collect();
                env.insert("TT_DIRECTION".into(), names.join("+"));
                env.insert("TT_EVENT".into(), "multi".into());
            }
            ButtonEvent::Press(d) => {
                env.insert("TT_DIRECTION".into(), d.to_string());
                env.insert("TT_EVENT".into(), "press".into());
            }
            ButtonEvent::DoubleTap(d) => {
                env.insert("TT_DIRECTION".into(), d.to_string());
                env.insert("TT_EVENT".into(), "double".into());
            }
            ButtonEvent::Hold(d) => {
                env.insert("TT_DIRECTION".into(), d.to_string());
                env.insert("TT_EVENT".into(), "hold".into());
            }
        }
        env
    }
}
