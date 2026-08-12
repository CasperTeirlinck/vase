//! vase's palette, derived from the OS. `UISettings` supplies the accent color and the light/dark
//! preference; the rest of the palette is the Fluent ramp below.

use windows::UI::ViewManagement::{UIColorType, UISettings};

use vase_core::chrome::theme::{Theme, ONE_DARK};

/// The Windows Fluent light palette. Greys follow the Fluent neutral ramp.
const FLUENT_LIGHT: Theme = Theme {
    bg: [0.973, 0.973, 0.973, 1.0],     // Layer / SolidBackgroundFillColorBase
    active: [0.902, 0.902, 0.902, 1.0], // ControlFillColorSecondary
    dim_bg: [0.937, 0.937, 0.937, 1.0],
    text: [0.100, 0.100, 0.100, 1.0], // TextFillColorPrimary
    dim: [0.400, 0.400, 0.400, 1.0],  // TextFillColorSecondary
    accent: [0.000, 0.475, 0.843, 1.0],
    badge: [0.769, 0.169, 0.110, 1.0], // SystemFillColorCritical
    border: [0.851, 0.851, 0.851, 1.0],
    hotkey: [0.200, 0.200, 0.200, 0.95],
};

/// The Windows Fluent dark palette.
const FLUENT_DARK: Theme = Theme {
    bg: [0.129, 0.129, 0.129, 1.0],
    active: [0.216, 0.216, 0.216, 1.0],
    dim_bg: [0.102, 0.102, 0.102, 1.0],
    text: [1.000, 1.000, 1.000, 1.0],
    dim: [0.612, 0.612, 0.612, 1.0],
    accent: [0.376, 0.694, 0.910, 1.0],
    badge: [1.000, 0.600, 0.600, 1.0],
    border: [0.267, 0.267, 0.267, 1.0],
    hotkey: [0.900, 0.900, 0.900, 0.95],
};

pub fn dark_mode() -> Option<bool> {
    let bg = UISettings::new().ok()?.GetColorValue(UIColorType::Background).ok()?;
    Some(luminance(bg.R, bg.G, bg.B) < 0.5)
}

/// The user's accent color as sRGBA in 0..1.
pub fn accent() -> Option<[f64; 4]> {
    let c = UISettings::new().ok()?.GetColorValue(UIColorType::Accent).ok()?;
    Some([c.R as f64 / 255.0, c.G as f64 / 255.0, c.B as f64 / 255.0, 1.0])
}

/// The Fluent palette matching the user's light/dark and accent settings, or vase's own default if the OS won't answer.
pub fn system_theme() -> Theme {
    let Some(dark) = dark_mode() else { return ONE_DARK };
    let mut theme = if dark { FLUENT_DARK } else { FLUENT_LIGHT };
    if let Some(accent) = accent() {
        // The system accent is tuned for its own background, so on the opposite one it can fall below readable contrast.
        if contrast(accent, theme.bg) >= 3.0 {
            theme.accent = accent;
        }
    }
    theme
}

/// Relative luminance (WCAG).
fn luminance(r: u8, g: u8, b: u8) -> f64 {
    let channel = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast(a: [f64; 4], b: [f64; 4]) -> f64 {
    let lum = |c: [f64; 4]| luminance((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
    let (x, y) = (lum(a), lum(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_fluent_palettes_keep_text_readable_on_their_own_strip() {
        // 4.5:1 is the WCAG AA threshold for body text; the bar labels sit right at body size.
        for theme in [FLUENT_LIGHT, FLUENT_DARK] {
            assert!(contrast(theme.text, theme.bg) >= 4.5, "primary text must stay readable");
            assert!(contrast(theme.text, theme.active) >= 4.5, "and on the selected tab too");
        }
    }

    #[test]
    fn a_low_contrast_accent_is_rejected_rather_than_used() {
        // A near-black accent on the dark strip would vanish.
        assert!(contrast([0.05, 0.05, 0.05, 1.0], FLUENT_DARK.bg) < 3.0);
        assert!(contrast([0.376, 0.694, 0.910, 1.0], FLUENT_DARK.bg) >= 3.0);
    }
}
