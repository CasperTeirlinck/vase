use super::render_and_focus;
use crate::model::{Effect, Model};
use crate::tree::{self, WindowId};

pub(super) fn stackify(mut model: Model) -> (Model, Vec<Effect>) {
    let Some(tab) = model.focused_tab() else {
        return (model, vec![]);
    };
    let focused = tab.focused;
    let Some(root) = tree::stackify(tab.root.clone(), focused) else {
        return (model, vec![]);
    };
    let tab = model.focused_tab_mut().unwrap();
    tab.root = root;
    // Focus stays on the stack's PaneId (unchanged by stackify).
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn stack_cycle(mut model: Model, delta: isize) -> (Model, Vec<Effect>) {
    let Some(tab) = model.focused_tab() else {
        return (model, vec![]);
    };
    let focused = tab.focused;
    let Some(root) = tree::stack_cycle(tab.root.clone(), focused, delta) else {
        return (model, vec![]);
    };
    model.focused_tab_mut().unwrap().root = root;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn stack_select(mut model: Model, n: usize) -> (Model, Vec<Effect>) {
    let Some(tab) = model.focused_tab() else {
        return (model, vec![]);
    };
    let focused = tab.focused;
    let Some(root) = tree::stack_select(tab.root.clone(), focused, n) else {
        return (model, vec![]);
    };
    model.focused_tab_mut().unwrap().root = root;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn stack_move(mut model: Model, delta: isize) -> (Model, Vec<Effect>) {
    let Some(tab) = model.focused_tab() else {
        return (model, vec![]);
    };
    let focused = tab.focused;
    let Some(root) = tree::stack_move(tab.root.clone(), focused, delta) else {
        return (model, vec![]);
    };
    model.focused_tab_mut().unwrap().root = root;
    let effects = render_and_focus(&model);
    (model, effects)
}

pub(super) fn set_stack_name(mut model: Model, name: Option<String>) -> (Model, Vec<Effect>) {
    let Some(wid) = model.focused_stack_window() else {
        return (model, vec![]);
    };
    match name {
        Some(n) => {
            model.names.insert(wid, n);
        }
        None => {
            model.names.remove(&wid);
        }
    }
    (model, vec![])
}

pub(super) fn select_stack_window(mut model: Model, wid: WindowId) -> (Model, Vec<Effect>) {
    // The clicked stack is in some screen's current tab.
    for si in 0..model.screens.len() {
        let Some(tab) = model.screens[si].current_tab_mut() else {
            continue;
        };
        if let Some(pid) = tree::select_stack_window(&mut tab.root, wid) {
            tab.focused = pid;
            model.focused_screen = si;
            let effects = render_and_focus(&model);
            return (model, effects);
        }
    }
    (model, vec![])
}
