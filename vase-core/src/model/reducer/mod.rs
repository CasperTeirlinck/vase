use crate::tree::{find_window, rebuild_children, Dir, Node, PaneId, WindowId};

use super::{Command, Effect, Model};

mod panes;
mod stacks;
mod tabs;
mod windows;

/// The (screen, tab, pane) holding `id`.
fn locate_window(model: &Model, id: WindowId) -> Option<(usize, usize, PaneId)> {
    model.screens.iter().enumerate().find_map(|(si, s)| s.tabs.iter().enumerate().find_map(|(ti, t)| find_window(&t.root, id).map(|pid| (si, ti, pid))))
}

/// `Render` plus `FocusWindow` when the focused pane is a window.
fn render_and_focus(model: &Model) -> Vec<Effect> {
    let mut effects = vec![Effect::Render(model.placements())];
    if let Some(w) = model.focused_window() {
        effects.push(Effect::FocusWindow(w));
    }
    effects
}

/// Reduce a command into a new model + effects. Pure: no OS calls.
pub fn apply(model: Model, command: Command) -> (Model, Vec<Effect>) {
    match command {
        Command::AddWindow(id, si) => windows::add_window(model, id, si),
        Command::RemoveWindow(id) => windows::remove_window(model, id),
        Command::Focus(dir) => windows::focus(model, dir),
        Command::NewTab => tabs::new_tab(model),
        Command::NextTab => cycle_tab(model, 1),
        Command::PrevTab => cycle_tab(model, -1),
        Command::SelectTab(i) => tabs::select_tab(model, i),
        Command::SelectScreenTab(si, ti) => tabs::select_screen_tab(model, si, ti),
        Command::Split(dir) => panes::split(model, dir),
        Command::MoveWindow(dir) => windows::move_window(model, dir),
        Command::MoveTab(offset) => tabs::move_tab(model, offset),
        Command::MoveTabToScreen(dir) => tabs::move_tab_to_screen(model, dir),
        Command::SetTabName(name) => tabs::set_tab_name(model, name),
        Command::Resize(dir) => panes::resize(model, dir),
        Command::ToggleZoom => panes::toggle_zoom(model),
        Command::SyncFocus(id) => windows::sync_focus(model, id),
        Command::Raise(id) => windows::raise(model, id),
        Command::FillPane(id) => panes::fill_pane(model, id),
        Command::CloseFocusedPane => panes::close_focused_pane(model),
        Command::BreakPane => panes::break_pane(model),
        Command::Stackify => stacks::stackify(model),
        Command::StackCycle(delta) => stacks::stack_cycle(model, delta),
        Command::StackSelect(n) => stacks::stack_select(model, n),
        Command::StackMove(delta) => stacks::stack_move(model, delta),
        Command::SetStackName(name) => stacks::set_stack_name(model, name),
        Command::SelectStackWindow(wid) => stacks::select_stack_window(model, wid),
    }
}

/// Advance by `delta` through the global flat tab order (matching the tab bar), wrapping.
fn cycle_tab(mut model: Model, delta: isize) -> (Model, Vec<Effect>) {
    let counts: Vec<usize> = model.screens.iter().map(|s| s.tabs.len()).collect();
    let total: isize = counts.iter().sum::<usize>() as isize;
    if total == 0 {
        return (model, vec![]);
    }
    // Current flat index = tabs on earlier screens + the focused screen's current.
    let flat: isize = counts[..model.focused_screen].iter().sum::<usize>() as isize + model.screens[model.focused_screen].current as isize;
    let next = ((flat + delta) % total + total) % total;
    // Map back to (screen, tab).
    let mut acc = 0isize;
    for (si, &c) in counts.iter().enumerate() {
        if next < acc + c as isize {
            model.focused_screen = si;
            model.screens[si].current = (next - acc) as usize;
            break;
        }
        acc += c as isize;
    }
    let effects = render_and_focus(&model);
    (model, effects)
}

const RESIZE_STEP: f64 = 0.05;
pub(crate) const MIN_RATIO: f64 = 0.1;

/// Nudge the ratio of the innermost `axis` split containing `focused`, clamped so no ratio drops below `MIN_RATIO`.
fn resize_focus(node: Node, focused: PaneId, axis: Dir, delta: f64) -> Option<Node> {
    match node {
        Node::Leaf { .. } => None,
        Node::Stack { .. } => None,
        Node::Split { dir, ratios, children } => {
            if let Some(children) = rebuild_children(children.clone(), |c| resize_focus(c, focused, axis, delta)) {
                return Some(Node::Split { dir, ratios, children });
            }
            if dir != axis {
                return None;
            }
            let i = children.iter().position(|c| subtree_contains(c, focused))?;
            let len = children.len();
            // Move the divider on the arrow's side of `focused` (delta > 0 = right/down): the near child grows, matching tmux resize-pane.
            let d = if delta > 0.0 {
                if i + 1 < len {
                    i
                } else if i > 0 {
                    i - 1
                } else {
                    return None;
                }
            } else if i > 0 {
                i - 1
            } else if i + 1 < len {
                i
            } else {
                return None;
            };
            let mut ratios = ratios;
            let step = delta.min(ratios[d + 1] - MIN_RATIO).max(-(ratios[d] - MIN_RATIO));
            if step == 0.0 {
                return None;
            }
            ratios[d] += step;
            ratios[d + 1] -= step;
            Some(Node::Split { dir, ratios, children })
        }
    }
}

/// Whether any leaf in the subtree has `id`.
fn subtree_contains(node: &Node, id: PaneId) -> bool {
    match node {
        Node::Leaf { id: pid, .. } => *pid == id,
        Node::Stack { id: pid, .. } => *pid == id,
        Node::Split { children, .. } => children.iter().any(|c| subtree_contains(c, id)),
    }
}
