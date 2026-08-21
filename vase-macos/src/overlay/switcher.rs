//! The switcher panel: a header line plus one row per item, either centered on a screen (the window switcher) or filling a pane (the pane picker).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBox, NSBoxType, NSColor, NSTextAlignment, NSTextField, NSTitlePosition, NSView};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};
use vase_core::chrome::theme::{style, Role, Style};
use vase_core::chrome::{scroll_offset, SwitchRow};
use vase_core::geometry::Rect;

use super::glass::backdrop;
use super::panel::Panel;
use super::text::{chrome_font, icon_run, segment};
use super::theme::*;
use super::{FAVORITE_MARK, FONT_SIZE, WORKSPACE_MARK};

const SWITCHER_ROW_H: f64 = 28.0;
const SWITCHER_WIDTH: f64 = 640.0;
// Cap the card at this fraction of the screen height before it starts scrolling.
const SWITCHER_MAX_SCREEN_FRAC: f64 = 0.85;

/// Where a list is framed and how its card is trimmed. The two framings differ only in these values; everything below the card is drawn the same way.
struct Frame {
    /// The panel's CG rect.
    rect: Rect,
    /// How many item rows fit under the header.
    rows: usize,
    /// Width and color of the card's outline, when it carries one.
    outline: Option<(f64, Retained<NSColor>)>,
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
        // Cap visible rows by screen height (less the header row), not a fixed count, so a taller screen shows more
        // before scrolling; the draw scrolls to keep the selection in view.
        let fit = ((screen.h * SWITCHER_MAX_SCREEN_FRAC - 2.0 * PANE_PAD) / SWITCHER_ROW_H) as usize;
        let rows = items.len().min(fit.saturating_sub(1).max(1));
        let h = (rows + 1) as f64 * SWITCHER_ROW_H + 2.0 * PANE_PAD;
        let rect = Rect::new(screen.x + (screen.w - SWITCHER_WIDTH) / 2.0, screen.y + (screen.h - h) / 2.0, SWITCHER_WIDTH, h);
        let native = matches!(style(), Style::Native);
        self.draw(
            &Frame {
                rect,
                rows,
                // Glass carries its own edge; a themed card needs a hairline to lift it off the desktop.
                outline: (!native).then(|| (1.0, tab_border())),
                // A themed card's highlight reaches its inner edges, just inside that hairline; a native one is an inset capsule.
                highlight: if native { (PANE_PAD / 2.0, SWITCHER_WIDTH - PANE_PAD) } else { (1.0, SWITCHER_WIDTH - 2.0) },
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
                outline: Some((2.0, pane_border())),
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

        // The card, matching the tab bar: a glass panel under the native style, a themed box under the
        // powerline one. Rows go inside it either way, which is what earns them the material's own
        // legibility treatment.
        let radius = card_radius();
        let inner = NSView::initWithFrame(NSView::alloc(self.mtm), content);
        match style() {
            Style::Native => container.addSubview(&backdrop(self.mtm, content, radius, Some(&inner))),
            Style::Powerline => {
                container.addSubview(&card(self.mtm, content, radius, &strip_bg()));
                container.addSubview(&inner);
            }
        }
        if let Some((width, color)) = &frame.outline {
            let outline = card(self.mtm, content, radius, &NSColor::clearColor());
            outline.setBorderWidth(*width);
            outline.setBorderColor(color);
            container.addSubview(&outline);
        }

        // Rows inset within the card padding; header is row 0 (AppKit bottom-left, so the top row has the largest y).
        let inner_w = (w - 2.0 * PANE_PAD).max(0.0);
        let top = h - PANE_PAD;
        let qy = top - SWITCHER_ROW_H;
        // Reserve the marker gutter on every row once any row carries a marker, so index numbers stay column-aligned.
        let reserve = items.iter().any(|r| r.off_workspace || r.favorite);
        let qlabel = self.make_label(0, "", &[], header, true, false, false, false, false, reserve, PANE_PAD, qy, inner_w);
        inner.addSubview(&qlabel);
        labels.push(qlabel);

        // Scroll a window of items so the selection stays visible.
        let offset = scroll_offset(selected, frame.rows);
        for vis in 0..frame.rows {
            let Some(row) = items.get(offset + vis) else { break };
            let ry = top - ((vis + 2) as f64) * SWITCHER_ROW_H;
            let picked = offset + vis == selected;
            if picked {
                let (hx, hw) = frame.highlight;
                inner.addSubview(&self.bar(hx, ry, hw, &active_bg(), radius));
            }
            // The left accent marks the focused window, so it stays marked as the selection moves away.
            if frame.accent && row.current {
                inner.addSubview(&self.bar(1.0, ry, 3.0, &accent(), 0.0));
            }
            let label = self.make_label(row.number, &row.prefix, &row.icons, &row.label, false, picked, row.dim, row.off_workspace, row.favorite, reserve, PANE_PAD, ry, inner_w);
            inner.addSubview(&label);
            labels.push(label);
        }

        self.panel.show(&container);
        self.labels = labels;
    }

    #[allow(clippy::too_many_arguments)]
    fn make_label(
        &self,
        number: usize,
        prefix: &str,
        icons: &[String],
        text: &str,
        is_query: bool,
        picked: bool,
        dim: bool,
        off_workspace: bool,
        favorite: bool,
        reserve: bool,
        x: f64,
        y: f64,
        width: f64,
    ) -> Retained<NSTextField> {
        // Content order: accent marker gutter, grey index number, tree glyph, app icons, then text.
        let font = chrome_font(FONT_SIZE);
        // A picked row sits on the selection fill, which the primary text color cannot always survive.
        let (text_color, dim_color) = if picked { (active_text(Role::Text), active_text(Role::Dim)) } else { (text_col(), dim_col()) };
        let text_color = if is_query { dim_col() } else { text_color };
        let combined = NSMutableAttributedString::new();
        // Leading gutter, reserved on every row when any row carries a marker so numbers stay aligned.
        if reserve {
            let glyph = if favorite {
                FAVORITE_MARK
            } else if off_workspace {
                WORKSPACE_MARK
            } else {
                " "
            };
            combined.appendAttributedString(&segment(glyph, &font, &accent(), None));
            combined.appendAttributedString(&segment(" ", &font, &text_color, None));
        }
        if number > 0 {
            // Right-aligned in a 2-wide gutter so single/double digits line up.
            combined.appendAttributedString(&segment(&format!("{number:>2} "), &font, &dim_color, None));
        }
        if !prefix.is_empty() {
            combined.appendAttributedString(&segment(prefix, &font, &dim_color, None));
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

    /// A full-height row bar: the selection highlight, or the thin focused-window accent.
    fn bar(&self, x: f64, y: f64, width: f64, color: &NSColor, radius: f64) -> Retained<NSBox> {
        let bar = NSBox::initWithFrame(NSBox::alloc(self.mtm), NSRect::new(NSPoint::new(x, y), NSSize::new(width, SWITCHER_ROW_H)));
        bar.setBoxType(NSBoxType::Custom);
        bar.setTitlePosition(NSTitlePosition::NoTitle);
        bar.setCornerRadius(radius);
        bar.setFillColor(color);
        bar.setBorderWidth(0.0);
        bar
    }
}

/// The list's card: a rounded, title-less box ready for a fill or an outline.
fn card(mtm: MainThreadMarker, frame: NSRect, radius: f64, fill: &NSColor) -> Retained<NSBox> {
    let card = NSBox::initWithFrame(NSBox::alloc(mtm), frame);
    card.setBoxType(NSBoxType::Custom);
    card.setTitlePosition(NSTitlePosition::NoTitle);
    card.setCornerRadius(radius);
    card.setFillColor(fill);
    card.setBorderWidth(0.0);
    card
}
