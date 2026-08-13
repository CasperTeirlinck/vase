//! Drawing the interlocking powerline tabs onto a themed strip.

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBezierPath, NSBox, NSBoxType, NSFont, NSImageScaling, NSImageView, NSTextField, NSTitlePosition};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CAShapeLayer;
use vase_core::chrome::bar::{BarLayout, Run, DOT_D, TAB_ICON};

use super::super::text::{app_icon, segment};
use super::super::theme::*;
use super::super::{BAR_HEIGHT, FONT_SIZE};
use super::paths::{tab_path, tab_path_cap_left};
use super::TabBar;

impl TabBar {
    /// Stroke a laid-out bar. Every position comes from `layout`; this only turns it into AppKit.
    pub fn show(&mut self, layout: &BarLayout) {
        let (container, content_view, shapes_layer, glyph_label) = self.begin(layout, layout.content_w);
        let scale = self.panel.scale();
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        let r = layout.radius;
        let mut labels: Vec<Retained<NSTextField>> = glyph_label.into_iter().collect();
        let mut hotkey_spans: Vec<(f64, f64, bool)> = Vec::new();

        for tab in &layout.tabs {
            let shape = CAShapeLayer::new();
            shape.setContentsScale(scale);
            shape.setPath(Some(&tab_outline(tab.x0, tab.x1, tab.cap_left, r).CGPath()));
            shape.setFillColor(Some(&role(tab.fill).CGColor()));
            shape.setStrokeColor(Some(&tab_border().CGColor()));
            shape.setLineWidth(1.0);
            shapes_layer.addSublayer(&shape);
            // Stroke hotkey outlines on top AFTER all tabs, so a neighbour's notch fill doesn't paint over the convex-right side.
            if tab.hotkey {
                hotkey_spans.push((tab.x0, tab.x1, tab.cap_left));
            }

            for run in &tab.content {
                match run {
                    Run::Text { x, text, color } => {
                        let seg = segment(text, &font, &role(*color), None);
                        let tsize = seg.size();
                        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
                        // Single line + a few px of width slack: a label defaults to word wrapping, so an exact-width frame wraps "Google Chrome" onto a second (clipped) line, showing only "Google".
                        label.setUsesSingleLineMode(true);
                        label.setFrame(NSRect::new(
                            // Snap to whole pixels (x accumulates fractional tab widths, y is a /2 center): text at a fractional origin renders soft on a 1x display. +1 nudge to sit centered against the icon.
                            NSPoint::new(x.round(), ((BAR_HEIGHT - tsize.height) / 2.0 + 1.0).round()),
                            NSSize::new(tsize.width + 6.0, tsize.height),
                        ));
                        label.setAttributedStringValue(&seg);
                        label.setDrawsBackground(false);
                        content_view.addSubview(&label);
                        labels.push(label);
                    }
                    // Icons are separate NSImageViews, not label attachments: an attachment inside the label intermittently swallowed the text.
                    // An unresolved icon leaves its slot empty rather than reflowing the bar as the cache warms.
                    Run::Icon { x, app, dim, badge } => {
                        let Some(img) = app_icon(app) else { continue };
                        let icon_y = (BAR_HEIGHT - TAB_ICON) / 2.0;
                        let iv = NSImageView::initWithFrame(NSImageView::alloc(self.mtm), NSRect::new(NSPoint::new(*x, icon_y), NSSize::new(TAB_ICON, TAB_ICON)));
                        iv.setImage(Some(&img));
                        iv.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                        if *dim {
                            iv.setAlphaValue(0.4);
                        }
                        content_view.addSubview(&iv);
                        // Notification badge: red dot at the icon's top-right, full-strength even on a dim tab.
                        if *badge {
                            let d = 6.0;
                            let dot = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(x + TAB_ICON - d + 1.0, icon_y + TAB_ICON - d + 1.0), NSSize::new(d, d)));
                            dot.setBoxType(NSBoxType::Custom);
                            dot.setTitlePosition(NSTitlePosition::NoTitle);
                            dot.setCornerRadius(d / 2.0);
                            dot.setFillColor(&badge_red());
                            dot.setBorderWidth(0.0);
                            content_view.addSubview(&dot);
                        }
                    }
                }
            }
        }
        // Hotkey outlines, stroked on top (fill-less) so no neighbour covers the convex-right side.
        for (x0, x1, cap) in hotkey_spans {
            let outline = CAShapeLayer::new();
            outline.setContentsScale(scale);
            outline.setPath(Some(&tab_outline(x0, x1, cap, r).CGPath()));
            outline.setFillColor(None);
            outline.setStrokeColor(Some(&hotkey_border().CGColor()));
            outline.setLineWidth(1.5);
            shapes_layer.addSublayer(&outline);
        }
        // Prefix indicator: accent when the prefix chord is armed, dim grey otherwise.
        if let Some((dot_x, armed)) = layout.dot {
            let dot = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(dot_x.round(), ((BAR_HEIGHT - DOT_D) / 2.0).round()), NSSize::new(DOT_D, DOT_D)));
            dot.setBoxType(NSBoxType::Custom);
            dot.setTitlePosition(NSTitlePosition::NoTitle);
            dot.setCornerRadius(DOT_D / 2.0);
            let dot_color = if armed { accent() } else { dim_col() };
            dot.setFillColor(&dot_color);
            dot.setBorderWidth(0.0);
            container.addSubview(&dot);
        }

        self.panel.show(&container);
        self.labels = labels;
    }
}

/// A tab's outline: a rounded cap on the left when it starts the bar, a nesting notch otherwise.
fn tab_outline(x0: f64, x1: f64, cap_left: bool, r: f64) -> Retained<NSBezierPath> {
    if cap_left {
        tab_path_cap_left(x0, x1, r, BAR_HEIGHT)
    } else {
        tab_path(x0, x1, r, BAR_HEIGHT)
    }
}
