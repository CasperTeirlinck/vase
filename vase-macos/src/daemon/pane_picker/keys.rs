//! The pane picker's open/close lifecycle, rendering, key handling, and pending-launch matching.

use std::time::Instant;

use vase_core::backend::WindowInfo;
use vase_core::input::{Key, Pick, Switcher};
use vase_core::model::Command;

use super::{PendingLaunch, PickItem};
use crate::daemon::Daemon;
use crate::overlay::SwitchRow;

/// Reconcile ticks a pending launch waits for its window before it's dropped.
const LAUNCH_ADOPT_TICKS: u32 = 100; // ~10 s at the ~100 ms reconcile poll

impl Daemon {
    /// Auto-open the picker over a focused empty pane, close it when the pane fills.
    pub(crate) fn refresh_pane_picker(&mut self) {
        // While a launch is in flight, show a "launching…" container instead of the picker.
        if let Some(p) = &self.pending_launch {
            let header = format!("launching {}…", p.app);
            let area = self.model.as_ref().unwrap().empty_panes().into_iter().find(|(_, focused)| *focused).map(|(rect, _)| rect);
            match area {
                Some(area) => {
                    self.overlays.show_list_in(area, &header, &[], 0);
                    return;
                }
                // Target pane gone (filled or focus moved): drop the launch, fall through to hide.
                None => self.pending_launch = None,
            }
        }
        let empty = self.model.as_ref().unwrap().focused_pane_is_empty();
        if empty && self.pane_picker.is_none() {
            self.pane_picker = Some(Switcher::new(self.build_pane_items()));
        } else if !empty && self.pane_picker.is_some() {
            self.close_pane_picker();
        }
        if self.pane_picker.is_some() {
            self.render_pane_picker();
        } else if self.switcher.is_none() {
            // Neither picker nor switcher wants the shared panel; hide any leftover view.
            self.overlays.hide_list();
        }
    }

    pub(crate) fn render_pane_picker(&mut self) {
        let Some(s) = &self.pane_picker else { return };
        // Index numbers count only existing-window rows (not headers or apps).
        let mut n = 0usize;
        let rows: Vec<SwitchRow> = s
            .visible()
            .into_iter()
            .map(|(item, _)| match item {
                PickItem::Window { icons, display, prefix, dim, off_space, .. } => {
                    n += 1;
                    SwitchRow { number: n, prefix, icons, label: display, dim, off_space, current: false }
                }
                PickItem::Header { icons, display, prefix, dim, off_space } => SwitchRow { number: 0, prefix, icons, label: display, dim, off_space, current: false },
                PickItem::Launch(i) => {
                    SwitchRow { number: 0, prefix: String::new(), icons: vec![self.apps[i].clone()], label: format!("⧉  {}", self.apps[i]), dim: false, off_space: false, current: false }
                }
            })
            .collect();
        let header = if s.is_searching() { format!("/ {}", s.query()) } else { "move here - 1-9 pick · j/k · / search · ⏎ move · esc cancel".to_string() };
        let selected = s.selected();
        let area = self.model.as_ref().unwrap().empty_panes().into_iter().find(|(_, focused)| *focused).map(|(rect, _)| rect);
        if let Some(area) = area {
            self.overlays.show_list_in(area, &header, &rows, selected);
        }
    }

    fn close_pane_picker(&mut self) {
        self.pane_picker = None;
        self.overlays.hide_list();
    }

    /// Move the selected window into the pane, or launch the selected app.
    fn activate_pick(&mut self, item: PickItem) {
        match item {
            PickItem::Window { id, .. } => {
                self.close_pane_picker();
                self.dispatch(Command::FillPane(id));
            }
            PickItem::Launch(i) => {
                let app = self.apps[i].clone();
                // `-n` opens a fresh instance so an already-running app still yields a new window for the pane. Singletons
                // refuse `-n`, so fall back to plain activation. Finder won't open a window on activation, so point it at $HOME.
                let cmd = if app == "Finder" {
                    "open ~".to_string()
                } else {
                    let q = app.replace('\'', r"'\''");
                    format!("open -na '{q}' || open -a '{q}'")
                };

                if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
                    eprintln!("failed to launch {app}: {e}");
                }
                self.pending_launch = Some(PendingLaunch { app, ticks: LAUNCH_ADOPT_TICKS });
                // Clear the picker but keep focus on the empty pane; the "launching…" container renders there.
                self.pane_picker = None;
                self.refresh_pane_picker();
            }
            PickItem::Header { .. } => {}
        }
    }

    /// Auto-commit a half-typed index once its deadline passes (run-loop tick).
    pub fn tick_pane_picker(&mut self) {
        let Some(s) = &mut self.pane_picker else { return };
        if let Pick::Chosen(item) = s.tick(Instant::now()) {
            self.activate_pick(item);
        }
    }

    /// Handle a key while the pane picker is open; consumes while open.
    pub fn pane_picker_key(&mut self, key: Key) -> bool {
        let Some(s) = &mut self.pane_picker else { return false };
        match s.key(key, Instant::now()) {
            Pick::Ignored => {}
            Pick::Redraw => self.render_pane_picker(),
            Pick::Chosen(item) => self.activate_pick(item),
            // Backing out of the picker backs out of the pane it opened over.
            Pick::Cancelled => {
                self.close_pane_picker();
                self.dispatch(Command::CloseFocusedPane);
            }
        }
        true
    }

    /// Whether `w` is the window a pending launch awaits: app name matches (fuzzy, since display name and bundle stem differ) and the target pane is still empty+focused.
    pub(crate) fn launch_matches(&self, w: &WindowInfo) -> bool {
        let Some(p) = &self.pending_launch else { return false };
        if !self.model.as_ref().unwrap().focused_pane_is_empty() {
            return false;
        }
        let (a, b) = (w.app.to_lowercase(), p.app.to_lowercase());
        a == b || a.contains(&b) || b.contains(&a)
    }
}
