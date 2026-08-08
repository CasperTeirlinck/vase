use super::Node;

mod split;
mod stack;

pub use split::{remove_leaf, remove_leaf_with_window, set_leaf_pane, split_pane, swap_leaves};
pub use stack::{remove_selected_pane, select_stack_window, stack_cycle, stack_move, stack_select, stackify};

/// Apply `f` to each child; `Some` if any changed, else `None`.
pub(crate) fn rebuild_children(children: Vec<Node>, mut f: impl FnMut(Node) -> Option<Node>) -> Option<Vec<Node>> {
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
fn collapse(kept: Vec<Node>, rebuild: impl FnOnce(Vec<Node>) -> Node) -> Option<Node> {
    match kept.len() {
        0 => None,
        1 => kept.into_iter().next(),
        _ => Some(rebuild(kept)),
    }
}
