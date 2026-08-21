//! Empty-pane placeholder containers and the focused-pane accent border.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBox, NSBoxType, NSColor, NSTitlePosition};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use vase_core::chrome::theme::{style, Style};
use vase_core::geometry::{bbox, Rect};

use super::glass::backdrop;
use super::panel::Panel;
use super::theme::*;

/// Placeholder overlay for a tab's empty panes: one rounded container per rect.
pub struct PaneOverlay {
    panel: Panel,
    mtm: MainThreadMarker,
}

impl PaneOverlay {
    pub fn new(mtm: MainThreadMarker) -> PaneOverlay {
        PaneOverlay { panel: Panel::new(mtm), mtm }
    }

    /// Draw one rounded container per empty-pane rect; the focused pane gets an accent border. Hides when empty.
    pub fn show(&mut self, panes: &[(Rect, bool)]) {
        if panes.is_empty() {
            self.hide();
            return;
        }
        // Cover just the pane rects' bounding box (all on one display in practice).
        let b = bbox(&panes.iter().map(|(r, _)| *r).collect::<Vec<_>>());
        let container = self.panel.place(b);

        for (rect, focused) in panes {
            // CG top-left → panel-local AppKit (origin at the bbox's bottom-left).
            let lx = rect.x - b.x;
            let ly = (b.y + b.h) - (rect.y + rect.h);
            let f = NSRect::new(NSPoint::new(lx, ly), NSSize::new(rect.w, rect.h));
            match style() {
                Style::Native => container.addSubview(&backdrop(self.mtm, f, card_radius(), None)),
                Style::Powerline => {
                    let box_ = rounded(self.mtm, f);
                    let fill = if *focused { active_bg() } else { strip_bg() };
                    box_.setFillColor(&fill);
                    box_.setBorderWidth(0.0);
                    container.addSubview(&box_);
                }
            }
            // The focused placeholder is outlined on top of its fill, so the accent reads on either.
            if *focused {
                let outline = rounded(self.mtm, f);
                outline.setFillColor(&NSColor::clearColor());
                outline.setBorderWidth(2.0);
                outline.setBorderColor(&pane_border());
                container.addSubview(&outline);
            }
        }
        self.panel.show(&container);
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}

/// A border-only overlay outlining the focused pane in the accent color.
pub struct FocusBorder {
    panel: Panel,
    mtm: MainThreadMarker,
}

impl FocusBorder {
    pub fn new(mtm: MainThreadMarker) -> FocusBorder {
        FocusBorder { panel: Panel::new(mtm), mtm }
    }

    /// Draw the accent outline around `rect` (a CG top-left rect); the interior is transparent so the window shows through.
    pub fn show(&mut self, rect: Rect) {
        let container = self.panel.place(rect);
        // Inset 1px so the 2px border isn't clipped at the panel edge.
        let f = NSRect::new(NSPoint::new(1.0, 1.0), NSSize::new(rect.w - 2.0, rect.h - 2.0));
        let box_ = rounded(self.mtm, f);
        box_.setFillColor(&NSColor::clearColor());
        box_.setBorderWidth(2.0);
        box_.setBorderColor(&pane_border());
        container.addSubview(&box_);
        self.panel.show(&container);
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}

/// A rounded, title-less custom box, ready for a fill and a border.
fn rounded(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSBox> {
    let box_ = NSBox::initWithFrame(NSBox::alloc(mtm), frame);
    box_.setBoxType(NSBoxType::Custom);
    box_.setTitlePosition(NSTitlePosition::NoTitle);
    box_.setCornerRadius(card_radius());
    box_
}
