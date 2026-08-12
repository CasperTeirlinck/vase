use super::super::{Node, Pane, PaneId, WindowId};
use super::{collapse, rebuild_children, remove_leaf};

/// Turn the `Leaf{Window}` `target` into a `Stack`, or push an `Empty` onto the `Stack` `target` and select it. `None` for an empty leaf.
pub fn stackify(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Leaf { id, pane: Pane::Window(w) } if id == target => Some(Node::Stack { id, items: vec![Pane::Window(w), Pane::Empty], selected: 1 }),
        Node::Stack { id, mut items, .. } if id == target => {
            items.push(Pane::Empty);
            let selected = items.len() - 1;
            Some(Node::Stack { id, items, selected })
        }
        Node::Split { dir, ratios, children } => rebuild_children(children, |c| stackify(c, target)).map(|children| Node::Split { dir, ratios, children }),
        _ => None,
    }
}

/// Advance the `Stack` `target`'s selected item by `delta` (wraps).
pub fn stack_cycle(node: Node, target: PaneId, delta: isize) -> Option<Node> {
    match node {
        Node::Stack { id, items, selected } if id == target => {
            let selected = (selected as isize + delta).rem_euclid(items.len() as isize) as usize;
            Some(Node::Stack { id, items, selected })
        }
        Node::Split { dir, ratios, children } => rebuild_children(children, |c| stack_cycle(c, target, delta)).map(|children| Node::Split { dir, ratios, children }),
        _ => None,
    }
}

/// Select the `n`-th (1-based) window item of stack `target`, skipping `Empty`.
pub fn stack_select(node: Node, target: PaneId, n: usize) -> Option<Node> {
    match node {
        Node::Stack { id, items, selected } if id == target => {
            let idx = items.iter().enumerate().filter(|(_, p)| matches!(p, Pane::Window(_))).nth(n.saturating_sub(1)).map(|(i, _)| i);
            Some(Node::Stack { id, items, selected: idx.unwrap_or(selected) })
        }
        Node::Split { dir, ratios, children } => rebuild_children(children, |c| stack_select(c, target, n)).map(|children| Node::Split { dir, ratios, children }),
        _ => None,
    }
}

/// Move stack `target`'s selected item by `delta` (clamped), keeping it selected.
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
        Node::Split { dir, ratios, children } => rebuild_children(children, |c| stack_move(c, target, delta)).map(|children| Node::Split { dir, ratios, children }),
        _ => None,
    }
}

/// Select the stack item holding `wid`, returning the stack's `PaneId`.
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

/// Drop `target`'s selected item (a `Stack`) or the whole leaf, collapsing splits. `None` if the subtree becomes empty.
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
            let kept: Vec<Node> = children.into_iter().filter_map(|c| remove_selected_pane(c, target)).collect();
            collapse(kept, |children| {
                let n = children.len();
                Node::Split { dir, ratios: vec![1.0 / n as f64; n], children }
            })
        }
        other => remove_leaf(other, target),
    }
}
