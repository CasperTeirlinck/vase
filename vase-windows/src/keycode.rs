//! Win32 virtual-key codes, mapped onto the platform-neutral `KeyCode`.
//!
//! A fixed table rather than `MapVirtualKeyW(MAPVK_VK_TO_CHAR)`: the keymap is written in US-layout
//! characters, and resolving through the active layout would move every binding on an AZERTY machine.
//! <https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes>

use vase_core::input::KeyCode;

/// Kill-chord escape, matched before the router sees the event.
pub const VK_ESCAPE: u32 = 0x1B;

/// The `KeyCode` a virtual-key code denotes on a US layout.
pub fn key_code(vk: u32) -> Option<KeyCode> {
    Some(match vk {
        0x1B => KeyCode::Escape,
        0x0D => KeyCode::Return,
        0x08 => KeyCode::Backspace,
        0x09 => KeyCode::Tab,
        0x25 => KeyCode::Left,
        0x27 => KeyCode::Right,
        0x26 => KeyCode::Up,
        0x28 => KeyCode::Down,
        0x20 => KeyCode::Char(' '),
        // VK_A..VK_Z and VK_0..VK_9 are the ASCII code points; the keymap uses lowercase letters.
        0x41..=0x5A => KeyCode::Char((vk as u8 - 0x41 + b'a') as char),
        0x30..=0x39 => KeyCode::Char((vk as u8 - 0x30 + b'0') as char),
        // OEM keys, US layout.
        0xBA => KeyCode::Char(';'),
        0xBB => KeyCode::Char('='),
        0xBC => KeyCode::Char(','),
        0xBD => KeyCode::Char('-'),
        0xBE => KeyCode::Char('.'),
        0xBF => KeyCode::Char('/'),
        0xC0 => KeyCode::Char('`'),
        0xDB => KeyCode::Char('['),
        0xDC => KeyCode::Char('\\'),
        0xDD => KeyCode::Char(']'),
        0xDE => KeyCode::Char('\''),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "keycode_test.rs"]
mod tests;
