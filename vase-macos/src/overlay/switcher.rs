//! The centered, non-activating switcher panel (window / pane pickers).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBackingStoreType, NSBox, NSBoxType, NSColor, NSFont,
    NSPanel, NSStatusWindowLevel, NSTextAlignment, NSTextField, NSTitlePosition, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};
use vase_core::geometry::Rect;

use super::screens::primary_screen_height;
use super::text::{icon_run, scroll_offset, segment};
use super::theme::*;
use super::FONT_SIZE;

const SWITCHER_ROW_H: f64 = 28.0;
const SWITCHER_WIDTH: f64 = 640.0;
const SWITCHER_MAX_ITEMS: usize = 12;

/// One switcher row's content: its 1-based `number` (grey, the press-to-select
/// shortcut), a tree-glyph `prefix`, the app `icons` (several on a parent row),
/// the display `label`, whether it's `dim` (on a non-focused monitor), and
/// whether it's the currently-focused window (a "you are here" marker).
pub struct SwitchRow {
    pub number: usize,
    pub prefix: String,
    pub icons: Vec<String>,
    pub label: String,
    pub dim: bool,
    pub current: bool,
}

/// A centered, non-activating switcher panel: a query line plus one row per item.
pub struct SwitcherView {
    panel: Retained<NSPanel>,
    labels: Vec<Retained<NSTextField>>,
    mtm: MainThreadMarker,
}

impl SwitcherView {
    pub fn new(mtm: MainThreadMarker) -> SwitcherView {
        let content =
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SWITCHER_WIDTH, SWITCHER_ROW_H));
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

        SwitcherView { panel, labels: Vec::new(), mtm }
    }

    /// Render centered on `screen`: a themed rounded card (tab-bar palette) with a
    /// header/query line on top and one row per item, `selected` highlighted.
    /// Each item is `(app, label, monitor)`; `monitor` prefixes a green number.
    pub fn show(&mut self, screen: Rect, header: &str, items: &[SwitchRow], selected: usize) {
        // Cap the visible rows so a long list can't grow off-screen; scroll a
        // window of items to keep the selection in view.
        let shown = items.len().min(SWITCHER_MAX_ITEMS);
        let h = (shown + 1) as f64 * SWITCHER_ROW_H + 2.0 * PANE_PAD;
        // Center within `screen` (CG coords), then flip to AppKit using the
        // primary height so it lands on that monitor.
        let ph = primary_screen_height(self.mtm).unwrap_or(900.0);
        let cg_x = screen.x + (screen.w - SWITCHER_WIDTH) / 2.0;
        let cg_y = screen.y + (screen.h - h) / 2.0;
        let frame = NSRect::new(NSPoint::new(cg_x, ph - (cg_y + h)), NSSize::new(SWITCHER_WIDTH, h));
        self.panel.setFrame_display(frame, true);

        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SWITCHER_WIDTH, h));
        let container = NSView::initWithFrame(NSView::alloc(self.mtm), content);
        let mut labels = Vec::new();

        // Themed rounded card matching the tab bar: strip bg + a dim tab-tone border.
        let bg = NSBox::initWithFrame(NSBox::alloc(self.mtm), content);
        bg.setBoxType(NSBoxType::Custom);
        bg.setTitlePosition(NSTitlePosition::NoTitle);
        bg.setCornerRadius(PANE_RADIUS);
        bg.setFillColor(&strip_bg());
        bg.setBorderWidth(1.0);
        bg.setBorderColor(&tab_border());
        container.addSubview(&bg);

        // Rows inset within the card padding; header is row 0 (AppKit bottom-left,
        // so the top row has the largest y).
        let inner_w = SWITCHER_WIDTH - 2.0 * PANE_PAD;
        let top = h - PANE_PAD;
        let qy = top - SWITCHER_ROW_H;
        let qlabel = self.make_label(0, "", &[], header, true, false, PANE_PAD, qy, inner_w);
        container.addSubview(&qlabel);
        labels.push(qlabel);
        let offset = scroll_offset(selected, shown);
        for vis in 0..shown {
            let Some(row) = items.get(offset + vis) else { break };
            let ry = top - ((vis + 2) as f64) * SWITCHER_ROW_H;
            // Full-width highlight reaching the card's inner edges (just inside the
            // 1px border); the text stays inset by PANE_PAD.
            if offset + vis == selected {
                container.addSubview(&self.highlight(1.0, ry, SWITCHER_WIDTH - 2.0));
            }
            // The currently-focused window keeps a green left accent bar, so it
            // stays marked even as the selection moves away from it.
            if row.current {
                container.addSubview(&self.accent(1.0, ry));
            }
            let label = self.make_label(
                row.number,
                &row.prefix,
                &row.icons,
                &row.label,
                false,
                row.dim,
                PANE_PAD,
                ry,
                inner_w,
            );
            container.addSubview(&label);
            labels.push(label);
        }

        self.panel.setContentView(Some(&container));
        self.labels = labels;
        // orderFront (never makeKeyAndOrderFront): show without taking focus.
        self.panel.orderFront(None);
    }

    /// Render inside `area` (a CG top-left rect): row 0 = header, rows 1.. =
    /// items, `selected` highlighted. Rows past `area.h` are clipped. Used by
    /// the pane picker, which lives inside the focused empty pane's rect.
    pub fn show_in(&mut self, area: Rect, header: &str, items: &[SwitchRow], selected: usize) {
        let screen_h = primary_screen_height(self.mtm).unwrap_or(900.0);
        // CG top-left → AppKit bottom-left, same flip as PaneOverlay.
        let y = screen_h - (area.y + area.h);
        let frame = NSRect::new(NSPoint::new(area.x, y), NSSize::new(area.w, area.h));
        self.panel.setFrame_display(frame, true);

        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(area.w, area.h));
        let container = NSView::initWithFrame(NSView::alloc(self.mtm), content);
        let mut labels = Vec::new();

        // Themed rounded container filling the pane (tab-bar bg + tab corner
        // radius, accent outline), so an empty pane reads as a container rather
        // than a void; the picker rows sit inset on top of it.
        let bg = NSBox::initWithFrame(NSBox::alloc(self.mtm), content);
        bg.setBoxType(NSBoxType::Custom);
        bg.setTitlePosition(NSTitlePosition::NoTitle);
        bg.setCornerRadius(PANE_RADIUS);
        bg.setFillColor(&strip_bg());
        bg.setBorderWidth(2.0);
        bg.setBorderColor(&pane_border());
        container.addSubview(&bg);

        // Rows inset within the container padding; header is row 0. Clip the tail
        // past the available height.
        let inner_w = (area.w - 2.0 * PANE_PAD).max(0.0);
        let top = area.h - PANE_PAD;
        let rows_fit = (((area.h - 2.0 * PANE_PAD) / SWITCHER_ROW_H).floor() as usize).max(1);
        let qy = top - SWITCHER_ROW_H;
        let qlabel = self.make_label(0, "", &[], header, true, false, PANE_PAD, qy, inner_w);
        container.addSubview(&qlabel);
        labels.push(qlabel);
        // Scroll a window of items so the selection stays visible in the pane.
        let item_rows = rows_fit.saturating_sub(1).max(1);
        let offset = scroll_offset(selected, item_rows);
        for vis in 0..item_rows {
            let Some(row) = items.get(offset + vis) else { break };
            let ry = top - ((vis + 2) as f64) * SWITCHER_ROW_H;
            if offset + vis == selected {
                container.addSubview(&self.highlight(PANE_PAD, ry, inner_w));
            }
            let label = self.make_label(
                row.number,
                &row.prefix,
                &row.icons,
                &row.label,
                false,
                row.dim,
                PANE_PAD,
                ry,
                inner_w,
            );
            container.addSubview(&label);
            labels.push(label);
        }

        self.panel.setContentView(Some(&container));
        self.labels = labels;
        // orderFront (never makeKeyAndOrderFront): show without taking focus.
        self.panel.orderFront(None);
    }

    #[allow(clippy::too_many_arguments)]
    fn make_label(
        &self,
        number: usize,
        prefix: &str,
        icons: &[String],
        text: &str,
        is_query: bool,
        dim: bool,
        x: f64,
        y: f64,
        width: f64,
    ) -> Retained<NSTextField> {
        // Tab-bar palette, monospaced. Content: grey index number, tree glyph,
        // the app icons (parents carry several), then the text.
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
        // Size the label to its content and center it vertically in the row
        // (NSTextField top-aligns text in a taller frame).
        let th = combined.size().height;
        let label = NSTextField::labelWithString(&NSString::from_str(""), self.mtm);
        label.setFrame(NSRect::new(
            NSPoint::new(x, (y + (SWITCHER_ROW_H - th) / 2.0).round()),
            NSSize::new(width, th),
        ));
        label.setAlignment(NSTextAlignment::Left);
        label.setDrawsBackground(false);
        label.setAttributedStringValue(&combined);
        // A row on a non-focused monitor is muted (text + icons together).
        if dim {
            label.setAlphaValue(0.45);
        }
        label
    }

    /// A selection highlight box (active-tab fill) spanning `[x, x+width]` at row
    /// `y`; squared corners so it can reach the card's inner edges.
    fn highlight(&self, x: f64, y: f64, width: f64) -> Retained<NSBox> {
        let hl = NSBox::initWithFrame(
            NSBox::alloc(self.mtm),
            NSRect::new(NSPoint::new(x, y), NSSize::new(width, SWITCHER_ROW_H)),
        );
        hl.setBoxType(NSBoxType::Custom);
        hl.setTitlePosition(NSTitlePosition::NoTitle);
        hl.setCornerRadius(0.0);
        hl.setFillColor(&active_bg());
        hl.setBorderWidth(0.0);
        hl
    }

    /// A thin green left-edge accent marking the currently-focused window, so it
    /// stays visible when the selection cursor moves elsewhere.
    fn accent(&self, x: f64, y: f64) -> Retained<NSBox> {
        let bar = NSBox::initWithFrame(
            NSBox::alloc(self.mtm),
            NSRect::new(NSPoint::new(x, y), NSSize::new(3.0, SWITCHER_ROW_H)),
        );
        bar.setBoxType(NSBoxType::Custom);
        bar.setTitlePosition(NSTitlePosition::NoTitle);
        bar.setCornerRadius(0.0);
        bar.setFillColor(&green());
        bar.setBorderWidth(0.0);
        bar
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }
}
