//! Live-window/display reconciliation: the 100 ms poll that adopts, drops, and
//! re-tiles windows and follows OS focus.

use std::collections::{HashMap, HashSet};

use vase_core::backend::{manageable, Backend};
use vase_core::geometry::Rect;
use vase_core::model::{Command, Effect, Screen, Tab};
use vase_core::tree::WindowId;

use super::util::{all_windows, screen_of};
use super::Daemon;

impl Daemon {
    /// Pull each managed window's current title via Accessibility; returns
    /// whether any changed so the caller can redraw the bar.
    fn refresh_titles(&mut self) -> bool {
        let ids = all_windows(self.model.as_ref().unwrap());
        let mut changed = false;
        for id in ids {
            if let Some(t) = self.backend.title(id) {
                if self.titles.get(&id).map(String::as_str) != Some(t.as_str()) {
                    self.titles.insert(id, t);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Detect a display reconfiguration (hotplug / resolution change) and adapt:
    /// rebuild the model's screens matched to the new display set by stable
    /// display id — so each tab stays on the same physical monitor regardless of
    /// index order — then recompute the main display and re-tile. Tabs on a
    /// display that went away migrate to the main display.
    fn reconcile_screens(&mut self) {
        let mut screens = crate::overlay::all_screens(self.mtm);
        if screens.is_empty() {
            return;
        }
        screens.sort_by(|a, b| {
            a.1.x.partial_cmp(&b.1.x).unwrap_or(std::cmp::Ordering::Equal).then(
                a.1.y.partial_cmp(&b.1.y).unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let new_ids: Vec<u32> = screens.iter().map(|(id, _, _)| *id).collect();
        let new_cg: Vec<Rect> = screens.iter().map(|(_, full, _)| *full).collect();
        if new_ids == self.display_ids && new_cg == self.screens_cg {
            return; // no change
        }
        let main_screen = new_cg.iter().position(|r| r.x == 0.0 && r.y == 0.0).unwrap_or(0);

        let Some(model) = self.model.as_mut() else { return };
        // Index old Screens by their display id, then rebuild in the new order,
        // carrying each surviving display's tabs and starting new displays empty.
        let mut old_by_id: HashMap<u32, Screen> =
            self.display_ids.iter().copied().zip(std::mem::take(&mut model.screens)).collect();
        let mut new_screens: Vec<Screen> = Vec::with_capacity(screens.len());
        for (i, (id, _, vis)) in screens.iter().enumerate() {
            let rect = if i == main_screen {
                Rect::new(vis.x, vis.y, vis.w, vis.h - crate::overlay::BAR_HEIGHT)
            } else {
                *vis
            };
            let screen = match old_by_id.remove(id) {
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
        model.focused_screen = focused_id
            .and_then(|fid| new_ids.iter().position(|id| *id == fid))
            .unwrap_or(main_screen)
            .min(new_screens.len() - 1);
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
        self.refresh_bar();
        self.refresh_stack_bars();
        self.refresh_panes();
        self.refresh_focus_border();
        self.execute(effects);
        self.save_state();
    }

    /// Diff the live manageable windows against the model: adopt new windows,
    /// remove closed ones. Only dispatches (and thus moves windows) on a change.
    pub fn reconcile(&mut self) {
        self.reconcile_screens();
        let current: Vec<_> = self.backend.list_windows().into_iter().filter(manageable).collect();
        let current_ids: HashSet<WindowId> = current.iter().map(|w| w.id).collect();
        let model_ids: Vec<WindowId> = all_windows(self.model.as_ref().unwrap());
        let model_set: HashSet<WindowId> = model_ids.iter().copied().collect();

        let mut changed = false;
        for w in &current {
            if !model_set.contains(&w.id) {
                // Only tile standard windows: a transient popup (a browser
                // download bubble, a panel) reports a non-standard AX subrole.
                // Adopting one steals focus and re-tiles, and it auto-dismisses
                // when it loses focus — a flicker loop. Fail open when the
                // subrole can't be read, so a slow-to-appear real window isn't
                // dropped (it re-checks each poll).
                if self.backend.subrole_info(w).is_some_and(|s| s != "AXStandardWindow") {
                    continue;
                }
                self.names.insert(w.id, w.app.clone());
                self.titles.insert(w.id, w.title.clone());
                self.originals.insert(w.id, w.frame);
                // Warm this window's app icon so its tab shows it right away.
                crate::overlay::prewarm_icon(&w.app);
                // A pending launch's window (matched by app name) fills the
                // focused empty pane instead of adopting as a new tab.
                if self.launch_matches(w) {
                    self.pending_launch = None;
                    self.dispatch(Command::FillPane(w.id));
                } else {
                    let si = screen_of(w.frame, &self.screens_cg);
                    self.dispatch(Command::AddWindow(w.id, si));
                }
                changed = true;
            }
        }
        for id in model_ids {
            if current_ids.contains(&id) {
                // Back on screen (or never left) → not minimized.
                self.minimized.remove(&id);
                continue;
            }
            // Off the on-screen list: minimized windows keep their tab (restored
            // on select); a window that can no longer be read is closed → remove.
            if self.backend.minimized(id) == Some(true) {
                self.minimized.insert(id);
            } else {
                self.dispatch(Command::RemoveWindow(id));
                self.backend.forget(id);
                self.names.remove(&id);
                self.titles.remove(&id);
                self.originals.remove(&id);
                self.minimized.remove(&id);
                changed = true;
            }
        }

        // Keep titles live via Accessibility (they change as e.g. browser tabs
        // switch, and kCGWindowName is stale/empty); redraw the bar on a change.
        if self.refresh_titles() {
            self.refresh_bar();
            self.refresh_stack_bars();
        }

        // On a quiet poll, follow the real OS focus (the frontmost window is the
        // first z-ordered entry) so a click on another window updates our model.
        // Two guards make this behave under multiple monitors with separate
        // Spaces:
        //  - edge-triggered: sync only when the frontmost actually CHANGES, not
        //    whenever it merely differs from our model;
        //  - same-screen only: a window on a *different* monitor being "frontmost"
        //    is the separate-Spaces artifact (the active Space always reports its
        //    own key window as global-front, so a window we focus on a secondary
        //    monitor never becomes front) — not a user action to chase. Following
        //    it would bounce focus back off the monitor we just moved to.
        // Skip during the post-command cooldown.
        if !changed {
            if self.focus_cooldown > 0 {
                self.focus_cooldown -= 1;
            } else if let Some(front) = current.first() {
                let front_changed = self.last_front != Some(front.id);
                self.last_front = Some(front.id);
                let front_screen = screen_of(front.frame, &self.screens_cg);
                let model = self.model.as_ref().unwrap();
                // Skip while the focused pane is empty (focused_window() is None).
                if front_changed
                    && model.focused_window().is_some_and(|f| f != front.id)
                    && front_screen == model.focused_screen
                {
                    self.dispatch(Command::SyncFocus(front.id));
                }
            }
        }

        // Expire a pending launch that never produced a window; the pane is
        // still empty+focused, so the picker reopens on the next refresh.
        if let Some(p) = &mut self.pending_launch {
            p.ticks -= 1;
            if p.ticks == 0 {
                self.pending_launch = None;
                self.refresh_pane_picker();
                self.refresh_panes();
            }
        }

        self.refresh_badges();
    }

    /// Poll the Dock for notification badges (throttled — badges change slowly)
    /// and redraw the bars when the badged-app set changes.
    fn refresh_badges(&mut self) {
        self.badge_tick = self.badge_tick.wrapping_add(1);
        if self.badge_tick % 5 != 0 {
            return; // ~every 5th reconcile (~500 ms)
        }
        let badges = crate::dock::badged_apps();
        if badges != self.badges {
            self.badges = badges;
            self.refresh_bar();
            self.refresh_stack_bars();
        }
    }
}
