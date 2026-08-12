//! Carbon HIToolbox (`kVK_*`) keycodes, mapped onto the platform-neutral `KeyCode`.
//! <https://developer.apple.com/documentation/coreservices/carbon_core/virtual_keycodes>

use vase_core::input::KeyCode;

/// Kill-chord escape, matched before the router sees the event.
pub const VK_ESC: u16 = 0x35;

/// The `KeyCode` a Carbon virtual keycode denotes on a US layout.
pub fn key_code(vk: u16) -> Option<KeyCode> {
    Some(match vk {
        0x35 => KeyCode::Escape,
        0x24 => KeyCode::Return,
        0x33 => KeyCode::Backspace,
        0x30 => KeyCode::Tab,
        0x7B => KeyCode::Left,
        0x7C => KeyCode::Right,
        0x7D => KeyCode::Down,
        0x7E => KeyCode::Up,
        _ => KeyCode::Char(char_for(vk)?),
    })
}

/// US-layout character for a keycode.
fn char_for(vk: u16) -> Option<char> {
    Some(match vk {
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
        0x2B => ',',
        0x2F => '.',
        0x1B => '-',
        0x18 => '=',
        0x29 => ';',
        0x27 => '\'',
        0x2A => '\\',
        0x21 => '[',
        0x1E => ']',
        0x32 => '`',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_the_default_bindings_use_all_resolve() {
        // Every chord in the default keymap has to survive the translation, or a binding silently dies.
        for (vk, want) in [
            (0x00, KeyCode::Char('a')),
            (0x0E, KeyCode::Char('e')),
            (0x2F, KeyCode::Char('.')),
            (0x2B, KeyCode::Char(',')),
            (0x2A, KeyCode::Char('\\')),
            (0x1B, KeyCode::Char('-')),
            (0x21, KeyCode::Char('[')),
            (0x1E, KeyCode::Char(']')),
            (0x29, KeyCode::Char(';')),
            (0x12, KeyCode::Char('1')),
            (0x7B, KeyCode::Left),
            (0x24, KeyCode::Return),
            (0x35, KeyCode::Escape),
            (0x33, KeyCode::Backspace),
        ] {
            assert_eq!(key_code(vk), Some(want), "keycode {vk:#04x}");
        }
    }

    #[test]
    fn a_key_with_no_name_is_dropped_rather_than_guessed() {
        assert_eq!(key_code(0x7A), None); // F1
        assert_eq!(key_code(0x3B), None); // Control
    }
}
