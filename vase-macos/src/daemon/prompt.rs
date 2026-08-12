//! The bar command line (tab rename and `:` command), vim-style, and the small `:` verb set it runs.

use vase_core::backend::Backend;
use vase_core::input::{Key, KeyCode};
use vase_core::model::Command;

use super::Daemon;

/// Which kind of bar command line is open.
#[derive(Clone, Copy)]
pub enum PromptKind {
    Rename,
    StackRename,
    Command,
}

impl PromptKind {
    /// Label shown before the typed text.
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            PromptKind::Rename => "rename: ",
            PromptKind::StackRename => "rename tab: ",
            PromptKind::Command => ":",
        }
    }
}

impl Daemon {
    /// Open the tab-rename prompt (prefix-t), seeded with the current tab's name.
    pub fn start_rename(&mut self) {
        let (tabs, cur) = self.model.as_ref().unwrap().bar_tabs();
        let seed = tabs.get(cur).and_then(|(_, _, n)| n.clone()).unwrap_or_default();
        self.prompt = Some((PromptKind::Rename, seed));
        self.refresh();
    }

    /// Open the nested-tab rename prompt, seeded with the focused stack item's name.
    pub fn start_stack_rename(&mut self) {
        let model = self.model.as_ref().unwrap();
        let Some(wid) = model.focused_stack_window() else { return };
        let seed = model.stack_names.get(&wid).cloned().unwrap_or_default();
        self.prompt = Some((PromptKind::StackRename, seed));
        self.refresh();
    }

    /// Open the `:` command line (prefix-:).
    pub fn start_command(&mut self) {
        self.prompt = Some((PromptKind::Command, String::new()));
        self.refresh();
    }

    fn close_prompt(&mut self) {
        self.prompt = None;
        self.refresh(); // restore the tab bar
    }

    /// Handle a key while the command line is open (modal): Enter runs, Esc cancels, Delete backspaces, else append.
    pub fn prompt_key(&mut self, key: Key) -> bool {
        let Some((kind, _)) = &self.prompt else { return false };
        let code = key.code;
        if code == KeyCode::Return {
            let (kind, buf) = self.prompt.take().unwrap();
            match kind {
                // Empty clears (auto title returns); a whitespace-only name is kept so the tab shows just its icon.
                PromptKind::Rename => self.dispatch(Command::SetTabName((!buf.is_empty()).then_some(buf))),
                PromptKind::StackRename => self.dispatch(Command::SetStackName((!buf.is_empty()).then_some(buf))),
                PromptKind::Command => {
                    self.refresh(); // restore the tabs before running
                    self.run_command(&buf);
                }
            }
            return true;
        }
        if code == KeyCode::Escape {
            self.close_prompt();
            return true;
        }
        let _ = kind;
        let (_, buf) = self.prompt.as_mut().unwrap();
        if code == KeyCode::Backspace {
            buf.pop();
        } else if key.mods.is_typing() {
            if let Some(c) = key.code.char() {
                buf.push(if key.mods.shift { c.to_ascii_uppercase() } else { c });
            }
        }
        self.refresh();
        true
    }

    /// Run a `:` command line (`word [arg...]`).
    fn run_command(&mut self, line: &str) {
        use vase_core::tree::Dir;
        let line = line.trim();
        let mut parts = line.splitn(2, char::is_whitespace);
        let verb = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();
        match verb {
            "" => {}
            "q" | "quit" => {
                self.restore();
                crate::request_quit();
            }
            "rename" => {
                let name = (!arg.is_empty()).then(|| arg.to_string());
                self.dispatch(Command::SetTabName(name));
            }
            "close" => match self.model.as_ref().unwrap().focused_window() {
                Some(id) => self.backend.close(id),
                None => self.dispatch(Command::CloseFocusedPane),
            },
            "split" => self.dispatch(Command::Split(Dir::Horizontal)),
            "vsplit" => self.dispatch(Command::Split(Dir::Vertical)),
            "zoom" => self.dispatch(Command::ToggleZoom),
            "tab" => {
                if let Ok(n) = arg.parse::<usize>() {
                    if n >= 1 {
                        self.dispatch(Command::SelectTab(n - 1));
                    }
                }
            }
            _ => {} // unknown verb: no-op
        }
    }
}
