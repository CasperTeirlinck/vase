//! Live-window/display reconciliation: a poll that adopts, drops, and re-tiles windows and follows OS focus.

use std::collections::{HashMap, HashSet};

use crate::backend::{manageable, Backend, WindowInfo};
use crate::geometry::{screen_of, Rect};
use crate::model::{Command, Effect, Screen, Tab};
use crate::tree::WindowId;

use super::Daemon;
use crate::chrome::Painter;

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// Pull each managed window's current title via Accessibility; returns whether any changed.
    fn refresh_titles(&mut self) -> bool {
        let ids = self.model.as_ref().unwrap().all_windows();
        let mut changed = false;
        for id in ids {
            if let Some(t) = self.backend.title(id) {
                changed |= self.windows.set_title(id, t);
            }
        }
        changed
    }

    /// Take a new window under management and give it a home in the layout.
    fn adopt(&mut self, w: &WindowInfo) {
        self.windows.adopt(w, false);
        // Warm this window's app icon so its tab shows it right away.
        self.chrome.prewarm_icon(&w.app);
        // A pending launch's window fills the focused empty pane instead of opening a new tab.
        if self.launch_matches(w) {
            self.pending_launch = None;
            self.dispatch(Command::FillPane(w.id));
        } else {
            let si = screen_of(w.frame, &self.screens_cg);
            self.dispatch(Command::AddWindow(w.id, si));
        }
    }

    /// Drop a window that is gone: out of the layout, out of the registry, out of the backend's caches.
    fn forget(&mut self, id: WindowId) {
        self.dispatch(Command::RemoveWindow(id));
        self.windows.forget(id);
        self.last_shown.remove(&id);
        self.backend.forget(id);
    }

    /// Detect a display reconfiguration (hotplug / resolution change) and rebuild the model's screens, matched by stable display id.
    fn reconcile_screens(&mut self) {
        let displays = self.backend.displays();
        if displays.is_empty() {
            return;
        }
        let new_ids: Vec<u32> = displays.iter().map(|d| d.id).collect();
        let new_cg: Vec<Rect> = displays.iter().map(|d| d.bounds).collect();
        if new_ids == self.display_ids && new_cg == self.screens_cg {
            return; // no change
        }
        let main_screen = new_cg.iter().position(|r| r.x == 0.0 && r.y == 0.0).unwrap_or(0);

        let Some(model) = self.model.as_mut() else { return };
        // Index old Screens by display id, rebuild in the new order, carrying surviving tabs.
        let mut old_by_id: HashMap<u32, Screen> = self.display_ids.iter().copied().zip(std::mem::take(&mut model.screens)).collect();
        let mut new_screens: Vec<Screen> = Vec::with_capacity(displays.len());
        for (i, display) in displays.iter().enumerate() {
            let rect = crate::chrome::usable(display.work_area, i == main_screen, self.bar_position);
            let screen = match old_by_id.remove(&display.id) {
                Some(mut s) => {
                    s.rect = rect;
                    s.current = s.current.min(s.tabs.len().saturating_sub(1));
                    s
                }
                None => Screen { rect, tabs: Vec::new(), current: 0 },
            };
            new_screens.push(screen);
        }
        // Displays that disappeared: migrate their tabs onto the main display.
        let orphans: Vec<Tab> = old_by_id.into_values().flat_map(|s| s.tabs).collect();
        new_screens[main_screen].tabs.extend(orphans);
        // Keep focus on the same physical display if it survived, else the main.
        let focused_id = self.display_ids.get(model.focused_screen).copied();
        model.focused_screen = focused_id.and_then(|fid| new_ids.iter().position(|id| *id == fid)).unwrap_or(main_screen).min(new_screens.len() - 1);
        model.screens = new_screens;

        self.display_ids = new_ids;
        self.screens_cg = new_cg;
        self.main_screen = main_screen;
        // Re-tile onto the new geometry; force a raise (the visible set is stale).
        self.last_shown.clear();
        let placements = self.model.as_ref().unwrap().placements();
        let mut effects = vec![Effect::Render(placements)];
        if let Some(w) = self.model.as_ref().unwrap().focused_window() {
            effects.push(Effect::FocusWindow(w));
        }
        self.refresh();
        self.execute(effects);
        self.save_state();
    }

    /// Diff live manageable windows against the model: adopt new, remove closed.
    pub fn reconcile(&mut self) {
        self.reconcile_screens();
        let current: Vec<_> = self.backend.list_windows().into_iter().filter(manageable).collect();
        let current_ids: HashSet<WindowId> = current.iter().map(|w| w.id).collect();
        let model_ids: Vec<WindowId> = self.model.as_ref().unwrap().all_windows();
        let model_set: HashSet<WindowId> = model_ids.iter().copied().collect();

        let mut changed = false;
        for w in &current {
            if !model_set.contains(&w.id) {
                // Only tile standard windows: a transient popup (download bubble, panel) reports a non-standard AX subrole and adopting one causes a focus/re-tile flicker loop.
                // Fail open when the subrole can't be read (re-checked each poll).
                if self.backend.tileable(w) == Some(false) {
                    continue;
                }
                self.adopt(w);
                changed = true;
            }
        }
        // A window off the on-screen list may just be on another Space (a native-fullscreen window lives on its
        // own Space, moving the rest off-screen), not closed. Fetch the all-Spaces list to tell them apart, only
        // when something is actually missing so the common quiet poll skips the extra CGWindowList call.
        let any_missing = model_ids.iter().any(|id| !current_ids.contains(id));
        let elsewhere: HashSet<WindowId> = if any_missing { self.backend.all_windows().into_iter().map(|w| w.id).collect() } else { HashSet::new() };

        let mut off_workspace: HashSet<WindowId> = HashSet::new();
        for id in model_ids {
            if current_ids.contains(&id) {
                // Back on screen (or never left) → not minimized.
                self.windows.set_minimized(id, false);
                continue;
            }
            // Off the on-screen list: minimized windows keep their tab; a window still on another Space keeps its
            // tab too; only a window gone from every Space is really closed, so remove it.
            if self.backend.minimized(id) == Some(true) {
                self.windows.set_minimized(id, true);
            } else if elsewhere.contains(&id) {
                off_workspace.insert(id);
                continue;
            } else {
                self.forget(id);
                changed = true;
            }
        }
        // Redraw when the set of windows on another Space changes, so their tab/row marker appears and clears.
        if off_workspace != self.off_workspace {
            self.off_workspace = off_workspace;
            self.refresh();
        }
        // Hide the overlays while the Space you're on shows a fullscreen window, so nothing is drawn over it. Key off
        // the frontmost window (always on the active Space), so a fullscreen video on another Space doesn't hide the
        // bar on the Space you're actually looking at.
        let fullscreen = current.first().and_then(|w| self.backend.fullscreen(w)).unwrap_or(false);
        if fullscreen != self.fullscreen {
            self.fullscreen = fullscreen;
            self.refresh();
        }

        // Keep titles live via Accessibility (kCGWindowName is stale/empty); redraw the bar on a change.
        if self.refresh_titles() {
            self.refresh();
        }

        // On a quiet poll, follow the real OS focus (frontmost = first z-ordered entry) so a click, on any
        // monitor, updates our model. Edge-triggered: sync only when the frontmost actually CHANGES. Right after
        // vase moves focus to another monitor, the monitor we left can re-report its key window as global-front;
        // tracking `last_front` through the post-command cooldown makes that persistent reassertion look unchanged,
        // so following focus across monitors never bounces back.
        if !changed {
            if self.focus_cooldown > 0 {
                self.focus_cooldown -= 1;
                self.last_front = current.first().map(|w| w.id);
            } else if let Some(front) = current.first() {
                let front_changed = self.last_front != Some(front.id);
                self.last_front = Some(front.id);
                let model = self.model.as_ref().unwrap();
                // Skip while the focused pane is empty (focused_window() is None).
                if front_changed && model.focused_window().is_some_and(|f| f != front.id) {
                    self.dispatch(Command::SyncFocus(front.id));
                }
            }
        }

        // Expire a pending launch that never produced a window; the picker reopens on the next refresh.
        if let Some(p) = &mut self.pending_launch {
            p.ticks -= 1;
            if p.ticks == 0 {
                self.pending_launch = None;
                self.refresh();
            }
        }

        self.refresh_badges();
    }

    /// Poll the Dock for notification badges (throttled) and redraw bars on a change.
    fn refresh_badges(&mut self) {
        self.badge_tick = self.badge_tick.wrapping_add(1);
        if !self.badge_tick.is_multiple_of(5) {
            return; // ~every 5th reconcile (~500 ms)
        }
        let badges = self.backend.badged_apps();
        if badges != self.badges {
            self.badges = badges;
            self.refresh();
        }
    }
}
