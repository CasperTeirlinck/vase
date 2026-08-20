use std::path::Path;

use serde::Deserialize;

use crate::chrome::theme::{by_name, parse_hex, Mark, Theme, ONE_DARK};
use crate::chrome::Position;
use crate::input::{Key, KeyCode, Mods};

pub const DEFAULT: &str = include_str!("../../docs/vase.example.toml");

/// A configured chord that toggles focus to `app`.
pub struct AppFocus {
    pub key: Key,
    pub app: String,
}

pub struct Config {
    pub app_focus: Vec<AppFocus>,
    pub favorites: Vec<String>,
    pub theme: Theme,
    pub mark: Mark,
    /// `None` leaves the edge to the platform, whose OS furniture decides which one is free.
    pub bar_position: Option<Position>,
    /// Draw the accent outline around the focused pane of a split tab; off by default.
    pub focus_border: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { app_focus: Vec::new(), favorites: Vec::new(), theme: ONE_DARK, mark: Mark::Logo, bar_position: None, focus_border: false }
    }
}

impl Config {
    pub fn load(path: &Path) -> Config {
        let data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(_) => {
                Self::ensure(path);
                DEFAULT.to_string()
            }
        };
        let raw: RawConfig = match toml::from_str(&data) {
            Ok(raw) => raw,
            Err(e) => {
                eprintln!("vase: config.toml is invalid TOML ({e}); using defaults");
                return Config::default();
            }
        };
        let RawConfig { app_focus, favorites, theme, tabbar, focus_border } = raw;
        let (mark, bar_position) = tabbar.resolve();
        Config { app_focus: hotkeys(app_focus), favorites, theme: theme.resolve(), mark, bar_position, focus_border }
    }

    pub fn ensure(path: &Path) {
        if path.exists() {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, DEFAULT);
    }

    /// Persist the favorite app list, keeping the file's comments and layout.
    pub fn save_favorites(path: &Path, favorites: &[String]) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let Ok(mut doc) = text.parse::<toml_edit::DocumentMut>() else {
            eprintln!("vase: config.toml is invalid TOML; not saving favorites");
            return;
        };
        let mut arr = toml_edit::Array::new();
        for f in favorites {
            arr.push(f.as_str());
        }
        doc["favorites"] = toml_edit::value(arr);
        let _ = std::fs::write(path, doc.to_string());
    }
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    app_focus: Vec<RawAppFocus>,
    #[serde(default)]
    favorites: Vec<String>,
    #[serde(default)]
    theme: RawTheme,
    #[serde(default)]
    tabbar: RawTabbar,
    #[serde(default)]
    focus_border: bool,
}

/// Resolve each configured chord, dropping (with a warning) the ones that name no key.
fn hotkeys(raw: Vec<RawAppFocus>) -> Vec<AppFocus> {
    raw.into_iter()
        .filter_map(|e| match parse_chord(&e.key) {
            Some(key) => Some(AppFocus { key, app: e.app }),
            None => {
                eprintln!("vase: config: unrecognized key chord {:?}; skipping", e.key);
                None
            }
        })
        .collect()
}

#[derive(Deserialize)]
struct RawAppFocus {
    key: String,
    app: String,
}

/// A named preset plus optional per-color hex overrides.
#[derive(Deserialize, Default)]
struct RawTheme {
    name: Option<String>,
    bg: Option<String>,
    active: Option<String>,
    dim_bg: Option<String>,
    text: Option<String>,
    dim: Option<String>,
    accent: Option<String>,
    badge: Option<String>,
    border: Option<String>,
    hotkey: Option<String>,
}

impl RawTheme {
    fn resolve(self) -> Theme {
        let mut theme = self.name.as_deref().and_then(by_name).unwrap_or(ONE_DARK);
        let apply = |slot: &mut [f64; 4], hex: &Option<String>| {
            if let Some(h) = hex {
                match parse_hex(h) {
                    Some(c) => *slot = c,
                    None => eprintln!("vase: config: invalid color {h:?}; ignoring"),
                }
            }
        };
        apply(&mut theme.bg, &self.bg);
        apply(&mut theme.active, &self.active);
        apply(&mut theme.dim_bg, &self.dim_bg);
        apply(&mut theme.text, &self.text);
        apply(&mut theme.dim, &self.dim);
        apply(&mut theme.accent, &self.accent);
        apply(&mut theme.badge, &self.badge);
        apply(&mut theme.border, &self.border);
        apply(&mut theme.hotkey, &self.hotkey);
        theme
    }
}

#[derive(Deserialize, Default)]
struct RawTabbar {
    mark: Option<String>,
    position: Option<String>,
}

impl RawTabbar {
    fn resolve(self) -> (Mark, Option<Position>) {
        let mark = match self.mark.as_deref() {
            None | Some("vase") => Mark::Logo,
            Some("") => Mark::Hidden,
            Some(glyph) => Mark::Glyph(glyph.to_string()),
        };
        let position = match self.position.as_deref() {
            None => None,
            Some("top") => Some(Position::Top),
            Some("bottom") => Some(Position::Bottom),
            Some(other) => {
                eprintln!("vase: config: unrecognized tabbar position {other:?}; leaving it to the platform");
                None
            }
        };
        (mark, position)
    }
}

/// Parse a chord like `ctrl+grave` or `cmd+shift+k` into a `Key`.
fn parse_chord(s: &str) -> Option<Key> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let (key_tok, mod_toks) = parts.split_last()?;
    let mut mods = Mods::default();
    for m in mod_toks {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "cmd" | "command" | "super" | "meta" | "win" => mods.meta = true,
            "alt" | "option" | "opt" => mods.alt = true,
            "shift" => mods.shift = true,
            _ => return None,
        }
    }
    Some(Key { code: KeyCode::from_name(&key_tok.to_lowercase())?, mods })
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
    fn save_favorites_upserts_a_readable_key_next_to_existing_tables() {
        let src = "# a comment\n[[app_focus]]\nkey = \"ctrl+grave\"\napp = \"Ghostty\"\n";
        let mut doc = src.parse::<toml_edit::DocumentMut>().unwrap();
        let mut arr = toml_edit::Array::new();
        arr.push("Brave Browser");
        doc["favorites"] = toml_edit::value(arr);
        let out = doc.to_string();
        // The rewritten file must stay valid TOML, keep the hotkey, and read the favorite back.
        let raw: RawConfig = toml::from_str(&out).unwrap();
        assert_eq!(raw.favorites, vec!["Brave Browser".to_string()]);
        assert_eq!(raw.app_focus.len(), 1);
        assert!(out.contains("# a comment"));
    }

    #[test]
    fn parse_chord_reads_modifiers_and_rejects_unknown() {
        let k = parse_chord("cmd+shift+k").unwrap();
        assert!(k.mods.meta && k.mods.shift && !k.mods.ctrl && !k.mods.alt);
        assert!(parse_chord("bogusmod+k").is_none());
    }
}
