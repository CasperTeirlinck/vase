use crate::geometry::Rect;
use crate::tree::{Node, PaneId, WindowId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod command;
mod query;
mod reducer;
mod topology;

pub use command::{Command, Effect};
pub use query::StackBar;
pub use reducer::apply;
#[cfg(test)]
pub(crate) use reducer::MIN_RATIO;

/// One tab, contains a pane tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub root: Node,
    pub focused: PaneId,
    pub name: Option<String>,
}

/// One monitor's state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// Usable area, CG top-left global coords.
    pub rect: Rect,
    pub tabs: Vec<Tab>,
    /// Index into `tabs`: the visible tab.
    pub current: usize,
}

impl Screen {
    fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.current)
    }

    fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.current)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub screens: Vec<Screen>,
    /// Which monitor has keyboard focus.
    pub focused_screen: usize,
    /// The focused screen's focused window fills that screen.
    pub zoomed: bool,
    /// Custom name per window,
    /// keyed by window so it survives splits, moves, and stacks.
    #[serde(default, alias = "stack_names")]
    pub names: HashMap<WindowId, String>,
    /// Monotonic `PaneId` allocator.
    pub next_pane_id: u64,
}
