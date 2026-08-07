use super::*;

fn leaf(id: u64, pane: Pane) -> Node {
    Node::Leaf { id: PaneId(id), pane }
}

fn win(id: u64) -> Pane {
    Pane::Window(WindowId(id))
}

#[test]
fn windows_lists_only_window_leaves() {
    let tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, Pane::Empty)],
    };
    assert_eq!(windows(&tree), vec![WindowId(1)]);
}

#[test]
fn leaves_lists_every_leaf() {
    let tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, Pane::Empty)],
    };
    assert_eq!(leaves(&tree), vec![(PaneId(0), win(1)), (PaneId(1), Pane::Empty)]);
}

#[test]
fn find_window_returns_the_holding_pane() {
    let tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, win(2))],
    };
    assert_eq!(find_window(&tree, WindowId(2)), Some(PaneId(1)));
    assert_eq!(find_window(&tree, WindowId(9)), None);
}

#[test]
fn split_pane_replaces_the_leaf_with_a_split_of_it_and_a_new_empty() {
    let tree = leaf(0, win(1));
    let result = split_pane(tree, PaneId(0), Dir::Horizontal, PaneId(7)).unwrap();
    assert_eq!(
        result,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![leaf(0, win(1)), leaf(7, Pane::Empty)],
        }
    );
}

#[test]
fn split_pane_recurses_to_find_the_target() {
    let tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, win(2))],
    };
    let result = split_pane(tree, PaneId(1), Dir::Vertical, PaneId(7)).unwrap();
    assert_eq!(
        result,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![
                leaf(0, win(1)),
                Node::Split {
                    dir: Dir::Vertical,
                    ratios: vec![0.5, 0.5],
                    children: vec![leaf(1, win(2)), leaf(7, Pane::Empty)],
                },
            ],
        }
    );
}

#[test]
fn remove_leaf_collapses_a_split_to_its_survivor() {
    let tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, win(2))],
    };
    assert_eq!(remove_leaf_with_window(tree, WindowId(1)), Some(leaf(1, win(2))));
}

#[test]
fn remove_the_only_window_yields_none() {
    assert_eq!(remove_leaf_with_window(leaf(0, win(1)), WindowId(1)), None);
}

#[test]
fn swap_leaves_swaps_payloads_keeping_ids() {
    let mut tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![leaf(0, win(1)), leaf(1, Pane::Empty)],
    };
    swap_leaves(&mut tree, PaneId(0), PaneId(1));
    assert_eq!(
        tree,
        Node::Split {
            dir: Dir::Horizontal,
            ratios: vec![0.5, 0.5],
            children: vec![leaf(0, Pane::Empty), leaf(1, win(1))],
        }
    );
}

#[test]
fn select_stack_window_selects_and_returns_stack_id() {
    let mut tree = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![
            leaf(0, win(1)),
            Node::Stack { id: PaneId(9), items: vec![win(2), win(3), win(4)], selected: 0 },
        ],
    };
    assert_eq!(select_stack_window(&mut tree, WindowId(3)), Some(PaneId(9)));
    let Node::Split { children, .. } = &tree else { panic!() };
    assert!(matches!(children[1], Node::Stack { selected: 1, .. }));
    // A window not in any stack leaves the tree untouched.
    assert_eq!(select_stack_window(&mut tree, WindowId(1)), None);
}

#[test]
fn stack_select_picks_nth_window_skipping_empty() {
    let stack = Node::Stack {
        id: PaneId(5),
        items: vec![win(1), Pane::Empty, win(2), win(3)],
        selected: 0,
    };
    // 1-based over WINDOW items: 3rd window is win(3) at item index 3.
    let out = stack_select(stack.clone(), PaneId(5), 3).unwrap();
    assert!(matches!(out, Node::Stack { selected: 3, .. }));
    // Out of range leaves selection unchanged.
    let out = stack_select(stack, PaneId(5), 9).unwrap();
    assert!(matches!(out, Node::Stack { selected: 0, .. }));
}

#[test]
fn stack_move_reorders_and_clamps() {
    let stack =
        Node::Stack { id: PaneId(5), items: vec![win(1), win(2), win(3)], selected: 1 };
    let Node::Stack { items, selected, .. } = stack_move(stack.clone(), PaneId(5), 1).unwrap()
    else {
        panic!()
    };
    assert_eq!(items, vec![win(1), win(3), win(2)]);
    assert_eq!(selected, 2);
    // Clamp at the top edge: selected 2 moving +1 stays put.
    let at_end = Node::Stack { id: PaneId(5), items: vec![win(1), win(2)], selected: 1 };
    let Node::Stack { items, selected, .. } = stack_move(at_end, PaneId(5), 1).unwrap() else {
        panic!()
    };
    assert_eq!((items, selected), (vec![win(1), win(2)], 1));
}
