use super::super::{leaf_pane, Dir, Node, Pane, PaneId, WindowId};
use super::{collapse, rebuild_children};

/// Replace `Leaf(target)` with `Split(dir)[Leaf(target), Leaf{new_id, Empty}]` at 0.5/0.5.
pub fn split_pane(node: Node, target: PaneId, dir: Dir, new_id: PaneId) -> Option<Node> {
    match node {
        Node::Leaf { id, pane } if id == target => Some(Node::Split { dir, ratios: vec![0.5, 0.5], children: vec![Node::Leaf { id, pane }, Node::Leaf { id: new_id, pane: Pane::Empty }] }),
        Node::Leaf { .. } => None,
        // A stack has no split axis; splitting it is a no-op.
        Node::Stack { .. } => None,
        Node::Split { dir: d, ratios, children } => rebuild_children(children, |c| split_pane(c, target, dir, new_id)).map(|children| Node::Split { dir: d, ratios, children }),
    }
}

/// Drop the leaf holding `id`, collapsing containers left empty or single-child. `None` if the whole subtree is now empty.
pub fn remove_leaf_with_window(node: Node, id: WindowId) -> Option<Node> {
    match node {
        Node::Leaf { id: pid, pane } => match pane {
            Pane::Window(w) if w == id => None,
            pane => Some(Node::Leaf { id: pid, pane }),
        },
        Node::Stack { id: pid, mut items, mut selected } => {
            let Some(i) = items.iter().position(|p| matches!(p, Pane::Window(w) if *w == id)) else {
                return Some(Node::Stack { id: pid, items, selected });
            };
            items.remove(i);
            match items.len() {
                0 => None,
                1 => Some(Node::Leaf { id: pid, pane: items[0] }),
                _ => {
                    selected = selected.min(items.len() - 1);
                    Some(Node::Stack { id: pid, items, selected })
                }
            }
        }
        Node::Split { dir, children, .. } => {
            let kept: Vec<Node> = children.into_iter().filter_map(|c| remove_leaf_with_window(c, id)).collect();
            collapse(kept, |children| {
                let n = children.len();
                Node::Split { dir, ratios: vec![1.0 / n as f64; n], children }
            })
        }
    }
}

/// Drop the leaf with `target`, collapsing containers left empty or single-child. `None` if the whole subtree is now empty.
pub fn remove_leaf(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Leaf { id, pane } => {
            if id == target {
                None
            } else {
                Some(Node::Leaf { id, pane })
            }
        }
        Node::Stack { id, items, selected } => {
            if id == target {
                None
            } else {
                Some(Node::Stack { id, items, selected })
            }
        }
        Node::Split { dir, children, .. } => {
            let kept: Vec<Node> = children.into_iter().filter_map(|c| remove_leaf(c, target)).collect();
            collapse(kept, |children| {
                let n = children.len();
                Node::Split { dir, ratios: vec![1.0 / n as f64; n], children }
            })
        }
    }
}

/// Swap the `pane` contents of the two leaves, keeping their `PaneId`s in place.
pub fn swap_leaves(node: &mut Node, a: PaneId, b: PaneId) {
    if let (Some(pa), Some(pb)) = (leaf_pane(node, a), leaf_pane(node, b)) {
        set_leaf_pane(node, a, pb);
        set_leaf_pane(node, b, pa);
    }
}

/// Set the content of the leaf with `target`, if present.
pub fn set_leaf_pane(node: &mut Node, target: PaneId, value: Pane) {
    match node {
        Node::Leaf { id, pane } if *id == target => *pane = value,
        Node::Leaf { .. } => {}
        Node::Stack { id, items, selected } if *id == target => items[*selected] = value,
        Node::Stack { .. } => {}
        Node::Split { children, .. } => {
            for c in children.iter_mut() {
                set_leaf_pane(c, target, value);
            }
        }
    }
}
