//! The tab-bar panel: interlocking rounded-powerline tabs and the command line.

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSFont, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use vase_core::geometry::Rect;

use super::panel::Panel;
use super::text::segment;
use super::theme::text_col;
use super::{BAR_HEIGHT, FONT_SIZE};

mod paths;
mod show;

pub(crate) use paths::vase_mark_bezier;

/// One tab's render inputs for the bar.
pub struct BarTab {
    pub icons: Vec<String>, // app names, one per window pane
    pub badges: Vec<bool>,  // parallel to `icons`: app has a Dock notification badge
    pub label: String,
    pub zoomed: bool,
    pub number: usize,   // 1-based position, shown grey; the `prefix-N` shortcut
    pub dim: bool,       // on a non-focused monitor: whole tab dimmed but its border
    pub off_space: bool, // a window in the tab is on another Space
    pub hotkey: bool,    // app has a focus-toggle hotkey
}

pub struct TabBar {
    panel: Panel,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker) -> TabBar {
        TabBar { panel: Panel::new(mtm), labels: Vec::new(), mtm }
    }

    /// Turn the bar into a command line: the leading vase mark stays (drawn by `begin`), and `prompt` fills the rest as a single-line text input.
    pub fn show_prompt(&mut self, content_rect: Rect, prompt: &str) {
        // The bar's own CG rect is the strip just below the content rect.
        let bar_rect = Rect::new(content_rect.x, content_rect.y + content_rect.h, content_rect.w, BAR_HEIGHT);
        // Full width: the command line has no prefix dot to avoid.
        let (container, content_view, _shapes, lead_w) = self.begin(bar_rect, bar_rect.w, true);
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        let text = segment(prompt, &font, &text_col(), None);
        let tsize = text.size();
        // Clear the pill's convex bulge (extends r past lead_w) plus a gap, so the prompt text doesn't sit on the mark.
        let x = lead_w + BAR_HEIGHT / 2.0 + 5.0;
        let w = (bar_rect.w - x - 8.0).max(0.0);
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setUsesSingleLineMode(true);
        label.setFrame(NSRect::new(NSPoint::new(x, (BAR_HEIGHT - tsize.height) / 2.0 + 2.0), NSSize::new(w, tsize.height)));
        label.setAttributedStringValue(&text);
        label.setDrawsBackground(false);
        content_view.addSubview(&label);
        self.labels = vec![label];
        self.panel.show(&container);
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}
