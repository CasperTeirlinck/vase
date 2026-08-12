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

/// The per-user and all-users Start Menu program folders, which hold the launchable shortcuts.
pub fn start_menu_dirs() -> Vec<PathBuf> {
    let user = std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Start Menu\Programs"));
    let all = std::env::var_os("ProgramData").map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Start Menu\Programs"));
    [user, all].into_iter().flatten().filter(|p| p.is_dir()).collect()
}
