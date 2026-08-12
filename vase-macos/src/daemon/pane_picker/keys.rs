//! The pane picker's open/close lifecycle, rendering, key handling, and pending-launch matching.

use std::time::Instant;

use vase_core::backend::Backend;
use vase_core::backend::WindowInfo;
use vase_core::input::{Key, Mods, Pick, Switcher};
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
                PickItem::Window { icons, display, prefix, dim, off_workspace, .. } => {
                    n += 1;
                    SwitchRow { number: n, prefix, icons, label: display, dim, off_workspace, favorite: false, current: false }
                }
                PickItem::Header { icons, display, prefix, dim, off_workspace } => SwitchRow { number: 0, prefix, icons, label: display, dim, off_workspace, favorite: false, current: false },
                PickItem::Launch(i) => SwitchRow {
                    number: 0,
                    prefix: String::new(),
                    icons: vec![self.apps[i].clone()],
                    label: format!("⧉  {}", self.apps[i]),
                    dim: false,
                    off_workspace: false,
                    favorite: self.is_favorite(&self.apps[i]),
                    current: false,
                },
            })
            .collect();
        let header = if s.is_searching() { format!("/ {}", s.query()) } else { "move here - 1-9 pick · j/k · / search · f ★ · ⏎ move · esc cancel".to_string() };
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
                self.backend.launch(&app);
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
        if self.pane_picker.is_none() {
            return false;
        }
        // "f" in navigate mode toggles the selected app's favorite.
        if key.mods == Mods::default() && key.code.char() == Some('f') {
            let app = {
                let s = self.pane_picker.as_ref().unwrap();
                match s.visible().into_iter().nth(s.selected()) {
                    Some((PickItem::Launch(i), _)) if !s.is_searching() => Some(self.apps[i].clone()),
                    _ => None,
                }
            };
            if let Some(app) = app {
                self.toggle_favorite(app);
                return true;
            }
        }
        let s = self.pane_picker.as_mut().unwrap();
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
