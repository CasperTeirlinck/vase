//! Low-level powerline path/geometry builders and the panel layout scaffolding.

use std::sync::LazyLock;

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBezierPath, NSFont, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CAShapeLayer};
use vase_core::geometry::Rect;

use super::super::text::segment;
use super::super::theme::*;
use super::super::{BAR_HEIGHT, FONT_SIZE};
use super::TabBar;

/// `begin`'s handles: `(container, content_view, content_layer, lead_w, glyph_label)`.
/// `glyph_label` is a user-glyph mark the caller must retain; `None` for the logo, a hidden, or a stack bar.
type BarParts = (Retained<NSView>, Retained<NSView>, Retained<CALayer>, f64, Option<Retained<NSTextField>>);

/// The vase brand mark parsed from docs/branding/vase-mark.svg: silhouette polygon normalized to a 0..1 box (`y` top→bottom), plus its aspect (width / height).
struct VaseMark {
    points: Vec<(f64, f64)>,
    aspect: f64,
}

static VASE_MARK: LazyLock<VaseMark> = LazyLock::new(|| {
    let svg = include_str!("../../../../docs/branding/vase-mark.svg");
    let attr = |name: &str| svg.split_once(&format!("{name}=\"")).unwrap().1.split_once('"').unwrap().0;
    let view_box: Vec<f64> = attr("viewBox").split_whitespace().map(|n| n.parse().unwrap()).collect();
    let (w, h) = (view_box[2], view_box[3]);
    let points = attr("points")
        .split_whitespace()
        .map(|pt| {
            let (px, py) = pt.split_once(',').unwrap();
            (px.parse::<f64>().unwrap() / w, py.parse::<f64>().unwrap() / h)
        })
        .collect();
    VaseMark { points, aspect: w / h }
});

/// The vase brand mark as a bezier path filling the box `[x, x+w] × [y, y+h]`; normalized `y` is flipped so the vase stands up (bottom-left origin).
pub(crate) fn vase_mark_bezier(x: f64, y: f64, w: f64, h: f64) -> Retained<NSBezierPath> {
    let path = NSBezierPath::new();
    for (i, (nx, ny)) in VASE_MARK.points.iter().enumerate() {
        let p = NSPoint::new(x + nx * w, y + (1.0 - ny) * h);
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
    /// Sets up the panel frame and background, returning `BarParts` for callers to populate.
    /// `content_w` clips the tab/command-line content so it never reaches the prefix dot;
    /// the strip background stays full-width.
    pub(super) fn begin(&self, bar_rect: Rect, content_w: f64, main: bool) -> BarParts {
        let container = self.panel.place(bar_rect);
        let scale = self.panel.scale();
        let full = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(bar_rect.w, BAR_HEIGHT));

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

        let r = BAR_HEIGHT / 2.0;
        let mark = mark();
        // No leading pill for a stack bar, or when the mark is hidden: tabs begin at the strip's rounded-left corner (first notch centered at x=r).
        if !main || matches!(mark, Mark::Hidden) {
            let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_w, BAR_HEIGHT));
            let content_view = NSView::initWithFrame(NSView::alloc(self.mtm), content_rect);
            content_view.setWantsLayer(true);
            content_view.setClipsToBounds(true);
            let content_layer = content_view.layer().unwrap();
            content_layer.setContentsScale(scale);
            container.addSubview(&content_view);
            return (container, content_view, content_layer, r, None);
        }

        // Leading powerline block carrying the mark, shared by the tab view and the command line.
        let cap = BAR_HEIGHT / 2.0;
        // A user glyph sizes the slot to its own width; the logo uses a fixed slot.
        let glyph = match &mark {
            Mark::Glyph(g) => Some(segment(g, &NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE + 1.0, 0.0), &accent(), None)),
            _ => None,
        };
        let glyph_w = glyph.as_ref().map_or(18.0, |s| s.size().width.max(14.0));
        let lead_w = cap + 3.0 + glyph_w + 4.0;
        let lead = CAShapeLayer::new();
        lead.setContentsScale(scale);
        lead.setPath(Some(&lead_path(lead_w, r, BAR_HEIGHT).CGPath()));
        lead.setStrokeColor(Some(&tab_border().CGColor()));
        lead.setLineWidth(1.0);
        bg_layer.addSublayer(&lead);
        let glyph_label = match glyph {
            // The vase silhouette in the accent color, centered in the pill's glyph slot.
            None => {
                let mark_h = BAR_HEIGHT - 8.0;
                let mark_w = mark_h * VASE_MARK.aspect;
                let mark_x = cap + 3.0 + (glyph_w - mark_w) / 2.0;
                let mark_y = (BAR_HEIGHT - mark_h) / 2.0;
                let vase = CAShapeLayer::new();
                vase.setContentsScale(scale);
                vase.setPath(Some(&vase_mark_bezier(mark_x, mark_y, mark_w, mark_h).CGPath()));
                vase.setFillColor(Some(&accent().CGColor()));
                bg_layer.addSublayer(&vase);
                None
            }
            // The user glyph as a text label, centered in the slot.
            Some(seg) => {
                let tsize = seg.size();
                let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
                label.setUsesSingleLineMode(true);
                let gx = cap + 3.0 + (glyph_w - tsize.width) / 2.0;
                label.setFrame(NSRect::new(NSPoint::new(gx.round(), ((BAR_HEIGHT - tsize.height) / 2.0 + 1.0).round()), NSSize::new(tsize.width + 4.0, tsize.height)));
                label.setAttributedStringValue(&seg);
                label.setDrawsBackground(false);
                container.addSubview(&label);
                Some(label)
            }
        };

        // Clipped content view so long content stops before the prefix dot.
        let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_w, BAR_HEIGHT));
        let content_view = NSView::initWithFrame(NSView::alloc(self.mtm), content_rect);
        content_view.setWantsLayer(true);
        content_view.setClipsToBounds(true);
        let content_layer = content_view.layer().unwrap();
        content_layer.setContentsScale(scale);
        container.addSubview(&content_view);

        (container, content_view, content_layer, lead_w, glyph_label)
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

#[cfg(test)]
mod tests {
    use super::VASE_MARK;

    #[test]
    fn vase_mark_svg_parses() {
        assert!(VASE_MARK.points.len() > 10);
        assert!((VASE_MARK.aspect - 677.0 / 744.0).abs() < 1e-9);
        assert!(VASE_MARK.points.iter().all(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)));
    }
}
