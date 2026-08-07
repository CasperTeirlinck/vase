//! Empty-pane placeholder containers and the focused-pane accent border.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSPanel, NSStatusWindowLevel, NSTitlePosition,
    NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use vase_core::geometry::Rect;

use super::screens::{bbox, primary_screen_height};
use super::theme::*;

/// Placeholder overlay for a tab's empty panes: one dark rounded container per
/// rect, the focused one brighter with an accent border.
pub struct PaneOverlay {
    panel: Retained<NSPanel>,
    mtm: MainThreadMarker,
}

impl PaneOverlay {
    pub fn new(mtm: MainThreadMarker) -> PaneOverlay {
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 400.0));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            content,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
        );
        panel.setIgnoresMouseEvents(true);

        PaneOverlay { panel, mtm }
    }

    /// Draw one dark rounded container per empty-pane rect (fill `#21252b`); the
    /// focused pane gets a brighter fill and an accent border. Hides when empty.
    pub fn show(&mut self, panes: &[(Rect, bool)]) {
        if panes.is_empty() {
            self.hide();
            return;
        }
        // Cover just the pane rects' bounding box (all on one display in practice);
        // a panel can't span displays under separate Spaces.
        let b = bbox(&panes.iter().map(|(r, _)| *r).collect::<Vec<_>>());
        let ph = primary_screen_height(self.mtm).unwrap_or(0.0);
        let frame = NSRect::new(NSPoint::new(b.x, ph - (b.y + b.h)), NSSize::new(b.w, b.h));
        self.panel.setFrame_display(frame, true);
        let local = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(b.w, b.h));
        let container = NSView::initWithFrame(NSView::alloc(self.mtm), local);

        for (rect, focused) in panes {
            // CG top-left → panel-local AppKit (origin at the bbox's bottom-left).
            let lx = rect.x - b.x;
            let ly = (b.y + b.h) - (rect.y + rect.h);
            let f = NSRect::new(NSPoint::new(lx, ly), NSSize::new(rect.w, rect.h));
            let box_ = NSBox::initWithFrame(NSBox::alloc(self.mtm), f);
            box_.setBoxType(NSBoxType::Custom);
            box_.setTitlePosition(NSTitlePosition::NoTitle);
            box_.setCornerRadius(PANE_RADIUS);
            if *focused {
                box_.setFillColor(&active_bg());
                box_.setBorderWidth(2.0);
                box_.setBorderColor(&pane_border());
            } else {
                box_.setFillColor(&strip_bg());
                box_.setBorderWidth(0.0);
            }
            container.addSubview(&box_);
        }
        self.panel.setContentView(Some(&container));
        // orderFront (never makeKeyAndOrderFront): show without taking focus.
        self.panel.orderFront(None);
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}

/// A border-only overlay outlining the focused pane in the accent color, so the
/// focused pane stands out within a split (empty panes already show this border
/// via their placeholder container).
pub struct FocusBorder {
    panel: Retained<NSPanel>,
    mtm: MainThreadMarker,
}

impl FocusBorder {
    pub fn new(mtm: MainThreadMarker) -> FocusBorder {
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 400.0));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            content,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
        );
        panel.setIgnoresMouseEvents(true);

        FocusBorder { panel, mtm }
    }

    /// Draw the accent outline around `rect` (a CG top-left rect); the interior
    /// is transparent so the window shows through.
    pub fn show(&mut self, rect: Rect) {
        // The panel covers just the pane rect (on one display); a panel can't span
        // displays under separate Spaces.
        let ph = primary_screen_height(self.mtm).unwrap_or(0.0);
        let frame =
            NSRect::new(NSPoint::new(rect.x, ph - (rect.y + rect.h)), NSSize::new(rect.w, rect.h));
        self.panel.setFrame_display(frame, true);
        let local = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(rect.w, rect.h));
        let container = NSView::initWithFrame(NSView::alloc(self.mtm), local);

        // Inset 1px so the 2px border isn't clipped at the panel edge.
        let f = NSRect::new(NSPoint::new(1.0, 1.0), NSSize::new(rect.w - 2.0, rect.h - 2.0));
        let box_ = NSBox::initWithFrame(NSBox::alloc(self.mtm), f);
        box_.setBoxType(NSBoxType::Custom);
        box_.setTitlePosition(NSTitlePosition::NoTitle);
        box_.setCornerRadius(PANE_RADIUS);
        box_.setFillColor(&NSColor::clearColor());
        box_.setBorderWidth(2.0);
        box_.setBorderColor(&pane_border());
        container.addSubview(&box_);

        self.panel.setContentView(Some(&container));
        self.panel.orderFront(None);
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}
