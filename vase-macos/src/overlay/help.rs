//! The shortcut sheet, drawn on the same card as the switcher: glass under the native style, a themed
//! box under the powerline one.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBox, NSBoxType, NSColor, NSFont, NSTextField, NSTitlePosition, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use vase_core::chrome::help::{Cell, CellKind, HelpLayout, TextStyle};
use vase_core::chrome::theme::{style, Style};
use vase_core::geometry::Rect;

use super::glass::backdrop;
use super::panel::Panel;
use super::text::segment;
use super::theme::*;

/// Corner radius of a pane miniature: small enough to read as a window at 13 points tall.
const CELL_RADIUS: f64 = 2.0;

pub struct HelpView {
    panel: Panel,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl HelpView {
    pub fn new(mtm: MainThreadMarker) -> HelpView {
        HelpView { panel: Panel::new(mtm), labels: Vec::new(), mtm }
    }

    pub fn show(&mut self, layout: &HelpLayout) {
        let container = self.panel.place(layout.rect);
        let card = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(layout.rect.w, layout.rect.h));
        // Everything sits inside the card, which is what earns the text the material's own legibility treatment.
        let inner = NSView::initWithFrame(NSView::alloc(self.mtm), card);
        match style() {
            Style::Native => container.addSubview(&backdrop(self.mtm, card, card_radius(), Some(&inner))),
            Style::Powerline => {
                let box_ = rounded(self.mtm, card, card_radius());
                box_.setFillColor(&strip_bg());
                box_.setBorderWidth(1.0);
                box_.setBorderColor(&tab_border());
                container.addSubview(&box_);
                container.addSubview(&inner);
            }
        }

        for cell in &layout.cells {
            inner.addSubview(&miniature(self.mtm, cell, layout.rect.h));
        }
        let mut labels = Vec::new();
        for text in &layout.texts {
            let label = self.line(&text.text, text.style, text.rect, layout.rect.h);
            inner.addSubview(&label);
            labels.push(label);
        }

        self.panel.show(&container);
        self.labels = labels;
    }

    pub fn hide(&self) {
        self.panel.hide();
    }

    /// One line of the sheet, in the typography its style calls for.
    fn line(&self, text: &str, style: TextStyle, at: Rect, card_h: f64) -> Retained<NSTextField> {
        let (font, color) = match style {
            TextStyle::Title => (NSFont::systemFontOfSize(12.0), dim_col()),
            TextStyle::Section => (NSFont::boldSystemFontOfSize(11.0), accent()),
            // The action reads first and the chord second, as a native shortcut list has it. Monospaced
            // digits and glyphs keep the chords aligned down their column.
            TextStyle::Keys => (NSFont::monospacedSystemFontOfSize_weight(11.0, 0.0), dim_col()),
            TextStyle::Label => (NSFont::systemFontOfSize(11.5), text_col()),
        };
        let seg = segment(text, &font, &color, None);
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setUsesSingleLineMode(true);
        label.setDrawsBackground(false);
        label.setAttributedStringValue(&seg);
        let h = seg.size().height;
        label.setFrame(flip(Rect::new(at.x, at.y + (at.h - h) / 2.0, at.w, h), card_h));
        label
    }
}

/// One pane of a miniature: filled for what the command acts on, outlined for an empty or vacated pane.
fn miniature(mtm: MainThreadMarker, cell: &Cell, card_h: f64) -> Retained<NSBox> {
    let box_ = rounded(mtm, flip(cell.rect, card_h), CELL_RADIUS);
    match cell.kind {
        CellKind::Active => {
            box_.setFillColor(&accent());
            box_.setBorderWidth(0.0);
        }
        CellKind::Plain => {
            box_.setFillColor(&active_bg());
            box_.setBorderWidth(0.0);
        }
        CellKind::Ghost => {
            box_.setFillColor(&NSColor::clearColor());
            box_.setBorderWidth(1.0);
            box_.setBorderColor(&dim_col());
        }
    }
    box_
}

fn rounded(mtm: MainThreadMarker, frame: NSRect, radius: f64) -> Retained<NSBox> {
    let box_ = NSBox::initWithFrame(NSBox::alloc(mtm), frame);
    box_.setBoxType(NSBoxType::Custom);
    box_.setTitlePosition(NSTitlePosition::NoTitle);
    box_.setCornerRadius(radius);
    box_
}

/// A card-local top-left rect as AppKit's bottom-left frame.
fn flip(r: Rect, card_h: f64) -> NSRect {
    NSRect::new(NSPoint::new(r.x, card_h - r.y - r.h), NSSize::new(r.w, r.h))
}
