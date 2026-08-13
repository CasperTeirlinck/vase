//! Where vase keeps its files on Windows, and where the launcher looks for apps.

use std::path::PathBuf;

fn support_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("vase"))
}

pub fn config() -> Option<PathBuf> {
    Some(support_dir()?.join("config.toml"))
}

pub fn state() -> Option<PathBuf> {
    Some(support_dir()?.join("state.json"))
}

pub fn log() -> Option<PathBuf> {
    Some(support_dir()?.join("vase.log"))
}

/// The config path, creating the default file on first access.
pub fn ensure_config() -> Option<PathBuf> {
    let path = config()?;
    vase_core::config::Config::ensure(&path);
    Some(path)
}

pub fn load_config() -> vase_core::config::Config {
    match config() {
        Some(path) => vase_core::config::Config::load(&path),
        None => vase_core::config::Config::default(),
    }
}
