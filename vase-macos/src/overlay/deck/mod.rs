//! Every surface vase paints on top of the windows, behind one redraw call.
//!
//! The surfaces have to agree with each other: the stack bars sit inside the rects the pane
//! placeholders leave, the focus border traces the pane the tab bar highlights, and the click maps
//! are only valid for what was last drawn. Redrawing them one at a time from the call site got that
//! wrong three different ways, so `sync` is the only way in.

use std::collections::HashSet;

use objc2::MainThreadMarker;
use vase_core::geometry::Rect;

use super::{FocusBorder, PaneOverlay, SwitchRow, SwitcherView, TabBar};
use crate::config::AppFocus;
use crate::registry::Registry;
use bars::ClickMap;

mod bars;

/// What the overlays need on top of the model to draw a frame.
pub struct Chrome<'a> {
    pub windows: &'a Registry,
    /// Apps showing a Dock notification badge; drives the red dot on their tabs.
    pub badges: &'a HashSet<String>,
    /// Apps with a focus-toggle hotkey; their tabs get a marker.
    pub hotkeys: &'a [AppFocus],
    /// Display the tab bar lives on.
    pub main_screen: usize,
    /// Whether the prefix chord is armed; drives the prefix dot.
    pub prefix_armed: bool,
    /// Command line contents, drawn in place of the tabs while it is open.
    pub prompt: Option<String>,
    /// The picker covers the focused empty pane, so no placeholder is drawn there.
    pub picker_open: bool,
}

pub struct Overlays {
    bar: TabBar,
    /// Pool of local powerline bars, one drawn in each visible stack's top strip.
    stack_bars: Vec<TabBar>,
    /// Placeholder overlay drawing the current tab's empty panes.
    panes: PaneOverlay,
    /// Accent outline drawn around the focused pane when the tab is split.
    focus_border: FocusBorder,
    /// Shared panel behind the window switcher, the pane picker, and the launching placeholder.
    list: SwitcherView,
    /// Hit ranges from the last `sync`, in the same order the bars were drawn.
    bar_hits: Option<(Rect, Vec<(f64, f64)>)>,
    stack_hits: Vec<ClickMap>,
    mtm: MainThreadMarker,
}

impl Overlays {
    pub fn new(mtm: MainThreadMarker) -> Overlays {
        Overlays {
            bar: TabBar::new(mtm),
            stack_bars: Vec::new(),
            panes: PaneOverlay::new(mtm),
            focus_border: FocusBorder::new(mtm),
            list: SwitcherView::new(mtm),
            bar_hits: None,
            stack_hits: Vec::new(),
            mtm,
        }
    }

    /// Draw a list (the switcher) centred on `screen`.
    pub fn show_list(&mut self, screen: Rect, header: &str, rows: &[SwitchRow], selected: usize) {
        self.list.show(screen, header, rows, selected);
    }

    /// Draw a list (the pane picker, or the launching placeholder) filling `area`.
    pub fn show_list_in(&mut self, area: Rect, header: &str, rows: &[SwitchRow], selected: usize) {
        self.list.show_in(area, header, rows, selected);
    }

    pub fn hide_list(&self) {
        self.list.hide();
    }

    /// Hide every bar, on the way out.
    pub fn hide_bars(&self) {
        self.bar.hide();
        for bar in &self.stack_bars {
            bar.hide();
        }
    }
}
