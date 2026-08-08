//! The window every overlay surface is drawn into.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBackingStoreType, NSColor, NSPanel, NSStatusWindowLevel, NSView, NSWindowCollectionBehavior, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use vase_core::geometry::Rect;

use super::screens::{primary_screen, primary_screen_height};

/// A transparent, always-on-top, click-through panel that can never become key, so drawing into it
/// never takes keyboard focus from the user's frontmost app. Present on every Space.
pub(crate) struct Panel {
    panel: Retained<NSPanel>,
    mtm: MainThreadMarker,
}

impl Panel {
    pub(crate) fn new(mtm: MainThreadMarker) -> Panel {
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 400.0));
        // Borderless + NonactivatingPanel: the panel can never become key, so it never steals keyboard
        // focus from the user's frontmost app.
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(mtm.alloc(), content, style, NSBackingStoreType::Buffered, false);
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary);
        panel.setIgnoresMouseEvents(true);

        Panel { panel, mtm }
    }

    /// Move the panel to a CG rect and return an empty container view of that size for the caller to
    /// fill, in AppKit's bottom-left coordinates.
    ///
    /// One panel cannot span two displays under "separate Spaces", so `rect` must sit on a single one.
    pub(crate) fn place(&self, rect: Rect) -> Retained<NSView> {
        // CG rects are top-left origin and measured from the primary display's top; AppKit windows are
        // bottom-left. A missing primary display means nothing is being drawn at all, so the fallback is
        // unreachable; 0.0 drops the panel below the screen rather than at a plausible wrong position.
        let ph = primary_screen_height(self.mtm).unwrap_or(0.0);
        let frame = NSRect::new(NSPoint::new(rect.x, ph - (rect.y + rect.h)), NSSize::new(rect.w, rect.h));
        self.panel.setFrame_display(frame, true);
        let local = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(rect.w, rect.h));
        NSView::initWithFrame(NSView::alloc(self.mtm), local)
    }

    /// Put `container` on screen. `orderFront`, never `makeKeyAndOrderFront`: show without taking focus.
    pub(crate) fn show(&self, container: &NSView) {
        self.panel.setContentView(Some(container));
        self.panel.orderFront(None);
    }

    pub(crate) fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// Backing scale of the display the panel lives on. Read from the screen, not the window: the panel
    /// can report a stale 1.0 mid-setup. Manually-created sublayers default to a contentsScale of 1.0 and
    /// rasterize blurry on a HiDPI display, so every sublayer has to be stamped with this.
    pub(crate) fn scale(&self) -> f64 {
        primary_screen(self.mtm).map(|s| s.backingScaleFactor()).unwrap_or_else(|| self.panel.backingScaleFactor())
    }
}
