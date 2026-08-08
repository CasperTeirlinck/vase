use super::{Node, Pane, PaneId, WindowId};

/// Every window in a subtree, in traversal order.
pub fn windows(node: &Node) -> Vec<WindowId> {
    let mut out = Vec::new();
    collect_windows(node, &mut out);
    out
}

fn collect_windows(node: &Node, out: &mut Vec<WindowId>) {
    match node {
        Node::Leaf { pane: Pane::Window(id), .. } => out.push(*id),
        Node::Leaf { .. } => {}
        Node::Stack { items, .. } => {
            for p in items {
                if let Pane::Window(id) = p {
                    out.push(*id);
                }
            }
        }
        Node::Split { children, .. } => {
            for c in children {
                collect_windows(c, out);
            }
        }
    }
}

/// Every leaf in a subtree, in traversal order.
pub fn leaves(node: &Node) -> Vec<(PaneId, Pane)> {
    let mut out = Vec::new();
    collect_leaves(node, &mut out);
    out
}

fn collect_leaves(node: &Node, out: &mut Vec<(PaneId, Pane)>) {
    match node {
        Node::Leaf { id, pane } => out.push((*id, *pane)),
        Node::Stack { id, items, selected } => out.push((*id, items[*selected])),
        Node::Split { children, .. } => {
            for c in children {
                collect_leaves(c, out);
            }
        }
    }
}

/// The pane holding `id`, if any.
pub fn find_window(node: &Node, id: WindowId) -> Option<PaneId> {
    match node {
        Node::Leaf { id: pid, pane: Pane::Window(w) } if *w == id => Some(*pid),
        Node::Leaf { .. } => None,
        Node::Stack { id: pid, items, .. } => items.iter().any(|p| matches!(p, Pane::Window(w) if *w == id)).then_some(*pid),
        Node::Split { children, .. } => children.iter().find_map(|c| find_window(c, id)),
    }
}

/// The content of the leaf with `target`, if present.
pub fn leaf_pane(node: &Node, target: PaneId) -> Option<Pane> {
    match node {
        Node::Leaf { id, pane } if *id == target => Some(*pane),
        Node::Leaf { .. } => None,
        Node::Stack { id, items, selected } if *id == target => Some(items[*selected]),
        Node::Stack { .. } => None,
        Node::Split { children, .. } => children.iter().find_map(|c| leaf_pane(c, target)),
    }
}

/// Stack `target`'s selected window, if its selected item is a window.
pub fn stack_selected_window(node: &Node, target: PaneId) -> Option<WindowId> {
    match node {
        Node::Stack { id, items, selected } if *id == target => match items[*selected] {
            Pane::Window(w) => Some(w),
            Pane::Empty => None,
        },
        Node::Split { children, .. } => children.iter().find_map(|c| stack_selected_window(c, target)),
        _ => None,
    }
}
