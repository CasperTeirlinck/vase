//! The tab bar in Windows' own idiom: a Fluent strip, its selected tab filled with the system accent.
//!
//! Windows 11's taskbar is the nearest thing the system has to vase's bar, so the strip runs flush along the screen
//! edge. The selected tab is a WinUI accent-filled control rather than the taskbar's accent indicator pill, which
//! has no room to read at a strip's height.

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{D2D1_ANTIALIAS_MODE_ALIASED, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT};

use vase_core::chrome::bar::{self, Bar, Hits, Measure, Metrics, Run};
use vase_core::chrome::theme::{mark, vase_mark, Mark, Role};
use vase_core::chrome::{bar_height, FONT_SIZE};
use vase_core::geometry::Rect;

use super::gpu::{color, Gpu, Surface};
use super::icons::Icons;
use super::system;
use super::{draw_text, ellipse, paths, rect_f};

/// Padding between the strip's edge and its content, Fluent's own.
const PAD: f64 = 12.0;
/// Gap between the leading mark and the first tab.
const MARK_GAP: f64 = 8.0;
/// Inset of a tab inside the strip, top and bottom.
const TAB_INSET: f64 = 2.0;
const TAB_GAP: f64 = 2.0;
/// Padding on both sides of a tab's content.
const TAB_PAD: f64 = 8.0;
/// Corner of a tab or a list row: Fluent's `ControlCornerRadius`.
pub(super) const CORNER: f64 = 4.0;
const ICON: f64 = 16.0;
const ICON_GAP: f64 = 6.0;
/// Gap between the position number and the first icon, past the number's own trailing space, which Segoe sets narrow.
const NUMBER_GAP: f64 = 4.0;
const MAX_LABEL: f64 = 140.0;
/// Diameter of the prefix indicator dot.
const DOT_D: f64 = 7.0;

const METRICS: Metrics = Metrics { content_left: TAB_PAD, right_pad: TAB_PAD, number_gap: NUMBER_GAP, icon: ICON, icon_gap: ICON_GAP, max_label: MAX_LABEL };

/// A bar laid out in the Fluent style, ready to paint.
pub(super) struct Strip {
    pub rect: Rect,
    slot: Option<MarkSlot>,
    tabs: Vec<Segment>,
    /// The prefix indicator: centre x, and whether it is armed.
    dot: Option<(f64, bool)>,
    /// Content past this x is clipped, so no tab reaches the prefix dot.
    content_w: f64,
    /// Where the tabs, or the command line's text, start: clear of the leading mark.
    left: f64,
}

/// The leading mark, placed.
enum MarkSlot {
    Logo(Rect),
    Glyph { x: f64, text: String, size: f64, width: f64 },
}

/// One tab's place in the strip: where its rounded rect sits, and the content inside it.
struct Segment {
    x0: f64,
    w: f64,
    /// The tab's own fill, if it carries one: a selected tab has one, a plain tab is bare strip.
    fill: Option<Role>,
    selected: bool,
    hotkey: bool,
    runs: Vec<Run>,
}

impl Strip {
    /// Each tab's clickable span, claiming half the gap on either side so no click falls between two tabs.
    pub(super) fn hits(&self) -> Hits {
        self.tabs.iter().map(|t| (t.x0 - TAB_GAP / 2.0, t.x0 + t.w + TAB_GAP / 2.0)).collect()
    }

    /// The strip alone, for the command line to draw over: no tabs, and no prefix dot.
    pub(super) fn bare(mut self) -> Strip {
        self.tabs.clear();
        self.dot = None;
        self
    }

    /// Where the command line's text starts.
    pub(super) fn prompt_x(&self) -> f64 {
        self.left
    }
}

/// Lay a bar out in the Fluent style: tabs left to right from the mark, a gap of bare strip between each.
pub(super) fn layout(bar: &Bar, measure: Measure) -> Strip {
    let dot_x = bar.rect.w - PAD - DOT_D;
    // Tabs stop short of the prefix dot; a stack bar carries none, so its content runs to the strip's end.
    let content_w = if bar.main { dot_x - PAD } else { bar.rect.w - PAD };
    // Only the screen's tab bar carries the mark; a stack bar starts at its own tabs.
    let slot = if bar.main { mark_slot(&mark(), measure) } else { None };
    let left = slot.as_ref().map_or(PAD, |slot| slot.right() + MARK_GAP);

    let mut x = left;
    let tabs = bar
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let (w, runs) = bar::content(tab, x, &METRICS, measure);
            // A selected tab on another monitor's bar is recessed rather than accented: selected there, but not here.
            let fill = match (i == bar.selected, tab.dim) {
                (true, false) => Some(Role::Accent),
                (true, true) => Some(Role::DimBg),
                (false, _) => None,
            };
            let segment = Segment { x0: x, w, fill, selected: i == bar.selected, hotkey: tab.hotkey, runs };
            x += w + TAB_GAP;
            segment
        })
        .collect();

    Strip { rect: bar.rect, slot, tabs, dot: bar.main.then_some((dot_x, bar.armed)), content_w, left }
}

/// Paint one laid-out strip into `surface`, and `prompt` over it when the bar is a command line.
pub(super) fn paint(gpu: &Gpu, icons: &Icons, surface: &mut Surface, strip: &Strip, prompt: Option<&str>) {
    let h = bar_height();
    let _ = surface.draw(gpu, strip.rect, |dc| unsafe {
        // Acrylic is out of reach on a composition surface, so the strip carries the color Fluent falls back to.
        if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Bg), None) {
            dc.FillRectangle(&rect_f(0.0, 0.0, strip.rect.w, h), &brush);
        }
        if let (Some(slot), Ok(accent)) = (&strip.slot, dc.CreateSolidColorBrush(&color(Role::Accent), None)) {
            match slot {
                MarkSlot::Logo(area) => {
                    if let Ok(mark) = paths::vase_mark(&gpu.factory, *area, h) {
                        dc.FillGeometry(&mark, &accent, None);
                    }
                }
                MarkSlot::Glyph { x, text, size, .. } => draw_text(gpu, dc, text, *size, *x, &accent),
            }
        }

        dc.PushAxisAlignedClip(&rect_f(0.0, 0.0, strip.content_w, h), D2D1_ANTIALIAS_MODE_ALIASED);
        for tab in &strip.tabs {
            let body = rounded(tab.x0, TAB_INSET, tab.w, h - 2.0 * TAB_INSET, CORNER);
            if let Some(fill) = tab.fill {
                if let Ok(brush) = dc.CreateSolidColorBrush(&color(fill), None) {
                    dc.FillRoundedRectangle(&body, &brush);
                }
            }
            if tab.hotkey {
                // On the accent fill the outline needs the color the text on it takes, or it disappears into the tab.
                let outline = if tab.fill == Some(Role::Accent) { on_accent(1.0) } else { color(Role::Hotkey) };
                if let Ok(brush) = dc.CreateSolidColorBrush(&outline, None) {
                    dc.DrawRoundedRectangle(&body, &brush, 1.0, None);
                }
            }
            for run in &tab.runs {
                match run {
                    Run::Text { x, text, color: role } => {
                        if let Ok(brush) = dc.CreateSolidColorBrush(&run_color(tab, *role), None) {
                            draw_text(gpu, dc, text, FONT_SIZE, *x, &brush);
                        }
                    }
                    Run::Icon { x, app, dim, badge } => {
                        let y = (h - ICON) / 2.0;
                        if let Some(bitmap) = icons.get(app) {
                            dc.DrawBitmap(bitmap, Some(&rect_f(*x, y, ICON, ICON)), if *dim { 0.4 } else { 1.0 }, D2D1_INTERPOLATION_MODE_LINEAR, None, None);
                        }
                        if *badge {
                            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Badge), None) {
                                let d = 6.0;
                                dc.FillEllipse(&ellipse(x + ICON - d / 2.0, y + d / 2.0, d / 2.0), &brush);
                            }
                        }
                    }
                }
            }
        }
        dc.PopAxisAlignedClip();

        if let Some((dot_x, armed)) = strip.dot {
            let role = if armed { Role::Accent } else { Role::Dim };
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(role), None) {
                dc.FillEllipse(&ellipse(dot_x + DOT_D / 2.0, h / 2.0, DOT_D / 2.0), &brush);
            }
        }
        if let Some(text) = prompt {
            if let Ok(brush) = dc.CreateSolidColorBrush(&color(Role::Text), None) {
                draw_text(gpu, dc, text, FONT_SIZE, strip.prompt_x(), &brush);
            }
        }
    });
}

/// The color a text run takes inside its tab.
fn run_color(tab: &Segment, role: Role) -> D2D1_COLOR_F {
    // On the accent fill the palette's own colors lose their contrast, so every run takes the system's pairing,
    // secondary runs a step down in emphasis.
    if tab.fill == Some(Role::Accent) {
        return on_accent(if role == Role::Text { 1.0 } else { 0.7 });
    }
    // Fluent steps an unselected item's label down to secondary text; a selected one keeps primary.
    color(if role == Role::Text && !tab.selected { Role::Dim } else { role })
}

/// What Windows draws on top of an accent fill: it tints the accent light for a dark appearance and dark for a
/// light one, and pairs each with the opposite ink.
fn on_accent(alpha: f32) -> D2D1_COLOR_F {
    let ink = if system::appearance().light { 1.0 } else { 0.0 };
    D2D1_COLOR_F { r: ink, g: ink, b: ink, a: alpha }
}

impl MarkSlot {
    /// The x the mark's own box ends at.
    fn right(&self) -> f64 {
        match self {
            MarkSlot::Logo(area) => area.x + area.w,
            MarkSlot::Glyph { x, width, .. } => x + width,
        }
    }
}

/// Place the leading mark at the strip's padding: the logo sized off the strip's height, a glyph off its own width.
fn mark_slot(mark: &Mark, measure: Measure) -> Option<MarkSlot> {
    let h = bar_height();
    match mark {
        Mark::Hidden => None,
        Mark::Glyph(g) => {
            let size = FONT_SIZE + 1.0;
            Some(MarkSlot::Glyph { x: PAD, text: g.clone(), size, width: measure(g, size) })
        }
        Mark::Logo => {
            let logo_h = h - 11.0;
            Some(MarkSlot::Logo(Rect::new(PAD, (h - logo_h) / 2.0, logo_h * vase_mark().aspect, logo_h)))
        }
    }
}

fn rounded(x: f64, y: f64, w: f64, h: f64, r: f64) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT { rect: rect_f(x, y, w, h), radiusX: r as f32, radiusY: r as f32 }
}

#[cfg(test)]
#[path = "fluent_test.rs"]
mod tests;
