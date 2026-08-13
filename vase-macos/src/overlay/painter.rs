//! The AppKit painter: every surface the core asks for, drawn as non-activating panels.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSFont};
use vase_core::chrome::bar::BarLayout;
use vase_core::chrome::{ListAt, Painter, SwitchRow};
use vase_core::geometry::Rect;

use super::panes::{FocusBorder, PaneOverlay};
use super::switcher::SwitcherView;
use super::tab_bar::TabBar;
use super::text::segment;
use super::theme::text_col;

pub struct AppKitPainter {
    bar: TabBar,
    /// Pool of local powerline bars, one drawn in each visible stack's top strip.
    stack_bars: Vec<TabBar>,
    panes: PaneOverlay,
    focus_border: FocusBorder,
    /// Shared panel behind the window switcher, the pane picker, and the launching placeholder.
    list: SwitcherView,
    mtm: MainThreadMarker,
}

impl AppKitPainter {
    pub fn new(mtm: MainThreadMarker) -> AppKitPainter {
        AppKitPainter { bar: TabBar::new(mtm), stack_bars: Vec::new(), panes: PaneOverlay::new(mtm), focus_border: FocusBorder::new(mtm), list: SwitcherView::new(mtm), mtm }
    }
}

impl Painter for AppKitPainter {
    fn measure(&self, text: &str, size: f64) -> f64 {
        let font = NSFont::monospacedSystemFontOfSize_weight(size, 0.0);
        segment(text, &font, &text_col(), None).size().width
    }

    fn bar(&mut self, layout: &BarLayout) {
        self.bar.show(layout);
    }

    fn prompt(&mut self, layout: &BarLayout, text: &str) {
        self.bar.show_prompt(layout, text);
    }

    fn hide_bar(&mut self) {
        self.bar.hide();
    }

    fn stack_bars(&mut self, layouts: &[BarLayout]) {
        while self.stack_bars.len() < layouts.len() {
            self.stack_bars.push(TabBar::new(self.mtm));
        }
        for (surface, layout) in self.stack_bars.iter_mut().zip(layouts) {
            surface.show(layout);
        }
        for surface in &self.stack_bars[layouts.len()..] {
            surface.hide();
        }
    }

    fn panes(&mut self, panes: &[(Rect, bool)]) {
        self.panes.show(panes);
    }

    fn focus_border(&mut self, rect: Option<Rect>) {
        match rect {
            Some(rect) => self.focus_border.show(rect),
            None => self.focus_border.hide(),
        }
    }

    fn list(&mut self, at: ListAt, header: &str, rows: &[SwitchRow], selected: usize) {
        match at {
            ListAt::Centered(screen) => self.list.show(screen, header, rows, selected),
            ListAt::Filling(area) => self.list.show_in(area, header, rows, selected),
        }
    }

    fn hide_list(&mut self) {
        self.list.hide();
    }

    fn hide_bars(&mut self) {
        self.bar.hide();
        for bar in &self.stack_bars {
            bar.hide();
        }
    }

    fn hide_all(&mut self) {
        self.hide_bars();
        self.panes.hide();
        self.focus_border.hide();
        self.list.hide();
    }

    fn prewarm_icon(&mut self, app: &str) {
        super::text::prewarm_icon(app);
    }
}
