//! macOS virtual keycodes (Carbon HIToolbox `kVK_*`) and the US-layout character table.
//! `Key::code` is one of these everywhere, so the modal grammars that read keys live here rather
//! than behind the platform seam.
//! <https://developer.apple.com/documentation/coreservices/carbon_core/virtual_keycodes>

pub const VK_A: u16 = 0x00;
pub const VK_E: u16 = 0x0E;
pub const VK_G: u16 = 0x05;
pub const VK_H: u16 = 0x04;
pub const VK_J: u16 = 0x26;
pub const VK_K: u16 = 0x28;
pub const VK_L: u16 = 0x25;
pub const VK_ESC: u16 = 0x35;
pub const VK_RETURN: u16 = 0x24;
pub const VK_DELETE: u16 = 0x33; // backspace
pub const VK_COMMA: u16 = 0x2B;
pub const VK_PERIOD: u16 = 0x2F;
pub const VK_LEFT: u16 = 0x7B;
pub const VK_RIGHT: u16 = 0x7C;
pub const VK_DOWN_ARROW: u16 = 0x7D;
pub const VK_UP_ARROW: u16 = 0x7E;

/// Virtual keycode for a key name used in config chords (`a`, `7`, `grave`, ...).
pub fn key_code_for_name(name: &str) -> Option<u16> {
    let code = match name {
        "grave" | "backtick" | "`" => 0x32,
        "space" => 0x31,
        "tab" => 0x30,
        "return" | "enter" => VK_RETURN,
        "escape" | "esc" => VK_ESC,
        "delete" | "backspace" => VK_DELETE,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "down" => VK_DOWN_ARROW,
        "up" => VK_UP_ARROW,
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

/// US-layout character for a keycode; `None` for non-text keys.
pub fn char_for_keycode(code: u16) -> Option<char> {
    Some(match code {
        0x00 => 'a',
        0x0B => 'b',
        0x08 => 'c',
        0x02 => 'd',
        0x0E => 'e',
        0x03 => 'f',
        0x05 => 'g',
        0x04 => 'h',
        0x22 => 'i',
        0x26 => 'j',
        0x28 => 'k',
        0x25 => 'l',
        0x2E => 'm',
        0x2D => 'n',
        0x1F => 'o',
        0x23 => 'p',
        0x0C => 'q',
        0x0F => 'r',
        0x01 => 's',
        0x11 => 't',
        0x20 => 'u',
        0x09 => 'v',
        0x0D => 'w',
        0x07 => 'x',
        0x10 => 'y',
        0x06 => 'z',
        0x1D => '0',
        0x12 => '1',
        0x13 => '2',
        0x14 => '3',
        0x15 => '4',
        0x17 => '5',
        0x16 => '6',
        0x1A => '7',
        0x1C => '8',
        0x19 => '9',
        0x31 => ' ',
        0x2C => '/',
        _ => return None,
    })
}
