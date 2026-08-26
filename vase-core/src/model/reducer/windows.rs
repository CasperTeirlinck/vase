use super::{locate_window, render_and_focus};
use crate::focus::{neighbor, Direction};
use crate::model::{Effect, Model, Tab};
use crate::tree::{leaf_pane, leaves, remove_leaf_with_window, select_stack_window, set_leaf_pane, swap_leaves, Pane, WindowId};

pub(super) fn add_window(mut model: Model, id: WindowId, si: usize) -> (Model, Vec<Effect>) {
    let pid = model.next_id();
    let screen = &mut model.screens[si];
    screen.tabs.push(Tab::single(pid, Pane::Window(id)));
    screen.current = screen.tabs.len() - 1;
    model.focused_screen = si;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn remove_window(mut model: Model, id: WindowId) -> (Model, Vec<Effect>) {
    model.names.remove(&id);
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

pub(super) fn focus(mut model: Model, dir: Direction) -> (Model, Vec<Effect>) {
    let targets = model.leaf_targets();
    let Some(from) = model.focused_tab().map(|t| t.focused) else {
        return (model, vec![]);
    };
    match neighbor(&targets, from, dir) {
        Some(next) => {
            if let Some(si) = model.screen_of_current_pane(next) {
                model.focused_screen = si;
                if let Some(tab) = model.screens[si].current_tab_mut() {
                    tab.focused = next;
                }
            }
            let effects = model.focused_window().map(Effect::FocusWindow).into_iter().collect();
            (model, effects)
        }
        None => (model, vec![]),
    }
}

pub(super) fn move_window(mut model: Model, dir: Direction) -> (Model, Vec<Effect>) {
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
                let tab = model.screens[from_si].current_tab_mut().unwrap();
                swap_leaves(&mut tab.root, from, other);
                // Payloads swapped in place, so focus follows the moved window to `other`.
                tab.focused = other;
            } else {
                // Different monitors: swap payloads across tabs; read both before mutating.
                let from_ti = model.screens[from_si].current;
                let to_ti = model.screens[to_si].current;
                let from_payload = leaf_pane(&model.screens[from_si].tabs[from_ti].root, from).unwrap();
                let other_payload = leaf_pane(&model.screens[to_si].tabs[to_ti].root, other).unwrap();
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

pub(super) fn sync_focus(mut model: Model, id: WindowId) -> (Model, Vec<Effect>) {
    let Some((si, ti, pid)) = locate_window(&model, id) else {
        return (model, vec![]);
    };
    model.focused_screen = si;
    model.screens[si].current = ti;
    model.screens[si].tabs[ti].focused = pid;
    // The OS already fronted `id`; the effect is what brings the rest of its tab up with it, so a click
    // on one pane of a split never leaves the others buried.
    (model, vec![Effect::FocusWindow(id)])
}

pub(super) fn raise(mut model: Model, id: WindowId) -> (Model, Vec<Effect>) {
    match locate_window(&model, id) {
        Some((si, ti, pid)) => {
            model.focused_screen = si;
            model.screens[si].current = ti;
            // If the window lives in a stack, reveal it (select its item).
            select_stack_window(&mut model.screens[si].tabs[ti].root, id);
            model.screens[si].tabs[ti].focused = pid;
            // May switch tabs, so Render to bring an occluded window back to the front.
            let effects = render_and_focus(&model);
            (model, effects)
        }
        None => (model, vec![]),
    }
}
