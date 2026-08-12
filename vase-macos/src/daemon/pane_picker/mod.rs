//! The auto-opening empty-pane picker: its item model and row building.

mod keys;

use std::collections::HashSet;

use vase_core::input::{Item, Switcher};
use vase_core::tree::{find_window, windows, Node, WindowId};

use super::switcher::{collect_children, Child};
use super::Daemon;

/// A pane-picker entry: an existing window, a display-only header, or a launchable app.
#[derive(Clone)]
pub enum PickItem {
    /// An existing window to move into the pane.
    Window { id: WindowId, icons: Vec<String>, display: String, prefix: String, dim: bool, off_workspace: bool },
    /// A tab/stack header: shown for context, not selectable.
    Header { icons: Vec<String>, display: String, prefix: String, dim: bool, off_workspace: bool },
    /// Launch a new instance of `apps[i]`.
    Launch(usize),
}

impl Item for PickItem {
    fn selectable(&self) -> bool {
        !matches!(self, PickItem::Header { .. })
    }

    /// Only existing windows carry an index; the launch rows are reached by name.
    fn numbered(&self) -> bool {
        matches!(self, PickItem::Window { .. })
    }
}

/// An app spawned into the focused empty pane, its window awaited for `ticks` more polls.
pub struct PendingLaunch {
    pub app: String,
    pub ticks: u32,
}

impl Daemon {
    /// The picker's rows: existing windows as a nested tree, then launchable apps. Excludes only windows already in the focused pane's own node.
    fn build_pane_items(&self) -> Vec<(PickItem, String)> {
        let model = self.model.as_ref().unwrap();
        let fscreen = model.focused_screen;
        let exclude: HashSet<WindowId> = model
            .focused_tab()
            .map(|t| {
                let fp = t.focused;
                windows(&t.root).into_iter().filter(|w| find_window(&t.root, *w) == Some(fp)).collect()
            })
            .unwrap_or_default();
        let mut out: Vec<(PickItem, String)> = Vec::new();
        for (si, screen) in model.screens.iter().enumerate() {
            let dim = si != fscreen;
            for tab in &screen.tabs {
                let wins: Vec<WindowId> = windows(&tab.root).into_iter().filter(|w| !exclude.contains(w)).collect();
                match wins.len() {
                    0 => {}
                    1 => {
                        let w = wins[0];
                        out.push((self.pick_window(w, String::new(), dim), self.switcher_label(w)));
                    }
                    _ => {
                        let icons: Vec<String> = wins.iter().map(|w| self.windows.app(*w).to_string()).collect();
                        let display = tab.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.title_of(wins[0]));
                        let off_workspace = wins.iter().any(|w| self.off_workspace.contains(w));
                        out.push((PickItem::Header { icons, display, prefix: String::new(), dim, off_workspace }, String::new()));
                        self.push_pick_children(&tab.root, dim, &exclude, &mut out);
                    }
                }
            }
        }
        // Favorites first, then the rest, each in discovery order.
        let (fav, rest): (Vec<usize>, Vec<usize>) = (0..self.apps.len()).partition(|&i| self.is_favorite(&self.apps[i]));
        for i in fav.into_iter().chain(rest) {
            out.push((PickItem::Launch(i), format!("⧉  {}", self.apps[i])));
        }
        out
    }

    pub(crate) fn is_favorite(&self, app: &str) -> bool {
        self.favorites.iter().any(|a| a == app)
    }

    /// Toggle an app's favorite state, persist it, and rebuild the picker keeping the cursor on that app.
    pub(crate) fn toggle_favorite(&mut self, app: String) {
        if let Some(pos) = self.favorites.iter().position(|a| *a == app) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(app.clone());
        }
        if let Some(path) = crate::paths::config() {
            vase_core::config::Config::save_favorites(&path, &self.favorites);
        }
        let items = self.build_pane_items();
        let mut s = Switcher::new(items);
        if let Some(idx) = s.visible().iter().position(|(it, _)| matches!(it, PickItem::Launch(i) if self.apps[*i] == app)) {
            s.select(idx);
        }
        self.pane_picker = Some(s);
        self.render_pane_picker();
    }

    /// Window rows beneath a multi-window tab's header, skipping windows in `exclude`.
    fn push_pick_children(&self, root: &Node, dim: bool, exclude: &HashSet<WindowId>, out: &mut Vec<(PickItem, String)>) {
        if let Node::Stack { .. } = root {
            let wins: Vec<WindowId> = windows(root).into_iter().filter(|w| !exclude.contains(w)).collect();
            let n = wins.len();
            for (j, w) in wins.iter().enumerate() {
                let glyph = if j + 1 == n { "└─ " } else { "├─ " };
                out.push((self.pick_window(*w, glyph.to_string(), dim), self.switcher_label(*w)));
            }
            return;
        }
        // Keep only children with a candidate window, so the ├─/└─ marker counts shown rows.
        let eff: Vec<Child> = collect_children(root)
            .into_iter()
            .filter_map(|kid| match kid {
                Child::Win(w) => (!exclude.contains(&w)).then_some(Child::Win(w)),
                Child::Stack { wins, selected } => {
                    let fw: Vec<WindowId> = wins.into_iter().filter(|w| !exclude.contains(w)).collect();
                    if fw.is_empty() {
                        return None;
                    }
                    let selected = if fw.contains(&selected) { selected } else { fw[0] };
                    Some(Child::Stack { wins: fw, selected })
                }
            })
            .collect();
        let n = eff.len();
        for (i, kid) in eff.iter().enumerate() {
            let g1 = if i + 1 == n { "└─ " } else { "├─ " };
            match kid {
                Child::Win(w) => {
                    out.push((self.pick_window(*w, g1.to_string(), dim), self.switcher_label(*w)));
                }
                Child::Stack { wins, .. } if wins.len() == 1 => {
                    out.push((self.pick_window(wins[0], g1.to_string(), dim), self.switcher_label(wins[0])));
                }
                Child::Stack { wins, selected } => {
                    let icons: Vec<String> = wins.iter().map(|w| self.windows.app(*w).to_string()).collect();
                    let off_workspace = wins.iter().any(|w| self.off_workspace.contains(w));
                    out.push((PickItem::Header { icons, display: self.title_of(*selected), prefix: g1.to_string(), dim, off_workspace }, String::new()));
                    let m = wins.len();
                    for (j, w) in wins.iter().enumerate() {
                        let g2 = if j + 1 == m { "   └─ " } else { "   ├─ " };
                        out.push((self.pick_window(*w, g2.to_string(), dim), self.switcher_label(*w)));
                    }
                }
            }
        }
    }

    fn pick_window(&self, id: WindowId, prefix: String, dim: bool) -> PickItem {
        let in_stack = !prefix.is_empty();
        PickItem::Window { id, icons: vec![self.windows.app(id).to_string()], display: self.window_display(id, in_stack), prefix, dim, off_workspace: self.off_workspace.contains(&id) }
    }
}
