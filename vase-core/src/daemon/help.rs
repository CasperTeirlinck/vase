//! The shortcut sheet: opened from the prefix or the menu, closed by the next key.

use super::Daemon;
use crate::backend::Backend;
use crate::chrome::Painter;

impl<B: Backend, C: Painter> Daemon<B, C> {
    /// Show the sheet on the focused screen, or hide it if it is already up.
    pub fn toggle_help(&mut self) {
        if self.help_open {
            self.close_help();
            return;
        }
        let Some(model) = &self.model else { return };
        let screen = model.screens[model.focused_screen].rect;
        self.help_open = true;
        self.chrome.help(screen);
    }

    pub fn close_help(&mut self) {
        if self.help_open {
            self.help_open = false;
            self.chrome.hide_help();
        }
    }
}
