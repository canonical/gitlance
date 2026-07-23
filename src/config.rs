// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

//! Repo-local configuration for `gitlance suggest` (`gitlance.toml`).

use serde::Deserialize;
use std::path::Path;

/// Built-in default model, used when no config file or CLI override is present.
pub const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";

const CONFIG_FILE_NAME: &str = "gitlance.toml";

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    model: Option<String>,
}

/// Resolved gitlance configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
        }
    }
}

impl Config {
    /// Loads configuration from `gitlance.toml` in `repo_path`, falling back
    /// to built-in defaults for any missing/absent values.
    pub fn load(repo_path: &str) -> Self {
        let config_path = Path::new(repo_path).join(CONFIG_FILE_NAME);

        let raw = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|contents| toml::from_str::<RawConfig>(&contents).ok())
            .unwrap_or_default();

        Self {
            model: raw.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_defaults_when_no_config_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config::load(temp_dir.path().to_str().expect("valid path"));
        assert_eq!(config.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_load_reads_model_from_config_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        std::fs::write(
            temp_dir.path().join("gitlance.toml"),
            "model = \"openai/gpt-4o\"\n",
        )
        .expect("Failed to write config file");

        let config = Config::load(temp_dir.path().to_str().expect("valid path"));
        assert_eq!(config.model, "openai/gpt-4o");
    }

    #[test]
    fn test_load_defaults_on_invalid_toml() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        std::fs::write(temp_dir.path().join("gitlance.toml"), "not valid toml =")
            .expect("Failed to write config file");

        let config = Config::load(temp_dir.path().to_str().expect("valid path"));
        assert_eq!(config.model, DEFAULT_MODEL);
    }
}
