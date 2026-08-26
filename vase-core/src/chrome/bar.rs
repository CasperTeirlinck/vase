//! What a bar shows, and the pieces every style lays its content out of.
//!
//! How a bar is *drawn* belongs to a style: `chrome::powerline` lays out vase's own powerline bar
//! for any platform, a native style lays itself out in its platform's crate.

use super::theme::Role;
use super::{FAVORITE_MARK, FONT_SIZE, WORKSPACE_MARK, ZOOM_MARK};
use crate::geometry::Rect;

/// Measures a string's width, at `size` points, in the painter's own font.
pub type Measure<'a> = &'a dyn Fn(&str, f64) -> f64;

/// Where each tab of a drawn bar can be clicked: bar-local x spans, in tab order.
pub type Hits = Vec<(f64, f64)>;

/// A bar to draw: the screen's tab bar, or one stack's local bar.
pub struct Bar<'a> {
    pub rect: Rect,
    pub tabs: &'a [BarTab],
    pub selected: usize,
    /// Running apps with no window of their own, trailing the tabs as bare icons. Only the screen's
    /// tab bar carries them.
    pub apps: &'a [String],
    /// The screen's tab bar, which carries the mark and the prefix indicator. A stack bar carries neither.
    pub main: bool,
    /// The prefix chord is armed.
    pub armed: bool,
}

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

/// A positioned piece of a tab's content, in whatever coordinates the style laid the tab out in.
#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    Text { x: f64, text: String, color: Role },
    Icon { x: f64, app: String, dim: bool, badge: bool },
}

/// The spacing a style lays a tab's content out by.
pub struct Metrics {
    /// Inset from the tab's own x0 to its first run, clearing whatever shape the style starts the tab with.
    pub content_left: f64,
    pub right_pad: f64,
    /// Gap between the position number and the first icon.
    pub number_gap: f64,
    pub icon: f64,
    pub icon_gap: f64,
    /// Label width past which a label ellipsizes.
    pub max_label: f64,
}

/// One tab's content: its width, and its runs placed from `x0`.
///
/// The order is the same in every style: markers, position number, app icons, label.
pub fn content(tab: &BarTab, x0: f64, m: &Metrics, measure: Measure) -> (f64, Vec<Run>) {
    let text_color = if tab.dim { Role::Dim } else { Role::Text };
    let label = ellipsize(&tab.label, m.max_label, measure);
    let label_w = if label.is_empty() { 0.0 } else { measure(&label, FONT_SIZE) };

    let marks = marks(tab);
    let number = format!("{} ", tab.number);
    let marks_w = marks.as_deref().map_or(0.0, |s| measure(s, FONT_SIZE));
    let number_w = measure(&number, FONT_SIZE);

    let n = tab.icons.len() as f64;
    // A trailing label needs a gap after the last icon; a tab that is icons-only does not.
    let icons_w = if label_w > 0.0 { n * (m.icon + m.icon_gap) } else { n * m.icon + (n - 1.0).max(0.0) * m.icon_gap };
    let body_w = m.content_left + marks_w + number_w + m.number_gap + icons_w + label_w + m.right_pad;

    let mut runs = Vec::new();
    let mut x = x0 + m.content_left;
    if let Some(marks) = marks {
        runs.push(Run::Text { x, text: marks, color: Role::Accent });
        x += marks_w;
    }
    runs.push(Run::Text { x, text: number, color: Role::Dim });
    x += number_w + m.number_gap;
    for (i, app) in tab.icons.iter().enumerate() {
        runs.push(Run::Icon { x, app: app.clone(), dim: tab.dim, badge: tab.badges.get(i).copied().unwrap_or(false) });
        x += m.icon + m.icon_gap;
    }
    if label_w > 0.0 {
        runs.push(Run::Text { x, text: label, color: text_color });
    }
    (body_w, runs)
}

/// A tab's leading markers: on another workspace, and zoomed.
fn marks(tab: &BarTab) -> Option<String> {
    let mut out = String::new();
    if tab.off_workspace {
        out.push_str(WORKSPACE_MARK);
    }
    if tab.zoomed {
        out.push_str(ZOOM_MARK);
    }
    (!out.is_empty()).then(|| format!("{out} "))
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

/// Each trailing app icon's x, in order, the cluster right-aligned to end at `right`.
pub fn app_icons(count: usize, right: f64, icon: f64, gap: f64) -> Vec<f64> {
    let left = right - count as f64 * (icon + gap) + gap;
    (0..count).map(|i| left + i as f64 * (icon + gap)).collect()
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
