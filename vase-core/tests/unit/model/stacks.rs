use super::*;

fn stack(items: Vec<Pane>, selected: usize) -> Node {
    Node::Stack { id: PaneId(0), items, selected }
}

#[test]
fn stackify_wraps_a_window_leaf_with_an_empty_selected() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify);
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 1));
    assert_eq!(m.screens[0].tabs[0].focused, PaneId(0));
    assert!(m.focused_pane_is_empty());
}

#[test]
fn stackify_a_stack_pushes_another_empty() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify);
    let (m, _) = apply(m, Command::Stackify);
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty, Pane::Empty], 2));
}

#[test]
fn stack_cycle_wraps() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [W, E] selected=1
    let (m, _) = apply(m, Command::StackCycle(1)); // -> 0
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 0));
    let (m, _) = apply(m, Command::StackCycle(1)); // wraps -> 1
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Empty], 1));
}

#[test]
fn fill_pane_sets_the_selected_stack_item() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify); // tab0 -> [W1, E] sel 1
    let (m, _) = apply(m, Command::FillPane(win(2)));
    assert_eq!(m.screens[0].tabs.len(), 1); // win(2)'s old tab emptied and dropped
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 1));
    assert_eq!(m.focused_window(), Some(win(2)));
}

#[test]
fn closing_a_stack_down_to_one_collapses_to_a_leaf() {
    let (m, _) = apply(one(&[win(1)]), Command::Stackify); // [W1, E] sel 1 (empty selected)
    let (m, _) = apply(m, Command::CloseFocusedPane);
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
}

#[test]
fn raise_reveals_a_hidden_stack_member() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // [W1, W2] sel 1 (W2 shown)
    let (m, _) = apply(m, Command::Raise(win(1))); // raising the hidden member
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 0));
    assert_eq!(m.focused_window(), Some(win(1)));
}

#[test]
fn stack_select_and_move_and_name() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // [W1, W2] sel 1
                                                      // Select the 1st window item.
    let (m, _) = apply(m, Command::StackSelect(1));
    assert!(matches!(m.screens[0].tabs[0].root, Node::Stack { selected: 0, .. }));
    // Name the selected item, then reorder it down.
    let (m, _) = apply(m, Command::SetStackName(Some("editor".into())));
    assert_eq!(m.names.get(&win(1)), Some(&"editor".to_string()));
    let (m, _) = apply(m, Command::StackMove(1)); // W1 moves to index 1, stays selected
    assert_eq!(m.screens[0].tabs[0].root, stack(vec![Pane::Window(win(2)), Pane::Window(win(1))], 1));
}

#[test]
fn placements_place_only_the_selected_stack_item() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // stack [W1, W2] sel 1
    let content = Rect::new(0.0, STACK_BAR_H, 100.0, 100.0 - STACK_BAR_H);
    assert_eq!(m.placements(), vec![(win(2), content)]);
}

#[test]
fn stacks_reports_the_stack_rect_and_items() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // stack [W1, W2] sel 1
    assert_eq!(m.stacks(), vec![StackBar { rect: SCREEN, items: vec![win(1), win(2)], selected: 1, focused: true }]);
}
