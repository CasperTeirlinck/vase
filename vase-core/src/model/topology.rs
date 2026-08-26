use super::{Model, Screen, Tab};
use crate::geometry::Rect;
use crate::tree::{leaf_pane, leaves, remove_leaf_with_window, windows, Node, Pane, PaneId, WindowId};
use std::collections::{HashMap, HashSet};

impl Model {
    /// One single-pane tab per window, on its paired screen index.
    pub fn adopt(screens: &[Rect], windows: &[(WindowId, usize)]) -> Model {
        if screens.is_empty() {
            return Model { screens: vec![], focused_screen: 0, names: HashMap::new(), next_pane_id: 0 };
        }
        let mut model_screens: Vec<Screen> = screens.iter().map(|rect| Screen { rect: *rect, tabs: vec![], current: 0 }).collect();
        let mut next = 0u64;
        for (w, si) in windows {
            let id = PaneId(next);
            next += 1;
            model_screens[*si].tabs.push(Tab::single(id, Pane::Window(*w)));
        }
        Model { screens: model_screens, focused_screen: 0, names: HashMap::new(), next_pane_id: next }
    }

    /// Append `w` as a new single-pane tab on `screen`, without moving focus.
    pub fn add_window(&mut self, w: WindowId, screen: usize) {
        if self.screens.is_empty() {
            return;
        }
        let si = screen.min(self.screens.len() - 1);
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        self.screens[si].tabs.push(Tab::single(id, Pane::Window(w)));
    }

    /// Drop `Window` leaves absent from `map`, then rename the survivors via `map`.
    pub fn remap_windows(&mut self, map: &HashMap<WindowId, WindowId>) {
        let keep: HashSet<WindowId> = map.keys().copied().collect();
        self.retain_windows(&keep);
        for screen in &mut self.screens {
            for tab in &mut screen.tabs {
                remap_node(&mut tab.root, map);
            }
        }
        self.names = std::mem::take(&mut self.names).into_iter().filter_map(|(k, v)| Some((*map.get(&k)?, v))).collect();
    }

    /// Drop `Window` leaves absent from `live` and any tab left empty.
    pub fn retain_windows(&mut self, live: &HashSet<WindowId>) {
        for screen in &mut self.screens {
            let mut i = 0;
            while i < screen.tabs.len() {
                let root = screen.tabs[i].root.clone();
                let dead: Vec<WindowId> = windows(&root).into_iter().filter(|w| !live.contains(w)).collect();
                let mut new_root = Some(root);
                for w in dead {
                    new_root = new_root.and_then(|r| remove_leaf_with_window(r, w));
                }
                match new_root {
                    Some(root) => {
                        let tab = &mut screen.tabs[i];
                        tab.root = root;
                        if leaf_pane(&tab.root, tab.focused).is_none() {
                            tab.focused = leaves(&tab.root)[0].0;
                        }
                        i += 1;
                    }
                    None => {
                        screen.tabs.remove(i);
                        if i < screen.current {
                            screen.current = screen.current.saturating_sub(1);
                        }
                    }
                }
            }
            screen.current = screen.current.min(screen.tabs.len().saturating_sub(1));
        }
        self.focused_screen = self.focused_screen.min(self.screens.len().saturating_sub(1));
    }

    /// Resize `screens` to match `rects`, migrating tabs off dropped screens.
    pub fn reconfigure(&mut self, rects: &[Rect]) {
        let n = rects.len();
        if n == 0 {
            return;
        }
        // No-op if the rects already match.
        if self.screens.len() == n && self.screens.iter().map(|s| s.rect).eq(rects.iter().copied()) {
            return;
        }
        if self.screens.len() > n {
            let migrated: Vec<Tab> = self.screens.drain(n..).flat_map(|s| s.tabs).collect();
            self.screens[n - 1].tabs.extend(migrated);
        }
        while self.screens.len() < n {
            self.screens.push(Screen { rect: Rect::new(0.0, 0.0, 0.0, 0.0), tabs: vec![], current: 0 });
        }
        for (screen, rect) in self.screens.iter_mut().zip(rects) {
            screen.rect = *rect;
            screen.current = screen.current.min(screen.tabs.len().saturating_sub(1));
        }
        self.focused_screen = self.focused_screen.min(n - 1);
    }

    pub(crate) fn fs(&self) -> Option<&Screen> {
        self.screens.get(self.focused_screen)
    }

    pub(crate) fn fs_mut(&mut self) -> Option<&mut Screen> {
        self.screens.get_mut(self.focused_screen)
    }

    pub fn focused_tab(&self) -> Option<&Tab> {
        self.fs()?.current_tab()
    }

    pub(crate) fn focused_tab_mut(&mut self) -> Option<&mut Tab> {
        let i = self.focused_screen;
        self.screens.get_mut(i)?.current_tab_mut()
    }

    pub(crate) fn next_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }
}

/// Rename `Window` leaf ids in place via `map`.
fn remap_node(node: &mut Node, map: &HashMap<WindowId, WindowId>) {
    match node {
        Node::Leaf { pane: Pane::Window(id), .. } => {
            if let Some(new) = map.get(id) {
                *id = *new;
            }
        }
        Node::Leaf { .. } => {}
        Node::Stack { items, .. } => {
            for p in items.iter_mut() {
                if let Pane::Window(id) = p {
                    if let Some(new) = map.get(id) {
                        *id = *new;
                    }
                }
            }
        }
        Node::Split { children, .. } => {
            for c in children {
                remap_node(c, map);
            }
        }
    }
}
