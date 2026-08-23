//! Laying out vase's own powerline bar: interlocking tabs, notch nesting into bulge.
//!
//! Pure geometry, shared by every platform that draws this style. Text width is the one thing a
//! platform must supply.

use super::bar::{self, Bar, Hits, Measure, Metrics, Run};
use super::theme::{Mark, Role};
use super::{bar_height, FONT_SIZE};
use crate::geometry::Rect;

pub const TAB_ICON: f64 = 14.0;
const TAB_ICON_GAP: f64 = 4.0;
const MAX_TAB_TEXT: f64 = 140.0;
/// Diameter of the prefix indicator dot.
pub const DOT_D: f64 = 8.0;
/// Padding on both sides of the prefix dot.
const DOT_PAD: f64 = 9.0;
/// Gap between a tab's notch and its first content.
const CONTENT_GAP: f64 = 5.0;
const RIGHT_PAD: f64 = 6.0;
/// Leading padding inside a cap-left tab.
const CAP_PAD: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TabShape {
    pub x0: f64,
    pub x1: f64,
    /// First tab of a bar with no leading pill: a rounded cap instead of a notch.
    pub cap_left: bool,
    pub fill: Role,
    pub hotkey: bool,
    pub content: Vec<Run>,
}

/// The leading powerline block carrying the brand mark.
#[derive(Debug, Clone, PartialEq)]
pub struct Lead {
    /// Right edge, where the first tab's notch nests.
    pub width: f64,
    pub glyph: LeadGlyph,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LeadGlyph {
    /// The vase silhouette.
    Logo(Rect),
    /// A user glyph.
    Glyph { x: f64, text: String, size: f64 },
}

/// A laid-out bar, ready to paint.
#[derive(Debug, Clone, PartialEq)]
pub struct BarLayout {
    /// The strip's rect in global top-left coordinates.
    pub rect: Rect,
    pub lead: Option<Lead>,
    pub tabs: Vec<TabShape>,
    /// The prefix indicator: center x, and whether it is armed.
    pub dot: Option<(f64, bool)>,
    /// Radius of every notch and bulge. A full semicircle, so tab ends run full height.
    pub radius: f64,
    /// Content past this x is clipped, so tabs never reach the trailing icons or the prefix dot.
    pub content_w: f64,
    /// x of each trailing windowless-app icon, `TAB_ICON` wide.
    pub apps: Vec<f64>,
}

impl BarLayout {
    /// Each tab's clickable span. Shifted `radius` right of the logical `[x0, x1]` so clicking a
    /// tab's right bulge selects that tab, not the one nesting into it.
    pub fn hits(&self) -> Hits {
        self.tabs.iter().map(|t| (t.x0 + self.radius, t.x1 + self.radius)).collect()
    }

    /// Each trailing icon's clickable span.
    pub fn app_hits(&self) -> Hits {
        self.apps.iter().map(|x| (*x, x + TAB_ICON)).collect()
    }

    /// The strip alone, for the command line to draw the mark over.
    pub fn bare(&self) -> BarLayout {
        BarLayout { rect: self.rect, lead: self.lead.clone(), tabs: Vec::new(), dot: None, radius: self.radius, content_w: self.content_w, apps: Vec::new() }
    }

    /// Where the command line's text starts: clear of the pill's convex bulge, plus a gap.
    pub fn prompt_x(&self) -> f64 {
        self.lead.as_ref().map_or(self.radius, |lead| lead.width) + self.radius + 5.0
    }
}

/// Lay a bar out in the powerline style.
pub fn layout(bar: &Bar, mark: &Mark, measure: Measure) -> BarLayout {
    let h = bar_height();
    let radius = h / 2.0;
    let dot_x = bar.rect.w - DOT_D - DOT_PAD;
    // Windowless apps trail the tabs as bare icons, between the last tab and the prefix dot.
    let apps = if bar.main { bar::app_icons(bar.apps.len(), dot_x - DOT_PAD, TAB_ICON, TAB_ICON_GAP) } else { Vec::new() };
    let content_end = apps.first().copied().unwrap_or(dot_x) - DOT_PAD;
    let content_w = if bar.main { content_end.max(0.0) } else { bar.rect.w };
    // No leading pill on a stack bar, or when the mark is hidden: the first tab caps at the strip's
    // rounded corner instead of nesting into a pill.
    let lead = if bar.main { lead_pill(mark, measure) } else { None };
    let capped_start = lead.is_none();
    let mut cursor = lead.as_ref().map_or(radius, |l| l.width);

    let shapes = bar
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let cap_left = capped_start && i == 0;
            // A capped-start tab clears its rounded cap; every other tab clears its notch.
            let content_left = if cap_left { CAP_PAD } else { radius + CONTENT_GAP };
            let (body_w, content) = bar::content(tab, cursor, &metrics(content_left), measure);
            let shape = TabShape {
                x0: cursor,
                x1: cursor + body_w,
                cap_left,
                fill: if tab.dim {
                    Role::DimBg
                } else if i == bar.selected {
                    Role::Active
                } else {
                    Role::Bg
                },
                hotkey: tab.hotkey,
                content,
            };
            cursor += body_w;
            shape
        })
        .collect();

    BarLayout { rect: bar.rect, lead, tabs: shapes, dot: bar.main.then_some((dot_x, bar.armed)), radius, content_w, apps }
}

fn metrics(content_left: f64) -> Metrics {
    // The monospaced number already carries a full character of trailing space.
    Metrics { content_left, right_pad: RIGHT_PAD, number_gap: 0.0, icon: TAB_ICON, icon_gap: TAB_ICON_GAP, max_label: MAX_TAB_TEXT }
}

/// The leading block: a rounded-left cap, a glyph slot, and a convex-right bulge the first tab nests into.
fn lead_pill(mark: &Mark, measure: Measure) -> Option<Lead> {
    let h = bar_height();
    let cap = h / 2.0;
    let slot_x = cap + 3.0;
    match mark {
        Mark::Hidden => None,
        // A user glyph sizes the slot to its own width; the logo uses a fixed slot.
        Mark::Glyph(g) => {
            let size = FONT_SIZE + 1.0;
            let text_w = measure(g, size);
            let slot_w = text_w.max(14.0);
            Some(Lead { width: slot_x + slot_w + 4.0, glyph: LeadGlyph::Glyph { x: slot_x + (slot_w - text_w) / 2.0, text: g.clone(), size } })
        }
        Mark::Logo => {
            let slot_w = 18.0;
            let logo_h = h - 8.0;
            let w = logo_h * super::theme::vase_mark().aspect;
            let rect = Rect::new(slot_x + (slot_w - w) / 2.0, (h - logo_h) / 2.0, w, logo_h);
            Some(Lead { width: slot_x + slot_w + 4.0, glyph: LeadGlyph::Logo(rect) })
        }
    }
}
