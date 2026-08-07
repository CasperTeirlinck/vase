use super::*;
use crate::tree::PaneId;

fn grid() -> Vec<(PaneId, Rect)> {
    // Two side-by-side panes.
    vec![
        (PaneId(1), Rect::new(0.0, 0.0, 50.0, 100.0)),
        (PaneId(2), Rect::new(50.0, 0.0, 50.0, 100.0)),
    ]
}

#[test]
fn right_moves_to_the_pane_on_the_right() {
    assert_eq!(neighbor(&grid(), PaneId(1), Direction::Right), Some(PaneId(2)));
}

#[test]
fn left_from_the_leftmost_is_none() {
    assert_eq!(neighbor(&grid(), PaneId(1), Direction::Left), None);
}

#[test]
fn up_when_all_are_side_by_side_is_none() {
    assert_eq!(neighbor(&grid(), PaneId(1), Direction::Up), None);
}
