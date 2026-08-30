//! The tab-bar panel, drawn in whichever style the theme names.

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBox, NSBoxType, NSImageScaling, NSImageView, NSTextField, NSTitlePosition, NSView};
use objc2_foundation::{NSAttributedString, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;
use vase_core::chrome::bar::{Bar, Hits};
use vase_core::chrome::theme::{style, Style};
use vase_core::chrome::{BarHits, Position};
use vase_core::geometry::Rect;

use super::bar_height;
use super::panel::Panel;
use super::text::app_icon;
use super::theme::{accent, badge_red, dim_col};

mod glass;
mod powerline;

pub struct TabBar {
    panel: Panel,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

/// A placed bar mid-draw: the strip is painted, the tabs are not.
struct Parts {
    container: Retained<NSView>,
    /// Clipped to the strip's content width, so long content stops before the prefix dot.
    content: Retained<NSView>,
    /// Layer of `content`, for the tab shapes.
    layer: Retained<CALayer>,
    /// A user-glyph mark, which the caller has to retain alongside its own labels.
    glyph: Option<Retained<NSTextField>>,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker) -> TabBar {
        TabBar { panel: Panel::new(mtm), labels: Vec::new(), mtm }
    }

    /// Draw `bar`, returning where its clickable pieces landed.
    pub fn show(&mut self, bar: &Bar) -> BarHits {
        match style() {
            Style::Native => self.show_glass(bar),
            Style::Powerline => self.show_powerline(bar),
        }
    }

    /// Turn the bar into a command line: the leading mark stays, and `prompt` fills the rest as a single line of text.
    pub fn show_prompt(&mut self, rect: Rect, position: Position, prompt: &str) {
        match style() {
            Style::Native => self.prompt_glass(rect, position, prompt),
            Style::Powerline => self.prompt_powerline(rect, position, prompt),
        }
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}

/// A transparent single-line label sized to `text`, at `x`, centered in the strip.
fn strip_label(mtm: MainThreadMarker, text: &NSAttributedString, x: f64) -> Retained<NSTextField> {
    let size = text.size();
    let h = bar_height();
    let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    // A label word-wraps by default, so an exact-width frame wraps "Google Chrome" onto a second (clipped) line, showing only "Google".
    label.setUsesSingleLineMode(true);
    // Snap to whole pixels (x accumulates fractional tab widths, y is a /2 center): text at a fractional origin renders soft on a 1x display. +1 nudge to sit centered against the icon.
    label.setFrame(NSRect::new(NSPoint::new(x.round(), ((h - size.height) / 2.0 + 1.0).round()), NSSize::new(size.width + 6.0, size.height)));
    label.setAttributedStringValue(text);
    label.setDrawsBackground(false);
    label
}

/// The command line's own label: it fills the strip from `x` to its right padding, rather than sizing to its text.
fn prompt_label(mtm: MainThreadMarker, text: &NSAttributedString, x: f64, strip_w: f64) -> Retained<NSTextField> {
    let label = strip_label(mtm, text, x);
    let th = text.size().height;
    label.setFrame(NSRect::new(NSPoint::new(x, (bar_height() - th) / 2.0 + 2.0), NSSize::new((strip_w - x - 8.0).max(0.0), th)));
    label
}

/// The trailing windowless-app icons: `apps[i]` drawn at `xs[i]`, `size` square and centered in the
/// strip. Every span comes back whether its icon resolved or not, so a cold icon cache cannot move
/// the click targets.
fn app_icons(mtm: MainThreadMarker, parent: &NSView, xs: &[f64], apps: &[String], size: f64) -> Hits {
    let y = (bar_height() - size) / 2.0;
    for (x, app) in xs.iter().zip(apps) {
        let Some(img) = app_icon(app) else { continue };
        let view = NSImageView::initWithFrame(NSImageView::alloc(mtm), NSRect::new(NSPoint::new(*x, y), NSSize::new(size, size)));
        view.setImage(Some(&img));
        view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        parent.addSubview(&view);
    }
    xs.iter().map(|x| (*x, x + size)).collect()
}

/// A view clipped to `width`, and its layer: everything past the strip's content width is cut off.
fn clipped_content(mtm: MainThreadMarker, width: f64, scale: f64) -> (Retained<NSView>, Retained<CALayer>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, bar_height()));
    let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
    view.setWantsLayer(true);
    view.setClipsToBounds(true);
    let layer = view.layer().unwrap();
    layer.setContentsScale(scale);
    (view, layer)
}

/// The notification dot on an app icon: full-strength red even on a dim tab.
fn badge_dot(mtm: MainThreadMarker, x: f64, y: f64) -> Retained<NSBox> {
    dot(mtm, x, y, 6.0, &badge_red())
}

/// The prefix indicator: accent when the prefix chord is armed, dim grey otherwise.
fn prefix_dot(mtm: MainThreadMarker, x: f64, d: f64, armed: bool) -> Retained<NSBox> {
    let color = if armed { accent() } else { dim_col() };
    dot(mtm, x.round(), ((bar_height() - d) / 2.0).round(), d, &color)
}

fn dot(mtm: MainThreadMarker, x: f64, y: f64, d: f64, color: &objc2_app_kit::NSColor) -> Retained<NSBox> {
    let dot = NSBox::initWithFrame(NSBox::alloc(mtm), NSRect::new(NSPoint::new(x, y), NSSize::new(d, d)));
    dot.setBoxType(NSBoxType::Custom);
    dot.setTitlePosition(NSTitlePosition::NoTitle);
    dot.setCornerRadius(d / 2.0);
    dot.setFillColor(color);
    dot.setBorderWidth(0.0);
    dot
}
