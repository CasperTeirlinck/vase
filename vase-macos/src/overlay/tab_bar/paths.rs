//! Low-level powerline path/geometry builders and the panel layout scaffolding.

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBezierPath, NSFont, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CAShapeLayer};
use vase_core::chrome::bar::{BarLayout, LeadGlyph};
use vase_core::chrome::theme::vase_mark;
use vase_core::geometry::Rect;

use super::super::text::segment;
use super::super::theme::*;
use super::super::BAR_HEIGHT;
use super::TabBar;

/// `begin`'s handles: `(container, content_view, content_layer, glyph_label)`.
/// `glyph_label` is a user-glyph mark the caller must retain.
type BarParts = (Retained<NSView>, Retained<NSView>, Retained<CALayer>, Option<Retained<NSTextField>>);

/// The vase brand mark as a bezier path filling the box `[x, x+w] × [y, y+h]`; normalized `y` is flipped so the vase stands up (bottom-left origin).
pub(crate) fn vase_mark_bezier(x: f64, y: f64, w: f64, h: f64) -> Retained<NSBezierPath> {
    let path = NSBezierPath::new();
    for (i, (px, py)) in vase_mark().polygon(Rect::new(x, y, w, h)).into_iter().enumerate() {
        let p = NSPoint::new(px, py);
        if i == 0 {
            path.moveToPoint(p);
        } else {
            path.lineToPoint(p);
        }
    }
    path.closePath();
    path
}

impl TabBar {
    /// Sets up the panel frame, background, and leading mark, returning `BarParts` for callers to
    /// populate. `content_w` clips the tab/command-line content so it never reaches the prefix dot;
    /// the strip background and the mark stay full-width.
    pub(super) fn begin(&self, layout: &BarLayout, content_w: f64) -> BarParts {
        let container = self.panel.place(layout.rect);
        let scale = self.panel.scale();
        let full = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(layout.rect.w, BAR_HEIGHT));

        // Full-width background: the rounded strip + the leading pill (fixed at the left, never clipped).
        let bg = NSView::initWithFrame(NSView::alloc(self.mtm), full);
        bg.setWantsLayer(true);
        let bg_layer = bg.layer().unwrap();
        bg_layer.setContentsScale(scale);
        container.addSubview(&bg);

        let strip = CALayer::new();
        strip.setFrame(full);
        strip.setContentsScale(scale);
        strip.setBackgroundColor(Some(&strip_bg().CGColor()));
        strip.setCornerRadius(BAR_HEIGHT / 2.0);
        bg_layer.addSublayer(&strip);

        let glyph_label = layout.lead.as_ref().and_then(|lead| {
            let pill = CAShapeLayer::new();
            pill.setContentsScale(scale);
            pill.setPath(Some(&lead_path(lead.width, layout.radius, BAR_HEIGHT).CGPath()));
            pill.setStrokeColor(Some(&tab_border().CGColor()));
            pill.setLineWidth(1.0);
            bg_layer.addSublayer(&pill);
            match &lead.glyph {
                // The vase silhouette in the accent color.
                LeadGlyph::Logo(rect) => {
                    let vase = CAShapeLayer::new();
                    vase.setContentsScale(scale);
                    vase.setPath(Some(&vase_mark_bezier(rect.x, rect.y, rect.w, rect.h).CGPath()));
                    vase.setFillColor(Some(&accent().CGColor()));
                    bg_layer.addSublayer(&vase);
                    None
                }
                // The user glyph as a text label.
                LeadGlyph::Glyph { x, text, size } => {
                    let seg = segment(text, &NSFont::monospacedSystemFontOfSize_weight(*size, 0.0), &accent(), None);
                    let tsize = seg.size();
                    let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
                    label.setUsesSingleLineMode(true);
                    label.setFrame(NSRect::new(NSPoint::new(x.round(), ((BAR_HEIGHT - tsize.height) / 2.0 + 1.0).round()), NSSize::new(tsize.width + 4.0, tsize.height)));
                    label.setAttributedStringValue(&seg);
                    label.setDrawsBackground(false);
                    container.addSubview(&label);
                    Some(label)
                }
            }
        });

        // Clipped content view so long content stops before the prefix dot.
        let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_w, BAR_HEIGHT));
        let content_view = NSView::initWithFrame(NSView::alloc(self.mtm), content_rect);
        content_view.setWantsLayer(true);
        content_view.setClipsToBounds(true);
        let content_layer = content_view.layer().unwrap();
        content_layer.setContentsScale(scale);
        container.addSubview(&content_view);

        (container, content_view, content_layer, glyph_label)
    }
}

/// A rounded-powerline tab outline (bottom-left origin): concave-left notch at x=`x0`, convex-right bulge at x=`x1`,
/// both radius `r`. Consecutive tabs share an arc center (`x1` of one = `x0` of the next) so the bulge nests the notch.
pub(super) fn tab_path(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let path = NSBezierPath::new();
    // The straight segments above/below the arc keep the tab full-height even when r < h/2 (smaller r = shallower notch = tighter tabs).
    path.moveToPoint(NSPoint::new(x0, 0.0));
    path.lineToPoint(NSPoint::new(x1, 0.0));
    path.lineToPoint(NSPoint::new(x1, cy - r));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(x1, cy), r, -90.0, 90.0, false);
    path.lineToPoint(NSPoint::new(x1, h));
    path.lineToPoint(NSPoint::new(x0, h));
    path.lineToPoint(NSPoint::new(x0, cy + r));
    // Concave-left arc, carving the notch.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(x0, cy), r, 90.0, -90.0, true);
    path.lineToPoint(NSPoint::new(x0, 0.0));
    path.closePath();
    path
}

/// Like `tab_path` but with a convex rounded-left cap matching the strip's corner, for the first tab of a bar (nothing to nest into on the left).
pub(super) fn tab_path_cap_left(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let path = NSBezierPath::new();
    path.moveToPoint(NSPoint::new(x0, 0.0));
    path.lineToPoint(NSPoint::new(x1, 0.0));
    path.lineToPoint(NSPoint::new(x1, cy - r));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(x1, cy), r, -90.0, 90.0, false);
    path.lineToPoint(NSPoint::new(x1, h));
    path.lineToPoint(NSPoint::new(x0, h));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(x0, cy), r, 90.0, 270.0, false);
    path.closePath();
    path
}

/// The leading logo segment: a rounded-left cap and a convex-right bulge at `lead_w` that nests into the first tab's notch.
fn lead_path(lead_w: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let cap = h / 2.0; // left cap uses the full corner radius to match the strip
    let path = NSBezierPath::new();
    path.moveToPoint(NSPoint::new(cap, 0.0));
    path.lineToPoint(NSPoint::new(lead_w, 0.0));
    path.lineToPoint(NSPoint::new(lead_w, cy - r));
    // Convex right bulge (radius r) → nests into tab 1's notch.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(lead_w, cy), r, -90.0, 90.0, false);
    path.lineToPoint(NSPoint::new(lead_w, h));
    path.lineToPoint(NSPoint::new(cap, h));
    // Full rounded left cap (bulges to x=0), matching the strip's rounded corner.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(cap, cy), cap, 90.0, 270.0, false);
    path.closePath();
    path
}
