use crate::chrome::bar_height;
use crate::focus::Direction;
use crate::geometry::Rect;
use crate::model::*;
use crate::tree::{windows, Dir, Node, Pane, PaneId, WindowId};
use std::collections::{HashMap, HashSet};

mod adopt_add;
mod close_break;
mod command;
mod fillpane;
mod focus_move;
mod monitors;
mod reconfigure;
mod split_resize;
mod stacks;
mod stacks_break;
mod tabs;

const SCREEN: Rect = Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };

fn win(id: u64) -> WindowId {
    WindowId(id)
}

/// Single-screen model: every window on screen 0.
fn one(ws: &[WindowId]) -> Model {
    Model::adopt(&[SCREEN], &ws.iter().map(|w| (*w, 0)).collect::<Vec<_>>())
}

fn three() -> Model {
    one(&[win(1), win(2), win(3)])
}

/// Two screens side by side in global coords: screen 0 at x[0,100], screen 1 at x[100,200], same height, so `neighbor` can cross between them.
fn two_screens() -> Model {
    Model::adopt(&[Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 }, Rect { x: 100.0, y: 0.0, w: 100.0, h: 100.0 }], &[(win(1), 0), (win(2), 1)])
}

fn h_split(focus_right: bool) -> Model {
    // A single tab holding two side-by-side windows.
    let mut m = one(&[win(1)]);
    m.screens[0].tabs[0].root = Node::Split {
        dir: Dir::Horizontal,
        ratios: vec![0.5, 0.5],
        children: vec![Node::Leaf { id: PaneId(0), pane: Pane::Window(win(1)) }, Node::Leaf { id: PaneId(1), pane: Pane::Window(win(2)) }],
    };
    m.screens[0].tabs[0].focused = if focus_right { PaneId(1) } else { PaneId(0) };
    m.next_pane_id = 2;
    m
}
