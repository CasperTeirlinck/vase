use super::*;

#[test]
fn the_keys_the_default_bindings_use_all_resolve() {
    // Every chord in the default keymap has to survive the translation, or a binding silently dies.
    for (vk, want) in [
        (0x41, KeyCode::Char('a')),
        (0x45, KeyCode::Char('e')),
        (0x5A, KeyCode::Char('z')),
        (0xBE, KeyCode::Char('.')),
        (0xBC, KeyCode::Char(',')),
        (0xDC, KeyCode::Char('\\')),
        (0xBD, KeyCode::Char('-')),
        (0xDB, KeyCode::Char('[')),
        (0xDD, KeyCode::Char(']')),
        (0xBA, KeyCode::Char(';')),
        (0x31, KeyCode::Char('1')),
        (0x39, KeyCode::Char('9')),
        (0x25, KeyCode::Left),
        (0x0D, KeyCode::Return),
        (0x1B, KeyCode::Escape),
        (0x08, KeyCode::Backspace),
    ] {
        assert_eq!(key_code(vk), Some(want), "vk {vk:#04x}");
    }
}

#[test]
fn a_key_with_no_name_is_dropped_rather_than_guessed() {
    assert_eq!(key_code(0x70), None); // F1
    assert_eq!(key_code(0x11), None); // Ctrl
    assert_eq!(key_code(0x60), None); // numpad 0
}
