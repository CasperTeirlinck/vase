use crate::geometry::{layout, layout_stacked, Rect};
use crate::tree::{leaf_pane, leaves, windows, Dir, Node, Pane, PaneId, WindowId};

use super::{Model, Tab};

/// A visible stack's local tab bar.
#[derive(Debug, Clone, PartialEq)]
pub struct StackBar {
    pub rect: Rect,
    pub items: Vec<WindowId>,
    pub selected: usize,
    pub focused: bool,
}

impl Model {
    /// Index of the screen whose CURRENT tab has a leaf with `pid`.
    pub(crate) fn screen_of_current_pane(&self, pid: PaneId) -> Option<usize> {
        self.screens.iter().position(|s| s.current_tab().map(|t| leaf_pane(&t.root, pid).is_some()).unwrap_or(false))
    }

    /// Split a flat (bar-order) tab index into `(screen, tab within that screen)`.
    pub fn screen_tab(&self, flat: usize) -> Option<(usize, usize)> {
        let mut acc = 0;
        for (si, s) in self.screens.iter().enumerate() {
            if flat < acc + s.tabs.len() {
                return Some((si, flat - acc));
            }
            acc += s.tabs.len();
        }
        None
    }

    /// Every managed window, across every screen's tabs (not just the visible ones).
    pub fn all_windows(&self) -> Vec<WindowId> {
        self.screens.iter().flat_map(|s| s.tabs.iter()).flat_map(|t| windows(&t.root)).collect()
    }

    /// Visible window placements across all screens.
    pub fn placements(&self) -> Vec<(WindowId, Rect)> {
        let mut out = Vec::new();
        for (si, screen) in self.screens.iter().enumerate() {
            let Some(tab) = screen.current_tab() else {
                continue;
            };
            if self.zoomed && si == self.focused_screen {
                if let Some(Pane::Window(w)) = leaf_pane(&tab.root, tab.focused) {
                    out.push((w, zoom_rect(screen.rect, tab)));
                    continue;
                }
            }
            let mut laid = Vec::new();
            layout(&tab.root, screen.rect, &mut laid);
            out.extend(laid.into_iter().filter_map(|(_, pane, rect)| match pane {
                Pane::Window(w) => Some((w, rect)),
                Pane::Empty => None,
            }));
        }
        out
    }

    /// Every window in every tab, on the rect its own tab's layout gives it, a stack's occluded items
    /// included. The zoom is ignored: it belongs to what is on screen, which `placements` answers.
    pub fn all_placements(&self) -> Vec<(WindowId, Rect)> {
        let mut out = Vec::new();
        for screen in &self.screens {
            for tab in &screen.tabs {
                let mut laid = Vec::new();
                layout_stacked(&tab.root, screen.rect, &mut laid);
                out.extend(laid.into_iter().filter_map(|(_, pane, rect)| match pane {
                    Pane::Window(w) => Some((w, rect)),
                    Pane::Empty => None,
                }));
            }
        }
        out
    }

    /// Every screen's current-tab empty-pane rects; the bool flags the focused one.
    pub fn empty_panes(&self) -> Vec<(Rect, bool)> {
        let mut out = Vec::new();
        for (si, screen) in self.screens.iter().enumerate() {
            let Some(tab) = screen.current_tab() else {
                continue;
            };
            let mut laid = Vec::new();
            layout(&tab.root, screen.rect, &mut laid);
            out.extend(laid.into_iter().filter_map(|(id, pane, rect)| match pane {
                Pane::Empty => Some((rect, si == self.focused_screen && id == tab.focused)),
                Pane::Window(_) => None,
            }));
        }
        out
    }

    /// Per tab across all screens: `(window panes, label window, custom name)`, and the flat index of the focused tab.
    #[allow(clippy::type_complexity)]
    pub fn bar_tabs(&self) -> (Vec<(Vec<WindowId>, Option<WindowId>, Option<String>)>, usize) {
        let mut tabs = Vec::new();
        let mut flat_current = 0;
        let mut offset = 0;
        for (si, screen) in self.screens.iter().enumerate() {
            if si == self.focused_screen {
                flat_current = offset + screen.current;
            }
            for t in &screen.tabs {
                let ws = windows(&t.root);
                // A single-window tab's name lives with the window; a multi-window tab carries a group name on the tab.
                let name = if ws.len() == 1 { self.names.get(&ws[0]).cloned().or_else(|| t.name.clone()) } else { t.name.clone() };
                tabs.push((ws, tab_label_window(t), name));
            }
            offset += screen.tabs.len();
        }
        (tabs, flat_current)
    }

    /// Whether the focused pane is an empty placeholder.
    pub fn focused_pane_is_empty(&self) -> bool {
        self.focused_tab().and_then(|t| leaf_pane(&t.root, t.focused)).map(|p| p == Pane::Empty).unwrap_or(false)
    }

    /// The focused stack's selected window, if the focused pane is a stack.
    pub fn focused_stack_window(&self) -> Option<WindowId> {
        let tab = self.focused_tab()?;
        crate::tree::stack_selected_window(&tab.root, tab.focused)
    }

    /// The focused pane's window, if any.
    pub fn focused_window(&self) -> Option<WindowId> {
        let tab = self.focused_tab()?;
        match leaf_pane(&tab.root, tab.focused)? {
            Pane::Window(w) => Some(w),
            Pane::Empty => None,
        }
    }

    /// The rect of the focused pane.
    pub fn focused_pane_rect(&self) -> Option<Rect> {
        let screen = self.fs()?;
        let tab = screen.current_tab()?;
        if self.zoomed && matches!(leaf_pane(&tab.root, tab.focused), Some(Pane::Window(_))) {
            return Some(zoom_rect(screen.rect, tab));
        }
        let mut out = Vec::new();
        layout(&tab.root, screen.rect, &mut out);
        out.into_iter().find(|(id, _, _)| *id == tab.focused).map(|(_, _, r)| r)
    }

    /// Number of panes (leaves) in the focused tab.
    pub fn current_pane_count(&self) -> usize {
        self.focused_tab().map(|t| leaves(&t.root).len()).unwrap_or(0)
    }

    /// Every screen's current-tab leaf rects, for directional focus.
    pub(crate) fn leaf_targets(&self) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        for screen in &self.screens {
            let Some(tab) = screen.current_tab() else {
                continue;
            };
            let mut laid = Vec::new();
            layout(&tab.root, screen.rect, &mut laid);
            out.extend(laid.into_iter().map(|(id, _, rect)| (id, rect)));
        }
        out
    }
}

impl Model {
    /// Every visible stack's local tab bar, across all screens' current tabs.
    pub fn stacks(&self) -> Vec<StackBar> {
        let mut out = Vec::new();
        for (si, screen) in self.screens.iter().enumerate() {
            let Some(tab) = screen.current_tab() else {
                continue;
            };
            let mut bars = Vec::new();
            collect_stacks(&tab.root, screen.rect, si == self.focused_screen, tab.focused, &mut bars);
            // Zoomed, the focused pane covers the screen: the other stacks are behind it, and the
            // one that is zoomed spans what it now fills rather than the slot it came from.
            if self.zoomed && si == self.focused_screen {
                bars.retain(|bar| bar.focused);
                for bar in &mut bars {
                    bar.rect = screen.rect;
                }
            }
            out.append(&mut bars);
        }
        out
    }
}

/// What a zoomed pane fills. A zoomed stack keeps its bar (`stacks` stretches it across the screen),
/// so its window gives up the strip the bar sits on, exactly as it does at its normal size.
fn zoom_rect(screen: Rect, tab: &Tab) -> Rect {
    if crate::tree::stack_selected_window(&tab.root, tab.focused).is_none() {
        return screen;
    }
    let strip = crate::chrome::bar_height();
    Rect::new(screen.x, screen.y + strip, screen.w, screen.h - strip)
}

/// Push a `StackBar` (full rect) for each `Stack`, subdividing `area` like `layout`.
fn collect_stacks(node: &Node, area: Rect, screen_focused: bool, focused: PaneId, out: &mut Vec<StackBar>) {
    match node {
        Node::Leaf { .. } => {}
        Node::Stack { id, items, selected } => {
            let win_items: Vec<WindowId> = items
                .iter()
                .filter_map(|p| match p {
                    Pane::Window(w) => Some(*w),
                    Pane::Empty => None,
                })
                .collect();
            // Index of the selected item among the Window items; 0 if it's Empty.
            let sel = match items.get(*selected) {
                Some(Pane::Window(_)) => items[..*selected].iter().filter(|p| matches!(p, Pane::Window(_))).count(),
                _ => 0,
            };
            out.push(StackBar { rect: area, items: win_items, selected: sel, focused: screen_focused && *id == focused });
        }
        Node::Split { dir, ratios, children } => {
            let mut offset = 0.0;
            for (child, ratio) in children.iter().zip(ratios) {
                let child_rect = match dir {
                    Dir::Horizontal => Rect::new(area.x + offset, area.y, area.w * ratio, area.h),
                    Dir::Vertical => Rect::new(area.x, area.y + offset, area.w, area.h * ratio),
                };
                collect_stacks(child, child_rect, screen_focused, focused, out);
                offset += match dir {
                    Dir::Horizontal => area.w * ratio,
                    Dir::Vertical => area.h * ratio,
                };
            }
        }
    }
}

/// The window whose name labels the tab: the focused pane's, else the first.
fn tab_label_window(tab: &Tab) -> Option<WindowId> {
    if let Some(Pane::Window(w)) = leaf_pane(&tab.root, tab.focused) {
        return Some(w);
    }
    windows(&tab.root).first().copied()
}
