use {
    anyhow::{anyhow, Error},
    fehler::throws,
    fs_err as fs,
    serde::Deserialize,
};

fn default_font_size() -> f64 {
    11.0
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f64,
}

impl Default for Config {
    fn default() -> Config {
        // TODO: for some reason an empty string doesn't work here.
        serde_yaml::from_str("x: y").unwrap()
    }
}

impl Config {
    #[throws]
    pub fn load() -> Config {
        let path = dirs::config_dir()
            .ok_or_else(|| anyhow!("config dir unknown"))?
            .join("emma/emma.yml");
        let raw = fs::read_to_string(path)?;

        serde_yaml::from_str(&raw)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        // The point of this test isn't to check the specific default
        // values, but rather to check that parsing a mostly-empty
        // input string doesn't panic. So only one field of the result
        // is checked to verify that it isn't zero.
        let config = Config::default();
        assert_eq!(config.font_size, 11.0);
    }
}
