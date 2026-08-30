//! The tab bar in macOS's own idiom: a floating glass capsule carrying segment-style tabs.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBox, NSBoxType, NSColor, NSImageScaling, NSImageView, NSTextField, NSTitlePosition, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAShapeLayer;
use vase_core::chrome::bar::{self, Bar, BarTab, Hits, Measure, Metrics, Run};
use vase_core::chrome::theme::{mark, vase_mark, Mark, Role};
use vase_core::chrome::{BarHits, Position};
use vase_core::geometry::Rect;

use super::super::glass::backdrop;
use super::super::text::{app_icon, chrome_font, measure, segment};
use super::super::theme::{accent, active_text, role, text_col, vase_mark_bezier};
use super::super::{bar_height, FONT_SIZE};
use super::{app_icons, badge_dot, clipped_content, prefix_dot, prompt_label, strip_label, Parts, TabBar};

/// Inset of a stack's floating strip inside its rect, on both sides. The screen's bar runs the full width,
/// flush with the tiled windows below it.
const INSET: f64 = 8.0;
/// Padding between the strip's rounded end and its first content.
const PAD: f64 = 9.0;
/// Gap between the leading mark and the first tab.
const MARK_GAP: f64 = 8.0;
/// Inset of a tab capsule inside the strip, top and bottom.
const TAB_INSET: f64 = 3.0;
const TAB_GAP: f64 = 3.0;
/// Padding on both sides of a tab's content.
const TAB_PAD: f64 = 9.0;
const ICON: f64 = 15.0;
const ICON_GAP: f64 = 5.0;
/// Gap between the position number and the first icon, past the number's own trailing space, which
/// the system font sets narrow.
const NUMBER_GAP: f64 = 4.0;
const MAX_LABEL: f64 = 140.0;
/// Diameter of the prefix indicator dot.
const DOT_D: f64 = 7.0;

impl TabBar {
    pub(super) fn show_glass(&mut self, bar: &Bar) -> BarHits {
        let inset = strip_inset(bar.main);
        let dot_x = bar.rect.w - inset - PAD - DOT_D;
        // Windowless apps trail the tabs as bare icons, between the last tab and the prefix dot. Both
        // sit in the panel's own coordinates, outside the strip-local content view.
        let icons = bar::app_icons(bar.apps.len(), dot_x - PAD, ICON, ICON_GAP);
        let content_end = icons.first().copied().unwrap_or(dot_x);
        // Tabs stop short of whatever trails them; a stack bar has neither, so its content runs to the strip's end.
        let content_w = if bar.main { content_end - inset - PAD } else { bar.rect.w - 2.0 * inset - PAD };
        let (parts, left) = self.begin_glass(bar.rect, content_w, bar.main, bar.position);
        let font = chrome_font(FONT_SIZE);
        let mut labels: Vec<Retained<NSTextField>> = parts.glyph.into_iter().collect();
        let segments = segments(bar.tabs, left, &measure);

        for (i, (seg, tab)) in segments.iter().zip(bar.tabs).enumerate() {
            // The emphasized accent fill is for the selected tab of the focused monitor; elsewhere the
            // system's unemphasized selection says "selected, but not here".
            let fill = match (i == bar.selected, tab.dim) {
                (true, false) => Some(Role::Active),
                (true, true) => Some(Role::DimBg),
                (false, _) => None,
            };
            if let Some(fill) = fill {
                parts.content.addSubview(&capsule(self.mtm, seg.x0, seg.w, Some(&role(fill)), None));
            }
            if tab.hotkey {
                parts.content.addSubview(&capsule(self.mtm, seg.x0, seg.w, None, Some(&accent())));
            }
            // On the accent fill the palette's greys lose their contrast, so those runs take the system's own pairing.
            let text_color = |r: Role| if fill == Some(Role::Active) { active_text(r) } else { role(r) };
            // A tab's label reads at the same weight as its number here: the icon and the selection
            // carry the emphasis, not the title.
            let secondary = |r: Role| if r == Role::Text { Role::Dim } else { r };

            for run in &seg.runs {
                match run {
                    Run::Text { x, text, color } => {
                        let label = strip_label(self.mtm, &segment(text, &font, &text_color(secondary(*color)), None), *x);
                        parts.content.addSubview(&label);
                        labels.push(label);
                    }
                    Run::Icon { x, app, dim, badge } => {
                        let Some(img) = app_icon(app) else { continue };
                        let icon_y = (bar_height() - ICON) / 2.0;
                        let iv = NSImageView::initWithFrame(NSImageView::alloc(self.mtm), NSRect::new(NSPoint::new(*x, icon_y), NSSize::new(ICON, ICON)));
                        iv.setImage(Some(&img));
                        iv.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                        if *dim {
                            iv.setAlphaValue(0.4);
                        }
                        parts.content.addSubview(&iv);
                        if *badge {
                            parts.content.addSubview(&badge_dot(self.mtm, *x + ICON - 5.0, icon_y + ICON - 5.0));
                        }
                    }
                }
            }
        }

        let apps = app_icons(self.mtm, &parts.container, &icons, bar.apps, ICON);
        if bar.main {
            parts.container.addSubview(&prefix_dot(self.mtm, dot_x, DOT_D, bar.armed));
        }
        self.panel.show(&parts.container);
        self.labels = labels;
        BarHits { tabs: hits(&segments, inset), apps }
    }

    pub(super) fn prompt_glass(&mut self, rect: Rect, position: Position, prompt: &str) {
        // Full width: the command line has no prefix dot to avoid.
        let (parts, x) = self.begin_glass(rect, rect.w - PAD, true, position);
        let text = segment(prompt, &chrome_font(FONT_SIZE), &text_col(), None);
        let label = prompt_label(self.mtm, &text, x, rect.w);
        parts.content.addSubview(&label);
        self.labels = parts.glyph.into_iter().chain(std::iter::once(label)).collect();
        self.panel.show(&parts.container);
    }

    /// Place the panel, lay the glass strip in it, and draw the leading mark. Returns the parts to
    /// fill and the strip-local x the content starts at.
    fn begin_glass(&self, rect: Rect, content_w: f64, main: bool, position: Position) -> (Parts, f64) {
        let container = self.panel.place(rect);
        let scale = self.panel.scale();
        let inset = strip_inset(main);
        let strip_w = (rect.w - 2.0 * inset).max(0.0);
        let h = bar_height();
        // The strip's own view is what the glass owns (or, for the screen's bar, rides on); the clipped
        // content nests inside it, so a resize of the content view can't undo the clipping that keeps
        // tabs off the prefix dot.
        let inner = NSView::initWithFrame(NSView::alloc(self.mtm), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(strip_w, h)));
        let (content, layer) = clipped_content(self.mtm, content_w, scale);
        inner.addSubview(&content);
        if main {
            // Flush against a screen edge: overhang the glass past that edge inside a clipping wrapper,
            // cutting its rounded corners there flat so the strip fills the screen corners. Clipping,
            // not maskedCorners, because Liquid Glass draws its shape from its own cornerRadius and
            // ignores the layer mask. The content rides on top as a sibling, in visible coordinates.
            // (view coordinates are y-up)
            let r = h / 2.0;
            let glass_y = match position {
                Position::Bottom => -r,
                Position::Top => 0.0,
            };
            let glass = backdrop(self.mtm, NSRect::new(NSPoint::new(0.0, glass_y), NSSize::new(strip_w, h + r)), r, None);
            let wrapper = NSView::initWithFrame(NSView::alloc(self.mtm), NSRect::new(NSPoint::new(inset, 0.0), NSSize::new(strip_w, h)));
            wrapper.setClipsToBounds(true);
            wrapper.addSubview(&glass);
            wrapper.addSubview(&inner);
            container.addSubview(&wrapper);
        } else {
            let strip = NSRect::new(NSPoint::new(inset, 0.0), NSSize::new(strip_w, h));
            container.addSubview(&backdrop(self.mtm, strip, h / 2.0, Some(&inner)));
        }

        let mut x = PAD;
        // Only the screen's tab bar carries the mark; a stack bar starts at its own tabs.
        let (glyph, x) = match (main, mark()) {
            (false, _) | (_, Mark::Hidden) => (None, x),
            (_, Mark::Logo) => {
                let area = logo_rect(x);
                let logo = CAShapeLayer::new();
                logo.setContentsScale(scale);
                logo.setPath(Some(&vase_mark_bezier(area.x, area.y, area.w, area.h).CGPath()));
                logo.setFillColor(Some(&accent().CGColor()));
                layer.addSublayer(&logo);
                (None, area.x + area.w + MARK_GAP)
            }
            (_, Mark::Glyph(g)) => {
                let size = FONT_SIZE + 1.0;
                let label = strip_label(self.mtm, &segment(&g, &chrome_font(size), &accent(), None), x);
                content.addSubview(&label);
                x += measure(&g, size) + MARK_GAP;
                (Some(label), x)
            }
        };
        (Parts { container, content, layer, glyph }, x)
    }
}

const METRICS: Metrics = Metrics { content_left: TAB_PAD, right_pad: TAB_PAD, number_gap: NUMBER_GAP, icon: ICON, icon_gap: ICON_GAP, max_label: MAX_LABEL };

/// One tab's place in the strip: where its capsule sits, and the content inside it.
struct Segment {
    x0: f64,
    w: f64,
    runs: Vec<Run>,
}

/// Lay the tabs out left to right from `left`, a gap of bare glass between each.
fn segments(tabs: &[BarTab], left: f64, measure: Measure) -> Vec<Segment> {
    let mut x = left;
    tabs.iter()
        .map(|tab| {
            let (w, runs) = bar::content(tab, x, &METRICS, measure);
            let seg = Segment { x0: x, w, runs };
            x += w + TAB_GAP;
            seg
        })
        .collect()
}

/// Side inset of the strip: the screen's bar runs edge to edge, a stack bar floats inside its rect.
fn strip_inset(main: bool) -> f64 {
    if main {
        0.0
    } else {
        INSET
    }
}

/// Each tab's clickable span, in the bar's own coordinates, claiming half the gap on either side so
/// no click falls between two tabs.
fn hits(segments: &[Segment], inset: f64) -> Hits {
    segments.iter().map(|s| (inset + s.x0 - TAB_GAP / 2.0, inset + s.x0 + s.w + TAB_GAP / 2.0)).collect()
}

/// The brand mark's box at `x`, sized off the strip's height.
fn logo_rect(x: f64) -> Rect {
    let strip = bar_height();
    let h = strip - 11.0;
    Rect::new(x, (strip - h) / 2.0, h * vase_mark().aspect, h)
}

/// A tab-height capsule spanning `[x, x + w]`, filled and/or outlined.
fn capsule(mtm: MainThreadMarker, x: f64, w: f64, fill: Option<&NSColor>, border: Option<&NSColor>) -> Retained<NSBox> {
    let h = bar_height() - 2.0 * TAB_INSET;
    let box_ = NSBox::initWithFrame(NSBox::alloc(mtm), NSRect::new(NSPoint::new(x, TAB_INSET), NSSize::new(w, h)));
    box_.setBoxType(NSBoxType::Custom);
    box_.setTitlePosition(NSTitlePosition::NoTitle);
    box_.setCornerRadius(h / 2.0);
    box_.setFillColor(fill.unwrap_or(&NSColor::clearColor()));
    match border {
        Some(color) => {
            box_.setBorderWidth(1.0);
            box_.setBorderColor(color);
        }
        None => box_.setBorderWidth(0.0),
    }
    box_
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed-pitch stand-in for the system font's metrics.
    fn measure(text: &str, size: f64) -> f64 {
        text.chars().count() as f64 * size * 0.6
    }

    fn tab(number: usize) -> BarTab {
        BarTab { icons: vec!["Ghostty".into()], badges: vec![false], label: "window".into(), zoomed: false, number, dim: false, off_workspace: false, hotkey: false }
    }

    #[test]
    fn tabs_are_separated_by_a_gap_whose_halves_stay_clickable() {
        let tabs = [tab(1), tab(2), tab(3)];
        let segments = segments(&tabs, PAD, &measure);
        assert_eq!(segments[0].x0, PAD);
        for pair in segments.windows(2) {
            assert_eq!(pair[1].x0 - (pair[0].x0 + pair[0].w), TAB_GAP, "tabs float apart, they do not interlock");
        }
        // Content sits inside its own capsule, and adjacent hit ranges meet exactly.
        let hits = hits(&segments, INSET);
        for (seg, (a, b)) in segments.iter().zip(&hits) {
            let xs: Vec<f64> = seg
                .runs
                .iter()
                .map(|r| match r {
                    Run::Text { x, .. } | Run::Icon { x, .. } => *x,
                })
                .collect();
            assert!(xs.iter().all(|x| *x > seg.x0 && *x < seg.x0 + seg.w));
            // Hit ranges are the bar's own coordinates, a strip inset right of the content's.
            assert!(*a <= INSET + seg.x0 && *b >= INSET + seg.x0 + seg.w);
        }
        assert!(hits.windows(2).all(|w| w[0].1 == w[1].0), "no click may fall between two tabs");
    }
}
