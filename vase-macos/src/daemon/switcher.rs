//! The prefix-w window switcher overlay and its vim-modal key handling.

use std::time::{Duration, Instant};

use vase_core::input::{Key, Mods, Switcher};
use vase_core::model::Command;
use vase_core::tree::{windows, Node, Pane, WindowId};

use super::util::clean_title;
use super::Daemon;
use crate::overlay::SwitchRow;

/// How long a half-typed index waits for a second digit before committing.
const SWITCHER_ENTRY_TIMEOUT: Duration = Duration::from_millis(120);

/// What picking a switcher row does.
#[derive(Clone, Copy)]
pub enum SwitchTarget {
    /// Raise (and reveal, if stacked) this window.
    Window(WindowId),
    /// A tab header: select that top-level tab (like clicking it in the bar).
    Tab(usize, usize),
}

/// One switcher row: what it points at plus its precomputed display fields
/// (snapshotted at open; the switcher filters/scrolls over these).
#[derive(Clone)]
pub struct SwitchItem {
    pub target: SwitchTarget,
    pub prefix: String,     // tree glyph for a nested row
    pub icons: Vec<String>, // app icons (several on a split/stack parent)
    pub display: String,
    pub dim: bool,     // on a non-focused monitor
    pub current: bool, // the currently-focused window
}

/// One child of a tab, flattening splits: a window, or a stack (with its window
/// items and the selected one).
pub(crate) enum Child {
    Win(WindowId),
    Stack { wins: Vec<WindowId>, selected: WindowId },
}

/// The tab's direct children in order, flattening nested splits so a split's
/// panes sit at one level and any stack among them keeps its items for a second.
pub(crate) fn collect_children(node: &Node) -> Vec<Child> {
    match node {
        Node::Leaf { pane: Pane::Window(w), .. } => vec![Child::Win(*w)],
        Node::Leaf { .. } => vec![],
        Node::Stack { items, selected, .. } => {
            let wins: Vec<WindowId> =
                items.iter().filter_map(|p| if let Pane::Window(w) = p { Some(*w) } else { None }).collect();
            if wins.is_empty() {
                return vec![];
            }
            let sel = match items.get(*selected) {
                Some(Pane::Window(w)) => *w,
                _ => wins[0],
            };
            vec![Child::Stack { wins, selected: sel }]
        }
        Node::Split { children, .. } => children.iter().flat_map(collect_children).collect(),
    }
}

impl Daemon {
    pub fn open_switcher(&mut self) {
        let model = self.model.as_ref().unwrap();
        let focused = model.focused_window();
        let focused_screen = model.focused_screen;
        let mut items: Vec<(SwitchItem, String)> = Vec::new();
        for (si, screen) in model.screens.iter().enumerate() {
            let dim = si != focused_screen;
            for (ti, tab) in screen.tabs.iter().enumerate() {
                let wins = windows(&tab.root);
                match wins.len() {
                    0 => {}
                    // A single-window tab is one flat row.
                    1 => {
                        let w = wins[0];
                        items.push((self.win_item(w, String::new(), dim, focused), self.switcher_label(w)));
                    }
                    // A split or stack tab: a parent header carrying every app
                    // icon (like the bottom bar), then its windows as a tree.
                    _ => {
                        let icons: Vec<String> =
                            wins.iter().filter_map(|w| self.names.get(w).cloned()).collect();
                        let rep = match &tab.root {
                            Node::Stack { items, selected, .. } => match items.get(*selected) {
                                Some(Pane::Window(w)) => *w,
                                _ => wins[0],
                            },
                            _ => wins[0],
                        };
                        let display = tab
                            .name
                            .clone()
                            .filter(|n| !n.trim().is_empty())
                            .unwrap_or_else(|| self.title_of(rep));
                        let mut search = display.clone();
                        for w in &wins {
                            search.push(' ');
                            search.push_str(&self.switcher_label(*w));
                        }
                        items.push((
                            SwitchItem {
                                target: SwitchTarget::Tab(si, ti),
                                prefix: String::new(),
                                icons,
                                display,
                                dim,
                                current: false,
                            },
                            search,
                        ));
                        self.push_children(&tab.root, dim, focused, &mut items);
                    }
                }
            }
        }
        // Preselect the currently-focused window rather than the top item.
        let start = items
            .iter()
            .position(|(it, _)| matches!(it.target, SwitchTarget::Window(w) if Some(w) == focused))
            .unwrap_or(0);
        let mut switcher = Switcher::new(items);
        switcher.select(start);
        self.switcher = Some(switcher);
        self.switcher_g_pending = false;
        self.switcher_entry = None;
        self.switcher_entry_deadline = None;
        self.render_switcher();
    }

    /// Push a multi-window tab's window rows beneath its header. A top-level
    /// stack's items sit at one level (the header already is the stack); a split
    /// lists its panes, and any stack among them expands to a second level.
    fn push_children(
        &self,
        root: &Node,
        dim: bool,
        focused: Option<WindowId>,
        items: &mut Vec<(SwitchItem, String)>,
    ) {
        if let Node::Stack { .. } = root {
            let wins = windows(root);
            let n = wins.len();
            for (j, w) in wins.iter().enumerate() {
                let glyph = if j + 1 == n { "└─ " } else { "├─ " };
                items.push((self.win_item(*w, glyph.to_string(), dim, focused), self.switcher_label(*w)));
            }
            return;
        }
        let kids = collect_children(root);
        let n = kids.len();
        for (i, kid) in kids.iter().enumerate() {
            let g1 = if i + 1 == n { "└─ " } else { "├─ " };
            match kid {
                Child::Win(w) => {
                    items.push((self.win_item(*w, g1.to_string(), dim, focused), self.switcher_label(*w)));
                }
                Child::Stack { wins, selected } => {
                    let icons: Vec<String> =
                        wins.iter().filter_map(|w| self.names.get(w).cloned()).collect();
                    let display = self.title_of(*selected);
                    let mut search = display.clone();
                    for w in wins {
                        search.push(' ');
                        search.push_str(&self.switcher_label(*w));
                    }
                    items.push((
                        SwitchItem {
                            target: SwitchTarget::Window(*selected),
                            prefix: g1.to_string(),
                            icons,
                            display,
                            dim,
                            current: false,
                        },
                        search,
                    ));
                    let m = wins.len();
                    for (j, w) in wins.iter().enumerate() {
                        let g2 = if j + 1 == m { "   └─ " } else { "   ├─ " };
                        items.push((self.win_item(*w, g2.to_string(), dim, focused), self.switcher_label(*w)));
                    }
                }
            }
        }
    }

    /// A window row: a single app icon and the window's display name.
    fn win_item(&self, id: WindowId, prefix: String, dim: bool, focused: Option<WindowId>) -> SwitchItem {
        let in_stack = !prefix.is_empty();
        SwitchItem {
            target: SwitchTarget::Window(id),
            prefix,
            icons: self.names.get(&id).cloned().into_iter().collect(),
            display: self.window_display(id, in_stack),
            dim,
            current: Some(id) == focused,
        }
    }

    /// A window's cleaned title (or its app when the title is empty) — used for
    /// parent-header labels, independent of any custom name.
    pub(crate) fn title_of(&self, id: WindowId) -> String {
        let app = self.names.get(&id).cloned().unwrap_or_default();
        let title = self.titles.get(&id).map(|t| clean_title(t, &app)).unwrap_or_default();
        if title.is_empty() { app } else { title }
    }

    /// A window's display name: its nested-stack custom name (`⌥e t`) if set;
    /// then, for a plain top-level window only, its tab's custom name; else the
    /// cleaned window title, else the app. A nested window skips the tab name —
    /// it names itself, not after the parent tab.
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

    /// "App — window title" (or just the app if the title is empty). Searched by
    /// the switcher, so filtering matches on both the app name and the title.
    pub(crate) fn switcher_label(&self, id: WindowId) -> String {
        let app = self.names.get(&id).map(String::as_str).unwrap_or("?");
        match self.titles.get(&id) {
            Some(title) => {
                let ct = clean_title(title, app);
                if ct.is_empty() { app.to_string() } else { format!("{app} — {ct}") }
            }
            None => app.to_string(),
        }
    }

    fn render_switcher(&mut self) {
        // Snapshot the visible rows (their display fields are precomputed on the
        // items) so the switcher borrow ends before touching switcher_view. The
        // grey index number is the visible position, so it tracks filtering.
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
                    current: it.current,
                })
                .collect();
            (rows, s.is_searching(), s.query().to_string(), s.selected())
        }) else {
            return;
        };
        let header = if is_searching {
            format!("/ {query}")
        } else {
            "  j / k  move    1-9  open    /  search    ⏎  open    esc  close".to_string()
        };
        // Always on the main (menu-bar) display.
        let screen = self.model.as_ref().unwrap().screens[self.main_screen].rect;
        self.switcher_view.show(screen, &header, &rows, selected);
    }

    fn close_switcher(&mut self) {
        self.switcher = None;
        self.switcher_entry = None;
        self.switcher_entry_deadline = None;
        self.switcher_view.hide();
    }

    /// Open the target of the visible row at `idx` (a `prefix-<number>` pick).
    fn open_switcher_index(&mut self, idx: usize) {
        let target = self.switcher.as_ref().and_then(|s| s.visible().get(idx).map(|(it, _)| it.target));
        if let Some(target) = target {
            self.close_switcher();
            match target {
                SwitchTarget::Window(id) => self.dispatch(Command::Raise(id)),
                SwitchTarget::Tab(si, ti) => self.dispatch(Command::SelectScreenTab(si, ti)),
            }
        }
    }

    /// Feed a digit into the index-entry buffer: extend it, commit at once when
    /// no larger index could follow, else wait for another digit (or timeout).
    fn switcher_digit(&mut self, d: usize) {
        let total = self.switcher.as_ref().map_or(0, |s| s.visible().len());
        let new = self.switcher_entry.unwrap_or(0) * 10 + d;
        if new == 0 || new > total {
            // Out of range: commit whatever was buffered; ignore this digit.
            self.commit_switcher_entry();
            return;
        }
        self.switcher_entry = Some(new);
        self.switcher_entry_deadline = Some(Instant::now() + SWITCHER_ENTRY_TIMEOUT);
        if new * 10 > total {
            self.commit_switcher_entry(); // no larger index possible → act now
        }
    }

    fn commit_switcher_entry(&mut self) {
        self.switcher_entry_deadline = None;
        if let Some(n) = self.switcher_entry.take() {
            self.open_switcher_index(n - 1);
        }
    }

    /// Auto-commit a half-typed index once its deadline passes (run-loop tick).
    pub fn tick_switcher_entry(&mut self) {
        if self.switcher_entry_deadline.is_some_and(|d| Instant::now() >= d) {
            self.commit_switcher_entry();
        }
    }

    /// Handle a key while the switcher is open (vim modal). Returns true
    /// (consume) while open. Nav mode: j/k (or arrows) move, a digit opens that
    /// row (double-digit), `/` searches, Enter opens, Esc closes.
    pub fn switcher_key(&mut self, key: Key) -> bool {
        use crate::keycodes::{char_for_keycode, VK_DELETE, VK_DOWN_ARROW, VK_RETURN, VK_UP_ARROW};
        if self.switcher.is_none() {
            return false;
        }
        let code = key.code as i64;
        let searching = self.switcher.as_ref().unwrap().is_searching();

        // Number entry (nav mode only): a digit picks a row by its grey index.
        if !searching && key.mods == Mods::default() {
            if let Some(d) = char_for_keycode(key.code).and_then(|c| c.to_digit(10)) {
                self.switcher_digit(d as usize);
                return true;
            }
        }
        // Any other key cancels a half-typed index.
        self.switcher_entry = None;
        self.switcher_entry_deadline = None;

        // Enter opens the highlighted row; Esc leaves search or closes.
        if code == VK_RETURN {
            let pick = self.switcher.as_ref().unwrap().selection();
            self.close_switcher();
            match pick.map(|it| it.target) {
                Some(SwitchTarget::Window(id)) => self.dispatch(Command::Raise(id)),
                Some(SwitchTarget::Tab(si, ti)) => self.dispatch(Command::SelectScreenTab(si, ti)),
                None => {}
            }
            return true;
        }
        if code == 0x35 {
            if searching {
                self.switcher.as_mut().unwrap().stop_search();
                self.render_switcher();
            } else {
                self.close_switcher();
            }
            return true;
        }

        const VK_G: u16 = 0x05;
        // Resolve a pending `g` (nav-mode `gg` → top) before borrowing the
        // switcher: a second unmodified `g` jumps to the top; anything else
        // cancels the pending state.
        let go_top = !searching
            && key.code == VK_G
            && !key.mods.shift
            && std::mem::take(&mut self.switcher_g_pending);
        if !searching && key.code == VK_G && !key.mods.shift && !go_top {
            self.switcher_g_pending = true;
        } else if key.code != VK_G || key.mods.shift || searching {
            self.switcher_g_pending = false;
        }

        let s = self.switcher.as_mut().unwrap();
        if code == VK_UP_ARROW {
            s.move_up();
        } else if code == VK_DOWN_ARROW {
            s.move_down();
        } else if searching {
            if code == VK_DELETE {
                s.backspace();
            } else if key.mods == Mods::default() {
                if let Some(c) = char_for_keycode(key.code) {
                    s.type_char(c);
                }
            }
        } else if go_top {
            s.move_top();
        } else if key.code == VK_G && key.mods.shift {
            s.move_bottom(); // G → bottom
        } else if key.code == VK_G {
            // first `g` of a possible `gg` — pending set above, no move yet
        } else if key.mods == Mods::default() {
            match char_for_keycode(key.code) {
                Some('j') => s.move_down(),
                Some('k') => s.move_up(),
                Some('/') => s.start_search(),
                _ => {}
            }
        }
        self.render_switcher();
        true
    }
}
