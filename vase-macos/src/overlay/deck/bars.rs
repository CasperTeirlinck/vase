//! Drawing the tab bars from the model and routing clicks back to commands.

use vase_core::geometry::Rect;
use vase_core::model::{Command, Model};
use vase_core::tree::WindowId;

use super::{Chrome, Overlays};
use crate::daemon::{app_matches, clean_title};
use crate::overlay::{BarTab, TabBar, BAR_HEIGHT};

/// A drawn bar's click map: its CG rect, per-tab hit ranges, and what each range selects.
pub(super) type ClickMap = (Rect, Vec<(f64, f64)>, Vec<WindowId>);

impl Overlays {
    /// Redraw every surface from the model. Nothing else may draw them.
    pub fn sync(&mut self, model: &Model, chrome: &Chrome) {
        self.sync_bar(model, chrome);
        self.sync_stack_bars(model, chrome);
        self.sync_panes(model, chrome);
        self.sync_focus_border(model);
    }

    /// Route a click at a CG point to the command it selects.
    pub fn hit(&self, model: &Model, px: f64, py: f64) -> Option<Command> {
        route_click(model, self.bar_hits.as_ref(), &self.stack_hits, px, py)
    }

    fn sync_bar(&mut self, model: &Model, chrome: &Chrome) {
        let screen = model.screens[chrome.main_screen].rect;
        // The bar's CG rect: the reserved strip below the content rect, full width.
        let bar_rect = Rect::new(screen.x, screen.y + screen.h, screen.w, BAR_HEIGHT);
        // While the command line is open it owns the bar; no tabs, and no click targets.
        if let Some(line) = &chrome.prompt {
            self.bar.show_prompt(screen, line);
            self.bar_hits = None;
            return;
        }
        let (tabs, selected) = model.bar_tabs();
        if tabs.is_empty() {
            self.bar.hide();
            self.bar_hits = None;
            return;
        }
        let bar_tabs: Vec<BarTab> = tabs
            .iter()
            .enumerate()
            .map(|(i, (windows, rep, name))| {
                let icons: Vec<String> = windows.iter().map(|id| chrome.windows.app(*id).to_string()).collect();
                let badges: Vec<bool> = icons.iter().map(|a| chrome.badges.contains(a)).collect();
                let hotkey = icons.iter().any(|a| chrome.hotkeys.iter().any(|h| app_matches(a, &h.app)));
                let app = rep.map(|id| chrome.windows.app(id).to_string()).unwrap_or_default();
                let label = match name {
                    // A whitespace-only custom name renders as just the icon.
                    Some(n) if n.trim().is_empty() => String::new(),
                    Some(n) => n.clone(),
                    None => {
                        let ct = clean_title(rep.map(|id| chrome.windows.title(id)).unwrap_or_default(), &app);
                        if ct.is_empty() {
                            app
                        } else {
                            ct
                        }
                    }
                };
                // Dim tabs not on the focused monitor; the number is the tab's `prefix-N` shortcut.
                let dim = flat_tab(model, i).is_some_and(|(si, _)| si != model.focused_screen);
                let off_space = windows.iter().any(|id| chrome.off_space.contains(id));
                BarTab { icons, badges, label, zoomed: model.zoomed && i == selected, number: i + 1, dim, off_space, hotkey }
            })
            .collect();
        let ranges = self.bar.show(bar_rect, &bar_tabs, selected, chrome.prefix_armed, true);
        self.bar_hits = Some((bar_rect, ranges));
    }

    fn sync_stack_bars(&mut self, model: &Model, chrome: &Chrome) {
        let stacks = model.stacks();
        while self.stack_bars.len() < stacks.len() {
            self.stack_bars.push(TabBar::new(self.mtm));
        }
        self.stack_hits.clear();
        for (bar, stack) in self.stack_bars.iter_mut().zip(&stacks) {
            let tabs: Vec<BarTab> = stack
                .items
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let app = chrome.windows.app(*id).to_string();
                    // A custom nested-tab name overrides the window title.
                    let label = match model.stack_names.get(id) {
                        Some(name) => name.clone(),
                        None => {
                            let ct = clean_title(chrome.windows.title(*id), &app);
                            if ct.is_empty() {
                                app.clone()
                            } else {
                                ct
                            }
                        }
                    };
                    let badged = chrome.badges.contains(&app);
                    let off_space = chrome.off_space.contains(id);
                    BarTab { icons: vec![app], badges: vec![badged], label, zoomed: false, number: i + 1, dim: false, off_space, hotkey: false }
                })
                .collect();
            let bar_rect = Rect::new(stack.rect.x, stack.rect.y, stack.rect.w, BAR_HEIGHT);
            let ranges = bar.show(bar_rect, &tabs, stack.selected, false, false);
            self.stack_hits.push((bar_rect, ranges, stack.items.clone()));
        }
        for bar in &self.stack_bars[stacks.len()..] {
            bar.hide();
        }
    }

    fn sync_panes(&mut self, model: &Model, chrome: &Chrome) {
        let panes = model.empty_panes();
        let boxes: Vec<(Rect, bool)> = if chrome.picker_open { panes.into_iter().filter(|(_, focused)| !focused).collect() } else { panes };
        self.panes.show(&boxes);
    }

    /// Outline the focused pane when the tab is split and the pane holds a window.
    fn sync_focus_border(&mut self, model: &Model) {
        if model.current_pane_count() > 1 && model.focused_window().is_some() {
            if let Some(rect) = model.focused_pane_rect() {
                self.focus_border.show(rect);
                return;
            }
        }
        self.focus_border.hide();
    }
}

/// The command a click resolves to, against the click maps left by the last `sync`.
/// Separated from `Overlays` so it can be tested without an AppKit main thread.
fn route_click(model: &Model, bar: Option<&(Rect, Vec<(f64, f64)>)>, stacks: &[ClickMap], px: f64, py: f64) -> Option<Command> {
    // A stack bar is drawn over the tab it belongs to, so it gets first refusal.
    for (rect, ranges, ids) in stacks {
        if let Some(id) = hit_range(*rect, ranges, px, py).and_then(|i| ids.get(i).copied()) {
            return Some(Command::SelectStackWindow(id));
        }
    }
    let (rect, ranges) = bar?;
    let flat = hit_range(*rect, ranges, px, py)?;
    let (si, ti) = flat_tab(model, flat)?;
    Some(Command::SelectScreenTab(si, ti))
}

/// Index of the horizontal range a point falls in, if the point is inside `rect` at all.
fn hit_range(rect: Rect, ranges: &[(f64, f64)], px: f64, py: f64) -> Option<usize> {
    if px < rect.x || px >= rect.x + rect.w || py < rect.y || py >= rect.y + rect.h {
        return None;
    }
    let local = px - rect.x;
    ranges.iter().position(|(a, b)| local >= *a && local < *b)
}

/// Split a flat (bar-order) tab index into `(screen, tab within that screen)`.
fn flat_tab(model: &Model, flat: usize) -> Option<(usize, usize)> {
    let mut acc = 0;
    for (si, s) in model.screens.iter().enumerate() {
        if flat < acc + s.tabs.len() {
            return Some((si, flat - acc));
        }
        acc += s.tabs.len();
    }
    None
}

#[cfg(test)]
#[path = "../deck_test.rs"]
mod tests;
