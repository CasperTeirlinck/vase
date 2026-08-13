//! The prefix-w window switcher overlay: its item model and row building.

mod keys;

use crate::input::{Item, Switcher};
use crate::tree::{windows, Node, Pane, WindowId};

use super::Daemon;
use crate::backend::Backend;
use crate::chrome::Painter;

/// What picking a switcher row does.
#[derive(Clone, Copy)]
pub enum SwitchTarget {
    /// Raise (and reveal, if stacked) this window.
    Window(WindowId),
    /// A tab header: select that top-level tab (like clicking it in the bar).
    Tab(usize, usize),
}

/// One switcher row: what it points at plus its precomputed display fields.
#[derive(Clone)]
pub struct SwitchItem {
    pub target: SwitchTarget,
    pub prefix: String,     // tree glyph for a nested row
    pub icons: Vec<String>, // app icons (several on a split/stack parent)
    pub display: String,
    pub dim: bool,           // on a non-focused monitor
    pub off_workspace: bool, // a window in the row is on another Space
    pub current: bool,       // the currently-focused window
}

/// Every switcher row is pickable, including tab headers.
impl Item for SwitchItem {}

/// One child of a tab, flattening splits: a window, or a stack (with its window items and the selected one).
pub(crate) enum Child {
    Win(WindowId),
    Stack { wins: Vec<WindowId>, selected: WindowId },
}

/// The tab's direct children in order, flattening nested splits; a stack keeps its items.
pub(crate) fn collect_children(node: &Node) -> Vec<Child> {
    match node {
        Node::Leaf { pane: Pane::Window(w), .. } => vec![Child::Win(*w)],
        Node::Leaf { .. } => vec![],
        Node::Stack { items, selected, .. } => {
            let wins: Vec<WindowId> = items.iter().filter_map(|p| if let Pane::Window(w) = p { Some(*w) } else { None }).collect();
            if wins.is_empty() {
                return vec![];
            }
            let sel = match items.get(*selected) {
                Some(Pane::Window(w)) => *w,
                _ => wins[0],
            };
            vec![Child::Stack { wins, selected: sel }]
        }
        Node::Split { children, .. } => children.iter().flat_map(collect_children).collect(),
    }
}

impl<B: Backend, C: Painter> Daemon<B, C> {
    pub fn open_switcher(&mut self) {
        let model = self.model.as_ref().unwrap();
        let focused = model.focused_window();
        let focused_screen = model.focused_screen;
        let mut items: Vec<(SwitchItem, String)> = Vec::new();
        for (si, screen) in model.screens.iter().enumerate() {
            let dim = si != focused_screen;
            for (ti, tab) in screen.tabs.iter().enumerate() {
                let wins = windows(&tab.root);
                match wins.len() {
                    0 => {}
                    // A single-window tab is one flat row.
                    1 => {
                        let w = wins[0];
                        items.push((self.win_item(w, String::new(), dim, focused), self.switcher_label(w)));
                    }
                    // A split/stack tab: a header with every app icon, then its windows as a tree.
                    _ => {
                        let icons: Vec<String> = wins.iter().map(|w| self.windows.app(*w).to_string()).collect();
                        let rep = match &tab.root {
                            Node::Stack { items, selected, .. } => match items.get(*selected) {
                                Some(Pane::Window(w)) => *w,
                                _ => wins[0],
                            },
                            _ => wins[0],
                        };
                        let display = tab.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.title_of(rep));
                        let mut search = display.clone();
                        for w in &wins {
                            search.push(' ');
                            search.push_str(&self.switcher_label(*w));
                        }
                        let off_workspace = wins.iter().any(|w| self.off_workspace.contains(w));
                        items.push((SwitchItem { target: SwitchTarget::Tab(si, ti), prefix: String::new(), icons, display, dim, off_workspace, current: false }, search));
                        self.push_children(&tab.root, dim, focused, &mut items);
                    }
                }
            }
        }
        // Preselect the currently-focused window rather than the top item.
        let start = items.iter().position(|(it, _)| matches!(it.target, SwitchTarget::Window(w) if Some(w) == focused)).unwrap_or(0);
        let mut switcher = Switcher::new(items);
        switcher.select(start);
        self.switcher = Some(switcher);
        self.render_switcher();
    }

    /// Push a multi-window tab's window rows beneath its header.
    fn push_children(&self, root: &Node, dim: bool, focused: Option<WindowId>, items: &mut Vec<(SwitchItem, String)>) {
        if let Node::Stack { .. } = root {
            let wins = windows(root);
            let n = wins.len();
            for (j, w) in wins.iter().enumerate() {
                let glyph = if j + 1 == n { "└─ " } else { "├─ " };
                items.push((self.win_item(*w, glyph.to_string(), dim, focused), self.switcher_label(*w)));
            }
            return;
        }
        let kids = collect_children(root);
        let n = kids.len();
        for (i, kid) in kids.iter().enumerate() {
            let g1 = if i + 1 == n { "└─ " } else { "├─ " };
            match kid {
                Child::Win(w) => {
                    items.push((self.win_item(*w, g1.to_string(), dim, focused), self.switcher_label(*w)));
                }
                Child::Stack { wins, selected } => {
                    let icons: Vec<String> = wins.iter().map(|w| self.windows.app(*w).to_string()).collect();
                    let display = self.title_of(*selected);
                    let mut search = display.clone();
                    for w in wins {
                        search.push(' ');
                        search.push_str(&self.switcher_label(*w));
                    }
                    let off_workspace = wins.iter().any(|w| self.off_workspace.contains(w));
                    items.push((SwitchItem { target: SwitchTarget::Window(*selected), prefix: g1.to_string(), icons, display, dim, off_workspace, current: false }, search));
                    let m = wins.len();
                    for (j, w) in wins.iter().enumerate() {
                        let g2 = if j + 1 == m { "   └─ " } else { "   ├─ " };
                        items.push((self.win_item(*w, g2.to_string(), dim, focused), self.switcher_label(*w)));
                    }
                }
            }
        }
    }

    /// A window row: a single app icon and the window's display name.
    fn win_item(&self, id: WindowId, prefix: String, dim: bool, focused: Option<WindowId>) -> SwitchItem {
        SwitchItem {
            target: SwitchTarget::Window(id),
            prefix,
            icons: vec![self.windows.app(id).to_string()],
            display: self.window_display(id),
            dim,
            off_workspace: self.off_workspace.contains(&id),
            current: Some(id) == focused,
        }
    }
}
