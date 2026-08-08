use super::*;

#[test]
fn focus_moves_onto_an_empty_pane_without_focuswindow() {
    let (m, _) = apply(three(), Command::Split(Dir::Horizontal));
    let (m, effects) = apply(m, Command::Focus(Direction::Left));
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(effects, vec![Effect::FocusWindow(win(1))]);
    let (m, effects) = apply(m, Command::Focus(Direction::Right));
    assert_eq!(m.focused_window(), None);
    assert_eq!(effects, vec![]);
}

#[test]
fn focus_at_the_edge_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::Focus(Direction::Left));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn move_window_swaps_panes_and_focus_follows_the_window() {
    let (m, _) = apply(one(&[win(1)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::Focus(Direction::Left)); // focus the window pane
    let (m, effects) = apply(m, Command::MoveWindow(Direction::Right));
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(1));
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split { dir: Dir::Horizontal, ratios: vec![0.5, 0.5], children: vec![Node::Leaf { id: PaneId(0), pane: Pane::Empty }, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) },] }
    );
    assert!(effects.contains(&Effect::Render(vec![(win(1), Rect::new(50.0, 0.0, 50.0, 100.0))])));
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn move_window_at_the_edge_is_a_noop() {
    let (m, _) = apply(one(&[win(1)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::Focus(Direction::Left));
    let (m2, effects) = apply(m.clone(), Command::MoveWindow(Direction::Left));
    assert_eq!(m2.screens[0].tabs[0].root, m.screens[0].tabs[0].root);
    assert_eq!(effects, vec![]);
}

#[test]
fn sync_focus_locates_the_tab_and_pane_across_tabs() {
    let (m, effects) = apply(three(), Command::SyncFocus(win(3)));
    assert_eq!(m.screens[0].current, 2);
    assert_eq!(m.focused_window(), Some(win(3)));
    assert_eq!(effects, vec![]);
}

#[test]
fn raise_locates_across_tabs_and_focuses() {
    let (m, effects) = apply(three(), Command::Raise(win(2)));
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(matches!(effects.first(), Some(Effect::Render(_))));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn raise_a_gone_window_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::Raise(win(99)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}
