//! The pure command reducer.

use crate::focus::{neighbor, Direction};
use crate::tree::{
    find_window, leaf_pane, leaves, rebuild_children, remove_leaf_with_window,
    remove_selected_pane, select_stack_window, set_leaf_pane, split_pane, stack_cycle, stack_move,
    stack_select, stackify, swap_leaves, windows, Dir, Node, Pane, PaneId, WindowId,
};

use super::{Command, Effect, Model, Tab};

/// The (screen index, tab index within that screen, pane) holding `id`, across
/// all screens' all tabs.
fn locate_window(model: &Model, id: WindowId) -> Option<(usize, usize, PaneId)> {
    model.screens.iter().enumerate().find_map(|(si, s)| {
        s.tabs.iter().enumerate().find_map(|(ti, t)| find_window(&t.root, id).map(|pid| (si, ti, pid)))
    })
}

/// `Render(placements)` plus `FocusWindow` when the focused pane is a window
/// (empty pane → no OS focus).
fn render_and_focus(model: &Model) -> Vec<Effect> {
    let mut effects = vec![Effect::Render(model.placements())];
    if let Some(w) = model.focused_window() {
        effects.push(Effect::FocusWindow(w));
    }
    effects
}

/// Reduce a command into a new model + effects. Pure: no OS calls.
pub fn apply(mut model: Model, command: Command) -> (Model, Vec<Effect>) {
    match command {
        Command::AddWindow(id, si) => {
            let pid = model.next_id();
            let screen = &mut model.screens[si];
            screen.tabs.push(Tab {
                root: Node::Leaf { id: pid, pane: Pane::Window(id) },
                focused: pid,
                name: None,
            });
            screen.current = screen.tabs.len() - 1;
            model.focused_screen = si;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::RemoveWindow(id) => {
            model.stack_names.remove(&id);
            let Some((si, ti, _)) = locate_window(&model, id) else {
                return (model, vec![]);
            };
            match remove_leaf_with_window(model.screens[si].tabs[ti].root.clone(), id) {
                Some(root) => {
                    let tab = &mut model.screens[si].tabs[ti];
                    tab.root = root;
                    if leaf_pane(&tab.root, tab.focused).is_none() {
                        tab.focused = leaves(&tab.root)[0].0;
                    }
                }
                None => {
                    let screen = &mut model.screens[si];
                    screen.tabs.remove(ti);
                    screen.current = screen.current.min(screen.tabs.len().saturating_sub(1));
                }
            }
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::Focus(dir) => {
            let targets = model.leaf_targets();
            let Some(from) = model.focused_tab().map(|t| t.focused) else {
                return (model, vec![]);
            };
            match neighbor(&targets, from, dir) {
                Some(next) => {
                    if let Some(si) = model.screen_of_current_pane(next) {
                        model.focused_screen = si;
                        if let Some(tab) = model.screens[si].cur_tab_mut() {
                            tab.focused = next;
                        }
                    }
                    let effects = model.focused_window().map(Effect::FocusWindow).into_iter().collect();
                    (model, effects)
                }
                None => (model, vec![]),
            }
        }
        Command::NewTab => {
            let id = model.next_id();
            let Some(screen) = model.fs_mut() else {
                return (model, vec![]);
            };
            screen.tabs.push(Tab { root: Node::Leaf { id, pane: Pane::Empty }, focused: id, name: None });
            screen.current = screen.tabs.len() - 1;
            // Empty pane → render (park the old tab's windows); no FocusWindow.
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::NextTab => cycle_tab(model, 1),
        Command::PrevTab => cycle_tab(model, -1),
        Command::SelectTab(i) => {
            let Some(screen) = model.fs_mut() else {
                return (model, vec![]);
            };
            if i < screen.tabs.len() {
                screen.current = i;
                // Render, not just focus: the target tab's window is parked
                // off-screen while another tab is current, so it must be moved
                // back on-screen (and the previous tab's windows parked).
                let effects = render_and_focus(&model);
                (model, effects)
            } else {
                (model, vec![])
            }
        }
        Command::SelectScreenTab(si, ti) => match model.screens.get_mut(si) {
            Some(screen) if ti < screen.tabs.len() => {
                screen.current = ti;
                model.focused_screen = si;
                let effects = render_and_focus(&model);
                (model, effects)
            }
            _ => (model, vec![]),
        },
        Command::Split(dir) => {
            let Some(from) = model.focused_tab().map(|t| t.focused) else {
                return (model, vec![]);
            };
            let new_id = model.next_id();
            let old_root = model.focused_tab().unwrap().root.clone();
            let Some(root) = split_pane(old_root, from, dir, new_id) else {
                return (model, vec![]);
            };
            if let Some(tab) = model.focused_tab_mut() {
                tab.root = root;
                tab.focused = new_id;
            }
            // render_and_focus omits FocusWindow here: the new pane is empty, so
            // focused_window() is None and OS focus stays put.
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::MoveWindow(dir) => {
            let targets = model.leaf_targets();
            let Some(from) = model.focused_tab().map(|t| t.focused) else {
                return (model, vec![]);
            };
            let from_si = model.focused_screen;
            match neighbor(&targets, from, dir) {
                Some(other) => {
                    let Some(to_si) = model.screen_of_current_pane(other) else {
                        return (model, vec![]);
                    };
                    if to_si == from_si {
                        let tab = model.screens[from_si].cur_tab_mut().unwrap();
                        swap_leaves(&mut tab.root, from, other);
                        // Payloads swapped in place: the moved content now lives
                        // in `other`, so focus follows the window the user moved.
                        tab.focused = other;
                    } else {
                        // Different monitors: swap the two panes' payloads across
                        // tabs. Read both before mutating.
                        let from_ti = model.screens[from_si].current;
                        let to_ti = model.screens[to_si].current;
                        let from_payload =
                            leaf_pane(&model.screens[from_si].tabs[from_ti].root, from).unwrap();
                        let other_payload =
                            leaf_pane(&model.screens[to_si].tabs[to_ti].root, other).unwrap();
                        set_leaf_pane(&mut model.screens[from_si].tabs[from_ti].root, from, other_payload);
                        set_leaf_pane(&mut model.screens[to_si].tabs[to_ti].root, other, from_payload);
                        model.focused_screen = to_si;
                        model.screens[to_si].tabs[to_ti].focused = other;
                    }
                    let effects = render_and_focus(&model);
                    (model, effects)
                }
                None => (model, vec![]),
            }
        }
        Command::MoveTab(offset) => {
            let fsi = model.focused_screen;
            let Some(screen) = model.screens.get_mut(fsi) else {
                return (model, vec![]);
            };
            let n = screen.tabs.len() as isize;
            if n == 0 {
                return (model, vec![]);
            }
            let cur = screen.current as isize;
            let want = cur + offset;
            if want >= 0 && want < n {
                // Reorder within the screen: no window changes monitor, so no render.
                let j = want as usize;
                let tab = screen.tabs.remove(cur as usize);
                screen.tabs.insert(j, tab);
                screen.current = j;
                return (model, vec![]);
            }
            // Past the screen's edge → carry the tab onto the adjacent screen in
            // bar order. Its window physically relocates, so render. No neighbor
            // in that direction → clamp (no-op).
            let dir = if offset > 0 { 1isize } else { -1 };
            let target = fsi as isize + dir;
            if target < 0 || target >= model.screens.len() as isize {
                return (model, vec![]);
            }
            let target = target as usize;
            let tab = model.screens[fsi].tabs.remove(cur as usize);
            let src_len = model.screens[fsi].tabs.len();
            model.screens[fsi].current = model.screens[fsi].current.min(src_len.saturating_sub(1));
            // Insert at the near edge: moving right lands at the target's front.
            let at = if dir > 0 { 0 } else { model.screens[target].tabs.len() };
            model.screens[target].tabs.insert(at, tab);
            model.focused_screen = target;
            model.screens[target].current = at;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::MoveTabToScreen(dir) => {
            let n = model.screens.len() as isize;
            let fsi = model.focused_screen;
            if n < 2 || model.screens[fsi].tabs.is_empty() {
                return (model, vec![]);
            }
            let target = (fsi as isize + dir).rem_euclid(n) as usize;
            let cur = model.screens[fsi].current;
            let tab = model.screens[fsi].tabs.remove(cur);
            let src_len = model.screens[fsi].tabs.len();
            model.screens[fsi].current = cur.min(src_len.saturating_sub(1));
            // Insert at the near edge: moving right lands at the target's front.
            let at = if dir > 0 { 0 } else { model.screens[target].tabs.len() };
            model.screens[target].tabs.insert(at, tab);
            model.focused_screen = target;
            model.screens[target].current = at;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::SetTabName(name) => {
            if let Some(tab) = model.focused_tab_mut() {
                // `None`/empty clears the override (title label returns); a
                // whitespace-only name is kept so the bar renders icon-only.
                tab.name = name.filter(|s| !s.is_empty());
            }
            (model, vec![])
        }
        Command::Resize(dir) => {
            let (axis, delta) = match dir {
                Direction::Right => (Dir::Horizontal, RESIZE_STEP),
                Direction::Left => (Dir::Horizontal, -RESIZE_STEP),
                Direction::Down => (Dir::Vertical, RESIZE_STEP),
                Direction::Up => (Dir::Vertical, -RESIZE_STEP),
            };
            let Some(from) = model.focused_tab().map(|t| t.focused) else {
                return (model, vec![]);
            };
            let old_root = model.focused_tab().unwrap().root.clone();
            let Some(root) = resize_focus(old_root, from, axis, delta) else {
                return (model, vec![]);
            };
            if let Some(tab) = model.focused_tab_mut() {
                tab.root = root;
            }
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::ToggleZoom => {
            model.zoomed = !model.zoomed;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::SyncFocus(id) => {
            if let Some((si, ti, pid)) = locate_window(&model, id) {
                model.focused_screen = si;
                model.screens[si].current = ti;
                model.screens[si].tabs[ti].focused = pid;
            }
            (model, vec![])
        }
        Command::Raise(id) => match locate_window(&model, id) {
            Some((si, ti, pid)) => {
                model.focused_screen = si;
                model.screens[si].current = ti;
                // If the window lives in a stack, reveal it (select its item).
                select_stack_window(&mut model.screens[si].tabs[ti].root, id);
                model.screens[si].tabs[ti].focused = pid;
                // May switch tabs, so Render to bring a parked window on-screen.
                let effects = render_and_focus(&model);
                (model, effects)
            }
            None => (model, vec![]),
        },
        Command::FillPane(id) => {
            // Reject only if `id` already lives in the focused pane's own node
            // (moving it there is a no-op); a window elsewhere in the same tab
            // (another split pane) is fair game.
            let in_focused_pane = model
                .focused_tab()
                .map(|t| find_window(&t.root, id) == Some(t.focused))
                .unwrap_or(false);
            if !model.focused_pane_is_empty() || in_focused_pane {
                return (model, vec![]);
            }
            let fsi = model.focused_screen;
            let pane_id = model.focused_tab().unwrap().focused;
            // Remove `id` from every tab across all screens; drop emptied tabs,
            // shifting each screen's `current` left by one for a dropped tab
            // before it. `id` isn't in the focused PANE (guarded), so `pane_id`
            // survives — even if removing it collapses a sibling split.
            for screen in model.screens.iter_mut() {
                let mut i = 0;
                while i < screen.tabs.len() {
                    if find_window(&screen.tabs[i].root, id).is_none() {
                        i += 1;
                        continue;
                    }
                    match remove_leaf_with_window(screen.tabs[i].root.clone(), id) {
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
                                screen.current -= 1;
                            }
                        }
                    }
                }
            }
            let cur = model.screens[fsi].current;
            set_leaf_pane(&mut model.screens[fsi].tabs[cur].root, pane_id, Pane::Window(id));
            model.screens[fsi].tabs[cur].focused = pane_id;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::CloseFocusedPane => {
            if !model.focused_pane_is_empty() {
                return (model, vec![]);
            }
            let fsi = model.focused_screen;
            let cur = model.screens[fsi].current;
            let focused = model.screens[fsi].tabs[cur].focused;
            // A focused stack drops only its selected empty item; a plain empty
            // leaf is removed whole. Both collapse the parent split if emptied.
            match remove_selected_pane(model.screens[fsi].tabs[cur].root.clone(), focused) {
                Some(root) => {
                    let tab = &mut model.screens[fsi].tabs[cur];
                    tab.root = root;
                    // A cancelled stackify collapses the stack back to a leaf with
                    // the SAME pane id, so keep focus there. Only when the focused
                    // pane vanished (an empty leaf removed from a split) fall back
                    // to a window leaf.
                    if leaf_pane(&tab.root, tab.focused).is_none() {
                        let ls = leaves(&tab.root);
                        tab.focused = ls
                            .iter()
                            .find(|(_, p)| matches!(p, Pane::Window(_)))
                            .map(|(id, _)| *id)
                            .unwrap_or(ls[0].0);
                    }
                }
                None => {
                    let screen = &mut model.screens[fsi];
                    screen.tabs.remove(cur);
                    screen.current = screen.current.min(screen.tabs.len().saturating_sub(1));
                }
            }
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::BreakPane => {
            let fsi = model.focused_screen;
            let cur = model.screens[fsi].current;
            let root = model.screens[fsi].tabs[cur].root.clone();
            // Meaningful when the tab holds more than one window (a split, or a
            // stack with >1 item); a single-window tab has nothing to break out.
            if windows(&root).len() < 2 {
                return (model, vec![]);
            }
            let focused = model.screens[fsi].tabs[cur].focused;
            let pane = leaf_pane(&root, focused);
            // Break out only the focused pane's SELECTED item: for a stack that
            // drops the one window and collapses the stack to a leaf in place
            // (the rest stay put), not the whole stack. The >1-window guard
            // guarantees something remains, so this is Some.
            let new_root = remove_selected_pane(root, focused).expect("a window remains");
            {
                let tab = &mut model.screens[fsi].tabs[cur];
                tab.root = new_root;
                let ls = leaves(&tab.root);
                tab.focused = ls
                    .iter()
                    .find(|(_, p)| matches!(p, Pane::Window(_)))
                    .map(|(id, _)| *id)
                    .unwrap_or(ls[0].0);
            }
            // A window pane pops out to its own new tab on the focused screen; an
            // empty pane just vanishes. Focus stays on the current tab. A custom
            // nested-tab name (⌥e t) carries over as the new tab's name.
            if let Some(Pane::Window(w)) = pane {
                let name = model.stack_names.remove(&w);
                let pid = model.next_id();
                model.screens[fsi].tabs.push(Tab {
                    root: Node::Leaf { id: pid, pane: Pane::Window(w) },
                    focused: pid,
                    name,
                });
            }
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::Stackify => {
            let Some(tab) = model.focused_tab() else {
                return (model, vec![]);
            };
            let focused = tab.focused;
            let Some(root) = stackify(tab.root.clone(), focused) else {
                return (model, vec![]);
            };
            let tab = model.focused_tab_mut().unwrap();
            tab.root = root;
            // Focus stays on the stack's PaneId (unchanged by stackify).
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::StackCycle(delta) => {
            let Some(tab) = model.focused_tab() else {
                return (model, vec![]);
            };
            let focused = tab.focused;
            let Some(root) = stack_cycle(tab.root.clone(), focused, delta) else {
                return (model, vec![]);
            };
            model.focused_tab_mut().unwrap().root = root;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::StackSelect(n) => {
            let Some(tab) = model.focused_tab() else {
                return (model, vec![]);
            };
            let focused = tab.focused;
            let Some(root) = stack_select(tab.root.clone(), focused, n) else {
                return (model, vec![]);
            };
            model.focused_tab_mut().unwrap().root = root;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::StackMove(delta) => {
            let Some(tab) = model.focused_tab() else {
                return (model, vec![]);
            };
            let focused = tab.focused;
            let Some(root) = stack_move(tab.root.clone(), focused, delta) else {
                return (model, vec![]);
            };
            model.focused_tab_mut().unwrap().root = root;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        Command::SetStackName(name) => {
            let Some(wid) = model.focused_stack_window() else {
                return (model, vec![]);
            };
            match name {
                Some(n) => {
                    model.stack_names.insert(wid, n);
                }
                None => {
                    model.stack_names.remove(&wid);
                }
            }
            (model, vec![])
        }
        Command::SelectStackWindow(wid) => {
            // The clicked stack is in some screen's CURRENT tab; find it, select
            // the item, and move focus onto that stack.
            for si in 0..model.screens.len() {
                let Some(tab) = model.screens[si].cur_tab_mut() else { continue };
                if let Some(pid) = select_stack_window(&mut tab.root, wid) {
                    tab.focused = pid;
                    model.focused_screen = si;
                    let effects = render_and_focus(&model);
                    return (model, effects);
                }
            }
            (model, vec![])
        }
    }
}

/// Advance by `delta` through the GLOBAL flat tab order (screen 0's tabs, then
/// screen 1's, …, matching the tab bar), wrapping. Crossing a screen boundary
/// moves focus to that screen and makes the landed tab its current one.
fn cycle_tab(mut model: Model, delta: isize) -> (Model, Vec<Effect>) {
    let counts: Vec<usize> = model.screens.iter().map(|s| s.tabs.len()).collect();
    let total: isize = counts.iter().sum::<usize>() as isize;
    if total == 0 {
        return (model, vec![]);
    }
    // Current flat index = tabs on earlier screens + the focused screen's current.
    let flat: isize = counts[..model.focused_screen].iter().sum::<usize>() as isize
        + model.screens[model.focused_screen].current as isize;
    let next = ((flat + delta) % total + total) % total;
    // Map back to (screen, tab-within-screen).
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

/// Nudge the ratio of the innermost split (matching `axis`) whose child subtree
/// contains `focused`. Recurses children first so the deepest match wins; clamps
/// so no ratio drops below `MIN_RATIO`. `None` if nothing matched.
fn resize_focus(node: Node, focused: PaneId, axis: Dir, delta: f64) -> Option<Node> {
    match node {
        Node::Leaf { .. } => None,
        Node::Stack { .. } => None,
        Node::Split { dir, ratios, children } => {
            if let Some(children) =
                rebuild_children(children.clone(), |c| resize_focus(c, focused, axis, delta))
            {
                return Some(Node::Split { dir, ratios, children });
            }
            if dir != axis {
                return None;
            }
            let i = children.iter().position(|c| subtree_contains(c, focused))?;
            let len = children.len();
            // Move the divider on the arrow's side of `focused` in the arrow's
            // screen direction (delta > 0 = right/down): the child left/above the
            // divider grows, the one right/below shrinks — matching tmux
            // resize-pane, so the effect is the same whichever pane is focused.
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
