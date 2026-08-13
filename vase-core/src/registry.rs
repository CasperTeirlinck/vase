//! Per-window bookkeeping that sits outside the layout model: what a window is called, where it was before vase adopted it, and where vase last put it.

use std::collections::HashMap;

use crate::backend::WindowInfo;
use crate::geometry::Rect;
use crate::tree::WindowId;

/// One adopted window.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub app: String,
    pub title: String,
    /// Frame at adoption, put back on exit.
    pub original: Rect,
    /// Minimized to the Dock: keeps its tab, but isn't placed until its tab is selected.
    pub minimized: bool,
    /// Rect of the last placement, to detect a move to another display.
    pub placed: Option<Rect>,
}

/// Every window vase has adopted. Adopting and forgetting are single calls so the fields above cannot go out of step with each other.
#[derive(Debug, Default)]
pub struct Registry {
    windows: HashMap<WindowId, Window>,
}

impl Registry {
    /// Take a window under management, or refresh what we know about one already managed (keeping its original frame, which is what exit restores it to).
    pub fn adopt(&mut self, info: &WindowInfo, minimized: bool) {
        let entry = self.windows.entry(info.id).or_insert_with(|| Window { app: String::new(), title: String::new(), original: info.frame, minimized, placed: None });
        entry.app = info.app.clone();
        entry.title = info.title.clone();
    }

    /// Stop tracking a window, dropping everything keyed to it.
    pub fn forget(&mut self, id: WindowId) {
        self.windows.remove(&id);
    }

    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.contains_key(&id)
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    /// Owning app's display name, empty for a window we don't track.
    pub fn app(&self, id: WindowId) -> &str {
        self.windows.get(&id).map_or("", |w| w.app.as_str())
    }

    /// Current window title, empty for a window we don't track.
    pub fn title(&self, id: WindowId) -> &str {
        self.windows.get(&id).map_or("", |w| w.title.as_str())
    }

    pub fn is_minimized(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|w| w.minimized)
    }

    pub fn set_minimized(&mut self, id: WindowId, minimized: bool) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.minimized = minimized;
        }
    }

    /// Replace a window's title; returns whether it actually changed.
    pub fn set_title(&mut self, id: WindowId, title: String) -> bool {
        match self.windows.get_mut(&id) {
            Some(w) if w.title != title => {
                w.title = title;
                true
            }
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (WindowId, &Window)> {
        self.windows.iter().map(|(id, w)| (*id, w))
    }
}

/// Case-insensitive, either-direction match of an app name against a configured app.
pub fn app_matches(name: &str, app: &str) -> bool {
    let (a, b) = (name.to_lowercase(), app.to_lowercase());
    a == b || a.contains(&b) || b.contains(&a)
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
