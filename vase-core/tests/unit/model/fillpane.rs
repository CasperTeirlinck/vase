use super::*;

#[test]
fn fill_pane_moves_a_window_from_another_tab_into_the_empty_pane() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    // Tab 0 is now [win1 | empty], empty focused; win2 lives in tab 1.
    let (m, effects) = apply(m, Command::FillPane(win(2)));
    assert_eq!(m.screens[0].tabs.len(), 1);
    assert_eq!(m.focused_window(), Some(win(2)));
    assert_eq!(m.placements(), vec![(win(1), Rect::new(0.0, 0.0, 50.0, 100.0)), (win(2), Rect::new(50.0, 0.0, 50.0, 100.0)),]);
    assert!(effects.contains(&Effect::FocusWindow(win(2))));
}

#[test]
fn fill_pane_is_noop_when_focused_pane_is_not_empty() {
    let m = three(); // focused pane holds win1
    let (m2, effects) = apply(m.clone(), Command::FillPane(win(2)));
    assert_eq!(m2, m);
    assert_eq!(effects, vec![]);
}

#[test]
fn fill_pane_moves_a_same_tab_window_from_another_pane() {
    // Tab 0 = [win1 | empty(focused)]; win1 is in the split's other pane.
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Split(Dir::Horizontal));
    let (m, _) = apply(m, Command::FillPane(win(1)));
    // win1 moves into the empty pane; the split collapses to just win1.
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(1)) });
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(m.screens[0].tabs.len(), 2); // tab 1 (win2) untouched
}

#[test]
fn fill_pane_is_noop_for_a_window_in_the_focused_pane() {
    // FillPane is a no-op when the target window already lives in the focused pane's node.
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [win1, empty] stack, empty selected
    let before = m.clone();
    // win1 is the stack's other item → in the focused pane's node → no-op.
    let (m2, effects) = apply(m, Command::FillPane(win(1)));
    assert_eq!(m2, before);
    assert_eq!(effects, vec![]);
}

#[test]
fn fill_pane_decrements_current_when_an_earlier_tab_is_dropped() {
    let m = three();
    let (m, _) = apply(m, Command::SelectTab(2));
    let (m, _) = apply(m, Command::Split(Dir::Horizontal)); // tab 2 = [win3 | empty]
    let (m, _) = apply(m, Command::FillPane(win(1))); // pulls win1 from tab 0
                                                      // Tab 0 dropped → current shifts 2 → 1, still the tab we split.
    assert_eq!(m.screens[0].tabs.len(), 2);
    assert_eq!(m.screens[0].current, 1);
    assert_eq!(m.focused_window(), Some(win(1)));
    assert_eq!(windows(&m.screens[0].tabs[1].root), vec![win(3), win(1)]);
}
