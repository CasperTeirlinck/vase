use std::collections::HashSet;

use crate::geometry::Rect;
use crate::tree::WindowId;

pub trait Backend {
    /// Every display, ordered left-to-right then top-to-bottom.
    fn displays(&self) -> Vec<Display>;

    /// Windows on the current workspace, frontmost first.
    fn list_windows(&mut self) -> Vec<WindowInfo>;

    /// Windows across every workspace.
    fn all_windows(&mut self) -> Vec<WindowInfo>;

    /// Move+resize a window to `frame`.
    fn set_frame(&mut self, window: WindowId, frame: Rect);

    /// Give `window` keyboard focus, wherever it is.
    fn focus(&mut self, window: WindowId);

    /// Raise `window` above others without giving it keyboard focus.
    fn raise(&mut self, window: WindowId);

    /// Forget a window that no longer exists, dropping any handle held for it.
    fn forget(&mut self, window: WindowId);

    /// The window's live title.
    fn title(&mut self, window: WindowId) -> Option<String>;

    /// Whether this is a real top-level window rather than a transient popup.
    fn tileable(&mut self, info: &WindowInfo) -> Option<bool>;

    /// Whether the window is minimized.
    fn minimized(&mut self, window: WindowId) -> Option<bool>;

    /// Like `minimized`, for a window not yet tracked.
    fn minimized_info(&mut self, info: &WindowInfo) -> Option<bool>;

    fn set_minimized(&mut self, window: WindowId, minimized: bool);

    /// Whether the window owns the whole display.
    fn fullscreen(&mut self, info: &WindowInfo) -> Option<bool>;

    /// Ask the window to close itself.
    fn close(&mut self, window: WindowId);

    /// Names of apps launchable from the pane picker.
    fn launchable_apps(&self) -> Vec<String>;

    /// Spawn a new instance of `app`.
    fn launch(&self, app: &str);

    /// Display names of apps currently showing a notification badge.
    fn badged_apps(&self) -> HashSet<String> {
        HashSet::new()
    }
}

/// A snapshot of one native window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub frame: Rect,
    /// Compositing layer: 0 is the normal application layer.
    /// Overlays, tool windows, and shell surfaces report nonzero.
    pub layer: i64,
}

/// One monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Stable id, so tabs re-match the same physical monitor across a hotplug or reorder.
    pub id: u32,
    /// Full bounds in global top-left coordinates.
    pub bounds: Rect,
    /// Usable area: `bounds` less the OS's own furniture (menu bar and Dock, or the taskbar).
    pub work_area: Rect,
}

/// Whether a window should be tiled.
pub fn manageable(w: &WindowInfo) -> bool {
    w.layer == 0 && !w.title.is_empty() && w.frame.w >= 40.0 && w.frame.h >= 40.0
}
