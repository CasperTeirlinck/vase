use vase_core::geometry::Rect;
use vase_core::model::Model;
use vase_core::tree::{windows, WindowId};

/// Index of the display whose full CG bounds contain `frame`'s center (else 0).
pub fn screen_of(frame: Rect, screens: &[Rect]) -> usize {
    let cx = frame.x + frame.w / 2.0;
    let cy = frame.y + frame.h / 2.0;
    screens.iter().position(|r| cx >= r.x && cx < r.x + r.w && cy >= r.y && cy < r.y + r.h).unwrap_or(0)
}

/// Case-insensitive, either-direction match of an app name against a configured app.
pub fn app_matches(name: &str, app: &str) -> bool {
    let (a, b) = (name.to_lowercase(), app.to_lowercase());
    a == b || a.contains(&b) || b.contains(&a)
}

/// Every managed window across all screens' tabs.
pub fn all_windows(model: &Model) -> Vec<WindowId> {
    model.screens.iter().flat_map(|s| s.tabs.iter()).flat_map(|t| windows(&t.root)).collect()
}

/// Strip a redundant occurrence of the app name from a window title.
pub fn clean_title(title: &str, app: &str) -> String {
    let title = title.trim();
    let tl = title.to_lowercase();
    let sep = |c: char| c.is_whitespace() || "-–—|·:".contains(c);
    for needle in [app.trim(), app.split_whitespace().next().unwrap_or("")] {
        if needle.is_empty() {
            continue;
        }
        if let Some(pos) = tl.find(&needle.to_lowercase()) {
            let end = pos + needle.len();
            // tl/title byte layout diverges only on length-changing lowercasing; bail if not on a boundary.
            if !title.is_char_boundary(pos) || !title.is_char_boundary(end) {
                continue;
            }
            let before = title[..pos].trim_matches(sep);
            if !before.is_empty() {
                return before.to_string();
            }
            return title[end..].trim_matches(sep).to_string();
        }
    }
    title.to_string()
}

/// Launchable app names (`.app` file stems) from the standard macOS app directories.
pub(crate) fn discover_apps() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = ["/Applications".to_string(), "/System/Applications".to_string(), "/System/Applications/Utilities".to_string(), format!("{home}/Applications")];
    let mut apps: Vec<String> = Vec::new();
    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("app") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    apps.push(stem.to_string());
                }
            }
        }
    }
    // Finder lives in /System/Library/CoreServices (not scanned), but it's a normal launchable app.
    apps.push("Finder".to_string());
    apps.sort_by_key(|a| a.to_lowercase());
    apps.dedup();
    apps
}
