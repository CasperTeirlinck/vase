use std::collections::HashSet;

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

/// Two panes side by side, and a foreign window over the right one.
fn stack(order: &[(u64, Rect)]) -> Vec<(WindowId, Rect)> {
    order.iter().map(|(id, r)| (WindowId(*id), *r)).collect()
}

#[test]
fn a_pane_counts_as_covered_only_by_a_window_above_it_that_overlaps() {
    let left = Rect::new(0.0, 0.0, 100.0, 100.0);
    let right = Rect::new(100.0, 0.0, 100.0, 100.0);
    let panes = HashSet::from([WindowId(1), WindowId(2)]);

    // Nothing else on screen: both panes are clear whatever their own order.
    assert!(!any_covered(&stack(&[(1, left), (2, right)]), &panes));
    // A foreign window in front, over the right pane.
    assert!(any_covered(&stack(&[(9, right), (1, left), (2, right)]), &panes));
    // In front but elsewhere on screen, so nothing is buried.
    assert!(!any_covered(&stack(&[(9, Rect::new(300.0, 0.0, 50.0, 50.0)), (1, left), (2, right)]), &panes));
    // Overlapping but behind both panes.
    assert!(!any_covered(&stack(&[(1, left), (2, right), (9, right)]), &panes));
    // A pane over its own sibling is the tab covering itself, not a hole in it.
    assert!(!any_covered(&stack(&[(1, right), (2, right)]), &panes));
}
