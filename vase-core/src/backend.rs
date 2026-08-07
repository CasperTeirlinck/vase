//! The platform backend interface. Core defines it; each OS implements it.
//! Core itself makes no OS calls — this is only signatures + pure helpers.

use crate::geometry::Rect;
use crate::tree::WindowId;

/// A snapshot of one native window as seen by the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub frame: Rect,
    /// CoreGraphics window layer; 0 is the normal application layer.
    pub layer: i64,
}

/// The ~handful of primitives every platform must provide.
pub trait Backend {
    /// Visible display rectangles in global coordinates.
    fn screens(&self) -> Vec<Rect>;
    /// All on-screen windows (unfiltered; callers apply `manageable`).
    fn list_windows(&mut self) -> Vec<WindowInfo>;
    /// Move+resize a window to `frame`. `&mut self`: the backend caches
    /// per-window OS handles across calls.
    fn set_frame(&mut self, window: WindowId, frame: Rect);
    /// Give `window` keyboard focus (front process + key window), across
    /// displays/Spaces.
    fn focus(&mut self, window: WindowId);
    /// Raise `window` above other windows without forcing keyboard focus to it
    /// (restacking on a tab switch).
    fn raise(&mut self, window: WindowId);
    /// Forget a window that no longer exists (release its cached OS handle).
    fn forget(&mut self, window: WindowId);
}

/// Whether a window should be tiled (vs left floating / ignored).
pub fn manageable(w: &WindowInfo) -> bool {
    w.layer == 0 && !w.title.is_empty() && w.frame.w >= 40.0 && w.frame.h >= 40.0
}

#[cfg(test)]
#[path = "backend_test.rs"]
mod tests;
