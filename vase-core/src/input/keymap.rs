//! The default key bindings. Not user-configurable yet.

use std::collections::HashMap;

use super::{InputCommand, Key, KeyCode, KeyRouter, Mods};

pub fn router() -> KeyRouter {
    KeyRouter::new(Key::alt(KeyCode::Char('a')), bindings()).with_prefix(Key::alt(KeyCode::Char('e')), bindings_nested())
}

pub(crate) fn bindings() -> HashMap<Key, InputCommand> {
    use InputCommand as I;
    let shift = Mods { shift: true, ..Mods::default() };
    let mut b = HashMap::new();
    for (c, cmd) in [
        ('.', I::StackNext),
        (',', I::StackPrev),
        ('l', I::LastTab),
        ('q', I::Quit),
        ('\\', I::SplitH),
        ('-', I::SplitV),
        ('w', I::WindowSwitcher),
        ('z', I::ZoomToggle),
        ('x', I::BreakPane),
        ('c', I::NewTab),
        ('s', I::Stackify),
        ('[', I::StackFocusPrev),
        (']', I::StackFocusNext),
        ('t', I::Rename),
        ('m', I::WarpCursor),
    ] {
        b.insert(Key::ch(c), cmd);
    }
    for (code, cmd) in [(KeyCode::Left, I::FocusLeft), (KeyCode::Right, I::FocusRight), (KeyCode::Up, I::FocusUp), (KeyCode::Down, I::FocusDown)] {
        b.insert(Key::plain(code), cmd);
    }
    // prefix-: (":" is Shift-semicolon on a US layout).
    b.insert(Key { code: KeyCode::Char(';'), mods: shift }, I::CommandLine);
    // Shift-HJKL resizes too, for vim hands.
    for (code, cmd) in [
        (KeyCode::Left, I::ResizeLeft),
        (KeyCode::Right, I::ResizeRight),
        (KeyCode::Up, I::ResizeUp),
        (KeyCode::Down, I::ResizeDown),
        (KeyCode::Char('h'), I::ResizeLeft),
        (KeyCode::Char('j'), I::ResizeDown),
        (KeyCode::Char('k'), I::ResizeUp),
        (KeyCode::Char('l'), I::ResizeRight),
    ] {
        b.insert(Key { code, mods: shift }, cmd);
    }
    // Move a pane: primarily on <mod>-HJKL (letters, which the arrow-exchange Karabiner rule can't touch). Arrows are kept too so an armed thumb-arrow doesn't leak to the terminal.
    // Bind meta/ctrl/alt for all three, since per-device modifier swaps land it differently per keyboard.
    for (code, mv) in [
        (KeyCode::Char('h'), I::MoveLeft),
        (KeyCode::Char('l'), I::MoveRight),
        (KeyCode::Char('k'), I::MoveUp),
        (KeyCode::Char('j'), I::MoveDown),
        (KeyCode::Left, I::MoveLeft),
        (KeyCode::Right, I::MoveRight),
        (KeyCode::Up, I::MoveUp),
        (KeyCode::Down, I::MoveDown),
    ] {
        for mods in [Mods { meta: true, ..Mods::default() }, Mods { ctrl: true, ..Mods::default() }, Mods { alt: true, ..Mods::default() }] {
            b.insert(Key { code, mods }, mv.clone());
        }
    }
    b.insert(Key { code: KeyCode::Char(','), mods: shift }, I::MoveTabPrev);
    b.insert(Key { code: KeyCode::Char('.'), mods: shift }, I::MoveTabNext);
    // Shift-[ / ] send the current tab to the left / right monitor.
    b.insert(Key { code: KeyCode::Char('['), mods: shift }, I::MoveTabMonitorPrev);
    b.insert(Key { code: KeyCode::Char(']'), mods: shift }, I::MoveTabMonitorNext);
    // prefix-1..9 select the Nth tab in bar order.
    for n in 1..=9usize {
        b.insert(Key::ch(digit(n)), I::SelectBarTab(n));
    }
    b
}

/// Like `bindings`, but the tab-management keys act on the focused stack instead of the screen's tabs.
pub(crate) fn bindings_nested() -> HashMap<Key, InputCommand> {
    use InputCommand as I;
    let shift = Mods { shift: true, ..Mods::default() };
    // Each override below mirrors the prefix key it shadows, one level down.
    let mut b = bindings();
    b.insert(Key::ch('.'), I::StackFocusNext);
    b.insert(Key::ch(','), I::StackFocusPrev);
    b.insert(Key::ch('t'), I::StackRename);
    b.insert(Key { code: KeyCode::Char(','), mods: shift }, I::StackMovePrev);
    b.insert(Key { code: KeyCode::Char('.'), mods: shift }, I::StackMoveNext);
    for n in 1..=9usize {
        b.insert(Key::ch(digit(n)), I::StackSelectItem(n));
    }
    b
}

fn digit(n: usize) -> char {
    char::from_digit(n as u32, 10).unwrap()
}
