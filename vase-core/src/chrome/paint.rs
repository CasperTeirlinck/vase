//! The painter seam: what a platform must draw, once the core has decided what goes where.

use crate::geometry::Rect;

use super::bar::BarLayout;

/// One row of a list.
pub struct SwitchRow {
    pub number: usize,
    /// Tree glyph for a nested row.
    pub prefix: String,
    /// App names, one icon each.
    pub icons: Vec<String>,
    pub label: String,
    /// On a non-focused monitor.
    pub dim: bool,
    pub off_workspace: bool,
    /// A favorite app, in a picker launch row.
    pub favorite: bool,
    /// The currently-focused window.
    pub current: bool,
}

/// Where a list is drawn.
pub enum ListAt {
    /// Centred on a screen, as a card: the window switcher.
    Centered(Rect),
    /// Filling a rect: the pane picker, inside the empty pane it opened over.
    Filling(Rect),
}

/// Everything vase paints, as native drawing calls.
///
/// The core lays every surface out and an implementation strokes what it is given, so the bar's
/// proportions are the same on every platform. `measure` is the one number only a platform can
/// supply, and the core feeds it back into the layout.
pub trait Painter {
    /// Width of `text` at `size` points, in the font this painter draws bar and list text in.
    fn measure(&self, text: &str, size: f64) -> f64;

    fn bar(&mut self, layout: &BarLayout);

    /// Draw the bar as a command line rather than tabs. `layout` carries the strip and the leading
    /// mark; it has no tabs.
    fn prompt(&mut self, layout: &BarLayout, text: &str);

    fn hide_bar(&mut self);

    /// Draw one local bar per visible stack, growing the surface pool to fit.
    fn stack_bars(&mut self, layouts: &[BarLayout]);

    /// Placeholder containers for the current tab's empty panes; the flag marks the focused one.
    fn panes(&mut self, panes: &[(Rect, bool)]);

    fn focus_border(&mut self, rect: Option<Rect>);

    fn list(&mut self, at: ListAt, header: &str, rows: &[SwitchRow], selected: usize);

    fn hide_list(&mut self);

    fn hide_bars(&mut self);

    fn hide_all(&mut self);

    /// Resolve and cache an app's icon. Called off the render path, since a lookup can block.
    fn prewarm_icon(&mut self, app: &str);
}
