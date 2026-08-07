//! User config (JSON) at `~/Library/Application Support/vase/config.json`.
//! Currently: global app-focus hotkeys.

use std::path::PathBuf;

use serde::Deserialize;
use vase_core::input::{Key, Mods};

use crate::keycodes::key_code_for_name;

/// A configured chord that toggles focus to `app`.
pub struct AppFocus {
    pub key: Key,
    pub app: String,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    app_focus: Vec<RawAppFocus>,
}

#[derive(Deserialize)]
struct RawAppFocus {
    key: String,
    app: String,
}

const DEFAULT: &str = r#"{
  "app_focus": [
    { "key": "ctrl+grave", "app": "Ghostty" }
  ]
}
"#;

fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/vase/config.json"))
}

/// The config path, creating the default file on first access so "Settings"
/// always has something to open.
pub fn ensure_path() -> Option<PathBuf> {
    let path = config_path()?;
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, DEFAULT);
    }
    Some(path)
}

/// Load the app-focus hotkeys. Creates a default config on first run so the
/// feature works out of the box and the format is visible to edit. Unparseable
/// entries are skipped with a warning.
pub fn load() -> Vec<AppFocus> {
    let Some(path) = config_path() else { return Vec::new() };
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(_) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, DEFAULT);
            eprintln!("vase: wrote default config to {}", path.display());
            DEFAULT.to_string()
        }
    };
    let Ok(raw) = serde_json::from_str::<RawConfig>(&data) else {
        eprintln!("vase: config.json is invalid JSON; ignoring");
        return Vec::new();
    };
    raw.app_focus
        .into_iter()
        .filter_map(|e| match parse_chord(&e.key) {
            Some(key) => Some(AppFocus { key, app: e.app }),
            None => {
                eprintln!("vase: config: unrecognized key chord {:?}; skipping", e.key);
                None
            }
        })
        .collect()
}

/// Parse a chord like `ctrl+grave` or `cmd+shift+k` into a `Key`.
fn parse_chord(s: &str) -> Option<Key> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let (key_tok, mod_toks) = parts.split_last()?;
    let mut mods = Mods::default();
    for m in mod_toks {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "cmd" | "command" | "super" | "meta" => mods.cmd = true,
            "alt" | "option" | "opt" => mods.alt = true,
            "shift" => mods.shift = true,
            _ => return None,
        }
    }
    Some(Key { code: key_code_for_name(&key_tok.to_lowercase())?, mods })
}
