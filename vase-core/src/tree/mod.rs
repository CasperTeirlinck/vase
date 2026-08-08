use serde::{Deserialize, Serialize};

mod edit;
mod query;

pub(crate) use edit::rebuild_children;
pub use edit::{remove_leaf, remove_leaf_with_window, remove_selected_pane, select_stack_window, set_leaf_pane, split_pane, stack_cycle, stack_move, stack_select, stackify, swap_leaves};
pub use query::{find_window, leaf_pane, leaves, stack_selected_window, windows};

/// Opaque handle to a native window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// Stable handle to a pane leaf, so focus can point at empty panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Split orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    Horizontal,
    Vertical,
}

/// A pane's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pane {
    Empty,
    Window(WindowId),
}

/// A node in a tab's pane tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Leaf {
        id: PaneId,
        pane: Pane,
    },
    Split {
        dir: Dir,
        ratios: Vec<f64>,
        children: Vec<Node>,
    },
    /// A tabbed container showing `items[selected]` under a local tab bar.
    Stack {
        id: PaneId,
        items: Vec<Pane>,
        selected: usize,
    },
}
