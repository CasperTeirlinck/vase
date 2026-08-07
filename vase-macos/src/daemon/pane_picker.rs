//! The auto-opening empty-pane picker (move a window in or launch an app) and
//! the pending-launch lifecycle.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use vase_core::backend::WindowInfo;
use vase_core::input::{Key, Mods, Switcher};
use vase_core::model::Command;
use vase_core::tree::{find_window, windows, Node, WindowId};

use super::switcher::{collect_children, Child};
use super::Daemon;
use crate::overlay::SwitchRow;

/// How long a half-typed index waits for a second digit before committing.
const PICK_ENTRY_TIMEOUT: Duration = Duration::from_millis(120);

/// A pane-picker entry. Existing windows are shown as the same nested tree as the
/// window switcher (tab/stack headers are display-only), then the launchable apps
/// as a plain list (`usize` indexes `Daemon::apps`).
#[derive(Clone)]
pub enum PickItem {
    /// An existing window to move into the pane.
    Window { id: WindowId, icons: Vec<String>, display: String, prefix: String, dim: bool },
    /// A tab/stack header: shown for context, not selectable.
    Header { icons: Vec<String>, display: String, prefix: String, dim: bool },
    /// Launch a new instance of `apps[i]`.
    Launch(usize),
}

impl PickItem {
    fn selectable(&self) -> bool {
        !matches!(self, PickItem::Header { .. })
    }
}

/// An app was spawned into the focused empty pane; its window is awaited for
/// `ticks` more polls before the launch is abandoned.
pub struct PendingLaunch {
    pub app: String,
    pub ticks: u32,
}

impl Daemon {
    /// Auto-open a picker when focus lands on an empty pane (listing windows
    /// from other tabs), auto-close it when the pane is no longer empty, and
    /// render it inside the focused empty pane's rect.
    pub(crate) fn refresh_pane_picker(&mut self) {
        // While a launch is in flight, the focused empty pane shows a
        // "launching…" container instead of the picker.
        if let Some(p) = &self.pending_launch {
            let header = format!("launching {}…", p.app);
            let area = self
                .model
                .as_ref()
                .unwrap()
                .empty_panes()
                .into_iter()
                .find(|(_, focused)| *focused)
                .map(|(rect, _)| rect);
            match area {
                Some(area) => {
                    self.switcher_view.show_in(area, &header, &[], 0);
                    return;
                }
                // The target pane is gone (filled, or focus moved) — the launch
                // is resolved; drop it and fall through so the overlay is hidden.
                None => self.pending_launch = None,
            }
        }
        let empty = self.model.as_ref().unwrap().focused_pane_is_empty();
        if empty && self.pane_picker.is_none() {
            let items = self.build_pane_items();
            let mut picker = Switcher::new(items);
            // Land the initial selection on a selectable row (skip a leading header).
            skip_headers(&mut picker, true);
            self.pane_picker = Some(picker);
            self.pane_picker_g_pending = false;
            self.pane_picker_entry = None;
            self.pane_picker_entry_deadline = None;
        } else if !empty && self.pane_picker.is_some() {
            self.close_pane_picker();
        }
        if self.pane_picker.is_some() {
            self.render_pane_picker();
        } else if self.switcher.is_none() {
            // Neither the pane picker nor the prefix-w switcher wants the shared
            // panel now — hide any leftover "launching…"/picker view.
            self.switcher_view.hide();
        }
    }

    /// The picker's rows: existing windows as a nested tree, then the launchable
    /// apps. Excludes only the windows already in the focused pane's own node
    /// (the stack/leaf being filled) — other panes of the same tab are offered,
    /// so a window can move into a split's stack from elsewhere in that tab.
    fn build_pane_items(&self) -> Vec<(PickItem, String)> {
        let model = self.model.as_ref().unwrap();
        let fscreen = model.focused_screen;
        let exclude: HashSet<WindowId> = model
            .focused_tab()
            .map(|t| {
                let fp = t.focused;
                windows(&t.root)
                    .into_iter()
                    .filter(|w| find_window(&t.root, *w) == Some(fp))
                    .collect()
            })
            .unwrap_or_default();
        let mut out: Vec<(PickItem, String)> = Vec::new();
        for (si, screen) in model.screens.iter().enumerate() {
            let dim = si != fscreen;
            for tab in &screen.tabs {
                let wins: Vec<WindowId> =
                    windows(&tab.root).into_iter().filter(|w| !exclude.contains(w)).collect();
                match wins.len() {
                    0 => {}
                    1 => {
                        let w = wins[0];
                        out.push((self.pick_window(w, String::new(), dim), self.switcher_label(w)));
                    }
                    _ => {
                        let icons: Vec<String> =
                            wins.iter().filter_map(|w| self.names.get(w).cloned()).collect();
                        let display = tab
                            .name
                            .clone()
                            .filter(|n| !n.trim().is_empty())
                            .unwrap_or_else(|| self.title_of(wins[0]));
                        out.push((
                            PickItem::Header { icons, display, prefix: String::new(), dim },
                            String::new(),
                        ));
                        self.push_pick_children(&tab.root, dim, &exclude, &mut out);
                    }
                }
            }
        }
        for (i, app) in self.apps.iter().enumerate() {
            out.push((PickItem::Launch(i), format!("⧉  {app}")));
        }
        out
    }

    /// Window rows beneath a multi-window tab's header (mirrors the switcher),
    /// skipping windows in `exclude`; a stack reduced to one candidate shows as
    /// a plain window rather than a stack header.
    fn push_pick_children(
        &self,
        root: &Node,
        dim: bool,
        exclude: &HashSet<WindowId>,
        out: &mut Vec<(PickItem, String)>,
    ) {
        if let Node::Stack { .. } = root {
            let wins: Vec<WindowId> =
                windows(root).into_iter().filter(|w| !exclude.contains(w)).collect();
            let n = wins.len();
            for (j, w) in wins.iter().enumerate() {
                let glyph = if j + 1 == n { "└─ " } else { "├─ " };
                out.push((self.pick_window(*w, glyph.to_string(), dim), self.switcher_label(*w)));
            }
            return;
        }
        // Keep only children that still have a candidate window, so the ├─/└─
        // last-marker counts the shown rows.
        let eff: Vec<Child> = collect_children(root)
            .into_iter()
            .filter_map(|kid| match kid {
                Child::Win(w) => (!exclude.contains(&w)).then_some(Child::Win(w)),
                Child::Stack { wins, selected } => {
                    let fw: Vec<WindowId> = wins.into_iter().filter(|w| !exclude.contains(w)).collect();
                    if fw.is_empty() {
                        return None;
                    }
                    let selected = if fw.contains(&selected) { selected } else { fw[0] };
                    Some(Child::Stack { wins: fw, selected })
                }
            })
            .collect();
        let n = eff.len();
        for (i, kid) in eff.iter().enumerate() {
            let g1 = if i + 1 == n { "└─ " } else { "├─ " };
            match kid {
                Child::Win(w) => {
                    out.push((self.pick_window(*w, g1.to_string(), dim), self.switcher_label(*w)));
                }
                Child::Stack { wins, .. } if wins.len() == 1 => {
                    out.push((self.pick_window(wins[0], g1.to_string(), dim), self.switcher_label(wins[0])));
                }
                Child::Stack { wins, selected } => {
                    let icons: Vec<String> =
                        wins.iter().filter_map(|w| self.names.get(w).cloned()).collect();
                    out.push((
                        PickItem::Header {
                            icons,
                            display: self.title_of(*selected),
                            prefix: g1.to_string(),
                            dim,
                        },
                        String::new(),
                    ));
                    let m = wins.len();
                    for (j, w) in wins.iter().enumerate() {
                        let g2 = if j + 1 == m { "   └─ " } else { "   ├─ " };
                        out.push((self.pick_window(*w, g2.to_string(), dim), self.switcher_label(*w)));
                    }
                }
            }
        }
    }

    fn pick_window(&self, id: WindowId, prefix: String, dim: bool) -> PickItem {
        let in_stack = !prefix.is_empty();
        PickItem::Window {
            id,
            icons: self.names.get(&id).cloned().into_iter().collect(),
            display: self.window_display(id, in_stack),
            prefix,
            dim,
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
                PickItem::Window { icons, display, prefix, dim, .. } => {
                    n += 1;
                    SwitchRow { number: n, prefix, icons, label: display, dim, current: false }
                }
                PickItem::Header { icons, display, prefix, dim } => {
                    SwitchRow { number: 0, prefix, icons, label: display, dim, current: false }
                }
                PickItem::Launch(i) => SwitchRow {
                    number: 0,
                    prefix: String::new(),
                    icons: vec![self.apps[i].clone()],
                    label: format!("⧉  {}", self.apps[i]),
                    dim: false,
                    current: false,
                },
            })
            .collect();
        let header = if s.is_searching() {
            format!("/ {}", s.query())
        } else {
            "move here — 1-9 pick · j/k · / search · ⏎ move · esc cancel".to_string()
        };
        let selected = s.selected();
        let area = self
            .model
            .as_ref()
            .unwrap()
            .empty_panes()
            .into_iter()
            .find(|(_, focused)| *focused)
            .map(|(rect, _)| rect);
        if let Some(area) = area {
            self.switcher_view.show_in(area, &header, &rows, selected);
        }
    }

    fn close_pane_picker(&mut self) {
        self.pane_picker = None;
        self.pane_picker_entry = None;
        self.pane_picker_entry_deadline = None;
        self.switcher_view.hide();
    }

    /// Move the selected window into the pane, or launch the selected app.
    fn activate_pick(&mut self, item: PickItem) {
        match item {
            PickItem::Window { id, .. } => {
                self.close_pane_picker();
                self.dispatch(Command::FillPane(id));
            }
            PickItem::Launch(i) => {
                if let Err(e) =
                    std::process::Command::new("open").arg("-na").arg(&self.apps[i]).spawn()
                {
                    eprintln!("vase: failed to launch {}: {e}", self.apps[i]);
                }
                // ~5 s at the 100 ms poll to adopt the launched window.
                self.pending_launch = Some(PendingLaunch { app: self.apps[i].clone(), ticks: 50 });
                // Clear the picker but keep focus on the empty pane (no collapse);
                // the "launching…" container renders in its place.
                self.pane_picker = None;
                self.pane_picker_g_pending = false;
                self.pane_picker_entry = None;
                self.pane_picker_entry_deadline = None;
                self.refresh_pane_picker();
            }
            PickItem::Header { .. } => {} // not selectable
        }
    }

    /// The id of the `n`-th (1-based) visible existing-window row.
    fn nth_pick_window(&self, n: usize) -> Option<WindowId> {
        let s = self.pane_picker.as_ref()?;
        s.visible()
            .into_iter()
            .filter_map(|(it, _)| match it {
                PickItem::Window { id, .. } => Some(id),
                _ => None,
            })
            .nth(n - 1)
    }

    fn pick_digit(&mut self, d: usize) {
        let total = self.pane_picker.as_ref().map_or(0, |s| {
            s.visible().iter().filter(|(it, _)| matches!(it, PickItem::Window { .. })).count()
        });
        let new = self.pane_picker_entry.unwrap_or(0) * 10 + d;
        if new == 0 || new > total {
            self.commit_pick_entry();
            return;
        }
        self.pane_picker_entry = Some(new);
        self.pane_picker_entry_deadline = Some(Instant::now() + PICK_ENTRY_TIMEOUT);
        if new * 10 > total {
            self.commit_pick_entry();
        }
    }

    fn commit_pick_entry(&mut self) {
        self.pane_picker_entry_deadline = None;
        if let Some(n) = self.pane_picker_entry.take() {
            if let Some(id) = self.nth_pick_window(n) {
                self.close_pane_picker();
                self.dispatch(Command::FillPane(id));
            }
        }
    }

    /// Auto-commit a half-typed index once its deadline passes (run-loop tick).
    pub fn tick_pane_picker_entry(&mut self) {
        if self.pane_picker_entry_deadline.is_some_and(|d| Instant::now() >= d) {
            self.commit_pick_entry();
        }
    }

    /// Handle a key while the pane picker is open (vim modal, mirrors
    /// `switcher_key`). Enter/index moves the selected window in; Esc in nav mode
    /// collapses the split. Consumes every key while open.
    pub fn pane_picker_key(&mut self, key: Key) -> bool {
        use crate::keycodes::{char_for_keycode, VK_DELETE, VK_DOWN_ARROW, VK_RETURN, VK_UP_ARROW};
        if self.pane_picker.is_none() {
            return false;
        }
        let code = key.code as i64;
        let searching = self.pane_picker.as_ref().unwrap().is_searching();

        // Index entry (nav mode only): a digit picks the n-th existing window.
        if !searching && key.mods == Mods::default() {
            if let Some(d) = char_for_keycode(key.code).and_then(|c| c.to_digit(10)) {
                self.pick_digit(d as usize);
                return true;
            }
        }
        self.pane_picker_entry = None;
        self.pane_picker_entry_deadline = None;

        if code == VK_RETURN {
            if let Some(item) = self.pane_picker.as_ref().unwrap().selection() {
                self.activate_pick(item);
            }
            return true;
        }
        if code == 0x35 {
            if searching {
                let s = self.pane_picker.as_mut().unwrap();
                s.stop_search();
                skip_headers(s, true);
                self.render_pane_picker();
            } else {
                self.close_pane_picker();
                self.dispatch(Command::CloseFocusedPane);
            }
            return true;
        }

        const VK_G: u16 = 0x05;
        let go_top = !searching
            && key.code == VK_G
            && !key.mods.shift
            && std::mem::take(&mut self.pane_picker_g_pending);
        if !searching && key.code == VK_G && !key.mods.shift && !go_top {
            self.pane_picker_g_pending = true;
        } else if key.code != VK_G || key.mods.shift || searching {
            self.pane_picker_g_pending = false;
        }

        let s = self.pane_picker.as_mut().unwrap();
        // Navigation lands only on selectable rows (headers are display-only).
        if code == VK_UP_ARROW {
            s.move_up();
            skip_headers(s, false);
        } else if code == VK_DOWN_ARROW {
            s.move_down();
            skip_headers(s, true);
        } else if searching {
            if code == VK_DELETE {
                s.backspace();
            } else if key.mods == Mods::default() {
                if let Some(c) = char_for_keycode(key.code) {
                    s.type_char(c);
                }
            }
            skip_headers(s, true);
        } else if go_top {
            s.move_top();
            skip_headers(s, true);
        } else if key.code == VK_G && key.mods.shift {
            s.move_bottom();
            skip_headers(s, false);
        } else if key.code == VK_G {
            // first `g` of a possible `gg` — pending set above, no move yet
        } else if key.mods == Mods::default() {
            match char_for_keycode(key.code) {
                Some('j') => {
                    s.move_down();
                    skip_headers(s, true);
                }
                Some('k') => {
                    s.move_up();
                    skip_headers(s, false);
                }
                Some('/') => s.start_search(),
                _ => {}
            }
        }
        self.render_pane_picker();
        true
    }

    /// Whether `w` is the window a pending launch is waiting for: its app name
    /// matches (case-insensitive, either direction — the display name and the
    /// bundle stem can differ) and the launch target pane is still empty+focused.
    pub(crate) fn launch_matches(&self, w: &WindowInfo) -> bool {
        let Some(p) = &self.pending_launch else { return false };
        if !self.model.as_ref().unwrap().focused_pane_is_empty() {
            return false;
        }
        let (a, b) = (w.app.to_lowercase(), p.app.to_lowercase());
        a == b || a.contains(&b) || b.contains(&a)
    }
}

/// Advance the selection past display-only header rows in `down` direction, so
/// the cursor never rests on one. Bounded by the item count (there is always at
/// least one selectable row — the launchable apps).
fn skip_headers(s: &mut Switcher<PickItem>, down: bool) {
    let n = s.visible().len();
    for _ in 0..n {
        let on_header = s
            .visible()
            .get(s.selected())
            .map(|(it, _)| !it.selectable())
            .unwrap_or(false);
        if !on_header {
            break;
        }
        if down {
            s.move_down();
        } else {
            s.move_up();
        }
    }
}
