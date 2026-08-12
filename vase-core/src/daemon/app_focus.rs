//! Global app-focus toggle hotkeys.

use crate::input::Key;
use crate::model::Command;
use crate::registry::app_matches;

use super::Daemon;
use crate::backend::Backend;
use crate::chrome::Painter;

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// The configured app to toggle focus to for `key`, if any.
    pub fn app_hotkey(&self, key: Key) -> Option<String> {
        self.app_hotkeys.iter().find(|h| h.key == key).map(|h| h.app.clone())
    }

    /// Re-read config.toml and apply it (the menu-bar "Reload config" action).
    pub fn reload_config(&mut self) {
        let config = self.paths.config.as_deref().map(crate::config::Config::load).unwrap_or_default();
        self.app_hotkeys = config.app_focus;
        self.favorites = config.favorites;
        crate::chrome::theme::set_theme(config.theme);
        crate::chrome::theme::set_mark(config.mark);
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
        let first = model.all_windows().into_iter().find(|id| app_matches(self.windows.app(*id), app));
        if let Some(id) = first {
            self.dispatch(Command::Raise(id));
        }
    }
}
