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

#[test]
fn screen_of_places_a_frame_by_its_center() {
    let screens = [Rect::new(0.0, 0.0, 1000.0, 800.0), Rect::new(1000.0, 0.0, 1000.0, 800.0)];
    assert_eq!(screen_of(Rect::new(10.0, 10.0, 100.0, 100.0), &screens), 0);
    assert_eq!(screen_of(Rect::new(1400.0, 10.0, 100.0, 100.0), &screens), 1);
    // A window straddling the seam belongs to whichever side holds its center.
    assert_eq!(screen_of(Rect::new(900.0, 10.0, 400.0, 100.0), &screens), 1);
    // Off every display falls back to the first.
    assert_eq!(screen_of(Rect::new(-9000.0, 0.0, 10.0, 10.0), &screens), 0);
}
