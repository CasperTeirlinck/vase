//! Translate macOS virtual keycodes + event flags into core input types.

use vase_core::input::{Key, Mods};

// macOS virtual keycodes (Carbon HIToolbox kVK_*).
pub const VK_A: u16 = 0x00;
pub const VK_E: u16 = 0x0E; // nested-stack prefix (⌥e)
pub const VK_ESC: i64 = 0x35;
pub const VK_RETURN: i64 = 0x24;
pub const VK_DELETE: i64 = 0x33; // backspace
pub const VK_DOWN_ARROW: i64 = 0x7D;
pub const VK_UP_ARROW: i64 = 0x7E;

/// Build core `Mods` from CGEventFlags bits.
pub fn mods_from_flags(cmd: bool, ctrl: bool, alt: bool, shift: bool) -> Mods {
    Mods { cmd, ctrl, alt, shift }
}

/// Build a core `Key` from a keycode and the current modifier booleans.
pub fn key_from(code: u16, mods: Mods) -> Key {
    Key { code, mods }
}

/// Virtual keycode for a key name used in config chords: single letters/digits
/// (`a`, `7`), punctuation typed as-is (`,`), or a named key (`grave`, `space`,
/// `left`). Case-insensitive callers should lowercase first.
pub fn key_code_for_name(name: &str) -> Option<u16> {
    let code = match name {
        "grave" | "backtick" | "`" => 0x32,
        "space" => 0x31,
        "tab" => 0x30,
        "return" | "enter" => 0x24,
        "escape" | "esc" => 0x35,
        "delete" | "backspace" => 0x33,
        "left" => 0x7B,
        "right" => 0x7C,
        "down" => 0x7D,
        "up" => 0x7E,
        "minus" | "-" => 0x1B,
        "equal" | "=" => 0x18,
        "comma" => 0x2B,
        "period" => 0x2F,
        "slash" => 0x2C,
        "semicolon" => 0x29,
        // Single char: letters/digits/punctuation, via the char↔code table.
        _ => {
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            return (0u16..0x80).find(|&code| char_for_keycode(code) == Some(c));
        }
    };
    Some(code)
}

/// US-layout character for a keycode (lowercase letters, digits, space, '/'),
/// for switcher typing. None for non-text keys.
pub fn char_for_keycode(code: u16) -> Option<char> {
    Some(match code {
        0x00 => 'a', 0x0B => 'b', 0x08 => 'c', 0x02 => 'd', 0x0E => 'e', 0x03 => 'f',
        0x05 => 'g', 0x04 => 'h', 0x22 => 'i', 0x26 => 'j', 0x28 => 'k', 0x25 => 'l',
        0x2E => 'm', 0x2D => 'n', 0x1F => 'o', 0x23 => 'p', 0x0C => 'q', 0x0F => 'r',
        0x01 => 's', 0x11 => 't', 0x20 => 'u', 0x09 => 'v', 0x0D => 'w', 0x07 => 'x',
        0x10 => 'y', 0x06 => 'z',
        0x1D => '0', 0x12 => '1', 0x13 => '2', 0x14 => '3', 0x15 => '4', 0x17 => '5',
        0x16 => '6', 0x1A => '7', 0x1C => '8', 0x19 => '9',
        0x31 => ' ', 0x2C => '/',
        _ => return None,
    })
}
