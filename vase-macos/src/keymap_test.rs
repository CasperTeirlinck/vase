use super::*;
use vase_core::input::keys::{VK_A, VK_E, VK_RIGHT};
use vase_core::input::Decision;

fn armed() -> KeyRouter {
    let mut r = router();
    assert_eq!(r.key(Key::alt(VK_A)), Decision::Consume);
    r
}

fn cmd() -> Mods {
    Mods { cmd: true, ..Mods::default() }
}

#[test]
fn prefix_then_modifier_arrow_moves_a_pane() {
    // Alt-a then Cmd/Ctrl-arrow must reach the Move* commands.
    assert_eq!(armed().key(Key { code: VK_LEFT, mods: cmd() }), Decision::ConsumeAndRun(InputCommand::MoveLeft));
    // Ctrl-arrow moves too (Karabiner remaps physical Cmd-arrow to Ctrl).
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    assert_eq!(armed().key(Key { code: VK_RIGHT, mods: ctrl }), Decision::ConsumeAndRun(InputCommand::MoveRight));
    // Letters move too (arrow-exchange-proof): Cmd-H → MoveLeft.
    assert_eq!(armed().key(Key { code: VK_H, mods: cmd() }), Decision::ConsumeAndRun(InputCommand::MoveLeft));
}

#[test]
fn bare_arrows_focus_and_shift_arrows_resize() {
    assert_eq!(armed().key(Key::plain(VK_LEFT)), Decision::ConsumeAndRun(InputCommand::FocusLeft));
    let shift = Mods { shift: true, ..Mods::default() };
    assert_eq!(armed().key(Key { code: VK_LEFT, mods: shift }), Decision::ConsumeAndRun(InputCommand::ResizeLeft));
}

#[test]
fn digits_select_a_tab_under_the_main_prefix() {
    let two = key_code_for_name("2").unwrap();
    assert_eq!(armed().key(Key::plain(two)), Decision::ConsumeAndRun(InputCommand::SelectBarTab(2)));
}

#[test]
fn the_nested_prefix_redirects_the_tab_keys_at_the_stack() {
    let mut r = router();
    assert_eq!(r.key(Key::alt(VK_E)), Decision::Consume);
    assert_eq!(r.key(Key::plain(VK_PERIOD)), Decision::ConsumeAndRun(InputCommand::StackFocusNext));

    let two = key_code_for_name("2").unwrap();
    r.key(Key::alt(VK_E));
    assert_eq!(r.key(Key::plain(two)), Decision::ConsumeAndRun(InputCommand::StackSelectItem(2)));

    r.key(Key::alt(VK_E));
    assert_eq!(r.key(Key::plain(VK_T)), Decision::ConsumeAndRun(InputCommand::StackRename));
}

#[test]
fn the_two_prefixes_keep_their_own_meanings_for_the_same_key() {
    // `.` is next-tab under the prefix and next-stack-item under the stack prefix.
    assert_eq!(armed().key(Key::plain(VK_PERIOD)), Decision::ConsumeAndRun(InputCommand::StackNext));
    let mut r = router();
    r.key(Key::alt(VK_E));
    assert_eq!(r.key(Key::plain(VK_PERIOD)), Decision::ConsumeAndRun(InputCommand::StackFocusNext));
}

#[test]
fn the_nested_map_inherits_the_keys_it_does_not_override() {
    let mut r = router();
    r.key(Key::alt(VK_E));
    assert_eq!(r.key(Key::plain(VK_LEFT)), Decision::ConsumeAndRun(InputCommand::FocusLeft));
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
