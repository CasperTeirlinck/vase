//! Global app-focus toggle hotkeys.

use crate::input::Key;
use crate::model::{Command, Effect};
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
        self.focus_border = config.focus_border;
        crate::chrome::theme::set_theme(config.theme);
        crate::chrome::theme::set_mark(config.mark);
        let position = config.bar_position.unwrap_or(self.backend.default_bar_position());
        if position != self.bar_position {
            self.bar_position = position;
            // The strip the bar reserves is now on the other edge, so every screen's tileable area
            // has to be recut and the windows moved into it.
            self.reserve_screens();
            if let Some(placements) = self.model.as_ref().map(|m| m.placements()) {
                self.execute(vec![Effect::Render(placements)]);
            }
        }
        self.refresh(); // palette, mark, and hotkey/favorite markers may have changed
    }

    /// Recut each screen's tileable area from its display, for a bar that has changed edge.
    fn reserve_screens(&mut self) {
        let displays = self.backend.displays();
        let (main, position) = (self.main_screen, self.bar_position);
        let Some(model) = self.model.as_mut() else { return };
        for (i, display) in displays.iter().enumerate() {
            if let Some(screen) = model.screens.get_mut(i) {
                screen.rect = crate::chrome::usable(display.work_area, i == main, position);
            }
        }
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
