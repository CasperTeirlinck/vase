//! The switcher panel: a header line plus one row per item, either centered on a screen (the window switcher) or filling a pane (the pane picker).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBox, NSBoxType, NSColor, NSFont, NSTextAlignment, NSTextField, NSTitlePosition};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};
use vase_core::geometry::Rect;

use super::panel::Panel;
use super::text::{icon_run, scroll_offset, segment};
use super::theme::*;
use super::FONT_SIZE;

const SWITCHER_ROW_H: f64 = 28.0;
const SWITCHER_WIDTH: f64 = 640.0;
const SWITCHER_MAX_ITEMS: usize = 12;

/// One switcher row.
pub struct SwitchRow {
    pub number: usize,
    pub prefix: String,
    pub icons: Vec<String>,
    pub label: String,
    pub dim: bool,     // on a non-focused monitor
    pub current: bool, // the currently-focused window
}

/// Where a list is framed and how its card is trimmed. The two framings differ only in these values; everything below the card is drawn the same way.
struct Frame {
    /// The panel's CG rect.
    rect: Rect,
    /// How many item rows fit under the header.
    rows: usize,
    border: f64,
    border_color: Retained<NSColor>,
    /// Left inset and width of the selection highlight.
    highlight: (f64, f64),
    /// Whether to mark the focused window with a left accent bar.
    accent: bool,
}

/// A non-activating list panel: a header/query line plus one row per item.
pub struct SwitcherView {
    panel: Panel,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl SwitcherView {
    pub fn new(mtm: MainThreadMarker) -> SwitcherView {
        SwitcherView { panel: Panel::new(mtm), labels: Vec::new(), mtm }
    }

    /// Render centered on `screen` as a themed rounded card, `selected` highlighted.
    pub fn show(&mut self, screen: Rect, header: &str, items: &[SwitchRow], selected: usize) {
        // Cap visible rows so a long list can't grow off-screen; the draw scrolls to keep the selection in view.
        let rows = items.len().min(SWITCHER_MAX_ITEMS);
        let h = (rows + 1) as f64 * SWITCHER_ROW_H + 2.0 * PANE_PAD;
        let rect = Rect::new(screen.x + (screen.w - SWITCHER_WIDTH) / 2.0, screen.y + (screen.h - h) / 2.0, SWITCHER_WIDTH, h);
        self.draw(
            &Frame {
                rect,
                rows,
                border: 1.0,
                border_color: tab_border(),
                // Reach the card's inner edges, just inside the 1px border; the text stays inset.
                highlight: (1.0, SWITCHER_WIDTH - 2.0),
                accent: true,
            },
            header,
            items,
            selected,
        );
    }

    /// Render filling `area` (a CG top-left rect), `selected` highlighted; rows past `area.h` are clipped. Used by the pane picker inside a focused empty pane.
    pub fn show_in(&mut self, area: Rect, header: &str, items: &[SwitchRow], selected: usize) {
        // Everything that fits, less the header row.
        let fit = (((area.h - 2.0 * PANE_PAD) / SWITCHER_ROW_H).floor() as usize).max(1);
        let inner_w = (area.w - 2.0 * PANE_PAD).max(0.0);
        self.draw(
            &Frame {
                rect: area,
                rows: fit.saturating_sub(1).max(1),
                // Heavier border in the accent color, so an empty pane reads as a container, not a void.
                border: 2.0,
                border_color: pane_border(),
                highlight: (PANE_PAD, inner_w),
                accent: false,
            },
            header,
            items,
            selected,
        );
    }

    pub fn hide(&self) {
        self.panel.hide();
    }

    fn draw(&mut self, frame: &Frame, header: &str, items: &[SwitchRow], selected: usize) {
        let (w, h) = (frame.rect.w, frame.rect.h);
        let container = self.panel.place(frame.rect);
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h));
        let mut labels = Vec::new();

        // Themed rounded card matching the tab bar.
        let bg = NSBox::initWithFrame(NSBox::alloc(self.mtm), content);
        bg.setBoxType(NSBoxType::Custom);
        bg.setTitlePosition(NSTitlePosition::NoTitle);
        bg.setCornerRadius(PANE_RADIUS);
        bg.setFillColor(&strip_bg());
        bg.setBorderWidth(frame.border);
        bg.setBorderColor(&frame.border_color);
        container.addSubview(&bg);

        // Rows inset within the card padding; header is row 0 (AppKit bottom-left, so the top row has the largest y).
        let inner_w = (w - 2.0 * PANE_PAD).max(0.0);
        let top = h - PANE_PAD;
        let qy = top - SWITCHER_ROW_H;
        let qlabel = self.make_label(0, "", &[], header, true, false, PANE_PAD, qy, inner_w);
        container.addSubview(&qlabel);
        labels.push(qlabel);

        // Scroll a window of items so the selection stays visible.
        let offset = scroll_offset(selected, frame.rows);
        for vis in 0..frame.rows {
            let Some(row) = items.get(offset + vis) else { break };
            let ry = top - ((vis + 2) as f64) * SWITCHER_ROW_H;
            if offset + vis == selected {
                let (hx, hw) = frame.highlight;
                container.addSubview(&self.bar(hx, ry, hw, &active_bg()));
            }
            // Green left accent marks the focused window, so it stays marked as the selection moves away.
            if frame.accent && row.current {
                container.addSubview(&self.bar(1.0, ry, 3.0, &green()));
            }
            let label = self.make_label(row.number, &row.prefix, &row.icons, &row.label, false, row.dim, PANE_PAD, ry, inner_w);
            container.addSubview(&label);
            labels.push(label);
        }

        self.panel.show(&container);
        self.labels = labels;
    }

    #[allow(clippy::too_many_arguments)]
    fn make_label(&self, number: usize, prefix: &str, icons: &[String], text: &str, is_query: bool, dim: bool, x: f64, y: f64, width: f64) -> Retained<NSTextField> {
        // Content order: grey index number, tree glyph, app icons, then text.
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, 0.0);
        let text_color = if is_query { dim_col() } else { text_col() };
        let combined = NSMutableAttributedString::new();
        if number > 0 {
            // Right-aligned in a 2-wide gutter so single/double digits line up.
            combined.appendAttributedString(&segment(&format!("{number:>2} "), &font, &dim_col(), None));
        }
        if !prefix.is_empty() {
            combined.appendAttributedString(&segment(prefix, &font, &dim_col(), None));
        }
        for app in icons {
            if let Some(icon) = icon_run(app, 16.0, &font) {
                combined.appendAttributedString(&icon);
                combined.appendAttributedString(&segment(" ", &font, &text_color, None));
            }
        }
        if !text.is_empty() {
            combined.appendAttributedString(&segment(text, &font, &text_color, None));
        }
        // Size the label to its content and center it vertically in the row (NSTextField top-aligns text in a taller frame).
        let th = combined.size().height;
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setFrame(NSRect::new(NSPoint::new(x, (y + (SWITCHER_ROW_H - th) / 2.0).round()), NSSize::new(width, th)));
        label.setAlignment(NSTextAlignment::Left);
        label.setDrawsBackground(false);
        label.setAttributedStringValue(&combined);
        // Mute a row on a non-focused monitor (text + icons together).
        if dim {
            label.setAlphaValue(0.45);
        }
        label
    }

    /// A full-height row bar: the selection highlight, or the thin focused-window accent. Squared corners so a highlight reaches the card's inner edges.
    fn bar(&self, x: f64, y: f64, width: f64, color: &NSColor) -> Retained<NSBox> {
        let bar = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(x, y), NSSize::new(width, SWITCHER_ROW_H)));
        bar.setBoxType(NSBoxType::Custom);
        bar.setTitlePosition(NSTitlePosition::NoTitle);
        bar.setCornerRadius(0.0);
        bar.setFillColor(color);
        bar.setBorderWidth(0.0);
        bar
    }
}
