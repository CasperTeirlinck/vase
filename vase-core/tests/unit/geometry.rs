use crate::geometry::*;
use crate::tree::{Dir, Node, Pane, PaneId, WindowId};

fn leaf(id: u64, pane: Pane) -> Node {
    Node::Leaf { id: PaneId(id), pane }
}

#[test]
fn single_leaf_fills_the_area() {
    let tree = leaf(0, Pane::Window(WindowId(1)));
    let mut out = Vec::new();
    layout(&tree, Rect::new(0.0, 0.0, 100.0, 80.0), &mut out);
    assert_eq!(out, vec![(PaneId(0), Pane::Window(WindowId(1)), Rect::new(0.0, 0.0, 100.0, 80.0))]);
}

#[test]
fn horizontal_split_divides_width_by_ratios() {
    let tree = Node::Split { dir: Dir::Horizontal, ratios: vec![0.25, 0.75], children: vec![leaf(0, Pane::Window(WindowId(1))), leaf(1, Pane::Empty)] };
    let mut out = Vec::new();
    layout(&tree, Rect::new(0.0, 0.0, 100.0, 80.0), &mut out);
    assert_eq!(out, vec![(PaneId(0), Pane::Window(WindowId(1)), Rect::new(0.0, 0.0, 25.0, 80.0)), (PaneId(1), Pane::Empty, Rect::new(25.0, 0.0, 75.0, 80.0)),]);
}

#[test]
fn vertical_split_divides_height_by_ratios() {
    let tree = Node::Split { dir: Dir::Vertical, ratios: vec![0.5, 0.5], children: vec![leaf(0, Pane::Window(WindowId(1))), leaf(1, Pane::Window(WindowId(2)))] };
    let mut out = Vec::new();
    layout(&tree, Rect::new(0.0, 0.0, 100.0, 80.0), &mut out);
    assert_eq!(out, vec![(PaneId(0), Pane::Window(WindowId(1)), Rect::new(0.0, 0.0, 100.0, 40.0)), (PaneId(1), Pane::Window(WindowId(2)), Rect::new(0.0, 40.0, 100.0, 40.0)),]);
}
