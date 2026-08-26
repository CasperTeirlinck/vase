use super::render_and_focus;
use crate::model::{Effect, Model, Tab};
use crate::tree::{windows, Pane};

pub(super) fn new_tab(mut model: Model) -> (Model, Vec<Effect>) {
    let id = model.next_id();
    let Some(screen) = model.fs_mut() else {
        return (model, vec![]);
    };
    screen.tabs.push(Tab::single(id, Pane::Empty));
    screen.current = screen.tabs.len() - 1;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn select_tab(mut model: Model, i: usize) -> (Model, Vec<Effect>) {
    let Some(screen) = model.fs_mut() else {
        return (model, vec![]);
    };
    if i < screen.tabs.len() {
        screen.current = i;
        // Render, not just focus: the target tab's windows are occluded and must be re-placed and raised.
        let effects = render_and_focus(&model);
        (model, effects)
    } else {
        (model, vec![])
    }
}

pub(super) fn select_screen_tab(mut model: Model, si: usize, ti: usize) -> (Model, Vec<Effect>) {
    match model.screens.get_mut(si) {
        Some(screen) if ti < screen.tabs.len() => {
            screen.current = ti;
            model.focused_screen = si;
            let effects = render_and_focus(&model);
            (model, effects)
        }
        _ => (model, vec![]),
    }
}

pub(super) fn move_tab(mut model: Model, offset: isize) -> (Model, Vec<Effect>) {
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
    // Past the edge: carry the tab onto the adjacent screen (its window relocates, so render); clamp if there's no neighbor.
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

pub(super) fn move_tab_to_screen(mut model: Model, dir: isize) -> (Model, Vec<Effect>) {
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

pub(super) fn set_tab_name(mut model: Model, name: Option<String>) -> (Model, Vec<Effect>) {
    // Empty clears the override; a whitespace-only name is kept.
    let name = name.filter(|s| !s.is_empty());
    let Some(tab) = model.focused_tab() else {
        return (model, vec![]);
    };
    let ws = windows(&tab.root);
    if ws.len() == 1 {
        // Naming a single-window tab names the window, so the name follows it across splits, moves, and stacks.
        let w = ws[0];
        model.focused_tab_mut().unwrap().name = None; // drop any stale group name; the per-window name wins
        match name {
            Some(n) => drop(model.names.insert(w, n)),
            None => drop(model.names.remove(&w)),
        }
    } else {
        model.focused_tab_mut().unwrap().name = name;
    }
    (model, vec![])
}
