//! `prefix-<number>` tab selection, and routing bar clicks back into commands.

use std::time::Instant;

use vase_core::input::keys::{char_for_keycode, VK_ESC};
use vase_core::input::{Entry, Key, Mods};
use vase_core::model::Command;

use super::Daemon;

impl Daemon {
    fn total_tabs(&self) -> usize {
        self.model.as_ref().map_or(0, |m| m.screens.iter().map(|s| s.tabs.len()).sum())
    }

    fn select_bar_tab(&mut self, n: usize) {
        if let Some((si, ti)) = self.flat_tab_to_screen(n - 1) {
            self.dispatch(Command::SelectScreenTab(si, ti));
        }
    }

    /// Begin a `prefix-<number>` selection; commit at once when no larger tab number could start with it.
    pub fn begin_tab_entry(&mut self, first: usize) {
        let max = self.total_tabs();
        if let Entry::Commit(n) = self.tab_entry.digit(first, max, Instant::now()) {
            self.select_bar_tab(n);
        }
    }

    /// Feed a key into an in-progress tab-number entry: a digit extends it, Esc cancels, else commit. Returns whether the key was consumed.
    pub fn tab_entry_key(&mut self, key: Key) -> bool {
        if !self.tab_entry.is_pending() {
            return false;
        }
        if key.mods == Mods::default() {
            if let Some(d) = char_for_keycode(key.code).and_then(|c| c.to_digit(10)) {
                let max = self.total_tabs();
                if let Entry::Commit(n) = self.tab_entry.digit(d as usize, max, Instant::now()) {
                    self.select_bar_tab(n);
                }
                return true;
            }
        }
        // Unlike an open switcher, a non-digit key here commits rather than discards: no list is on screen, so the digits already typed are the whole of the user's intent.
        if key.code == VK_ESC {
            self.tab_entry.cancel();
        } else if let Entry::Commit(n) = self.tab_entry.flush() {
            self.select_bar_tab(n);
        }
        true
    }

    /// Auto-commit a pending tab-number entry once its deadline passes.
    pub fn tick_tab_entry(&mut self) {
        if let Entry::Commit(n) = self.tab_entry.tick(Instant::now()) {
            self.select_bar_tab(n);
        }
    }

    /// Map a flat tab index (bar order) to `(screen index, tab-within-screen index)`.
    fn flat_tab_to_screen(&self, flat: usize) -> Option<(usize, usize)> {
        let model = self.model.as_ref()?;
        let mut acc = 0;
        for (si, s) in model.screens.iter().enumerate() {
            if flat < acc + s.tabs.len() {
                return Some((si, flat - acc));
            }
            acc += s.tabs.len();
        }
        None
    }

    /// Route a click at a CG point to whichever bar it landed on; returns whether it was ours.
    pub fn click(&mut self, px: f64, py: f64) -> bool {
        let Some(model) = &self.model else { return false };
        match self.overlays.hit(model, px, py) {
            Some(cmd) => {
                self.dispatch(cmd);
                true
            }
            None => false,
        }
    }
}
