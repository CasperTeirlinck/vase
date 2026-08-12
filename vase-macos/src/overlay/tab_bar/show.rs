//! Drawing the interlocking powerline tabs onto a themed strip.

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBox, NSBoxType, NSFont, NSImage, NSImageScaling, NSImageView, NSTextField, NSTitlePosition};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CAShapeLayer;
use vase_core::geometry::Rect;

use super::super::text::{app_icon, segment};
use super::super::theme::*;
use super::super::{BAR_HEIGHT, FONT_SIZE, WORKSPACE_MARK};
use super::paths::{tab_path, tab_path_cap_left};
use super::TabBar;
use vase_core::chrome::bar::BarTab;

const TAB_ICON: f64 = 14.0; // app-icon size in a tab
const TAB_ICON_GAP: f64 = 4.0; // gap between the icon and the app name
const MAX_TAB_TEXT: f64 = 140.0; // max label width; longer names ellipsize

impl TabBar {
    /// Draw the tabs on a themed strip; `selected` is filled. Returns each tab's `(x_start, x_end)` in view coords for hit-testing.
    pub fn show(&mut self, bar_rect: Rect, tabs: &[BarTab], selected: usize, armed: bool, main: bool) -> Vec<(f64, f64)> {
        // Prefix dot (main bar only): fixed at the far right with equal padding on both sides;
        // the content view is clipped to `content_w` so tabs never reach it. Stack bars use the full width
        // and draw no dot.
        let dot_d = 8.0;
        let dot_x = bar_rect.w - dot_d - 9.0;
        let content_w = if main { (dot_x - 9.0).max(0.0) } else { bar_rect.w };
        let (container, content_view, shapes_layer, lead_w, glyph_label) = self.begin(bar_rect, content_w, main);
        // No leading pill (a stack bar, or the mark hidden) means the first tab caps like a stack bar's.
        let capped_start = !main || matches!(mark(), Mark::Hidden);
        let scale = self.panel.scale();
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        // Full-height rounded ends: a full-semicircle notch/bulge.
        let r = BAR_HEIGHT / 2.0;
        // Content starts just past the notch (`r` deep) plus a small gap.
        let left_pad = r + 5.0;
        let right_pad = 6.0;
        let mut labels: Vec<Retained<NSTextField>> = glyph_label.into_iter().collect();
        let mut ranges = Vec::new();
        let mut hotkey_spans: Vec<(f64, f64, bool)> = Vec::new();

        let mut cursor = lead_w;
        // Icons are separate NSImageViews, not label attachments: an attachment inside the label intermittently swallowed the text.
        for (i, BarTab { icons: icon_apps, badges, label: label_text, zoomed, number, dim, off_workspace, hotkey }) in tabs.iter().enumerate() {
            // Pair each resolved icon with its badge flag; unresolved icons drop from both, staying aligned.
            let icons: Vec<(Retained<NSImage>, bool)> =
                icon_apps.iter().zip(badges.iter().copied().chain(std::iter::repeat(false))).filter_map(|(a, badged)| app_icon(a).map(|img| (img, badged))).collect();
            let txt = if *dim { dim_col() } else { text_col() };
            let mut label_seg = segment(label_text, &font, &txt, None);
            // Monospaced: estimate the fitting char count from average char width, then ellipsize.
            if label_seg.size().width > MAX_TAB_TEXT {
                let n = label_text.chars().count().max(1);
                let char_w = label_seg.size().width / n as f64;
                let fit = ((MAX_TAB_TEXT / char_w).floor() as usize).saturating_sub(1).max(1);
                let truncated: String = label_text.chars().take(fit).collect::<String>() + "…";
                label_seg = segment(&truncated, &font, &txt, None);
            }
            let text = NSMutableAttributedString::new();
            text.appendAttributedString(&label_seg);
            if *zoomed {
                text.appendAttributedString(&segment(" Z", &font, &txt, None));
            }
            let tsize = text.size();
            let n = icons.len() as f64;
            // Grey position number in front of the icon (the `prefix-N` shortcut), with a leading Space marker
            let num_seg = NSMutableAttributedString::new();
            if *off_workspace {
                num_seg.appendAttributedString(&segment(WORKSPACE_MARK, &font, &accent(), None));
                num_seg.appendAttributedString(&segment(" ", &font, &dim_col(), None));
            }
            num_seg.appendAttributedString(&segment(&format!("{number} "), &font, &dim_col(), None));
            let num_w = num_seg.size().width;
            // A capped-start first tab has a rounded-left cap, not a notch, so its content clears the cap plus a small pad.
            let cap_left = capped_start && i == 0;
            let content_left = if cap_left { 8.0 } else { left_pad };
            let body_w = if tsize.width > 0.0 {
                let iw = n * (TAB_ICON + TAB_ICON_GAP);
                content_left + num_w + iw + tsize.width + right_pad
            } else {
                let iw = n * TAB_ICON + (n - 1.0).max(0.0) * TAB_ICON_GAP;
                content_left + num_w + iw + right_pad
            };
            // Left arc center at `cursor`, right at `cursor + body_w`; the next tab's cursor equals this right center,
            // so its notch nests this bulge.
            let shape = CAShapeLayer::new();
            shape.setContentsScale(scale);
            let path = if cap_left { tab_path_cap_left(cursor, cursor + body_w, r, BAR_HEIGHT) } else { tab_path(cursor, cursor + body_w, r, BAR_HEIGHT) };
            shape.setPath(Some(&path.CGPath()));
            // Off-monitor tabs get a recessed fill; the selected tab highlights.
            let fill = if *dim {
                dim_bg()
            } else if i == selected {
                active_bg()
            } else {
                strip_bg()
            };
            shape.setFillColor(Some(&fill.CGColor()));
            shape.setStrokeColor(Some(&tab_border().CGColor()));
            shape.setLineWidth(1.0);
            shapes_layer.addSublayer(&shape);
            // Stroke hotkey outlines on top AFTER all tabs, so a neighbour's notch fill doesn't paint over the convex-right side.
            if *hotkey {
                hotkey_spans.push((cursor, cursor + body_w, cap_left));
            }

            let mut x = cursor + content_left;
            let nh = num_seg.size().height;
            let nl = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
            nl.setUsesSingleLineMode(true);
            nl.setFrame(NSRect::new(
                // +1 (not the label's +2): the number aligns with the geometrically-centered icon, not the title's text baseline.
                NSPoint::new(x.round(), ((BAR_HEIGHT - nh) / 2.0 + 1.0).round()),
                NSSize::new(num_w + 4.0, nh),
            ));
            nl.setAttributedStringValue(&num_seg);
            nl.setDrawsBackground(false);
            content_view.addSubview(&nl);
            labels.push(nl);
            x += num_w;
            for (img, badged) in &icons {
                let icon_y = (BAR_HEIGHT - TAB_ICON) / 2.0;
                let iv = NSImageView::initWithFrame(NSImageView::alloc(self.mtm), NSRect::new(NSPoint::new(x, icon_y), NSSize::new(TAB_ICON, TAB_ICON)));
                iv.setImage(Some(img));
                iv.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                if *dim {
                    iv.setAlphaValue(0.4);
                }
                content_view.addSubview(&iv);
                // Notification badge: red dot at the icon's top-right, full-strength even on a dim tab.
                if *badged {
                    let d = 6.0;
                    let dot = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(x + TAB_ICON - d + 1.0, icon_y + TAB_ICON - d + 1.0), NSSize::new(d, d)));
                    dot.setBoxType(NSBoxType::Custom);
                    dot.setTitlePosition(NSTitlePosition::NoTitle);
                    dot.setCornerRadius(d / 2.0);
                    dot.setFillColor(&badge_red());
                    dot.setBorderWidth(0.0);
                    content_view.addSubview(&dot);
                }
                x += TAB_ICON + TAB_ICON_GAP;
            }
            let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
            // Single line + a few px of width slack: a label defaults to word wrapping, so an exact-width frame wraps "Google Chrome" onto a second (clipped) line, showing only "Google".
            label.setUsesSingleLineMode(true);
            label.setFrame(NSRect::new(
                // Snap to whole pixels (x accumulates fractional tab widths, y is a /2 center): text at a fractional origin renders soft on a 1x display. +1 nudge to sit centered against the icon.
                NSPoint::new(x.round(), ((BAR_HEIGHT - tsize.height) / 2.0 + 1.0).round()),
                NSSize::new(tsize.width + 6.0, tsize.height),
            ));
            label.setAttributedStringValue(&text);
            label.setDrawsBackground(false);
            content_view.addSubview(&label);
            labels.push(label);
            // Click range = the tab's VISUAL span: notch tip (x0 + r) to bulge tip (x1 + r), shifted `r` right
            // of the logical [x0, x1] so clicking a tab's right bulge selects that tab, not the one nesting into it.
            ranges.push((cursor + r, cursor + body_w + r));
            cursor += body_w;
        }
        // Hotkey outlines, stroked on top (fill-less) so no neighbour covers the convex-right side.
        for (x0, x1, cap) in hotkey_spans {
            let outline = CAShapeLayer::new();
            outline.setContentsScale(scale);
            let path = if cap { tab_path_cap_left(x0, x1, r, BAR_HEIGHT) } else { tab_path(x0, x1, r, BAR_HEIGHT) };
            outline.setPath(Some(&path.CGPath()));
            outline.setFillColor(None);
            outline.setStrokeColor(Some(&hotkey_border().CGColor()));
            outline.setLineWidth(1.5);
            shapes_layer.addSublayer(&outline);
        }
        // Prefix indicator (main bar only): green when the prefix chord is armed, dim grey otherwise.
        if main {
            let dot = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(dot_x.round(), ((BAR_HEIGHT - dot_d) / 2.0).round()), NSSize::new(dot_d, dot_d)));
            dot.setBoxType(NSBoxType::Custom);
            dot.setTitlePosition(NSTitlePosition::NoTitle);
            dot.setCornerRadius(dot_d / 2.0);
            let dot_color = if armed { accent() } else { dim_col() };
            dot.setFillColor(&dot_color);
            dot.setBorderWidth(0.0);
            container.addSubview(&dot);
        }

        self.panel.show(&container);
        self.labels = labels;
        ranges
    }
}
