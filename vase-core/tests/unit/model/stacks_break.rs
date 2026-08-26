use super::*;

#[test]
fn break_pane_from_a_stack_in_a_split_keeps_the_rest_in_the_split() {
    use std::collections::HashMap;
    // A split whose right pane is a 2-window stack (win 3 selected).
    let root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) }, Node::Stack { id: PaneId(2), items: vec![Pane::Window(win(2)), Pane::Window(win(3))], selected: 1 }],
    };
    let m = Model {
        screens: vec![Screen { rect: SCREEN, tabs: vec![Tab { root, focused: PaneId(2), name: None, zoomed: false }], current: 0 }],
        focused_screen: 0,
        names: HashMap::new(),
        next_pane_id: 3,
    };
    let (m, _) = apply(m, Command::BreakPane);
    assert_eq!(m.screens[0].tabs.len(), 2);
    // Only the selected window (win 3) pops out to its own tab.
    assert_eq!(m.screens[0].tabs[1].root, Node::Leaf { id: PaneId(3), pane: Pane::Window(win(3)) });
    // The other stack window (win 2) stays in the split, now a plain leaf.
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) },],
        }
    );
}

#[test]
fn break_pane_from_a_plain_stack_tab_pops_the_item_and_keeps_its_name() {
    use std::collections::HashMap;
    // A whole-tab stack of two windows, win 2 selected and named "editor".
    let mut names = HashMap::new();
    names.insert(win(2), "editor".to_string());
    let m = Model {
        screens: vec![Screen {
            rect: SCREEN,
            tabs: vec![Tab { root: Node::Stack { id: PaneId(0), items: vec![Pane::Window(win(1)), Pane::Window(win(2))], selected: 1 }, focused: PaneId(0), name: None, zoomed: false }],
            current: 0,
        }],
        focused_screen: 0,
        names,
        next_pane_id: 1,
    };
    let (m, _) = apply(m, Command::BreakPane);
    assert_eq!(m.screens[0].tabs.len(), 2);
    // The stack collapses to its remaining window (win 1).
    assert_eq!(m.screens[0].tabs[0].root, Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) });
    // win 2 pops to its own tab and keeps its name, which stays keyed by the window (not moved onto the tab).
    assert_eq!(m.screens[0].tabs[1].root, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) });
    assert_eq!(m.screens[0].tabs[1].name, None);
    assert_eq!(m.names.get(&win(2)), Some(&"editor".to_string()));
}

#[test]
fn cancel_stackify_on_a_split_pane_keeps_focus_on_that_pane() {
    use std::collections::HashMap;
    // Split with win2 (the second pane) focused.
    let root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) }],
    };
    let m = Model {
        screens: vec![Screen { rect: SCREEN, tabs: vec![Tab { root, focused: PaneId(2), name: None, zoomed: false }], current: 0 }],
        focused_screen: 0,
        names: HashMap::new(),
        next_pane_id: 3,
    };
    let (m, _) = apply(m, Command::Stackify); // win2 → stack with an empty slot
    assert!(m.focused_pane_is_empty());
    let (m, _) = apply(m, Command::CloseFocusedPane); // cancel the stackify
                                                      // The stack collapses back to win2 in place; focus stays on it, not win1.
    assert_eq!(m.focused_window(), Some(win(2)));
    assert_eq!(
        m.screens[0].tabs[0].root,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![Node::Leaf { id: PaneId(1), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(2), pane: Pane::Window(win(2)) },],
        }
    );
}
