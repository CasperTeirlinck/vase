use std::path::PathBuf;

use serde::Deserialize;
use vase_core::input::keys::key_code_for_name;
use vase_core::input::{Key, Mods};

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

const DEFAULT: &str = include_str!("../../docs/vase.example.toml");

fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/vase/config.toml"))
}

/// The config path, creating the default file on first access.
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

/// Load the app-focus hotkeys, writing a default config on first run.
pub fn load() -> Vec<AppFocus> {
    let Some(path) = config_path() else {
        return Vec::new();
    };
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
    let Ok(raw) = toml::from_str::<RawConfig>(&data) else {
        eprintln!("vase: config.toml is invalid TOML; ignoring");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses_and_its_chord_resolves() {
        let raw: RawConfig = toml::from_str(DEFAULT).unwrap();
        assert_eq!(raw.app_focus.len(), 1);
        assert_eq!(raw.app_focus[0].app, "Ghostty");
        assert!(parse_chord(&raw.app_focus[0].key).is_some());
    }

    #[test]
    fn parse_chord_reads_modifiers_and_rejects_unknown() {
        let k = parse_chord("cmd+shift+k").unwrap();
        assert!(k.mods.cmd && k.mods.shift && !k.mods.ctrl && !k.mods.alt);
        assert!(parse_chord("bogusmod+k").is_none());
    }
}
