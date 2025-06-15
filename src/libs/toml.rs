use std::fs;

use anyhow::anyhow;
use serde::de::DeserializeOwned;
use toml;

pub fn parse_file<T: DeserializeOwned>(filename: &str) -> anyhow::Result<T> {
    let contents = match fs::read_to_string(filename) {
        Ok(contents) => contents,
        Err(e) => return Err(anyhow!("Could not open file {}: {}", filename, e)),
    };

    let data: T = match toml::from_str(&contents) {
        Ok(data) => data,
        Err(e) => return Err(anyhow!("Could not parse file {}: {}", filename, e)),
    };

    Ok(data)
}
