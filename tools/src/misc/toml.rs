use std::{fs, path::Path};

use anyhow::Context;
use serde::de::DeserializeOwned;
use toml;

/// Parses a TOML file into a struct that implements `DeserializeOwned`.
///
/// This utility function reads the contents of a TOML file from disk, deserializes it using
/// `toml::from_str`, and returns the parsed data. It provides contextual error messages for
/// file I/O and parsing failures.
///
/// # Type Parameters
/// * `T` - The target type that must implement `serde::de::DeserializeOwned`.
///
/// # Arguments
/// * `filename` - Path to the TOML file to parse.
///
/// # Errors
/// Returns an `anyhow::Error` if:
/// - The file cannot be read (e.g., does not exist or permission denied).
/// - The file contents are invalid TOML (deserialization fails).
///
/// # Examples
/// ```
/// use anyhow::Result;
/// use serde::Deserialize;
/// use tools::toml::parse_file;
///
/// #[derive(Deserialize)]
/// struct Config {
///     name: String,
/// }
///
/// let config: Result<Config> = parse_file("config.toml");
/// ```
///
/// # Panics
/// This function does not panic.
pub fn parse_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let path = path.as_ref();

    let contents = fs::read_to_string(path)
        .with_context(|| format!("Could not open file {:?}", path.display()))?;

    let data: T = toml::from_str(&contents)
        .with_context(|| format!("Could not parse TOML in file {:?}", path.display()))?;

    Ok(data)
}
