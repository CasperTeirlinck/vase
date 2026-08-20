use super::*;
use crate::input::keymap::{bindings, bindings_nested};

fn armed() -> KeyRouter {
    let mut r = router();
    assert_eq!(r.key(Key::alt(KeyCode::Char('a'))), Decision::Consume);
    r
}

fn meta() -> Mods {
    Mods { meta: true, ..Mods::default() }
}

#[test]
fn prefix_then_modifier_arrow_moves_a_pane() {
    // The prefix then Meta/Ctrl-arrow must reach the Move* commands.
    assert_eq!(armed().key(Key { code: KeyCode::Left, mods: meta() }), Decision::ConsumeAndRun(InputCommand::MoveLeft));
    // Ctrl-arrow moves too (Karabiner remaps physical Cmd-arrow to Ctrl).
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    assert_eq!(armed().key(Key { code: KeyCode::Right, mods: ctrl }), Decision::ConsumeAndRun(InputCommand::MoveRight));
    // Letters move too (arrow-exchange-proof): Meta-H → MoveLeft.
    assert_eq!(armed().key(Key { code: KeyCode::Char('h'), mods: meta() }), Decision::ConsumeAndRun(InputCommand::MoveLeft));
}

#[test]
fn bare_arrows_focus_and_shift_arrows_resize() {
    assert_eq!(armed().key(Key::plain(KeyCode::Left)), Decision::ConsumeAndRun(InputCommand::FocusLeft));
    let shift = Mods { shift: true, ..Mods::default() };
    assert_eq!(armed().key(Key { code: KeyCode::Left, mods: shift }), Decision::ConsumeAndRun(InputCommand::ResizeLeft));
}

#[test]
fn prefix_ctrl_r_resyncs() {
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    assert_eq!(armed().key(Key { code: KeyCode::Char('r'), mods: ctrl }), Decision::ConsumeAndRun(InputCommand::Resync));
}

#[test]
fn digits_select_a_tab_under_the_main_prefix() {
    assert_eq!(armed().key(Key::ch('2')), Decision::ConsumeAndRun(InputCommand::SelectBarTab(2)));
}

#[test]
fn the_nested_prefix_redirects_the_tab_keys_at_the_stack() {
    let mut r = router();
    assert_eq!(r.key(Key::alt(KeyCode::Char('e'))), Decision::Consume);
    assert_eq!(r.key(Key::ch('.')), Decision::ConsumeAndRun(InputCommand::StackFocusNext));

    r.key(Key::alt(KeyCode::Char('e')));
    assert_eq!(r.key(Key::ch('2')), Decision::ConsumeAndRun(InputCommand::StackSelectItem(2)));

    r.key(Key::alt(KeyCode::Char('e')));
    assert_eq!(r.key(Key::ch('t')), Decision::ConsumeAndRun(InputCommand::StackRename));
}

#[test]
fn the_two_prefixes_keep_their_own_meanings_for_the_same_key() {
    // `.` is next-tab under the prefix and next-stack-item under the stack prefix.
    assert_eq!(armed().key(Key::ch('.')), Decision::ConsumeAndRun(InputCommand::StackNext));
    let mut r = router();
    r.key(Key::alt(KeyCode::Char('e')));
    assert_eq!(r.key(Key::ch('.')), Decision::ConsumeAndRun(InputCommand::StackFocusNext));
}

#[test]
fn the_nested_map_inherits_the_keys_it_does_not_override() {
    let mut r = router();
    r.key(Key::alt(KeyCode::Char('e')));
    assert_eq!(r.key(Key::plain(KeyCode::Left)), Decision::ConsumeAndRun(InputCommand::FocusLeft));
}

#[test]
fn every_bound_command_is_reachable_from_one_of_the_two_maps() {
    let mut seen: Vec<InputCommand> = bindings().into_values().collect();
    seen.extend(bindings_nested().into_values());
    for want in [
        InputCommand::NewTab,
        InputCommand::Quit,
        InputCommand::WindowSwitcher,
        InputCommand::CommandLine,
        InputCommand::BreakPane,
        InputCommand::Stackify,
        InputCommand::Rename,
        InputCommand::StackRename,
        InputCommand::ZoomToggle,
        InputCommand::LastTab,
        InputCommand::MoveTabMonitorPrev,
        InputCommand::MoveTabMonitorNext,
    ] {
        assert!(seen.contains(&want), "{want:?} has no binding");
    }
}
