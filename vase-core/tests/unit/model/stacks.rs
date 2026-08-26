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
    let content = Rect::new(0.0, bar_height(), 100.0, 100.0 - bar_height());
    assert_eq!(m.placements(), vec![(win(2), content)]);
}

/// A split whose left pane is a stack of two windows, focused on the stack.
fn split_with_a_stack() -> Model {
    let mut m = h_split(false);
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Stack { id: PaneId(0), items: vec![Pane::Window(win(1)), Pane::Window(win(3))], selected: 0 }, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) }],
    };
    m
}

#[test]
fn a_zoomed_stack_bar_spans_the_screen_the_stack_now_fills() {
    let mut m = split_with_a_stack();
    assert_eq!(m.stacks()[0].rect, Rect::new(0.0, 0.0, 50.0, 100.0), "unzoomed it sits in its half");
    m.zoomed = true;
    // Zoomed, the stack covers the screen, so its bar has to span the screen too rather than hang
    // over the middle of the window.
    assert_eq!(m.stacks(), vec![StackBar { rect: SCREEN, items: vec![win(1), win(3)], selected: 0, focused: true }]);
}

#[test]
fn zooming_a_pane_hides_the_bars_of_the_stacks_it_covers() {
    let mut m = split_with_a_stack();
    m.screens[0].tabs[0].focused = PaneId(1); // the plain window, not the stack
    m.zoomed = true;
    assert!(m.stacks().is_empty(), "the stack is behind the zoomed window");
}

#[test]
fn stacks_reports_the_stack_rect_and_items() {
    let (m, _) = apply(one(&[win(1), win(2)]), Command::Stackify);
    let (m, _) = apply(m, Command::FillPane(win(2))); // stack [W1, W2] sel 1
    assert_eq!(m.stacks(), vec![StackBar { rect: SCREEN, items: vec![win(1), win(2)], selected: 1, focused: true }]);
}

#[test]
fn a_resync_places_a_stack_s_occluded_items_behind_the_selected_one() {
    // Two windows in one stack: one selected and on screen, one behind it.
    let mut m = one(&[win(1), win(2)]);
    m.screens[0].tabs[0].root = stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 0);
    m.screens[0].tabs.truncate(1);

    let content = Rect::new(SCREEN.x, SCREEN.y + bar_height(), SCREEN.w, SCREEN.h - bar_height());
    assert_eq!(m.placements(), vec![(win(1), content)], "only the selected item is on screen");

    // Both share the one rect, so the occluded one is put back exactly behind the selected one.
    let all = m.all_placements();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&(win(1), content)));
    assert!(all.contains(&(win(2), content)), "the item behind gets the same rect, not its stale frame");
}

#[test]
fn a_zoomed_stack_leaves_its_bar_the_strip_it_sits_on() {
    // A stack sharing a split with a plain pane, the stack focused and zoomed.
    let mut m = one(&[win(1), win(2)]);
    m.screens[0].tabs.truncate(1);
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![stack(vec![Pane::Window(win(1)), Pane::Window(win(2))], 0), Node::Leaf { id: PaneId(9), pane: Pane::Window(win(3)) }],
    };
    m.screens[0].tabs[0].focused = PaneId(0);
    m.zoomed = true;

    // The bar spans the screen, so the window starts below it rather than under it.
    let below = Rect::new(SCREEN.x, SCREEN.y + bar_height(), SCREEN.w, SCREEN.h - bar_height());
    assert_eq!(m.placements(), vec![(win(1), below)]);
    assert_eq!(m.focused_pane_rect(), Some(below), "the focus border traces what the window actually fills");
    let bars = m.stacks();
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].rect, SCREEN, "the zoomed stack's bar stretches across the screen");

    // A plain pane has no bar of its own, so zoomed it owns the whole screen.
    m.screens[0].tabs[0].focused = PaneId(9);
    assert_eq!(m.placements(), vec![(win(3), SCREEN)]);
    assert_eq!(m.focused_pane_rect(), Some(SCREEN));
    assert!(m.stacks().is_empty(), "the stack behind a zoomed pane draws no bar");
}
