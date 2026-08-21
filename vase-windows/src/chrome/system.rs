//! The colors the native style draws in: Fluent's own ramp, in the appearance and the accent the user picked.

use std::cell::Cell;

use windows::UI::ViewManagement::{UIColorType, UISettings};

use vase_core::chrome::theme::{Palette, FLUENT_DARK, FLUENT_LIGHT};

/// What the system says about its own appearance.
#[derive(Clone, Copy)]
pub struct Appearance {
    pub light: bool,
    pub palette: Palette,
}

thread_local! {
    // A redraw asks for a color per run, so what is behind them is read once and cached until Windows says it changed.
    static CACHED: Cell<Option<Appearance>> = const { Cell::new(None) };
}

/// Drop the cached appearance: it changed, so the next draw reads it again.
pub fn invalidate() {
    CACHED.with(|c| c.set(None));
}

pub fn appearance() -> Appearance {
    if let Some(cached) = CACHED.with(|c| c.get()) {
        return cached;
    }
    let appearance = read().unwrap_or(Appearance { light: false, palette: FLUENT_DARK });
    CACHED.with(|c| c.set(Some(appearance)));
    appearance
}

/// The colors WinUI reads for its own controls. `Background` is white in light mode and black in dark,
/// which is how Windows itself asks the question; the accent comes tinted for the appearance it is on.
/// <https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/apply-windows-themes>
fn read() -> Option<Appearance> {
    let settings = UISettings::new().ok()?;
    let background = settings.GetColorValue(UIColorType::Background).ok()?;
    let light = background.R > 128;
    let mut palette = if light { FLUENT_LIGHT } else { FLUENT_DARK };
    if let Ok(accent) = settings.GetColorValue(if light { UIColorType::AccentDark1 } else { UIColorType::AccentLight2 }) {
        let channel = |v: u8| v as f64 / 255.0;
        palette.accent = [channel(accent.R), channel(accent.G), channel(accent.B), 1.0];
        palette.hotkey = palette.accent;
    }
    Some(Appearance { light, palette })
}
