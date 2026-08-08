//! Atom One Dark palette helpers shared across the overlay windows.

use objc2::rc::Retained;
use objc2_app_kit::NSColor;

pub(crate) const PANE_RADIUS: f64 = 8.0;
pub(crate) const PANE_PAD: f64 = 8.0;

// Atom One Dark, sRGB.
fn theme_color(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}
// Ghostty "Atom One Dark" background (#21252b), the actual terminal bg.
pub(crate) fn strip_bg() -> Retained<NSColor> {
    theme_color(0.129, 0.145, 0.169, 1.0)
}
pub(crate) fn active_bg() -> Retained<NSColor> {
    theme_color(0.231, 0.247, 0.298, 1.0)
}
// Fill for a tab on a non-focused monitor: darker than the strip so it recedes.
pub(crate) fn dim_bg() -> Retained<NSColor> {
    theme_color(0.09, 0.10, 0.125, 1.0)
}
pub(crate) fn text_col() -> Retained<NSColor> {
    theme_color(0.671, 0.698, 0.749, 1.0)
}
pub(crate) fn dim_col() -> Retained<NSColor> {
    theme_color(0.361, 0.388, 0.439, 1.0)
}
// One Dark green (#98c379), the armed-prefix dot.
pub(crate) fn green() -> Retained<NSColor> {
    theme_color(0.596, 0.765, 0.475, 1.0)
}
// vase brand terracotta (#C7663C), the leading logo mark.
pub(crate) fn clay() -> Retained<NSColor> {
    theme_color(0.780, 0.400, 0.235, 1.0)
}
// Notification-badge red (#ff453a, Apple system red), the dot on a tab.
pub(crate) fn badge_red() -> Retained<NSColor> {
    theme_color(1.0, 0.271, 0.227, 1.0)
}
// Stroke on every tab outline (#5c6370), matching the thin-cap separator in the user's tmux status bar.
pub(crate) fn tab_border() -> Retained<NSColor> {
    theme_color(0.361, 0.388, 0.439, 1.0)
}
// Bright outline marking a tab whose app has a focus-toggle hotkey.
pub(crate) fn hotkey_border() -> Retained<NSColor> {
    theme_color(0.85, 0.87, 0.90, 0.95)
}
// The focused-pane accent (outline / placeholder border): One Dark green.
pub(crate) fn pane_border() -> Retained<NSColor> {
    green()
}
