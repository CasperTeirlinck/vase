//! Applying model effects to the OS: placing/raising windows and restoring frames.

use std::collections::HashSet;

use vase_core::backend::Backend;
use vase_core::geometry::{screen_of, Rect};
use vase_core::model::Effect;
use vase_core::tree::WindowId;

use super::{Daemon, REFRAME_SETTLE};

impl Daemon {
    pub fn execute(&mut self, effects: Vec<Effect>) {
        // A batch with a Render is a bring-forward (tab switch, Raise); a bare FocusWindow is a within-tab move. Only the former co-surfaces the tab's other panes.
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
                        // A minimized window keeps its tab but isn't placed.
                        if self.windows.is_minimized(*id) {
                            continue;
                        }
                        // Re-frame after a monitor change once it settles (some apps land short otherwise).
                        let previous = self.windows.get(*id).and_then(|w| w.placed);
                        let moved_display = previous.is_some_and(|old| screen_of(old, &self.screens_cg) != screen_of(*rect, &self.screens_cg));
                        if let Some(w) = self.windows.get_mut(*id) {
                            w.placed = Some(*rect);
                        }
                        self.backend.set_frame(*id, *rect);
                        if moved_display {
                            self.pending_reframe.push((*id, *rect));
                        }
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
                // `raise` fronts a whole app and can lift its off-tab windows over a sibling pane. Only VISIBLE panes (in `last_shown`) are surfaced. Gated to bring-forward batches
                // so within-tab focus moves don't flick.
                Effect::FocusWindow(id) => {
                    if bringing_forward {
                        if let Some(tab) = self.model.as_ref().and_then(|m| m.focused_tab()) {
                            for sib in vase_core::tree::windows(&tab.root) {
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
        self.overlays.hide_bars();
        let originals: Vec<(WindowId, Rect)> = self.windows.iter().map(|(id, w)| (id, w.original)).collect();
        for (id, rect) in &originals {
            self.backend.set_frame(*id, *rect);
        }
        println!("vase: restored {} windows.", originals.len());
    }
}
