//! Where vase keeps its files on macOS.

use std::path::PathBuf;

fn support_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/vase"))
}

pub fn config() -> Option<PathBuf> {
    Some(support_dir()?.join("config.toml"))
}

pub fn state() -> Option<PathBuf> {
    Some(support_dir()?.join("state.json"))
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
