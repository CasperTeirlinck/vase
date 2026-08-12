//! Laying out a powerline bar.
//!
//! Pure geometry. Text width is the one thing a platform must supply.

use super::theme::{Mark, Role};
use super::{BAR_HEIGHT, FAVORITE_MARK, FONT_SIZE, WORKSPACE_MARK};
use crate::geometry::Rect;

pub const TAB_ICON: f64 = 14.0;
const TAB_ICON_GAP: f64 = 4.0;
/// Label width past which a label ellipsizes.
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

/// Measures a string's width, at `size` points, in the painter's own font.
pub type Measure<'a> = &'a dyn Fn(&str, f64) -> f64;

#[derive(Debug, Clone, PartialEq)]
pub struct BarTab {
    /// App names, one per window pane in the tab.
    pub icons: Vec<String>,
    /// Parallel to `icons` but for notification badges.
    pub badges: Vec<bool>,
    pub label: String,
    pub zoomed: bool,
    /// 1-based position index in the bar.
    pub number: usize,
    /// On a non-focused monitor.
    pub dim: bool,
    pub off_workspace: bool,
    /// The app has a focus-toggle hotkey.
    pub hotkey: bool,
}

/// A positioned piece of a tab's content, in bar-local bottom-left coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    Text { x: f64, text: String, color: Role },
    Icon { x: f64, app: String, dim: bool, badge: bool },
}

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
    /// Content past this x is clipped, so tabs never reach the prefix dot.
    pub content_w: f64,
}

impl BarLayout {
    /// Each tab's clickable span. Shifted `radius` right of the logical `[x0, x1]` so clicking a
    /// tab's right bulge selects that tab, not the one nesting into it.
    pub fn hit_ranges(&self) -> Vec<(f64, f64)> {
        self.tabs.iter().map(|t| (t.x0 + self.radius, t.x1 + self.radius)).collect()
    }
}

/// Lay out a bar. `main` is the screen's tab bar, which carries the mark and the prefix dot.
pub fn layout(rect: Rect, tabs: &[BarTab], selected: usize, armed: bool, main: bool, mark: &Mark, measure: Measure) -> BarLayout {
    let radius = BAR_HEIGHT / 2.0;
    let dot_x = rect.w - DOT_D - DOT_PAD;
    let content_w = if main { (dot_x - DOT_PAD).max(0.0) } else { rect.w };
    // No leading pill on a stack bar, or when the mark is hidden: the first tab caps at the strip's
    // rounded corner instead of nesting into a pill.
    let lead = if main { lead_pill(mark, measure) } else { None };
    let capped_start = lead.is_none();
    let mut cursor = lead.as_ref().map_or(radius, |l| l.width);

    let shapes = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let cap_left = capped_start && i == 0;
            let (body_w, content) = tab_content(tab, cursor, cap_left, measure);
            let shape = TabShape {
                x0: cursor,
                x1: cursor + body_w,
                cap_left,
                fill: if tab.dim {
                    Role::DimBg
                } else if i == selected {
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

    BarLayout { rect, lead, tabs: shapes, dot: main.then_some((dot_x, armed)), radius, content_w }
}

/// The leading block: a rounded-left cap, a glyph slot, and a convex-right bulge the first tab nests into.
fn lead_pill(mark: &Mark, measure: Measure) -> Option<Lead> {
    let cap = BAR_HEIGHT / 2.0;
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
            let h = BAR_HEIGHT - 8.0;
            let w = h * super::theme::vase_mark().aspect;
            let rect = Rect::new(slot_x + (slot_w - w) / 2.0, (BAR_HEIGHT - h) / 2.0, w, h);
            Some(Lead { width: slot_x + slot_w + 4.0, glyph: LeadGlyph::Logo(rect) })
        }
    }
}

fn tab_content(tab: &BarTab, x0: f64, cap_left: bool, measure: Measure) -> (f64, Vec<Run>) {
    let text_color = if tab.dim { Role::Dim } else { Role::Text };
    let label = ellipsize(&tab.label, MAX_TAB_TEXT, measure);
    let label = if tab.zoomed { format!("{label} Z") } else { label };
    let label_w = if label.is_empty() { 0.0 } else { measure(&label, FONT_SIZE) };

    // Grey position number, with a leading marker when a window in the tab is on another workspace.
    let workspace = tab.off_workspace.then(|| format!("{WORKSPACE_MARK} "));
    let number = format!("{} ", tab.number);
    let workspace_w = workspace.as_deref().map_or(0.0, |s| measure(s, FONT_SIZE));
    let number_w = measure(&number, FONT_SIZE);

    // A capped-start tab clears its rounded cap; every other tab clears its notch.
    let content_left = if cap_left { CAP_PAD } else { BAR_HEIGHT / 2.0 + CONTENT_GAP };
    let n = tab.icons.len() as f64;
    // A trailing label needs a gap after the last icon; a tab that is icons-only does not.
    let icons_w = if label_w > 0.0 { n * (TAB_ICON + TAB_ICON_GAP) } else { n * TAB_ICON + (n - 1.0).max(0.0) * TAB_ICON_GAP };
    let body_w = content_left + workspace_w + number_w + icons_w + label_w + RIGHT_PAD;

    let mut runs = Vec::new();
    let mut x = x0 + content_left;
    if let Some(w) = workspace {
        runs.push(Run::Text { x, text: w, color: Role::Accent });
        x += workspace_w;
    }
    runs.push(Run::Text { x, text: number, color: Role::Dim });
    x += number_w;
    for (i, app) in tab.icons.iter().enumerate() {
        runs.push(Run::Icon { x, app: app.clone(), dim: tab.dim, badge: tab.badges.get(i).copied().unwrap_or(false) });
        x += TAB_ICON + TAB_ICON_GAP;
    }
    if label_w > 0.0 {
        runs.push(Run::Text { x, text: label, color: text_color });
    }
    (body_w, runs)
}

/// Trim `text` to the longest prefix that fits `max` once an ellipsis is appended.
fn ellipsize(text: &str, max: f64, measure: Measure) -> String {
    if text.is_empty() || measure(text, FONT_SIZE) <= max {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let fits = |n: usize| {
        let candidate: String = chars[..n].iter().chain(std::iter::once(&'…')).collect();
        measure(&candidate, FONT_SIZE) <= max
    };
    // Binary search: width grows monotonically with the prefix length.
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().chain(std::iter::once(&'…')).collect()
}

/// A picker row's leading marker.
pub fn row_marker(favorite: bool, off_workspace: bool) -> &'static str {
    if favorite {
        FAVORITE_MARK
    } else if off_workspace {
        WORKSPACE_MARK
    } else {
        " " // reserve the gutter, so index numbers stay column-aligned
    }
}
