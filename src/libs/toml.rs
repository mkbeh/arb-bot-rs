use std::fs;

use anyhow::Context;
use serde::de::DeserializeOwned;
use toml;

pub fn parse_file<T: DeserializeOwned>(filename: &str) -> anyhow::Result<T> {
    let contents = fs::read_to_string(filename)
        .with_context(|| format!("Could not open file {}", filename))?;

    let data: T = toml::from_str(&contents)
        .with_context(|| format!("Could not parse TOML in file {}", filename))?;

    Ok(data)
}
