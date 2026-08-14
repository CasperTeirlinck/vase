use std::collections::HashMap;

mod entry;
mod keycode;
pub(crate) mod keymap;
mod switcher;

pub use entry::*;
pub use keycode::KeyCode;
pub use keymap::router;
pub use switcher::*;

/// Active modifier set on a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    /// Command on macOS, the Windows key on Windows.
    pub meta: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Mods {
    /// Whether only Shift is held, for the sticky repeat keys that outlive the prefix.
    pub fn shift_only(self) -> bool {
        self.shift && !self.meta && !self.ctrl && !self.alt
    }

    /// Whether the key types text rather than invoking a chord.
    pub fn is_typing(self) -> bool {
        !self.meta && !self.ctrl && !self.alt
    }
}

/// A key press: key identity plus modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

impl Key {
    /// A key with no modifiers.
    pub fn plain(code: KeyCode) -> Self {
        Key { code, mods: Mods::default() }
    }

    /// A key with only the Meta (Command / Windows) modifier.
    pub fn meta(code: KeyCode) -> Self {
        Key { code, mods: Mods { meta: true, ..Mods::default() } }
    }

    /// A key with only the Alt/Option modifier.
    pub fn alt(code: KeyCode) -> Self {
        Key { code, mods: Mods { alt: true, ..Mods::default() } }
    }

    /// A plain letter/digit/punctuation key.
    pub fn ch(c: char) -> Self {
        Key::plain(KeyCode::Char(c))
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
    /// Send the current tab to the monitor on the left / right.
    MoveTabMonitorPrev,
    MoveTabMonitorNext,
    /// Select the Nth tab in bar order (1-based). tmux `prefix-<number>`.
    SelectBarTab(usize),
    /// Break the focused pane out of its split into its own tab (tmux prefix-x).
    BreakPane,
    /// Turn the focused pane into a stack / add a stacked tab (prefix-s).
    Stackify,
    /// Cycle the focused stack's selected window (prefix-[ / prefix-]).
    StackFocusPrev,
    StackFocusNext,
    /// Rename the current tab (tmux prefix-t).
    Rename,
    /// Nested-stack analogs of the top-level tab keys, acting on the focused stack.
    StackSelectItem(usize),
    StackMovePrev,
    StackMoveNext,
    StackRename,
    /// Open the `:` command line (tmux prefix-:).
    CommandLine,
    /// Move the mouse cursor to the center of the focused pane (prefix-m).
    WarpCursor,
    Quit,
}

/// What the OS tap should do with an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Let the event reach the focused app unchanged.
    PassThrough,
    /// Swallow the event (the prefix chord arms the router); run nothing.
    Consume,
    /// Swallow the event and run this command.
    ConsumeAndRun(InputCommand),
}

/// Modal tmux-style key router: a prefix chord arms it, then one key resolves to a command.
#[derive(Debug)]
pub struct KeyRouter {
    /// One `(prefix chord, bindings)` per mode.
    modes: Vec<(Key, HashMap<Key, InputCommand>)>,
    armed: Option<usize>,
}

impl KeyRouter {
    pub fn new(prefix: Key, bindings: HashMap<Key, InputCommand>) -> Self {
        KeyRouter { modes: vec![(prefix, bindings)], armed: None }
    }

    /// Add a second prefix chord with its own bindings.
    pub fn with_prefix(mut self, prefix: Key, bindings: HashMap<Key, InputCommand>) -> Self {
        self.modes.push((prefix, bindings));
        self
    }

    /// Route one key press.
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
