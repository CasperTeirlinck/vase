use super::{render_and_focus, resize_focus, RESIZE_STEP};
use crate::focus::Direction;
use crate::model::{Effect, Model, Tab};
use crate::tree::{find_window, leaf_pane, leaves, remove_leaf_with_window, remove_selected_pane, set_leaf_pane, split_pane, windows, Dir, Node, Pane, WindowId};

pub(super) fn split(mut model: Model, dir: Dir) -> (Model, Vec<Effect>) {
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
    // New pane is empty, so render_and_focus adds no FocusWindow.
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn resize(mut model: Model, dir: Direction) -> (Model, Vec<Effect>) {
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

pub(super) fn toggle_zoom(mut model: Model) -> (Model, Vec<Effect>) {
    model.zoomed = !model.zoomed;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn fill_pane(mut model: Model, id: WindowId) -> (Model, Vec<Effect>) {
    // No-op only if `id` is already in the focused pane; a window elsewhere in the same tab is fair game.
    let in_focused_pane = model.focused_tab().map(|t| find_window(&t.root, id) == Some(t.focused)).unwrap_or(false);
    if !model.focused_pane_is_empty() || in_focused_pane {
        return (model, vec![]);
    }
    let fsi = model.focused_screen;
    let pane_id = model.focused_tab().unwrap().focused;
    // Remove `id` from every tab, dropping emptied ones and shifting `current` left for a dropped tab before it.
    // `pane_id` is guarded to survive (see above), even if a sibling split collapses.
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

pub(super) fn close_focused_pane(mut model: Model) -> (Model, Vec<Effect>) {
    if !model.focused_pane_is_empty() {
        return (model, vec![]);
    }
    let fsi = model.focused_screen;
    let cur = model.screens[fsi].current;
    let focused = model.screens[fsi].tabs[cur].focused;
    // A focused stack drops only its selected empty item; a plain empty leaf is removed whole. Both collapse the parent split if emptied.
    match remove_selected_pane(model.screens[fsi].tabs[cur].root.clone(), focused) {
        Some(root) => {
            let tab = &mut model.screens[fsi].tabs[cur];
            tab.root = root;
            // A cancelled stackify collapses the stack to a leaf with the same pane id, so keep focus there; fall back only when the focused pane vanished.
            if leaf_pane(&tab.root, tab.focused).is_none() {
                let ls = leaves(&tab.root);
                tab.focused = ls.iter().find(|(_, p)| matches!(p, Pane::Window(_))).map(|(id, _)| *id).unwrap_or(ls[0].0);
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

pub(super) fn break_pane(mut model: Model) -> (Model, Vec<Effect>) {
    let fsi = model.focused_screen;
    let cur = model.screens[fsi].current;
    let root = model.screens[fsi].tabs[cur].root.clone();
    // Nothing to break out of a single-window tab.
    if windows(&root).len() < 2 {
        return (model, vec![]);
    }
    let focused = model.screens[fsi].tabs[cur].focused;
    let pane = leaf_pane(&root, focused);
    // Break out only the focused pane's selected item, not the whole stack. The >1-window guard guarantees something remains.
    let new_root = remove_selected_pane(root, focused).unwrap();
    {
        let tab = &mut model.screens[fsi].tabs[cur];
        tab.root = new_root;
        let ls = leaves(&tab.root);
        tab.focused = ls.iter().find(|(_, p)| matches!(p, Pane::Window(_))).map(|(id, _)| *id).unwrap_or(ls[0].0);
    }
    // A window pane pops out to its own new tab; an empty pane just vanishes. Its custom name lives in `names`,
    // keyed by the window, so it follows the window without any transfer here. Focus stays on the current tab.
    if let Some(Pane::Window(w)) = pane {
        let pid = model.next_id();
        model.screens[fsi].tabs.push(Tab { root: Node::Leaf { id: pid, pane: Pane::Window(w) }, focused: pid, name: None });
    }
    let effects = render_and_focus(&model);
    (model, effects)
}
