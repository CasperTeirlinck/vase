//! The tab-bar panel: interlocking rounded-powerline tabs and the command line.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBackingStoreType, NSBezierPath, NSBox, NSBoxType, NSColor,
    NSFont, NSImage, NSImageScaling, NSImageView, NSPanel, NSStatusWindowLevel, NSTextField,
    NSTitlePosition, NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CAShapeLayer};
use vase_core::geometry::Rect;

use super::screens::{primary_screen, primary_screen_height};
use super::text::{app_icon, segment};
use super::theme::*;
use super::{BAR_HEIGHT, FONT_SIZE};

/// `begin`'s handles: `(container, content_view, content_layer, lead_w)`.
type BarParts = (Retained<NSView>, Retained<NSView>, Retained<CALayer>, f64);

const TAB_ICON: f64 = 14.0; // app-icon size in a tab
const TAB_ICON_GAP: f64 = 4.0; // gap between the icon and the app name
const MAX_TAB_TEXT: f64 = 140.0; // max label width; longer names ellipsize

// The vase brand mark: silhouette outline normalized to a 0..1 box (traced from
// the logo, tiles dropped — see docs/branding/vase-mark.svg). `y` runs top→bottom.
pub(crate) const VASE_MARK: &[(f64, f64)] = &[
    (0.7282, 0.9778), (0.2511, 0.9738), (0.2149, 0.9462), (0.1189, 0.7836), (0.0332, 0.5927),
    (0.0229, 0.5457), (0.0229, 0.4435), (0.0583, 0.3642), (0.209, 0.2298), (0.24, 0.1868),
    (0.2356, 0.1277), (0.1987, 0.0444), (0.2127, 0.0235), (0.774, 0.0208), (0.7999, 0.0363),
    (0.7644, 0.1263), (0.7585, 0.1801), (0.7851, 0.2231), (0.9121, 0.332), (0.9668, 0.4046),
    (0.9756, 0.4328), (0.9756, 0.5457), (0.9254, 0.6935), (0.825, 0.8844), (0.7806, 0.9516),
    (0.7592, 0.9698),
];
pub(crate) const VASE_ASPECT: f64 = 677.0 / 744.0; // mark box width / height

/// The vase brand mark as a bezier path filling the box `[x, x+w] × [y, y+h]`
/// (bottom-left origin — the normalized `y` is flipped so the vase stands up).
pub(crate) fn vase_mark_bezier(x: f64, y: f64, w: f64, h: f64) -> Retained<NSBezierPath> {
    let path = NSBezierPath::new();
    for (i, (nx, ny)) in VASE_MARK.iter().enumerate() {
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

/// One tab's render inputs for the bar.
pub struct BarTab {
    pub icons: Vec<String>, // app names, one per window pane
    pub badges: Vec<bool>,  // parallel to `icons`: app has a Dock notification badge
    pub label: String,
    pub zoomed: bool,
    pub number: usize, // 1-based position, shown grey; the `prefix-N` shortcut
    pub dim: bool,     // on a non-focused monitor: whole tab dimmed but its border
    pub hotkey: bool,  // app has a focus-toggle hotkey
}

pub struct TabBar {
    panel: Retained<NSPanel>,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker) -> TabBar {
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, BAR_HEIGHT));
        // Borderless + NonactivatingPanel: the panel can never become key, so it
        // never steals keyboard focus from the user's frontmost app.
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

        TabBar { panel, labels: Vec::new(), mtm }
    }

    /// Draw tabs left-to-right, each sized to its measured text, on a themed
    /// strip as interlocking rounded-powerline shapes (concave-left notch,
    /// convex-right bulge); `selected` is filled, all are stroked. Returns each
    /// tab's `(x_start, x_end)` in view coords for hit-testing.
    pub fn show(
        &mut self,
        bar_rect: Rect,
        tabs: &[BarTab],
        selected: usize,
        armed: bool,
        main: bool,
    ) -> Vec<(f64, f64)> {
        // Prefix dot (main bar only): fixed at the far right with equal padding on
        // both sides; the content view is clipped to `content_w` so tabs never
        // reach it. Stack bars use the full width and draw no dot.
        let dot_d = 8.0;
        let dot_x = bar_rect.w - dot_d - 9.0;
        let content_w = if main { (dot_x - 9.0).max(0.0) } else { bar_rect.w };
        let (container, content_view, shapes_layer, lead_w) =
            self.begin(bar_rect, content_w, main);
        let scale = self.scale();
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        // Full-height rounded ends: radius = half the bar height (a full
        // semicircle notch/bulge covering the whole tab height).
        let r = BAR_HEIGHT / 2.0;
        // Content starts just past the notch (which is `r` deep); minimal gap so
        // tabs sit tight to their content.
        let left_pad = r + 5.0;
        let right_pad = 6.0;
        let mut labels: Vec<Retained<NSTextField>> = Vec::new();
        let mut ranges = Vec::new();
        let mut hotkey_spans: Vec<(f64, f64)> = Vec::new();

        let mut cursor = lead_w;
        // Icons are NSImageViews and the text a plain label — an icon attachment
        // inside the label intermittently swallowed the text.
        for (i, BarTab { icons: icon_apps, badges, label: label_text, zoomed, number, dim, hotkey }) in
            tabs.iter().enumerate()
        {
            // Pair each resolved icon with whether its app is badged (parallel to
            // `icon_apps`); unresolved icons drop out of both, staying aligned.
            let icons: Vec<(Retained<NSImage>, bool)> = icon_apps
                .iter()
                .zip(badges.iter().copied().chain(std::iter::repeat(false)))
                .filter_map(|(a, badged)| app_icon(a).map(|img| (img, badged)))
                .collect();
            let txt = if *dim { dim_col() } else { text_col() };
            let mut label_seg = segment(label_text, &font, &txt, None);
            // Monospaced font: estimate the fitting char count from the average
            // char width and ellipsize when the label overflows.
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
            // Grey position number in front of the icon (the `prefix-N` shortcut).
            let num_seg = segment(&format!("{number} "), &font, &dim_col(), None);
            let num_w = num_seg.size().width;
            // A stack bar's first tab has a rounded-left cap (matching the strip
            // corner) instead of a notch, so its content clears the cap plus a
            // small pad rather than the notch depth.
            let cap_left = !main && i == 0;
            let content_left = if cap_left { 8.0 } else { left_pad };
            let body_w = if tsize.width > 0.0 {
                let iw = n * (TAB_ICON + TAB_ICON_GAP);
                content_left + num_w + iw + tsize.width + right_pad
            } else {
                let iw = n * TAB_ICON + (n - 1.0).max(0.0) * TAB_ICON_GAP;
                content_left + num_w + iw + right_pad
            };
            // Left arc center at `cursor`, right at `cursor + body_w`; the next
            // tab's cursor equals this right center, so its notch nests this bulge.
            let shape = CAShapeLayer::new();
            shape.setContentsScale(scale);
            let path = if cap_left {
                tab_path_cap_left(cursor, cursor + body_w, r, BAR_HEIGHT)
            } else {
                tab_path(cursor, cursor + body_w, r, BAR_HEIGHT)
            };
            shape.setPath(Some(&path.CGPath()));
            // Off-monitor: recessed fill. Else the selected tab highlights, the
            // rest take the strip bg.
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
            // A hotkey-app tab is outlined brighter; collect it to stroke on top
            // AFTER all tabs, so the next tab's notch fill doesn't paint over its
            // convex-right side.
            if *hotkey {
                hotkey_spans.push((cursor, cursor + body_w));
            }

            let mut x = cursor + content_left;
            let nh = num_seg.size().height;
            let nl = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
            nl.setUsesSingleLineMode(true);
            nl.setFrame(NSRect::new(
                // +1 (not the label's +2): the number aligns with the
                // geometrically-centered icon, not the title's text baseline.
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
                let iv = NSImageView::initWithFrame(
                    NSImageView::alloc(self.mtm),
                    NSRect::new(NSPoint::new(x, icon_y), NSSize::new(TAB_ICON, TAB_ICON)),
                );
                iv.setImage(Some(img));
                iv.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                if *dim {
                    iv.setAlphaValue(0.4);
                }
                content_view.addSubview(&iv);
                // Notification badge: a red dot at the icon's top-right corner,
                // mirroring the Dock. Drawn full-strength even on a dim tab.
                if *badged {
                    let d = 6.0;
                    let dot = NSBox::initWithFrame(
                        NSBox::alloc(self.mtm),
                        NSRect::new(
                            NSPoint::new(x + TAB_ICON - d + 1.0, icon_y + TAB_ICON - d + 1.0),
                            NSSize::new(d, d),
                        ),
                    );
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
            // Single line + a few px of width slack: a label defaults to word
            // wrapping, so an exact-width frame wraps "Google Chrome" onto a
            // second (clipped) line, showing only "Google".
            label.setUsesSingleLineMode(true);
            label.setFrame(NSRect::new(
                // Snap to whole pixels (x accumulates fractional tab widths, y is
                // a /2 center): text at a fractional origin renders soft on a 1x
                // display. +1 nudge to sit centered against the icon.
                NSPoint::new(x.round(), ((BAR_HEIGHT - tsize.height) / 2.0 + 1.0).round()),
                NSSize::new(tsize.width + 6.0, tsize.height),
            ));
            label.setAttributedStringValue(&text);
            label.setDrawsBackground(false);
            content_view.addSubview(&label);
            labels.push(label);
            // Click range = the tab's VISUAL span: notch tip (x0 + r) to bulge tip
            // (x1 + r), shifted `r` right of the logical [x0, x1] so clicking a
            // tab's right bulge selects that tab, not the one nesting into it.
            ranges.push((cursor + r, cursor + body_w + r));
            cursor += body_w;
        }
        // Bright hotkey-app outlines, stroked on top of all tabs (fill-less) so no
        // neighbouring tab covers the convex-right side.
        for (x0, x1) in hotkey_spans {
            let outline = CAShapeLayer::new();
            outline.setContentsScale(scale);
            outline.setPath(Some(&tab_path(x0, x1, r, BAR_HEIGHT).CGPath()));
            outline.setFillColor(None);
            outline.setStrokeColor(Some(&hotkey_border().CGColor()));
            outline.setLineWidth(1.5);
            shapes_layer.addSublayer(&outline);
        }
        // Prefix indicator (tmux-style, main bar only): green when the prefix
        // chord is armed (awaiting a command key), dim grey otherwise.
        if main {
            let dot = NSBox::initWithFrame(
                NSBox::alloc(self.mtm),
                NSRect::new(
                    NSPoint::new(dot_x.round(), ((BAR_HEIGHT - dot_d) / 2.0).round()),
                    NSSize::new(dot_d, dot_d),
                ),
            );
            dot.setBoxType(NSBoxType::Custom);
            dot.setTitlePosition(NSTitlePosition::NoTitle);
            dot.setCornerRadius(dot_d / 2.0);
            let dot_color = if armed { green() } else { dim_col() };
            dot.setFillColor(&dot_color);
            dot.setBorderWidth(0.0);
            container.addSubview(&dot);
        }

        self.panel.setContentView(Some(&container));
        self.labels = labels;
        // orderFront (never makeKeyAndOrderFront): show without taking focus.
        self.panel.orderFront(None);
        ranges
    }

    /// Backing scale of the display the bar lives on (the primary). Read from the
    /// screen, not the window — the panel can report a stale 1.0 mid-setup.
    fn scale(&self) -> f64 {
        primary_screen(self.mtm)
            .map(|s| s.backingScaleFactor())
            .unwrap_or_else(|| self.panel.backingScaleFactor())
    }

    /// `content_w` is the width of the (clipped) content area for tabs / the
    /// command line — everything past it is hidden, so tab content never reaches
    /// the prefix dot on the far right. The strip background stays full-width.
    /// Returns `(container, content_view, content_layer, lead_w, glyph_label)`:
    /// callers add their content to `content_view` / `content_layer`.
    fn begin(&self, bar_rect: Rect, content_w: f64, main: bool) -> BarParts {
        let screen_h = primary_screen_height(self.mtm).unwrap_or(0.0);
        // CG rects are top-left origin; AppKit windows are bottom-left; `bar_rect`
        // is the bar's own CG rect, so flip its top edge to a bottom-left origin.
        let y = screen_h - (bar_rect.y + BAR_HEIGHT);
        let frame = NSRect::new(NSPoint::new(bar_rect.x, y), NSSize::new(bar_rect.w, BAR_HEIGHT));
        self.panel.setFrame_display(frame, true);
        // Backing scale of the display the bar now sits on. Manually-created
        // sublayers default to contentsScale 1.0 and rasterize blurry on a HiDPI
        // display, so every sublayer below is stamped with this.
        let scale = self.scale();

        let full = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(bar_rect.w, BAR_HEIGHT));
        let container = NSView::initWithFrame(NSView::alloc(self.mtm), full);

        // Full-width background: the rounded strip + the leading pill (fixed at
        // the left, never clipped).
        let bg = NSView::initWithFrame(NSView::alloc(self.mtm), full);
        bg.setWantsLayer(true);
        let bg_layer = bg.layer().expect("layer-backed view has a layer");
        bg_layer.setContentsScale(scale);
        container.addSubview(&bg);

        let strip = CALayer::new();
        strip.setFrame(full);
        strip.setContentsScale(scale);
        strip.setBackgroundColor(Some(&strip_bg().CGColor()));
        strip.setCornerRadius(BAR_HEIGHT / 2.0);
        bg_layer.addSublayer(&strip);

        let r = BAR_HEIGHT / 2.0;
        // Stack bars carry no leading pill; their tabs begin at the strip's
        // rounded-left corner (first notch centered at x=r).
        if !main {
            let content_rect =
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_w, BAR_HEIGHT));
            let content_view = NSView::initWithFrame(NSView::alloc(self.mtm), content_rect);
            content_view.setWantsLayer(true);
            content_view.setClipsToBounds(true);
            let content_layer = content_view.layer().expect("layer-backed view has a layer");
            content_layer.setContentsScale(scale);
            container.addSubview(&content_view);
            return (container, content_view, content_layer, r);
        }

        // Leading powerline block carrying the vase brand mark, shared by the tab
        // view and the command line.
        let cap = BAR_HEIGHT / 2.0;
        let glyph_w = 18.0;
        let lead_w = cap + 3.0 + glyph_w + 4.0;
        let lead = CAShapeLayer::new();
        lead.setContentsScale(scale);
        lead.setPath(Some(&lead_path(lead_w, r, BAR_HEIGHT).CGPath()));
        lead.setStrokeColor(Some(&tab_border().CGColor()));
        lead.setLineWidth(1.0);
        bg_layer.addSublayer(&lead);
        // The vase silhouette, terracotta, centered in the pill's glyph slot.
        let mark_h = BAR_HEIGHT - 8.0;
        let mark_w = mark_h * VASE_ASPECT;
        let mark_x = cap + 3.0 + (glyph_w - mark_w) / 2.0;
        let mark_y = (BAR_HEIGHT - mark_h) / 2.0;
        let vase = CAShapeLayer::new();
        vase.setContentsScale(scale);
        vase.setPath(Some(&vase_mark_bezier(mark_x, mark_y, mark_w, mark_h).CGPath()));
        vase.setFillColor(Some(&clay().CGColor()));
        bg_layer.addSublayer(&vase);

        // Clipped content view for tabs / command line, so long content stops
        // before the prefix dot instead of overlapping it.
        let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_w, BAR_HEIGHT));
        let content_view = NSView::initWithFrame(NSView::alloc(self.mtm), content_rect);
        content_view.setWantsLayer(true);
        content_view.setClipsToBounds(true);
        let content_layer = content_view.layer().expect("layer-backed view has a layer");
        content_layer.setContentsScale(scale);
        container.addSubview(&content_view);

        (container, content_view, content_layer, lead_w)
    }

    /// Turn the bar into a command line: the leading vase mark stays (drawn by
    /// `begin`), and `prompt` fills the rest as a single-line text input.
    pub fn show_prompt(&mut self, content_rect: Rect, prompt: &str) {
        // The bar's own CG rect is the strip just below the content rect.
        let bar_rect =
            Rect::new(content_rect.x, content_rect.y + content_rect.h, content_rect.w, BAR_HEIGHT);
        // Full width: the command line has no prefix dot to avoid.
        let (container, content_view, _shapes, lead_w) = self.begin(bar_rect, bar_rect.w, true);
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        let text = segment(prompt, &font, &text_col(), None);
        let tsize = text.size();
        // Clear the pill's convex bulge (extends r past lead_w) plus a gap, so
        // the prompt text doesn't sit on the mark.
        let x = lead_w + BAR_HEIGHT / 2.0 + 5.0;
        let w = (bar_rect.w - x - 8.0).max(0.0);
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setUsesSingleLineMode(true);
        label.setFrame(NSRect::new(
            NSPoint::new(x, (BAR_HEIGHT - tsize.height) / 2.0 + 2.0),
            NSSize::new(w, tsize.height),
        ));
        label.setAttributedStringValue(&text);
        label.setDrawsBackground(false);
        content_view.addSubview(&label);
        self.labels = vec![label];
        self.panel.setContentView(Some(&container));
        self.panel.orderFront(None);
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}

/// A rounded-powerline tab outline in container coords (bottom-left origin,
/// height `h`): straight top/bottom, a concave-left notch centered at x=`x0` and
/// a convex-right bulge centered at x=`x1`, both radius `r`. Consecutive tabs
/// share an arc center (`x1` of one = `x0` of the next) so the bulge nests the notch.
fn tab_path(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let path = NSBezierPath::new();
    // Bottom edge, then up the right edge to the arc, convex-right bulge, up to
    // the top; the straight segments above/below the arc keep the tab full-height
    // even when r < h/2 (a smaller r = shallower notch/bulge = tighter tabs).
    path.moveToPoint(NSPoint::new(x0, 0.0));
    path.lineToPoint(NSPoint::new(x1, 0.0));
    path.lineToPoint(NSPoint::new(x1, cy - r));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(x1, cy), r, -90.0, 90.0, false,
    );
    path.lineToPoint(NSPoint::new(x1, h));
    path.lineToPoint(NSPoint::new(x0, h));
    path.lineToPoint(NSPoint::new(x0, cy + r));
    // Concave left: curve the left edge in to (x0 + r, cy), carving the notch.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(x0, cy), r, 90.0, -90.0, true,
    );
    path.lineToPoint(NSPoint::new(x0, 0.0));
    path.closePath();
    path
}

/// Like `tab_path` but with a convex rounded-left cap (radius `r`, bulging to
/// `x0 - r`) matching the strip's rounded corner — for the first tab of a bar
/// with nothing to nest into on the left (the stack bars).
fn tab_path_cap_left(x0: f64, x1: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let path = NSBezierPath::new();
    path.moveToPoint(NSPoint::new(x0, 0.0));
    path.lineToPoint(NSPoint::new(x1, 0.0));
    path.lineToPoint(NSPoint::new(x1, cy - r));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(x1, cy), r, -90.0, 90.0, false,
    );
    path.lineToPoint(NSPoint::new(x1, h));
    path.lineToPoint(NSPoint::new(x0, h));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(x0, cy), r, 90.0, 270.0, false,
    );
    path.closePath();
    path
}

/// The leading logo segment: a rounded-left cap (matching the strip's left
/// corner) and a convex-right bulge centered at `lead_w` (which nests into the
/// first tab's concave notch). Body runs from x=`r` to `lead_w`.
fn lead_path(lead_w: f64, r: f64, h: f64) -> Retained<NSBezierPath> {
    let cy = h / 2.0;
    let cap = h / 2.0; // left cap uses the full corner radius to match the strip
    let path = NSBezierPath::new();
    path.moveToPoint(NSPoint::new(cap, 0.0));
    path.lineToPoint(NSPoint::new(lead_w, 0.0));
    path.lineToPoint(NSPoint::new(lead_w, cy - r));
    // Convex right bulge (radius r) → nests into tab 1's notch.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(lead_w, cy), r, -90.0, 90.0, false,
    );
    path.lineToPoint(NSPoint::new(lead_w, h));
    path.lineToPoint(NSPoint::new(cap, h));
    // Full rounded left cap (bulges to x=0), matching the strip's rounded corner.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        NSPoint::new(cap, cy), cap, 90.0, 270.0, false,
    );
    path.closePath();
    path
}
