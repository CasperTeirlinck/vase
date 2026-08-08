use super::*;

#[test]
fn remove_window_collapses_a_split_and_keeps_the_survivor() {
    let m = h_split(false);
    let (m, effects) = apply(m, Command::RemoveWindow(win(1)));
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) });
    // Focus was on the removed pane → moves to the survivor.
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn remove_window_drops_an_emptied_tab_and_clamps_current() {
    let m = three();
    let (m, _) = apply(m, Command::SelectTab(2));
    let (m, effects) = apply(m, Command::RemoveWindow(win(3)));
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn remove_an_unknown_window_is_a_noop() {
    let m = three();
    let (m2, effects) = apply(m.clone(), Command::RemoveWindow(win(99)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn close_focused_pane_collapses_the_split() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    // Tab 0 = [win1 | empty], empty focused.
    let (m, effects) = apply(m, Command::CloseFocusedPane);
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(m.placements(), vec![(win(1), SCREEN)]);
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn close_focused_pane_is_noop_on_a_window_pane() {
    let m = three(); // focused pane holds win1
    let (m2, effects) = apply(m.clone(), Command::CloseFocusedPane);
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn break_pane_pops_the_window_to_its_own_tab_and_collapses_the_split() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::FillPane(win(2))); // tab 0 = [win1 | win2], win2 focused
    assert_eq!(m.screens[0].tabs.len(), 1);
    let (m, effects) = apply(m, Command::BreakPane);
    // Current tab collapses back to win1; win2 becomes its own new tab.
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 0);
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(windows(&m.screens[0].tabs[0].root), vec![win(1)]);
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(2)]);
    assert!(effects.contains(&Effect::FocusWindow(win(1))));
}

#[test]
fn break_pane_is_noop_on_a_single_pane_tab() {
    let m = three(); // each tab is a single window pane
    let (m2, effects) = apply(m.clone(), Command::BreakPane);
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}
