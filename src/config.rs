use anyhow::{anyhow, Result};
use fs_err as fs;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

fn default_font_size() -> f64 {
    12.0
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f64,
}

impl Default for Config {
    fn default() -> Self {
        // TODO: for some reason an empty string doesn't work here.
        serde_yaml::from_str("x: y").unwrap()
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("config dir unknown"))?
            .join("emma");

        Self::load_from_dir(&dir)
    }

    fn load_from_dir(dir: &Path) -> Result<Self> {
        // Try to create the directory. Ignore the error, it might
        // already exist.
        let _ = fs::create_dir_all(dir);

        let config_path = dir.join("emma.yml");

        // Write out a default config, but only if no config already
        // exists.
        if let Ok(mut file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_path)
        {
            let default_config = Self::default();
            let default_config_str = serde_yaml::to_string(&default_config)?;
            file.write_all(default_config_str.as_bytes())?;
        }

        // Read and parse the config.
        let raw = fs::read_to_string(config_path)?;
        Ok(serde_yaml::from_str(&raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default() {
        // The point of this test isn't to check the specific default
        // values, but rather to check that parsing a mostly-empty
        // input string doesn't panic. So only one field of the result
        // is checked to verify that it isn't zero.
        let config = Config::default();
        assert_eq!(config.font_size, 12.0);
    }

    #[test]
    fn test_load() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let tmp_dir = tmp_dir.path();

        // Test that the dir and default config get created.
        let dir = tmp_dir.join("emma");
        let _config = Config::load_from_dir(&dir)?;
        assert!(dir.exists());
        let config_path = dir.join("emma.yml");
        assert!(config_path.exists());

        // Modify the config, verify it doesn't get overwritten on load.
        fs::write(&config_path, "x: y")?;
        let _config = Config::load_from_dir(&dir)?;
        assert_eq!(fs::read_to_string(&config_path)?, "x: y");

        Ok(())
    }
}
