//! Running a bound command, and the sticky repeat modes that outlive the prefix.

use crate::focus::Direction;
use crate::input::{InputCommand, Key, KeyCode};
use crate::model::Command;

use super::Daemon;
use crate::backend::Backend;
use crate::chrome::Painter;

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// Whether an overlay is taking keys. Kept in step with the guards at the top of `intercept_key`,
    /// for a platform that has to answer "is this key the overlay's?" at a moment when it cannot ask
    /// the daemon itself.
    pub fn modal(&self) -> bool {
        self.pending_launch.is_some() || self.prompt.is_some() || self.switcher.is_some() || self.pane_picker.is_some() || self.tab_entry.is_pending()
    }

    /// Offer a key to whatever is modal, before the prefix router sees it. Returns whether the key was swallowed. The order here *is* the modal precedence.
    pub fn intercept_key(&mut self, key: Key) -> bool {
        // A launch in flight makes its pane modal: only Esc, which cancels and collapses the pane.
        if self.pending_launch.is_some() {
            if key.code == KeyCode::Escape {
                self.pending_launch = None;
                self.dispatch(Command::CloseFocusedPane);
            }
            return true;
        }
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        if self.switcher.is_some() {
            return self.switcher_key(key);
        }
        if self.pane_picker.is_some() {
            return self.pane_picker_key(key);
        }
        // Mid `prefix-<number>` entry: capture further digits (the router only routes the first key after the prefix) so multi-digit tab numbers work.
        if self.tab_entry.is_pending() {
            return self.tab_entry_key(key);
        }
        // Configurable global app-focus hotkeys (e.g. Ctrl-` → Ghostty). Direct chords, not behind the prefix; the modal overlays above take precedence.
        if let Some(app) = self.app_hotkey(key) {
            self.toggle_app_focus(&app);
            return true;
        }
        // After a prefix resize or move-tab, the bare repeat keys keep working without re-prefixing.
        self.sticky_key(key)
    }

    /// Run one bound command. Model edits go through the reducer; the rest drive the daemon itself.
    pub fn run(&mut self, cmd: InputCommand) {
        use InputCommand as I;
        // A resize / move-tab arms its sticky mode; any other prefix command leaves both.
        self.resize_sticky = matches!(cmd, I::ResizeLeft | I::ResizeRight | I::ResizeUp | I::ResizeDown);
        self.movetab_sticky = matches!(cmd, I::MoveTabPrev | I::MoveTabNext);
        if let Some(c) = Command::from_input(&cmd) {
            self.dispatch(c);
            return;
        }
        match cmd {
            // tmux last-window: jump back to the previously-focused window.
            I::LastTab => {
                if let Some(w) = self.last_focused {
                    self.dispatch(Command::Raise(w));
                }
            }
            I::Quit => {
                self.restore();
                self.quit = true;
            }
            I::WindowSwitcher => self.open_switcher(),
            I::Rename => self.start_rename(),
            I::StackRename => self.start_stack_rename(),
            I::CommandLine => self.start_command(),
            I::WarpCursor => self.warp_cursor_to_focus(),
            I::SelectBarTab(n) => self.begin_tab_entry(n),
            I::SendPrefix => {}
            // Everything else is a model edit `from_input` already returned.
            _ => {}
        }
    }

    /// Move the mouse cursor to the center of the focused pane.
    fn warp_cursor_to_focus(&self) {
        if let Some(r) = self.model.as_ref().and_then(|m| m.focused_pane_rect()) {
            self.backend.warp_cursor(r.x + r.w / 2.0, r.y + r.h / 2.0);
        }
    }

    /// After a prefix resize or move-tab, the bare repeat keys keep acting until any other key. Returns whether the key was consumed.
    pub fn sticky_key(&mut self, key: Key) -> bool {
        let shift_only = key.mods.shift_only();
        if self.resize_sticky {
            // Arrows and vim keys both, matching the prefix bindings.
            let dir = match key.code {
                KeyCode::Left | KeyCode::Char('h') => Some(Direction::Left),
                KeyCode::Right | KeyCode::Char('l') => Some(Direction::Right),
                KeyCode::Up | KeyCode::Char('k') => Some(Direction::Up),
                KeyCode::Down | KeyCode::Char('j') => Some(Direction::Down),
                _ => None,
            };
            if let (true, Some(dir)) = (shift_only, dir) {
                self.dispatch(Command::Resize(dir));
                return true;
            }
            self.resize_sticky = false;
        }
        if self.movetab_sticky {
            let offset = match key.code {
                KeyCode::Char(',') => Some(-1),
                KeyCode::Char('.') => Some(1),
                _ => None,
            };
            if let (true, Some(offset)) = (shift_only, offset) {
                self.dispatch(Command::MoveTab(offset));
                return true;
            }
            self.movetab_sticky = false;
        }
        false
    }
}
