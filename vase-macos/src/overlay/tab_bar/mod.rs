//! The tab-bar panel: interlocking rounded-powerline tabs and the command line.

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSFont, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use vase_core::chrome::bar::BarLayout;

use super::panel::Panel;
use super::text::segment;
use super::theme::text_col;
use super::{BAR_HEIGHT, FONT_SIZE};

mod paths;
mod show;

pub(crate) use paths::vase_mark_bezier;

pub struct TabBar {
    panel: Panel,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl TabBar {
    pub fn new(mtm: MainThreadMarker) -> TabBar {
        TabBar { panel: Panel::new(mtm), labels: Vec::new(), mtm }
    }

    /// Turn the bar into a command line: the leading mark stays (drawn by `begin`), and `prompt` fills the rest as a single-line text input.
    pub fn show_prompt(&mut self, layout: &BarLayout, prompt: &str) {
        // Full width: the command line has no prefix dot to avoid.
        let (container, content_view, _shapes, glyph_label) = self.begin(layout, layout.rect.w);
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        let text = segment(prompt, &font, &text_col(), None);
        let tsize = text.size();
        // Clear the pill's convex bulge (extends `radius` past its width) plus a gap, so the prompt text doesn't sit on the mark.
        let lead_w = layout.lead.as_ref().map_or(layout.radius, |lead| lead.width);
        let x = lead_w + layout.radius + 5.0;
        let w = (layout.rect.w - x - 8.0).max(0.0);
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setUsesSingleLineMode(true);
        label.setFrame(NSRect::new(NSPoint::new(x, (BAR_HEIGHT - tsize.height) / 2.0 + 2.0), NSSize::new(w, tsize.height)));
        label.setAttributedStringValue(&text);
        label.setDrawsBackground(false);
        content_view.addSubview(&label);
        let mut labels: Vec<_> = glyph_label.into_iter().collect();
        labels.push(label);
        self.labels = labels;
        self.panel.show(&container);
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}
