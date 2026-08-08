//! The default key bindings: the prefix map, and the stack prefix map that redirects the tab keys at the focused stack. Not yet user-configurable; `config.rs` covers only `[[app_focus]]` so far.

use std::collections::HashMap;

use vase_core::input::keys::{key_code_for_name, VK_COMMA, VK_DOWN_ARROW as VK_DOWN, VK_H, VK_J, VK_K, VK_L, VK_LEFT, VK_PERIOD, VK_RIGHT, VK_UP_ARROW as VK_UP};
use vase_core::input::keys::{VK_A, VK_E};
use vase_core::input::{InputCommand, Key, KeyRouter, Mods};

const VK_Q: u16 = 0x0C;
const VK_BACKSLASH: u16 = 0x2A;
const VK_MINUS: u16 = 0x1B;
const VK_W: u16 = 0x0D;
const VK_Z: u16 = 0x06;
const VK_X: u16 = 0x07;
const VK_T: u16 = 0x11;
const VK_S: u16 = 0x01;
const VK_C: u16 = 0x08;
const VK_LBRACKET: u16 = 0x21;
const VK_RBRACKET: u16 = 0x1E;
const VK_SEMICOLON: u16 = 0x29;

/// The modal router: the prefix drives tabs and panes, the stack prefix redirects the tab keys at the focused stack.
pub fn router() -> KeyRouter {
    KeyRouter::new(Key::alt(VK_A), bindings()).with_prefix(Key::alt(VK_E), bindings_nested())
}

fn bindings() -> HashMap<Key, InputCommand> {
    let mut b = HashMap::new();
    b.insert(Key::plain(VK_PERIOD), InputCommand::StackNext);
    b.insert(Key::plain(VK_COMMA), InputCommand::StackPrev);
    b.insert(Key::plain(VK_L), InputCommand::LastTab);
    b.insert(Key::plain(VK_Q), InputCommand::Quit);
    b.insert(Key::plain(VK_BACKSLASH), InputCommand::SplitH);
    b.insert(Key::plain(VK_MINUS), InputCommand::SplitV);
    b.insert(Key::plain(VK_LEFT), InputCommand::FocusLeft);
    b.insert(Key::plain(VK_RIGHT), InputCommand::FocusRight);
    b.insert(Key::plain(VK_UP), InputCommand::FocusUp);
    b.insert(Key::plain(VK_DOWN), InputCommand::FocusDown);
    b.insert(Key::plain(VK_W), InputCommand::WindowSwitcher);
    b.insert(Key::plain(VK_Z), InputCommand::ZoomToggle);
    b.insert(Key::plain(VK_X), InputCommand::BreakPane);
    b.insert(Key::plain(VK_C), InputCommand::NewTab);
    b.insert(Key::plain(VK_S), InputCommand::Stackify);
    b.insert(Key::plain(VK_LBRACKET), InputCommand::StackFocusPrev);
    b.insert(Key::plain(VK_RBRACKET), InputCommand::StackFocusNext);
    b.insert(Key::plain(VK_T), InputCommand::Rename);
    let shift = Mods { shift: true, ..Mods::default() };
    // prefix-: (":" is Shift-semicolon on a US layout).
    b.insert(Key { code: VK_SEMICOLON, mods: shift }, InputCommand::CommandLine);
    let cmd = Mods { cmd: true, ..Mods::default() };
    b.insert(Key { code: VK_LEFT, mods: shift }, InputCommand::ResizeLeft);
    b.insert(Key { code: VK_RIGHT, mods: shift }, InputCommand::ResizeRight);
    b.insert(Key { code: VK_UP, mods: shift }, InputCommand::ResizeUp);
    b.insert(Key { code: VK_DOWN, mods: shift }, InputCommand::ResizeDown);
    // Resize also on Shift-HJKL (vim), consistent with Shift-arrows.
    b.insert(Key { code: VK_H, mods: shift }, InputCommand::ResizeLeft);
    b.insert(Key { code: VK_J, mods: shift }, InputCommand::ResizeDown);
    b.insert(Key { code: VK_K, mods: shift }, InputCommand::ResizeUp);
    b.insert(Key { code: VK_L, mods: shift }, InputCommand::ResizeRight);
    // Move a pane: primarily on <mod>-HJKL (letters, which the arrow-exchange Karabiner rule can't touch). Arrows are kept too so an armed thumb-arrow doesn't leak to the terminal.
    // Bind cmd/ctrl/alt for all three, since per-device modifier swaps land it differently per keyboard.
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let alt = Mods { alt: true, ..Mods::default() };
    for (code, mv) in [
        (VK_H, InputCommand::MoveLeft),
        (VK_L, InputCommand::MoveRight),
        (VK_K, InputCommand::MoveUp),
        (VK_J, InputCommand::MoveDown),
        (VK_LEFT, InputCommand::MoveLeft),
        (VK_RIGHT, InputCommand::MoveRight),
        (VK_UP, InputCommand::MoveUp),
        (VK_DOWN, InputCommand::MoveDown),
    ] {
        b.insert(Key { code, mods: cmd }, mv.clone());
        b.insert(Key { code, mods: ctrl }, mv.clone());
        b.insert(Key { code, mods: alt }, mv);
    }
    b.insert(Key { code: VK_COMMA, mods: shift }, InputCommand::MoveTabPrev);
    b.insert(Key { code: VK_PERIOD, mods: shift }, InputCommand::MoveTabNext);
    // Shift-[ / ] send the current tab to the left / right monitor.
    b.insert(Key { code: VK_LBRACKET, mods: shift }, InputCommand::MoveTabMonitorPrev);
    b.insert(Key { code: VK_RBRACKET, mods: shift }, InputCommand::MoveTabMonitorNext);
    // prefix-1..9 select the Nth tab in bar order.
    for n in 1..=9usize {
        if let Some(code) = key_code_for_name(&n.to_string()) {
            b.insert(Key::plain(code), InputCommand::SelectBarTab(n));
        }
    }
    b
}

/// The nested-stack binding set: like the prefix, but tab-management keys act on the focused stack instead of the screen's tabs.
fn bindings_nested() -> HashMap<Key, InputCommand> {
    let mut b = bindings();
    // . / , cycle the focused stack (mirrors the prefix next/prev tab).
    b.insert(Key::plain(VK_PERIOD), InputCommand::StackFocusNext);
    b.insert(Key::plain(VK_COMMA), InputCommand::StackFocusPrev);
    // t renames the selected stack item (mirrors the prefix rename).
    b.insert(Key::plain(VK_T), InputCommand::StackRename);
    // ⇧, / ⇧. reorder the selected stack item (mirrors the prefix reorder tab).
    let shift = Mods { shift: true, ..Mods::default() };
    b.insert(Key { code: VK_COMMA, mods: shift }, InputCommand::StackMovePrev);
    b.insert(Key { code: VK_PERIOD, mods: shift }, InputCommand::StackMoveNext);
    // 1-9 select the Nth stack item (mirrors the prefix select tab).
    for n in 1..=9usize {
        if let Some(code) = key_code_for_name(&n.to_string()) {
            b.insert(Key::plain(code), InputCommand::StackSelectItem(n));
        }
    }
    b
}

#[cfg(test)]
#[path = "keymap_test.rs"]
mod tests;
