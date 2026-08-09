//! Global app-focus toggle hotkeys.

use vase_core::input::Key;
use vase_core::model::Command;

use super::util::{all_windows, app_matches};
use super::Daemon;

impl Daemon {
    /// The configured app to toggle focus to for `key`, if any.
    pub fn app_hotkey(&self, key: Key) -> Option<String> {
        self.app_hotkeys.iter().find(|h| h.key == key).map(|h| h.app.clone())
    }

    /// Re-read config.toml and apply it (the menu-bar "Reload config" action).
    pub fn reload_config(&mut self) {
        self.app_hotkeys = crate::config::load();
        self.favorites = crate::config::favorites();
        crate::overlay::set_theme(crate::config::load_theme());
        crate::overlay::set_mark(crate::config::load_mark());
        self.refresh(); // palette, mark, and hotkey/favorite markers may have changed
    }

    /// Toggle focus to `app`: if already on it, jump back to the previous window, else focus its first.
    pub fn toggle_app_focus(&mut self, app: &str) {
        let model = self.model.as_ref().unwrap();
        let on_app = model.focused_window().is_some_and(|id| app_matches(self.windows.app(id), app));
        if on_app {
            if let Some(back) = self.last_focused {
                self.dispatch(Command::Raise(back));
            }
            return;
        }
        let first = all_windows(model).into_iter().find(|id| app_matches(self.windows.app(*id), app));
        if let Some(id) = first {
            self.dispatch(Command::Raise(id));
        }
    }
}
