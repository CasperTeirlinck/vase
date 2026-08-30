//! The painter seam: what a platform must draw, once the core has decided what goes where.

use crate::geometry::Rect;

use super::bar::{Bar, Hits};
use super::help::HelpLayout;
use super::Position;

/// Where a drawn tab bar's clickable pieces landed.
#[derive(Default)]
pub struct BarHits {
    pub tabs: Hits,
    /// The trailing windowless-app icons, in the order they were given.
    pub apps: Hits,
}

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
/// The core decides *what* every surface shows and where it sits; the style decides how it is drawn.
/// A painter drawing vase's own powerline bar lays it out with `chrome::powerline`; one drawing its
/// platform's native chrome lays that out itself, and hands back where the tabs landed so clicks
/// still route.
pub trait Painter {
    /// Width of `text` at `size` points, in the font this painter draws bar and list text in.
    fn measure(&self, text: &str, size: f64) -> f64;

    /// Draw the screen's tab bar, returning where its clickable pieces landed.
    fn bar(&mut self, bar: &Bar) -> BarHits;

    /// Draw the bar as a command line rather than tabs: the strip and the leading mark, then `text`.
    fn prompt(&mut self, rect: Rect, position: Position, text: &str);

    fn hide_bar(&mut self);

    /// Draw one local bar per visible stack, growing the surface pool to fit.
    fn stack_bars(&mut self, bars: &[Bar]) -> Vec<Hits>;

    /// Placeholder containers for the current tab's empty panes; the flag marks the focused one.
    fn panes(&mut self, panes: &[(Rect, bool)]);

    fn focus_border(&mut self, rect: Option<Rect>);

    fn list(&mut self, at: ListAt, header: &str, rows: &[SwitchRow], selected: usize);

    fn hide_list(&mut self);

    /// Draw the shortcut sheet as a card, centered where the layout puts it.
    fn help(&mut self, layout: &HelpLayout);

    fn hide_help(&mut self);

    fn hide_bars(&mut self);

    fn hide_all(&mut self);

    /// Resolve and cache an app's icon. Called off the render path, since a lookup can block.
    fn prewarm_icon(&mut self, app: &str);
}
