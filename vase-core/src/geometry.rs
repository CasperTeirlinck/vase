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

/// Height of a stack's local tab-bar strip. Mirrors the macOS tab-bar height.
pub const STACK_BAR_H: f64 = 22.0;

/// Append `(pane_id, pane, rect)` for every leaf under `node` within `area`.
pub fn layout(node: &Node, area: Rect, out: &mut Vec<(PaneId, Pane, Rect)>) {
    match node {
        Node::Leaf { id, pane } => out.push((*id, *pane, area)),
        Node::Stack { id, items, selected } => {
            // Reserve the top strip for the bar; place only the selected item below.
            let content = Rect::new(area.x, area.y + STACK_BAR_H, area.w, area.h - STACK_BAR_H);
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
