//! The platform-neutral key identity. Each backend maps its own scancodes onto this at the event hook.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// The US-layout character this key types unmodified.
    Char(char),
    Escape,
    Return,
    Backspace,
    Tab,
    Left,
    Right,
    Up,
    Down,
}

impl KeyCode {
    pub fn char(self) -> Option<char> {
        match self {
            KeyCode::Char(c) => Some(c),
            _ => None,
        }
    }

    /// The key as a user reads it: the character it types, or a glyph for the named keys.
    pub fn label(self) -> String {
        match self {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Escape => "esc".to_string(),
            KeyCode::Return => "⏎".to_string(),
            KeyCode::Backspace => "⌫".to_string(),
            KeyCode::Tab => "⇥".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
        }
    }

    /// The key a config chord names (`a`, `7`, `grave`, `escape`, ...).
    pub fn from_name(name: &str) -> Option<KeyCode> {
        Some(match name {
            "grave" | "backtick" | "`" => KeyCode::Char('`'),
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "return" | "enter" => KeyCode::Return,
            "escape" | "esc" => KeyCode::Escape,
            "delete" | "backspace" => KeyCode::Backspace,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "down" => KeyCode::Down,
            "up" => KeyCode::Up,
            "minus" => KeyCode::Char('-'),
            "equal" => KeyCode::Char('='),
            "comma" => KeyCode::Char(','),
            "period" => KeyCode::Char('.'),
            "slash" => KeyCode::Char('/'),
            "semicolon" => KeyCode::Char(';'),
            "backslash" => KeyCode::Char('\\'),
            _ => {
                let mut chars = name.chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                KeyCode::Char(c)
            }
        })
    }
}
