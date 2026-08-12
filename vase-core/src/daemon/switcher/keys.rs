//! The switcher's display-name helpers, rendering, and vim-modal key handling.

use std::time::Instant;

use crate::input::{Key, Pick};
use crate::model::Command;
use crate::registry::clean_title;
use crate::tree::{windows, WindowId};

use super::SwitchTarget;
use crate::backend::Backend;
use crate::chrome::Painter;
use crate::chrome::{ListAt, SwitchRow};
use crate::daemon::Daemon;

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// A window's cleaned title, or its app when the title is empty.
    pub(crate) fn title_of(&self, id: WindowId) -> String {
        let app = self.windows.app(id);
        let title = clean_title(self.windows.title(id), app);
        if title.is_empty() {
            app.to_string()
        } else {
            title
        }
    }

    /// A window's display name: nested-stack custom name, else tab name (top-level only), else title, else app.
    pub(crate) fn window_display(&self, id: WindowId, in_stack: bool) -> String {
        let model = self.model.as_ref().unwrap();
        if let Some(n) = model.stack_names.get(&id).filter(|n| !n.trim().is_empty()) {
            return n.clone();
        }
        if !in_stack {
            if let Some(n) = self.window_tab_name(id).filter(|n| !n.trim().is_empty()) {
                return n;
            }
        }
        self.title_of(id)
    }

    /// The custom name of the tab holding this window, if it has one.
    fn window_tab_name(&self, id: WindowId) -> Option<String> {
        let model = self.model.as_ref()?;
        for s in &model.screens {
            for t in &s.tabs {
                if windows(&t.root).contains(&id) {
                    return t.name.clone();
                }
            }
        }
        None
    }

    /// "App - window title", or just the app if the title is empty.
    pub(crate) fn switcher_label(&self, id: WindowId) -> String {
        let app = match self.windows.app(id) {
            "" => "?",
            app => app,
        };
        let ct = clean_title(self.windows.title(id), app);
        if ct.is_empty() {
            app.to_string()
        } else {
            format!("{app} - {ct}")
        }
    }

    pub(super) fn render_switcher(&mut self) {
        // Snapshot the visible rows so the switcher borrow ends before touching the overlays.
        let Some((rows, is_searching, query, selected)) = self.switcher.as_ref().map(|s| {
            let rows: Vec<SwitchRow> = s
                .visible()
                .into_iter()
                .enumerate()
                .map(|(i, (it, _))| SwitchRow {
                    number: i + 1,
                    prefix: it.prefix,
                    icons: it.icons,
                    label: it.display,
                    dim: it.dim,
                    off_workspace: it.off_workspace,
                    favorite: false,
                    current: it.current,
                })
                .collect();
            (rows, s.is_searching(), s.query().to_string(), s.selected())
        }) else {
            return;
        };
        let header = if is_searching { format!("/ {query}") } else { "  j / k  move    1-9  open    /  search    ⏎  open    esc  close".to_string() };
        // Always on the main (menu-bar) display.
        let screen = self.model.as_ref().unwrap().screens[self.main_screen].rect;
        self.chrome.list(ListAt::Centered(screen), &header, &rows, selected);
    }

    fn close_switcher(&mut self) {
        self.switcher = None;
        self.chrome.hide_list();
    }

    fn open_switch_target(&mut self, target: SwitchTarget) {
        self.close_switcher();
        match target {
            SwitchTarget::Window(id) => self.dispatch(Command::Raise(id)),
            SwitchTarget::Tab(si, ti) => self.dispatch(Command::SelectScreenTab(si, ti)),
        }
    }

    /// Auto-commit a half-typed index once its deadline passes (run-loop tick).
    pub fn tick_switcher(&mut self) {
        let Some(s) = &mut self.switcher else { return };
        if let Pick::Chosen(item) = s.tick(Instant::now()) {
            self.open_switch_target(item.target);
        }
    }

    /// Handle a key while the switcher is open; consumes while open.
    pub fn switcher_key(&mut self, key: Key) -> bool {
        let Some(s) = &mut self.switcher else { return false };
        match s.key(key, Instant::now()) {
            Pick::Ignored => {}
            Pick::Redraw => self.render_switcher(),
            Pick::Chosen(item) => self.open_switch_target(item.target),
            Pick::Cancelled => self.close_switcher(),
        }
        true
    }
}
