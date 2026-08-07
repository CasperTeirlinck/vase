//! Pure modal key routing. No OS calls — the macOS event tap feeds this and
//! obeys its decisions.

use std::collections::HashMap;

/// Active modifier set on a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// A concrete key press: a virtual keycode plus the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: u16,
    pub mods: Mods,
}

impl Key {
    /// A key with no modifiers.
    pub fn plain(code: u16) -> Self {
        Key { code, mods: Mods::default() }
    }

    /// A key with only the Command modifier (the common prefix shape).
    pub fn cmd(code: u16) -> Self {
        Key { code, mods: Mods { cmd: true, ..Mods::default() } }
    }

    /// A key with only the Alt/Option modifier.
    pub fn alt(code: u16) -> Self {
        Key { code, mods: Mods { alt: true, ..Mods::default() } }
    }
}

/// A command a binding maps to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputCommand {
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    SendPrefix,
    StackNext,
    StackPrev,
    /// Create a new, empty tab and focus it (tmux `prefix-c`).
    NewTab,
    SplitH,
    SplitV,
    /// Switch focus to the most-recently-focused window (tmux `last-window`).
    LastTab,
    /// Open the window switcher (tmux prefix-w).
    WindowSwitcher,
    ZoomToggle,
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveTabPrev,
    MoveTabNext,
    /// Send the current tab to the monitor on the left / right (⌥a { / }).
    MoveTabMonitorPrev,
    MoveTabMonitorNext,
    /// Select the Nth tab in bar order (1-based) — tmux `prefix-<number>`.
    SelectBarTab(usize),
    /// Break the focused pane out of its split into its own tab (tmux prefix-x).
    BreakPane,
    /// Turn the focused pane into a stack / add a stacked tab (prefix-s).
    Stackify,
    /// Cycle the focused stack's selected window (prefix-[ / prefix-]).
    StackFocusPrev,
    StackFocusNext,
    /// Rename the current tab (tmux prefix-t): open the rename prompt.
    Rename,
    /// Nested-stack prefix (⌥e) analogs of the top-level tab keys, acting on the
    /// focused stack instead of the screen's tabs.
    StackSelectItem(usize),
    StackMovePrev,
    StackMoveNext,
    StackRename,
    /// Open the `:` command line (tmux prefix-:).
    CommandLine,
    Quit,
}

/// What the OS tap should do with an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Let the event reach the focused app unchanged.
    PassThrough,
    /// Swallow the event (the prefix chord, arming the router); run nothing.
    Consume,
    /// Swallow the event and run this command.
    ConsumeAndRun(InputCommand),
}

/// Modal router: idle until the prefix chord arms it, then routes exactly one
/// key. tmux-style — press the prefix (e.g. Cmd-a), release, then a command key.
#[derive(Debug)]
pub struct KeyRouter {
    /// One `(prefix chord, its bindings)` per mode (e.g. ⌥a top-level, ⌥e
    /// nested-stack); the armed prefix decides which bindings a key resolves in.
    modes: Vec<(Key, HashMap<Key, InputCommand>)>,
    armed: Option<usize>,
}

impl KeyRouter {
    pub fn new(prefix: Key, bindings: HashMap<Key, InputCommand>) -> Self {
        KeyRouter { modes: vec![(prefix, bindings)], armed: None }
    }

    /// Add a second prefix chord with its own binding set (e.g. ⌥e for
    /// nested-stack ops alongside ⌥a for top-level tabs).
    pub fn with_prefix(mut self, prefix: Key, bindings: HashMap<Key, InputCommand>) -> Self {
        self.modes.push((prefix, bindings));
        self
    }

    /// Route one key press. When armed, look it up in that prefix's bindings and
    /// disarm (always — the router is never stuck armed). When idle, a prefix
    /// chord arms its mode and is consumed; every other key passes through.
    pub fn key(&mut self, key: Key) -> Decision {
        if let Some(i) = self.armed.take() {
            return match self.modes[i].1.get(&key) {
                Some(cmd) => Decision::ConsumeAndRun(cmd.clone()),
                None => Decision::PassThrough,
            };
        }
        if let Some(i) = self.modes.iter().position(|(p, _)| *p == key) {
            self.armed = Some(i);
            Decision::Consume
        } else {
            Decision::PassThrough
        }
    }

    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }
}

/// A filterable list of items with a moving selection. Pure; the daemon feeds
/// it keys and renders `visible()`.
#[derive(Debug, Clone)]
pub struct Switcher<T> {
    items: Vec<(T, String)>,
    query: String,
    selected: usize,
    /// Vim-style mode: false = navigate (j/k), true = search (typing filters).
    searching: bool,
}

impl<T: Clone> Switcher<T> {
    pub fn new(items: Vec<(T, String)>) -> Self {
        Switcher { items, query: String::new(), selected: 0, searching: false }
    }

    /// Number of items matching the current query — without cloning them (the
    /// nav/clamp hot path needs only the count).
    fn visible_len(&self) -> usize {
        let q = self.query.to_lowercase();
        self.items.iter().filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q)).count()
    }

    pub fn is_searching(&self) -> bool {
        self.searching
    }

    /// Enter search mode (`/`) with a fresh query.
    pub fn start_search(&mut self) {
        self.searching = true;
        self.query.clear();
        self.clamp();
    }

    /// Leave search mode (Esc from search), clearing the filter.
    pub fn stop_search(&mut self) {
        self.searching = false;
        self.query.clear();
        self.clamp();
    }

    /// The (item, label) rows matching the current query, in order.
    pub fn visible(&self) -> Vec<(T, &str)> {
        let q = self.query.to_lowercase();
        self.items
            .iter()
            .filter(|(_, name)| q.is_empty() || name.to_lowercase().contains(&q))
            .map(|(id, name)| (id.clone(), name.as_str()))
            .collect()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Set the current selection (clamped to the visible range).
    pub fn select(&mut self, i: usize) {
        let n = self.visible_len();
        self.selected = if n == 0 { 0 } else { i.min(n - 1) };
    }

    pub fn move_up(&mut self) {
        let n = self.visible_len();
        if n == 0 {
            return;
        }
        // Wrap: up from the top goes to the bottom.
        self.selected = if self.selected == 0 { n - 1 } else { self.selected - 1 };
    }

    pub fn move_down(&mut self) {
        let n = self.visible_len();
        if n == 0 {
            return;
        }
        // Wrap: down from the bottom goes to the top.
        self.selected = if self.selected + 1 >= n { 0 } else { self.selected + 1 };
    }

    pub fn move_top(&mut self) {
        self.selected = 0;
    }

    pub fn move_bottom(&mut self) {
        self.selected = self.visible_len().saturating_sub(1);
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.clamp();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.clamp();
    }

    /// The currently-highlighted item, if any.
    pub fn selection(&self) -> Option<T> {
        self.visible().get(self.selected).map(|(id, _)| id.clone())
    }

    fn clamp(&mut self) {
        let n = self.visible_len();
        self.selected = self.selected.min(n.saturating_sub(1));
    }
}

#[cfg(test)]
#[path = "input_test.rs"]
mod tests;
