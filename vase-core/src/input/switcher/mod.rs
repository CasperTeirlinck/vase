use std::time::Instant;

use super::{Entry, Key, KeyCode, Mods, NumberEntry};

mod nav;

/// A row a `Switcher` can hold.
pub trait Item: Clone {
    /// Whether the selection may rest here. Display-only rows (headers) return false.
    fn selectable(&self) -> bool {
        true
    }

    /// Whether a digit can pick this row. Numbered rows are counted in display order, so a row can be selectable yet unnumbered.
    fn numbered(&self) -> bool {
        self.selectable()
    }
}

/// What a key press resolved to in an open `Switcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick<T> {
    /// Consumed, but nothing on screen changed.
    Ignored,
    /// State changed; redraw.
    Redraw,
    /// The user chose this row.
    Chosen(T),
    /// The user dismissed the switcher.
    Cancelled,
}

/// A filterable list driven by a vim-style modal grammar: `j`/`k` and arrows move, `gg`/`G` jump, `/` searches, a digit picks a numbered row, `⏎` chooses and `esc` backs out.
#[derive(Debug)]
pub struct Switcher<T> {
    items: Vec<(T, String)>,
    query: String,
    selected: usize,
    /// Vim-style mode: false = navigate (j/k), true = search (typing filters).
    searching: bool,
    /// A `g` was pressed in navigate mode, awaiting a second `g` (gg).
    g_pending: bool,
    entry: NumberEntry,
}

impl<T: Item> Switcher<T> {
    pub fn new(items: Vec<(T, String)>) -> Self {
        let mut s = Switcher { items, query: String::new(), selected: 0, searching: false, g_pending: false, entry: NumberEntry::default() };
        s.settle(1);
        s
    }

    /// Route one key press while the switcher is open.
    pub fn key(&mut self, key: Key, now: Instant) -> Pick<T> {
        // A digit in navigate mode picks a numbered row.
        if !self.searching && key.mods == Mods::default() {
            if let Some(d) = key.code.char().and_then(|c| c.to_digit(10)) {
                return match self.entry.digit(d as usize, self.numbered_len(), now) {
                    Entry::Commit(n) => self.take_numbered(n),
                    _ => Pick::Ignored,
                };
            }
        }
        // Any other key abandons a half-typed index rather than committing it: here the list is still on screen, so the keystroke is navigation, not the end of a pick.
        self.entry.cancel();

        if key.code == KeyCode::Return {
            return self.take_selected();
        }
        if key.code == KeyCode::Escape {
            if !self.searching {
                return Pick::Cancelled;
            }
            self.searching = false;
            self.query.clear();
            self.settle(1);
            return Pick::Redraw;
        }

        // Resolve a pending `g` (gg → top) before moving.
        let g = key.code == KeyCode::Char('g') && !key.mods.shift && !self.searching;
        let go_top = g && std::mem::take(&mut self.g_pending);
        if g && !go_top {
            self.g_pending = true;
            return Pick::Redraw; // first `g` of a possible `gg`; no move yet
        }
        self.g_pending = false;

        if key.code == KeyCode::Up {
            self.step(-1);
        } else if key.code == KeyCode::Down {
            self.step(1);
        } else if go_top {
            self.selected = 0;
            self.settle(1);
        } else if key.code == KeyCode::Char('g') && key.mods.shift && !self.searching {
            self.selected = self.visible_len().saturating_sub(1);
            self.settle(-1);
        } else if self.searching {
            if key.code == KeyCode::Backspace {
                self.query.pop();
            } else if key.mods == Mods::default() {
                match key.code.char() {
                    Some(c) => self.query.push(c),
                    None => return Pick::Ignored,
                }
            } else {
                return Pick::Ignored;
            }
            self.clamp();
            self.settle(1);
        } else if key.mods == Mods::default() {
            match key.code.char() {
                Some('j') => self.step(1),
                Some('k') => self.step(-1),
                Some('/') => {
                    self.searching = true;
                    self.query.clear();
                    self.clamp();
                }
                _ => return Pick::Ignored,
            }
        } else {
            return Pick::Ignored;
        }
        Pick::Redraw
    }

    /// Commit a half-typed index once its deadline passes; call from the run loop.
    pub fn tick(&mut self, now: Instant) -> Pick<T> {
        match self.entry.tick(now) {
            Entry::Commit(n) => self.take_numbered(n),
            _ => Pick::Ignored,
        }
    }

    /// Set the current selection (clamped, then settled onto a selectable row).
    pub fn select(&mut self, i: usize) {
        let n = self.visible_len();
        self.selected = if n == 0 { 0 } else { i.min(n - 1) };
        self.settle(1);
    }

    /// The rows matching the current query.
    pub fn visible(&self) -> Vec<(T, &str)> {
        self.matching().map(|(id, name)| (id.clone(), name.as_str())).collect()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn is_searching(&self) -> bool {
        self.searching
    }

    pub fn selected(&self) -> usize {
        self.selected
    }
}
