//! The palette as AppKit sees it: each accessor resolves a core `Role` into an `NSColor`.

use objc2::rc::Retained;
use objc2_app_kit::NSColor;
use vase_core::chrome::theme::{theme, Role};

pub(crate) use vase_core::chrome::theme::{mark, Mark, PANE_PAD, PANE_RADIUS};

/// Resolve a palette role to an `NSColor`, for laid-out chrome that names colors by role.
pub(crate) fn role(r: Role) -> Retained<NSColor> {
    color(r)
}

fn color(role: Role) -> Retained<NSColor> {
    let c = theme().color(role);
    NSColor::colorWithSRGBRed_green_blue_alpha(c[0], c[1], c[2], c[3])
}

pub(crate) fn strip_bg() -> Retained<NSColor> {
    color(Role::Bg)
}
pub(crate) fn active_bg() -> Retained<NSColor> {
    color(Role::Active)
}
pub(crate) fn dim_bg() -> Retained<NSColor> {
    color(Role::DimBg)
}
pub(crate) fn text_col() -> Retained<NSColor> {
    color(Role::Text)
}
pub(crate) fn dim_col() -> Retained<NSColor> {
    color(Role::Dim)
}
// The mark and every highlight (focus outline, current-row accent, off-workspace and favorite markers, armed-prefix dot).
pub(crate) fn accent() -> Retained<NSColor> {
    color(Role::Accent)
}
pub(crate) fn badge_red() -> Retained<NSColor> {
    color(Role::Badge)
}
pub(crate) fn tab_border() -> Retained<NSColor> {
    color(Role::Border)
}
pub(crate) fn hotkey_border() -> Retained<NSColor> {
    color(Role::Hotkey)
}
// The focused-pane accent (outline / placeholder border).
pub(crate) fn pane_border() -> Retained<NSColor> {
    accent()
}
