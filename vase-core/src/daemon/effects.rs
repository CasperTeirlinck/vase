//! Applying model effects to the OS: placing/raising windows and restoring frames.

use std::collections::HashSet;

use crate::backend::Backend;
use crate::geometry::{any_covered, screen_of, Rect};
use crate::model::Effect;
use crate::tree::WindowId;

use super::{Daemon, REFRAME_SETTLE};
use crate::chrome::Painter;

impl<B: Backend, C: Painter> Daemon<B, C> {
    pub fn execute(&mut self, effects: Vec<Effect>) {
        // A batch with a Render has just moved windows, so it re-fronts the focused tab whatever the
        // z-order says; a bare FocusWindow checks first, since re-fronting a clear tab costs a flick.
        let bringing_forward = effects.iter().any(|e| matches!(e, Effect::Render(_)));
        // Selecting a minimized window's tab restores it: un-minimize before the Render places it.
        if let Some(w) = effects.iter().find_map(|e| match e {
            Effect::FocusWindow(w) => Some(*w),
            _ => None,
        }) {
            if self.windows.is_minimized(w) {
                self.windows.set_minimized(w, false);
                self.backend.set_minimized(w, false);
            }
        }
        for effect in effects {
            match effect {
                Effect::Render(placements) => {
                    let shown: HashSet<WindowId> = placements.iter().map(|(id, _)| *id).collect();
                    for (id, rect) in &placements {
                        self.place(*id, *rect);
                    }
                    if !self.pending_reframe.is_empty() {
                        self.reframe_deadline = Some(std::time::Instant::now() + REFRAME_SETTLE);
                    }
                    // Raise only NEWLY-shown windows: re-raising an already-visible window on another display fronts its app and flickers that display. Use `raise`, not `focus`:
                    // the trailing FocusWindow effect sets real focus.
                    if shown != self.last_shown {
                        for (id, _) in &placements {
                            if self.windows.is_minimized(*id) || self.last_shown.contains(id) {
                                continue;
                            }
                            self.backend.raise(*id);
                        }
                        self.last_shown = shown;
                    }
                }
                // Bring every visible pane of the focused tab to the front, then focus the target last: a tab's layout is all-or-nothing, no holes. `focus` (SkyLight) raises each specific window;
                // `raise` fronts a whole app and can lift its off-tab windows over a sibling pane. Only VISIBLE panes (in `last_shown`) are surfaced.
                Effect::FocusWindow(id) => {
                    if bringing_forward || self.tab_partly_covered() {
                        if let Some(tab) = self.model.as_ref().and_then(|m| m.focused_tab()) {
                            for sib in crate::tree::windows(&tab.root) {
                                if sib != id && self.last_shown.contains(&sib) {
                                    self.backend.focus(sib);
                                }
                            }
                        }
                    }
                    self.backend.focus(id);
                }
            }
        }
    }

    /// Whether another window covers one of the focused tab's visible panes, which a focus move has to
    /// clear so the tab is never left showing in part.
    fn tab_partly_covered(&mut self) -> bool {
        let Some(tab) = self.model.as_ref().and_then(|m| m.focused_tab()) else { return false };
        let panes: HashSet<WindowId> = crate::tree::windows(&tab.root).into_iter().filter(|w| self.last_shown.contains(w)).collect();
        // A single pane is fronted by the focus itself; only siblings can be left behind.
        if panes.len() < 2 {
            return false;
        }
        // Panels, menus and vase's own chrome sit above by design, so only normal windows can cover a pane.
        let stack: Vec<(WindowId, Rect)> = self.backend.list_windows().into_iter().filter(|w| w.layer == 0).map(|w| (w.id, w.frame)).collect();
        any_covered(&stack, &panes)
    }

    /// Move one window onto `rect` and remember the placement. A minimized window keeps its tab but is
    /// not placed.
    pub(super) fn place(&mut self, id: WindowId, rect: Rect) {
        if self.windows.is_minimized(id) {
            return;
        }
        // Re-frame after a monitor change once it settles (some apps land short otherwise).
        let previous = self.windows.get(id).and_then(|w| w.placed);
        let moved_display = previous.is_some_and(|old| screen_of(old, &self.screens_cg) != screen_of(rect, &self.screens_cg));
        if let Some(w) = self.windows.get_mut(id) {
            w.placed = Some(rect);
        }
        self.backend.set_frame(id, rect);
        if moved_display {
            self.pending_reframe.push((id, rect));
        }
    }

    /// Re-assert the frames of windows that moved to another monitor, once they've settled (after `REFRAME_SETTLE`).
    pub fn tick_reframe(&mut self) {
        if self.reframe_deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            self.reframe_deadline = None;
            for (id, rect) in std::mem::take(&mut self.pending_reframe) {
                if !self.windows.is_minimized(id) {
                    self.backend.set_frame(id, rect);
                }
            }
        }
    }

    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        self.save_state(); // capture final layout before teardown
        self.chrome.hide_bars();
        let originals: Vec<(WindowId, Rect)> = self.windows.iter().map(|(id, w)| (id, w.original)).collect();
        for (id, rect) in &originals {
            self.backend.set_frame(*id, *rect);
        }
        println!("vase: restored {} windows.", originals.len());
    }
}
