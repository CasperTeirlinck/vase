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

    /// Re-read config.json and apply it (the menu-bar "Reload config" action).
    pub fn reload_config(&mut self) {
        self.app_hotkeys = crate::config::load();
        self.refresh_bar(); // hotkey-app tab markers may have changed
    }

    /// Toggle focus to `app`: if already focused on one of its windows, jump back
    /// to the previously-focused window; otherwise focus the app's first window
    /// (in tab order). No-op if the app has no managed window.
    pub fn toggle_app_focus(&mut self, app: &str) {
        let model = self.model.as_ref().unwrap();
        let on_app = model
            .focused_window()
            .and_then(|id| self.names.get(&id))
            .is_some_and(|name| app_matches(name, app));
        if on_app {
            if let Some(back) = self.last_focused {
                self.dispatch(Command::Raise(back));
            }
            return;
        }
        let first = all_windows(model)
            .into_iter()
            .find(|id| self.names.get(id).is_some_and(|name| app_matches(name, app)));
        if let Some(id) = first {
            self.dispatch(Command::Raise(id));
        }
    }
}
