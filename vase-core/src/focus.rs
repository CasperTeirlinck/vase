//! Directional focus resolution over computed leaf rects.

use crate::geometry::Rect;

/// A focus-movement direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// The target nearest to `from` in `dir`, or `None` at the edge. Id-agnostic:
/// works over pane or window ids.
pub fn neighbor<Id: Copy + PartialEq>(
    targets: &[(Id, Rect)],
    from: Id,
    dir: Direction,
) -> Option<Id> {
    let center = |r: &Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let from_rect = targets.iter().find(|(id, _)| *id == from).map(|(_, r)| r)?;
    let (fx, fy) = center(from_rect);

    targets
        .iter()
        .filter(|(id, _)| *id != from)
        .filter(|(_, r)| {
            let (cx, cy) = center(r);
            match dir {
                Direction::Left => cx < fx,
                Direction::Right => cx > fx,
                Direction::Up => cy < fy,
                Direction::Down => cy > fy,
            }
        })
        .min_by(|(_, a), (_, b)| {
            let d = |r: &Rect| {
                let (cx, cy) = center(r);
                (cx - fx).powi(2) + (cy - fy).powi(2)
            };
            d(a).partial_cmp(&d(b)).unwrap()
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
#[path = "focus_test.rs"]
mod tests;
