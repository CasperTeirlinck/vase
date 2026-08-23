//! vase's own bar: interlocking rounded-powerline tabs on a themed strip.

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSBezierPath, NSImageScaling, NSImageView, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::{CALayer, CAShapeLayer};
use vase_core::chrome::bar::{Bar, Run};
use vase_core::chrome::powerline::{self, BarLayout, LeadGlyph, DOT_D, TAB_ICON};
use vase_core::chrome::theme::mark;
use vase_core::chrome::BarHits;
use vase_core::geometry::Rect;

use super::super::text::{app_icon, chrome_font, measure, segment};
use super::super::theme::*;
use super::super::{bar_height, FONT_SIZE};
use super::{app_icons, badge_dot, clipped_content, prefix_dot, prompt_label, strip_label, Parts, TabBar};

impl TabBar {
    /// Stroke a laid-out bar. Every position comes from the layout; this only turns it into AppKit.
    pub(super) fn show_powerline(&mut self, bar: &Bar) -> BarHits {
        let layout = powerline::layout(bar, &mark(), &measure);
        let parts = self.begin_powerline(&layout, layout.content_w);
        let scale = self.panel.scale();
        let font = chrome_font(FONT_SIZE);
        let (h, r) = (bar_height(), layout.radius);
        let mut labels: Vec<Retained<NSTextField>> = parts.glyph.into_iter().collect();
        let mut hotkey_spans: Vec<(f64, f64, bool)> = Vec::new();

        for tab in &layout.tabs {
            let shape = CAShapeLayer::new();
            shape.setContentsScale(scale);
            shape.setPath(Some(&tab_outline(tab.x0, tab.x1, tab.cap_left, r, h).CGPath()));
            shape.setFillColor(Some(&role(tab.fill).CGColor()));
            shape.setStrokeColor(Some(&tab_border().CGColor()));
            shape.setLineWidth(1.0);
            parts.layer.addSublayer(&shape);
            // Stroke hotkey outlines on top AFTER all tabs, so a neighbour's notch fill doesn't paint over the convex-right side.
            if tab.hotkey {
                hotkey_spans.push((tab.x0, tab.x1, tab.cap_left));
            }

            for run in &tab.content {
                match run {
                    Run::Text { x, text, color } => {
                        let label = strip_label(self.mtm, &segment(text, &font, &role(*color), None), *x);
                        parts.content.addSubview(&label);
                        labels.push(label);
                    }
                    // Icons are separate NSImageViews, not label attachments: an attachment inside the label intermittently swallowed the text.
                    // An unresolved icon leaves its slot empty rather than reflowing the bar as the cache warms.
                    Run::Icon { x, app, dim, badge } => {
                        let Some(img) = app_icon(app) else { continue };
                        let icon_y = (h - TAB_ICON) / 2.0;
                        let iv = NSImageView::initWithFrame(NSImageView::alloc(self.mtm), NSRect::new(NSPoint::new(*x, icon_y), NSSize::new(TAB_ICON, TAB_ICON)));
                        iv.setImage(Some(&img));
                        iv.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                        if *dim {
                            iv.setAlphaValue(0.4);
                        }
                        parts.content.addSubview(&iv);
                        if *badge {
                            parts.content.addSubview(&badge_dot(self.mtm, *x + TAB_ICON - 5.0, icon_y + TAB_ICON - 5.0));
                        }
                    }
                }
            }
        }
        // Hotkey outlines, stroked on top (fill-less) so no neighbour covers the convex-right side.
        for (x0, x1, cap) in hotkey_spans {
            let outline = CAShapeLayer::new();
            outline.setContentsScale(scale);
            outline.setPath(Some(&tab_outline(x0, x1, cap, r, h).CGPath()));
            outline.setFillColor(None);
            outline.setStrokeColor(Some(&hotkey_border().CGColor()));
            outline.setLineWidth(1.5);
            parts.layer.addSublayer(&outline);
        }
        // Outside the clipped content view: the trailing icons and the dot sit past where tabs stop.
        let apps = app_icons(self.mtm, &parts.container, &layout.apps, bar.apps, TAB_ICON);
        if let Some((dot_x, armed)) = layout.dot {
            parts.container.addSubview(&prefix_dot(self.mtm, dot_x, DOT_D, armed));
        }

        self.panel.show(&parts.container);
        self.labels = labels;
        BarHits { tabs: layout.hits(), apps }
    }

    pub(super) fn prompt_powerline(&mut self, rect: Rect, prompt: &str) {
        let bar = Bar { rect, tabs: &[], apps: &[], selected: 0, main: true, armed: false };
        let layout = powerline::layout(&bar, &mark(), &measure).bare();
        // Full width: the command line has no prefix dot to avoid.
        let parts = self.begin_powerline(&layout, rect.w);
        let text = segment(prompt, &chrome_font(FONT_SIZE), &text_col(), None);
        let label = prompt_label(self.mtm, &text, layout.prompt_x(), rect.w);
        parts.content.addSubview(&label);
        self.labels = parts.glyph.into_iter().chain(std::iter::once(label)).collect();
        self.panel.show(&parts.container);
    }

    /// Paint the panel frame, the strip, and the leading pill, leaving the tabs to the caller.
    /// `content_w` clips the content so it never reaches the prefix dot; the strip and the mark stay full-width.
    fn begin_powerline(&self, layout: &BarLayout, content_w: f64) -> Parts {
        let container = self.panel.place(layout.rect);
        let scale = self.panel.scale();
        let h = bar_height();
        let full = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(layout.rect.w, h));

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
        strip.setCornerRadius(h / 2.0);
        bg_layer.addSublayer(&strip);

        let glyph = layout.lead.as_ref().and_then(|lead| {
            let pill = CAShapeLayer::new();
            pill.setContentsScale(scale);
            pill.setPath(Some(&lead_path(lead.width, layout.radius, h).CGPath()));
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
                    let label = strip_label(self.mtm, &segment(text, &chrome_font(*size), &accent(), None), *x);
                    container.addSubview(&label);
                    Some(label)
                }
            }
        });

        let (content, layer) = clipped_content(self.mtm, content_w, scale);
        container.addSubview(&content);
        Parts { container, content, layer, glyph }
    }
}

/// A tab's outline: a rounded cap on the left when it starts the bar, a nesting notch otherwise.
fn tab_outline(x0: f64, x1: f64, cap_left: bool, r: f64, h: f64) -> Retained<NSBezierPath> {
    if cap_left {
        tab_path_cap_left(x0, x1, r, h)
    } else {
        tab_path(x0, x1, r, h)
    }
}

/// A rounded-powerline tab outline (bottom-left origin): concave-left notch at x=`x0`, convex-right bulge at x=`x1`,
/// both radius `r`. Consecutive tabs share an arc center (`x1` of one = `x0` of the next) so the bulge nests the notch.
fn tab_path(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
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
fn tab_path_cap_left(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
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
