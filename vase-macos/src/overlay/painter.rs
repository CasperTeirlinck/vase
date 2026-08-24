//! The AppKit painter: every surface the core asks for, drawn as non-activating panels.

use objc2::MainThreadMarker;
use vase_core::chrome::bar::{Bar, Hits};
use vase_core::chrome::help::HelpLayout;
use vase_core::chrome::BarHits;
use vase_core::chrome::{ListAt, Painter, SwitchRow};
use vase_core::geometry::Rect;

use super::help::HelpView;
use super::panes::{FocusBorder, PaneOverlay};
use super::switcher::SwitcherView;
use super::tab_bar::TabBar;

pub struct AppKitPainter {
    bar: TabBar,
    /// Pool of local bars, one drawn in each visible stack's top strip.
    stack_bars: Vec<TabBar>,
    panes: PaneOverlay,
    focus_border: FocusBorder,
    /// Shared panel behind the window switcher, the pane picker, and the launching placeholder.
    list: SwitcherView,
    help: HelpView,
    mtm: MainThreadMarker,
}

impl AppKitPainter {
    pub fn new(mtm: MainThreadMarker) -> AppKitPainter {
        AppKitPainter { bar: TabBar::new(mtm), stack_bars: Vec::new(), panes: PaneOverlay::new(mtm), focus_border: FocusBorder::new(mtm), list: SwitcherView::new(mtm), help: HelpView::new(mtm), mtm }
    }
}

impl Painter for AppKitPainter {
    fn measure(&self, text: &str, size: f64) -> f64 {
        super::text::measure(text, size)
    }

    fn bar(&mut self, bar: &Bar) -> BarHits {
        self.bar.show(bar)
    }

    fn prompt(&mut self, rect: Rect, text: &str) {
        self.bar.show_prompt(rect, text);
    }

    fn hide_bar(&mut self) {
        self.bar.hide();
    }

    fn stack_bars(&mut self, bars: &[Bar]) -> Vec<Hits> {
        while self.stack_bars.len() < bars.len() {
            self.stack_bars.push(TabBar::new(self.mtm));
        }
        // A stack bar carries no trailing icons, so only its tab spans can be clicked.
        let hits = self.stack_bars.iter_mut().zip(bars).map(|(surface, bar)| surface.show(bar).tabs).collect();
        for surface in &self.stack_bars[bars.len()..] {
            surface.hide();
        }
        hits
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

    fn help(&mut self, layout: &HelpLayout) {
        self.help.show(layout);
    }

    fn hide_help(&mut self) {
        self.help.hide();
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
        self.help.hide();
    }

    fn prewarm_icon(&mut self, app: &str) {
        super::text::prewarm_icon(app);
    }
}
