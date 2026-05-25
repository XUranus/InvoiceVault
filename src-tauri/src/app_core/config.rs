use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

use crate::extractor::{BadgeConfig, BadgeGroupConfig};

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PriceConfig {
    pub llm_input_price_per_1k: f64,
    pub llm_output_price_per_1k: f64,
    pub embedding_input_price_per_1k: f64,
    pub embedding_output_price_per_1k: f64,
}

impl Default for PriceConfig {
    fn default() -> Self {
        Self {
            llm_input_price_per_1k: 0.0008,
            llm_output_price_per_1k: 0.002,
            embedding_input_price_per_1k: 0.0007,
            embedding_output_price_per_1k: 0.0007,
        }
    }
}

pub fn sanitize_badge_config(config: BadgeConfig) -> BadgeConfig {
    let mut groups = Vec::new();
    for group in config.groups {
        let name = group.name.trim();
        if name.is_empty() {
            continue;
        }

        let mut options = Vec::new();
        for option in group.options {
            let value = option.trim();
            if value.is_empty() || options.iter().any(|existing| existing == value) {
                continue;
            }
            options.push(value.to_owned());
        }

        if groups
            .iter()
            .any(|existing: &BadgeGroupConfig| existing.name == name)
        {
            continue;
        }

        groups.push(BadgeGroupConfig {
            name: name.to_owned(),
            options,
        });
    }

    BadgeConfig { groups }
}

pub fn load_config_raw<T: DeserializeOwned>(app_data_dir: &Path, filename: &str) -> Option<T> {
    let path = app_data_dir.join(filename);
    let json = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&json).ok()
}

pub fn write_config<T: Serialize>(
    app_data_dir: &Path,
    filename: &str,
    value: &T,
) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    std::fs::write(app_data_dir.join(filename), json)
}
