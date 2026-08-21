use serde::{Deserialize, Serialize};

use crate::tree::{Dir, Node, Pane, PaneId};

/// An axis-aligned rectangle in screen points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }
}

/// Index of the display whose bounds contain `frame`'s center (else 0).
pub fn screen_of(frame: Rect, screens: &[Rect]) -> usize {
    let cx = frame.x + frame.w / 2.0;
    let cy = frame.y + frame.h / 2.0;
    screens.iter().position(|r| cx >= r.x && cx < r.x + r.w && cy >= r.y && cy < r.y + r.h).unwrap_or(0)
}

pub fn bbox(rects: &[Rect]) -> Rect {
    let Some(first) = rects.first() else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    let (mut x0, mut y0) = (first.x, first.y);
    let (mut x1, mut y1) = (first.x + first.w, first.y + first.h);
    for r in &rects[1..] {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Append `(pane_id, pane, rect)` for every leaf under `node` within `area`.
pub fn layout(node: &Node, area: Rect, out: &mut Vec<(PaneId, Pane, Rect)>) {
    match node {
        Node::Leaf { id, pane } => out.push((*id, *pane, area)),
        Node::Stack { id, items, selected } => {
            // Reserve the top strip for the bar; place only the selected item below.
            let strip = crate::chrome::bar_height();
            let content = Rect::new(area.x, area.y + strip, area.w, area.h - strip);
            out.push((*id, items[*selected], content));
        }
        Node::Split { dir, ratios, children } => {
            let mut offset = 0.0;
            for (child, ratio) in children.iter().zip(ratios) {
                let child_rect = match dir {
                    Dir::Horizontal => Rect::new(area.x + offset, area.y, area.w * ratio, area.h),
                    Dir::Vertical => Rect::new(area.x, area.y + offset, area.w, area.h * ratio),
                };
                layout(child, child_rect, out);
                offset += match dir {
                    Dir::Horizontal => area.w * ratio,
                    Dir::Vertical => area.h * ratio,
                };
            }
        }
    }
}
