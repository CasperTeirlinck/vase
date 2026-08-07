//! The model and the pure command reducer.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::focus::Direction;
use crate::geometry::Rect;
use crate::tree::{
    leaf_pane, leaves, remove_leaf_with_window, windows, Dir, Node, Pane, PaneId, WindowId,
};

mod query;
mod reducer;

pub use query::StackBar;
pub use reducer::apply;
#[cfg(test)]
use reducer::MIN_RATIO;

/// One tab: a pane tree, the focused pane within it, and an optional custom
/// name (set via rename; overrides the window-title label in the bar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub root: Node,
    pub focused: PaneId,
    pub name: Option<String>,
}

/// One monitor's state: its usable rect plus its own tab list and visible tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// Usable area for this monitor, CG top-left global coords.
    pub rect: Rect,
    pub tabs: Vec<Tab>,
    /// Index into `tabs`: the visible tab on this monitor (0 when empty).
    pub current: usize,
}

impl Screen {
    fn cur_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.current)
    }

    fn cur_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.current)
    }
}

/// The whole multi-monitor state. `PaneId`s are globally unique, so a given
/// PaneId lives in exactly one screen's one tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub screens: Vec<Screen>,
    /// Which monitor has keyboard focus.
    pub focused_screen: usize,
    /// When set, the focused screen's current-tab focused window fills that screen.
    pub zoomed: bool,
    /// Custom names for windows shown as stack items (nested-tab renames), keyed
    /// by window id — the stack analog of `Tab.name` for top-level tabs.
    #[serde(default)]
    pub stack_names: HashMap<WindowId, String>,
    /// Monotonic `PaneId` allocator (pure/deterministic).
    pub next_pane_id: u64,
}

/// A command from a keybinding, the CLI, or tmux edge-forwarding.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// A new managed window appeared; open it as a new single-pane tab on the
    /// given screen index.
    AddWindow(WindowId, usize),
    /// Remove a window (it closed); collapse its pane, drop the tab if empty.
    RemoveWindow(WindowId),
    /// Move pane focus in a direction (crosses monitors via global coords).
    Focus(Direction),
    /// Append a new, empty tab to the focused screen and focus it (tmux
    /// `new-window`); the empty pane opens the picker.
    NewTab,
    /// Select the next tab on the focused screen (wraps).
    NextTab,
    /// Select the previous tab on the focused screen (wraps).
    PrevTab,
    /// Set the focused screen's current tab by index.
    SelectTab(usize),
    /// Select a tab by (screen index, tab-within-screen index) and focus that
    /// screen. For tab-bar clicks, which address tabs across all screens.
    SelectScreenTab(usize, usize),
    /// Split the focused pane; the new pane is empty and becomes focused.
    Split(Dir),
    /// Swap the focused pane with its neighbor in a direction (crosses monitors).
    MoveWindow(Direction),
    /// Reorder the focused screen's current tab by `offset` (no wrap).
    MoveTab(isize),
    /// Send the focused screen's current tab to the adjacent monitor in `dir`
    /// (+1 right, -1 left), wrapping — one press, regardless of its position.
    MoveTabToScreen(isize),
    /// Set (or clear, with `None`) the focused screen's current tab's custom name.
    SetTabName(Option<String>),
    /// Resize the focused pane by nudging its split ratio (divider model).
    Resize(Direction),
    /// Toggle maximize: the focused window fills the focused screen.
    ToggleZoom,
    /// The OS focus moved to this window; sync screen + tab + focused pane.
    SyncFocus(WindowId),
    /// Focus and raise a specific window: set screen + tab + pane, raise it.
    Raise(WindowId),
    /// Move a window into the focused (empty) pane, removing it from whatever
    /// tab/pane currently holds it. No-op if the focused pane isn't empty or the
    /// window is already in the focused tab.
    FillPane(WindowId),
    /// Remove the focused pane if it's empty, collapsing the split back. No-op if
    /// the focused pane holds a window.
    CloseFocusedPane,
    /// Pop the focused pane out of its tab (tmux `break-pane`): collapse the
    /// split, and if the pane held a window, re-home it as its own new single-pane
    /// tab on the focused screen. No-op unless the current tab is split.
    BreakPane,
    /// Turn the focused pane into a stack (or grow one): a window leaf becomes a
    /// stack of [that window, empty] with the empty selected; a stack pushes a new
    /// empty item and selects it. No-op on an empty leaf.
    Stackify,
    /// Cycle the focused stack's selected item by `delta` (wraps). No-op if the
    /// focused pane isn't a stack.
    StackCycle(isize),
    /// Select the stack item holding this window (a click on a local stack bar)
    /// and move focus to it.
    SelectStackWindow(WindowId),
    /// Select the Nth (1-based) item of the focused stack (⌥e 1-9).
    StackSelect(usize),
    /// Move the focused stack's selected item by `delta` within the stack,
    /// clamped at the ends (⌥e ⇧,/⇧.).
    StackMove(isize),
    /// Set (or clear, with `None`) the custom name of the focused stack's
    /// selected item (⌥e t).
    SetStackName(Option<String>),
}

/// A side effect for the backend to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Place these windows at these rects; park every other managed window.
    Render(Vec<(WindowId, Rect)>),
    /// Give OS focus to this window.
    FocusWindow(WindowId),
}

impl Model {
    /// One single-pane tab per window, placed on the screen index paired with it.
    /// `screens` are the monitor rects; `windows` pairs each window with its
    /// screen index. focused_screen = 0; each screen.current = 0.
    pub fn adopt(screens: &[Rect], windows: &[(WindowId, usize)]) -> Model {
        if screens.is_empty() {
            return Model {
                screens: vec![],
                focused_screen: 0,
                zoomed: false,
                stack_names: HashMap::new(),
                next_pane_id: 0,
            };
        }
        let mut model_screens: Vec<Screen> =
            screens.iter().map(|rect| Screen { rect: *rect, tabs: vec![], current: 0 }).collect();
        let mut next = 0u64;
        for (w, si) in windows {
            let id = PaneId(next);
            next += 1;
            model_screens[*si].tabs.push(Tab {
                root: Node::Leaf { id, pane: Pane::Window(*w) },
                focused: id,
                name: None,
            });
        }
        Model {
            screens: model_screens,
            focused_screen: 0,
            zoomed: false,
            stack_names: HashMap::new(),
            next_pane_id: next,
        }
    }

    /// Append `w` as a new single-pane tab on `screen` (clamped), without moving
    /// focus. For restore: adopt live windows the saved layout didn't include.
    pub fn add_window(&mut self, w: WindowId, screen: usize) {
        if self.screens.is_empty() {
            return;
        }
        let si = screen.min(self.screens.len() - 1);
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        self.screens[si].tabs.push(Tab {
            root: Node::Leaf { id, pane: Pane::Window(w) },
            focused: id,
            name: None,
        });
    }

    /// Rewrite window ids: drop `Window` leaves whose id isn't a key of `map`
    /// (collapsing splits, dropping emptied tabs), then rename the survivors via
    /// `map`. For restore across a reboot, where the OS reassigns window ids and
    /// saved tabs are re-matched to live windows by `(app, title)`.
    pub fn remap_windows(&mut self, map: &HashMap<WindowId, WindowId>) {
        let keep: HashSet<WindowId> = map.keys().copied().collect();
        self.retain_windows(&keep);
        for screen in &mut self.screens {
            for tab in &mut screen.tabs {
                remap_node(&mut tab.root, map);
            }
        }
        self.stack_names =
            std::mem::take(&mut self.stack_names).into_iter().filter_map(|(k, v)| Some((*map.get(&k)?, v))).collect();
    }

    /// Drop `Window` leaves whose id isn't in `live` (collapsing splits) and any
    /// tab that ends up with no leaves; clamp `current`/`focused_screen`. Used on
    /// restore to prune windows that closed while vase wasn't running.
    pub fn retain_windows(&mut self, live: &HashSet<WindowId>) {
        for screen in &mut self.screens {
            let mut i = 0;
            while i < screen.tabs.len() {
                let root = screen.tabs[i].root.clone();
                let dead: Vec<WindowId> =
                    windows(&root).into_iter().filter(|w| !live.contains(w)).collect();
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

    /// Resize `screens` to `rects.len()`, migrating tabs from dropped screens into
    /// the last surviving screen and appending empty screens when growing, then
    /// set every screen's rect. For load onto a different display set and for
    /// hotplug. No-op if the rects already match.
    pub fn reconfigure(&mut self, rects: &[Rect]) {
        let n = rects.len();
        if n == 0 {
            return;
        }
        // No-op if the rects already match (common reconcile tick).
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

    /// The focused screen's current tab.
    pub fn focused_tab(&self) -> Option<&Tab> {
        self.fs()?.cur_tab()
    }

    pub(crate) fn focused_tab_mut(&mut self) -> Option<&mut Tab> {
        let i = self.focused_screen;
        self.screens.get_mut(i)?.cur_tab_mut()
    }

    pub(crate) fn next_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }
}

/// Rename `Window` leaf ids in place via `map` (ids absent from `map` are left
/// as-is; `remap_windows` prunes those first).
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

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
