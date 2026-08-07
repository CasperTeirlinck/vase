//! Tab-bar, pane-placeholder, and focus-border rendering, plus the bar-index
//! and per-window monitor helpers they use.

use vase_core::geometry::Rect;

use super::util::{app_matches, clean_title};
use super::Daemon;
use crate::overlay::BarTab;

/// How long an ambiguous `prefix-<number>` waits for the next digit before
/// auto-committing.
const TAB_ENTRY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(120);

impl Daemon {
    fn total_tabs(&self) -> usize {
        self.model.as_ref().map_or(0, |m| m.screens.iter().map(|s| s.tabs.len()).sum())
    }

    fn select_bar_tab(&mut self, n: usize) {
        if let Some((si, ti)) = self.flat_tab_to_screen(n - 1) {
            self.dispatch(vase_core::model::Command::SelectScreenTab(si, ti));
        }
    }

    /// Begin a `prefix-<number>` selection with the first digit. Commits at once
    /// when no larger tab number could start with it; otherwise waits for more
    /// digits (see `tab_entry_key` / `tick_tab_entry`).
    pub fn begin_tab_entry(&mut self, first: usize) {
        if first * 10 > self.total_tabs() {
            self.select_bar_tab(first);
        } else {
            self.tab_entry = Some(first);
            self.tab_entry_deadline = Some(std::time::Instant::now() + TAB_ENTRY_TIMEOUT);
        }
    }

    /// Feed a key into an in-progress tab-number entry: a digit extends it (and
    /// commits once unambiguous), Esc cancels, anything else commits the number
    /// so far. Returns whether the key was consumed.
    pub fn tab_entry_key(&mut self, key: vase_core::input::Key) -> bool {
        let Some(n) = self.tab_entry else { return false };
        if key.mods == vase_core::input::Mods::default() {
            if let Some(d) = crate::keycodes::char_for_keycode(key.code).and_then(|c| c.to_digit(10))
            {
                let total = self.total_tabs();
                let new = n * 10 + d as usize;
                if new <= total {
                    self.tab_entry = Some(new);
                    self.tab_entry_deadline = Some(std::time::Instant::now() + TAB_ENTRY_TIMEOUT);
                    if new * 10 > total {
                        self.commit_tab_entry();
                    }
                    return true;
                }
            }
        }
        const VK_ESC: u16 = 0x35;
        if key.code == VK_ESC {
            self.tab_entry = None;
        } else {
            self.commit_tab_entry();
        }
        true
    }

    fn commit_tab_entry(&mut self) {
        self.tab_entry_deadline = None;
        if let Some(n) = self.tab_entry.take() {
            self.select_bar_tab(n);
        }
    }

    /// Auto-commit a pending tab-number entry once its deadline passes. Called
    /// on each (fine-grained) run-loop wake so the timeout stays crisp.
    pub fn tick_tab_entry(&mut self) {
        if self.tab_entry_deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            self.commit_tab_entry();
        }
    }

    /// Map a flat tab index (bar order: all of screen 0's tabs, then screen 1's,
    /// …) to `(screen index, tab-within-screen index)`.
    pub fn flat_tab_to_screen(&self, flat: usize) -> Option<(usize, usize)> {
        let model = self.model.as_ref()?;
        let mut acc = 0;
        for (si, s) in model.screens.iter().enumerate() {
            if flat < acc + s.tabs.len() {
                return Some((si, flat - acc));
            }
            acc += s.tabs.len();
        }
        None
    }

    /// Outline the focused pane when the current tab is split and the focused
    /// pane holds a window (an empty focused pane already shows its own border).
    pub(crate) fn refresh_focus_border(&mut self) {
        let model = self.model.as_ref().unwrap();
        if model.current_pane_count() > 1 && model.focused_window().is_some() {
            if let Some(rect) = model.focused_pane_rect() {
                self.focus_border.show(rect);
                return;
            }
        }
        self.focus_border.hide();
    }

    /// Draw the current tab's empty panes as dark placeholder containers (or
    /// hide the overlay when the current tab has none). When the pane picker is
    /// open it covers the focused empty pane, so skip that one box (the picker
    /// draws there); the non-focused empty panes still get plain boxes.
    pub fn refresh_panes(&mut self) {
        let panes = self.model.as_ref().unwrap().empty_panes();
        let boxes: Vec<(Rect, bool)> = if self.pane_picker.is_some() {
            panes.into_iter().filter(|(_, focused)| !focused).collect()
        } else {
            panes
        };
        self.pane_overlay.show(&boxes);
    }

    /// Draw the tab bar pinned to the reserved strip at the bottom of the screen,
    /// full width — one entry per tab (its representative window, or "(empty)").
    pub fn refresh_bar(&mut self) {
        // While the rename command line owns the bar, don't redraw over it (the
        // 100ms title poll would otherwise clobber the prompt).
        if self.prompt.is_some() {
            return;
        }
        let model = self.model.as_ref().unwrap();
        let screen = model.screens[self.main_screen].rect;
        let zoomed = model.zoomed;
        let focused_screen = model.focused_screen;
        let (tabs, selected) = model.bar_tabs();
        if tabs.is_empty() {
            self.bar.hide();
            self.bar_rect = None;
            self.bar_ranges = Vec::new();
            return;
        }
        let bar_tabs: Vec<BarTab> = tabs
            .iter()
            .enumerate()
            .map(|(i, (windows, rep, name))| {
                let icons: Vec<String> = windows
                    .iter()
                    .filter_map(|id| self.names.get(id).cloned())
                    .collect();
                // Red dot per icon whose app shows a Dock notification badge.
                let badges: Vec<bool> = icons.iter().map(|a| self.badges.contains(a)).collect();
                // Mark the tab if any of its windows' apps has a focus hotkey.
                let hotkey = windows
                    .iter()
                    .any(|id| self.names.get(id).is_some_and(|n| self.is_hotkey_app(n)));
                let app = rep
                    .and_then(|id| self.names.get(&id).cloned())
                    .unwrap_or_default();
                let label = match name {
                    // A whitespace-only custom name renders as just the icon.
                    Some(n) if n.trim().is_empty() => String::new(),
                    Some(n) => n.clone(),
                    None => {
                        let title = rep
                            .and_then(|id| self.titles.get(&id).cloned())
                            .unwrap_or_default();
                        let ct = clean_title(&title, &app);
                        if ct.is_empty() { app } else { ct }
                    }
                };
                // Dim tabs not on the focused monitor; the number is the tab's
                // 1-based bar position (its `prefix-N` shortcut).
                let dim = self.flat_tab_to_screen(i).is_some_and(|(si, _)| si != focused_screen);
                BarTab {
                    icons,
                    badges,
                    label,
                    zoomed: zoomed && i == selected,
                    number: i + 1,
                    dim,
                    hotkey,
                }
            })
            .collect();
        // The bar's own CG rect: the reserved strip just below the content rect,
        // at the screen bottom, full width.
        let bar_rect =
            Rect::new(screen.x, screen.y + screen.h, screen.w, crate::overlay::BAR_HEIGHT);
        let ranges = self.bar.show(bar_rect, &bar_tabs, selected, self.prefix_armed, true);
        self.bar_rect = Some(bar_rect);
        self.bar_ranges = ranges;
    }

    /// Draw one local powerline bar in each visible stack's top strip (tabs only,
    /// no main chrome); hide the surplus pool bars.
    pub fn refresh_stack_bars(&mut self) {
        let stacks = self.model.as_ref().unwrap().stacks();
        while self.stack_bars.len() < stacks.len() {
            self.stack_bars.push(crate::TabBar::new(self.mtm));
        }
        self.stack_click.clear();
        for (bar, stack) in self.stack_bars.iter_mut().zip(&stacks) {
            let tabs: Vec<BarTab> = stack
                .items
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let app = self.names.get(id).cloned().unwrap_or_default();
                    // A custom nested-tab name (⌥e t) overrides the window title.
                    let label = match self.model.as_ref().unwrap().stack_names.get(id) {
                        Some(name) => name.clone(),
                        None => {
                            let title = self.titles.get(id).cloned().unwrap_or_default();
                            let ct = clean_title(&title, &app);
                            if ct.is_empty() { app.clone() } else { ct }
                        }
                    };
                    let badged = self.badges.contains(&app);
                    BarTab {
                        icons: vec![app],
                        badges: vec![badged],
                        label,
                        zoomed: false,
                        number: i + 1,
                        dim: false,
                        hotkey: false,
                    }
                })
                .collect();
            let bar_rect =
                Rect::new(stack.rect.x, stack.rect.y, stack.rect.w, crate::overlay::BAR_HEIGHT);
            let ranges = bar.show(bar_rect, &tabs, stack.selected, false, false);
            self.stack_click.push((bar_rect, ranges, stack.items.clone()));
        }
        for bar in &self.stack_bars[stacks.len()..] {
            bar.hide();
        }
    }


    /// Whether an app name has a focus-toggle hotkey (marks its tab in the bar).
    fn is_hotkey_app(&self, name: &str) -> bool {
        self.app_hotkeys.iter().any(|h| app_matches(name, &h.app))
    }
}
