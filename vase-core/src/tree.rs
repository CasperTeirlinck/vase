//! Window/pane ids and the recursive pane tree.

use serde::{Deserialize, Serialize};

/// Opaque handle to a native window, assigned by the platform backend.
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

/// A pane's content: an empty placeholder, or a managed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pane {
    Empty,
    Window(WindowId),
}

/// A node in a tab's pane tree. Leaves carry a stable `PaneId` so focus can
/// point at empty panes (which have no `WindowId`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Leaf { id: PaneId, pane: Pane },
    Split { dir: Dir, ratios: Vec<f64>, children: Vec<Node> },
    /// A tabbed container: one focusable `PaneId` whose `items` share the pane
    /// region, showing `items[selected]` under a local tab bar.
    Stack { id: PaneId, items: Vec<Pane>, selected: usize },
}

/// Every window in a subtree, in traversal order (skips empty panes).
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
        Node::Stack { id: pid, items, .. } => {
            items.iter().any(|p| matches!(p, Pane::Window(w) if *w == id)).then_some(*pid)
        }
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

/// Replace `Leaf(target)` with `Split(dir)[Leaf(target), Leaf{new_id, Empty}]`
/// at 0.5/0.5. `None` if `target` isn't found.
pub fn split_pane(node: Node, target: PaneId, dir: Dir, new_id: PaneId) -> Option<Node> {
    match node {
        Node::Leaf { id, pane } if id == target => Some(Node::Split {
            dir,
            ratios: vec![0.5, 0.5],
            children: vec![
                Node::Leaf { id, pane },
                Node::Leaf { id: new_id, pane: Pane::Empty },
            ],
        }),
        Node::Leaf { .. } => None,
        // ponytail: v1 doesn't split a stack; leave splitting-a-stack a no-op.
        Node::Stack { .. } => None,
        Node::Split { dir: d, ratios, children } => {
            rebuild_children(children, |c| split_pane(c, target, dir, new_id))
                .map(|children| Node::Split { dir: d, ratios, children })
        }
    }
}

/// Drop the leaf holding `id`, collapsing containers that end up empty (→
/// dropped) or single-child (→ replaced by that child). `None` if the whole
/// subtree is now empty.
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
            let kept: Vec<Node> =
                children.into_iter().filter_map(|c| remove_leaf_with_window(c, id)).collect();
            collapse(kept, |children| {
                let n = children.len();
                Node::Split { dir, ratios: vec![1.0 / n as f64; n], children }
            })
        }
    }
}

/// Turn the focused `Leaf{Window}` with `target` into a `Stack` of that window
/// plus a new selected `Empty`; or push an `Empty` onto the focused `Stack` and
/// select it. `None` (no-op) for an empty leaf or a target that isn't found.
pub fn stackify(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Leaf { id, pane: Pane::Window(w) } if id == target => {
            Some(Node::Stack { id, items: vec![Pane::Window(w), Pane::Empty], selected: 1 })
        }
        Node::Stack { id, mut items, .. } if id == target => {
            items.push(Pane::Empty);
            let selected = items.len() - 1;
            Some(Node::Stack { id, items, selected })
        }
        Node::Split { dir, ratios, children } => rebuild_children(children, |c| stackify(c, target))
            .map(|children| Node::Split { dir, ratios, children }),
        _ => None,
    }
}

/// Advance the selected item of the `Stack` with `target` by `delta` (wraps).
/// `None` (no-op) if `target` isn't a stack.
pub fn stack_cycle(node: Node, target: PaneId, delta: isize) -> Option<Node> {
    match node {
        Node::Stack { id, items, selected } if id == target => {
            let selected = (selected as isize + delta).rem_euclid(items.len() as isize) as usize;
            Some(Node::Stack { id, items, selected })
        }
        Node::Split { dir, ratios, children } => {
            rebuild_children(children, |c| stack_cycle(c, target, delta))
                .map(|children| Node::Split { dir, ratios, children })
        }
        _ => None,
    }
}

/// Select the `n`-th (1-based) window item of the stack `target`, skipping any
/// `Empty` items. No-op if out of range.
pub fn stack_select(node: Node, target: PaneId, n: usize) -> Option<Node> {
    match node {
        Node::Stack { id, items, selected } if id == target => {
            let idx = items
                .iter()
                .enumerate()
                .filter(|(_, p)| matches!(p, Pane::Window(_)))
                .nth(n.saturating_sub(1))
                .map(|(i, _)| i);
            Some(Node::Stack { id, items, selected: idx.unwrap_or(selected) })
        }
        Node::Split { dir, ratios, children } => {
            rebuild_children(children, |c| stack_select(c, target, n))
                .map(|children| Node::Split { dir, ratios, children })
        }
        _ => None,
    }
}

/// Move the stack `target`'s selected item by `delta` within the stack (clamped
/// at the ends), keeping it selected.
pub fn stack_move(node: Node, target: PaneId, delta: isize) -> Option<Node> {
    match node {
        Node::Stack { id, mut items, selected } if id == target => {
            let want = selected as isize + delta;
            let selected = if want >= 0 && (want as usize) < items.len() {
                items.swap(selected, want as usize);
                want as usize
            } else {
                selected
            };
            Some(Node::Stack { id, items, selected })
        }
        Node::Split { dir, ratios, children } => {
            rebuild_children(children, |c| stack_move(c, target, delta))
                .map(|children| Node::Split { dir, ratios, children })
        }
        _ => None,
    }
}

/// The stack `target`'s selected window — `Some` only when `target` names a
/// `Stack` whose selected item is a window (the nested-tab rename target).
pub fn stack_selected_window(node: &Node, target: PaneId) -> Option<WindowId> {
    match node {
        Node::Stack { id, items, selected } if *id == target => match items[*selected] {
            Pane::Window(w) => Some(w),
            Pane::Empty => None,
        },
        Node::Split { children, .. } => {
            children.iter().find_map(|c| stack_selected_window(c, target))
        }
        _ => None,
    }
}

/// Select the stack item holding `wid` (e.g. a click on its local tab bar),
/// returning the containing stack's `PaneId` so the caller can focus it.
pub fn select_stack_window(node: &mut Node, wid: WindowId) -> Option<PaneId> {
    match node {
        Node::Stack { id, items, selected } => {
            let i = items.iter().position(|p| matches!(p, Pane::Window(w) if *w == wid))?;
            *selected = i;
            Some(*id)
        }
        Node::Split { children, .. } => children.iter_mut().find_map(|c| select_stack_window(c, wid)),
        Node::Leaf { .. } => None,
    }
}

/// Close the focused empty pane `target`: a `Stack` drops its selected item
/// (collapsing to a `Leaf` at 1, gone at 0, clamped selected otherwise); any
/// other leaf is removed whole. Splits recurse/collapse. `None` if the subtree
/// becomes empty.
pub fn remove_selected_pane(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Stack { id, mut items, mut selected } if id == target => {
            items.remove(selected);
            match items.len() {
                0 => None,
                1 => Some(Node::Leaf { id, pane: items[0] }),
                _ => {
                    selected = selected.min(items.len() - 1);
                    Some(Node::Stack { id, items, selected })
                }
            }
        }
        Node::Split { dir, children, .. } => {
            let kept: Vec<Node> =
                children.into_iter().filter_map(|c| remove_selected_pane(c, target)).collect();
            collapse(kept, |children| {
                let n = children.len();
                Node::Split { dir, ratios: vec![1.0 / n as f64; n], children }
            })
        }
        other => remove_leaf(other, target),
    }
}

/// Drop the leaf with `target`, collapsing containers that end up empty (→
/// dropped) or single-child (→ replaced by that child). `None` if the whole
/// subtree is now empty.
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
            let kept: Vec<Node> =
                children.into_iter().filter_map(|c| remove_leaf(c, target)).collect();
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

/// Apply `f` to each child; return `Some(new_children)` if any child changed,
/// else `None`. Preserves unchanged children as-is.
pub(crate) fn rebuild_children(
    children: Vec<Node>,
    mut f: impl FnMut(Node) -> Option<Node>,
) -> Option<Vec<Node>> {
    let mut changed = false;
    let mut out = Vec::with_capacity(children.len());
    for child in children {
        match f(child.clone()) {
            Some(new) => {
                changed = true;
                out.push(new);
            }
            None => out.push(child),
        }
    }
    if changed {
        Some(out)
    } else {
        None
    }
}

/// Empty → None, single child → that child (unwrap the container), else rebuild.
pub(crate) fn collapse(kept: Vec<Node>, rebuild: impl FnOnce(Vec<Node>) -> Node) -> Option<Node> {
    match kept.len() {
        0 => None,
        1 => kept.into_iter().next(),
        _ => Some(rebuild(kept)),
    }
}

#[cfg(test)]
#[path = "tree_test.rs"]
mod tests;
