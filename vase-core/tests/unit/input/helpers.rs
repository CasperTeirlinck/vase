use super::*;
use std::time::Instant;

pub fn t0() -> Instant {
    Instant::now()
}

/// A row that is a header (unselectable), a plain entry, or selectable-but-unnumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Head(u8),
    Win(u8),
    Launch(u8),
}

impl Item for Row {
    fn selectable(&self) -> bool {
        !matches!(self, Row::Head(_))
    }
    fn numbered(&self) -> bool {
        matches!(self, Row::Win(_))
    }
}

/// Three plain rows: Ghostty, Google Chrome, Obsidian.
pub fn sw() -> Switcher<Row> {
    Switcher::new(vec![(Row::Win(1), "Ghostty".into()), (Row::Win(2), "Google Chrome".into()), (Row::Win(3), "Obsidian".into())])
}

pub fn press(s: &mut Switcher<Row>, code: KeyCode) -> Pick<Row> {
    s.key(Key::plain(code), t0())
}

pub fn ch(s: &mut Switcher<Row>, c: char) -> Pick<Row> {
    press(s, KeyCode::Char(c))
}

pub fn ids(s: &Switcher<Row>) -> Vec<Row> {
    s.visible().into_iter().map(|(r, _)| r).collect()
}
