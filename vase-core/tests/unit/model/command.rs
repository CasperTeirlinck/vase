use super::*;
use crate::focus::Direction;
use crate::input::InputCommand as I;
use crate::tree::Dir;

/// The bindings that drive the daemon rather than the model. Exhaustive on purpose: adding an
/// `InputCommand` breaks this match, so the new binding has to be classified here too.
fn drives_the_daemon(cmd: &I) -> bool {
    match cmd {
        I::LastTab | I::Quit | I::SendPrefix | I::WindowSwitcher | I::Rename | I::StackRename | I::CommandLine | I::WarpCursor | I::Resync | I::SelectBarTab(_) => true,
        I::NewTab
        | I::StackNext
        | I::StackPrev
        | I::SplitH
        | I::SplitV
        | I::FocusLeft
        | I::FocusRight
        | I::FocusUp
        | I::FocusDown
        | I::ResizeLeft
        | I::ResizeRight
        | I::ResizeUp
        | I::ResizeDown
        | I::MoveLeft
        | I::MoveRight
        | I::MoveUp
        | I::MoveDown
        | I::MoveTabPrev
        | I::MoveTabNext
        | I::MoveTabMonitorPrev
        | I::MoveTabMonitorNext
        | I::ZoomToggle
        | I::BreakPane
        | I::Stackify
        | I::StackFocusPrev
        | I::StackFocusNext
        | I::StackSelectItem(_)
        | I::StackMovePrev
        | I::StackMoveNext => false,
    }
}

fn all() -> Vec<I> {
    vec![
        I::LastTab,
        I::Quit,
        I::SendPrefix,
        I::WindowSwitcher,
        I::Rename,
        I::StackRename,
        I::CommandLine,
        I::WarpCursor,
        I::Resync,
        I::SelectBarTab(3),
        I::NewTab,
        I::StackNext,
        I::StackPrev,
        I::SplitH,
        I::SplitV,
        I::FocusLeft,
        I::FocusRight,
        I::FocusUp,
        I::FocusDown,
        I::ResizeLeft,
        I::ResizeRight,
        I::ResizeUp,
        I::ResizeDown,
        I::MoveLeft,
        I::MoveRight,
        I::MoveUp,
        I::MoveDown,
        I::MoveTabPrev,
        I::MoveTabNext,
        I::MoveTabMonitorPrev,
        I::MoveTabMonitorNext,
        I::ZoomToggle,
        I::BreakPane,
        I::Stackify,
        I::StackFocusPrev,
        I::StackFocusNext,
        I::StackSelectItem(2),
        I::StackMovePrev,
        I::StackMoveNext,
    ]
}

#[test]
fn a_binding_maps_to_a_command_exactly_when_it_is_not_a_daemon_action() {
    for cmd in all() {
        assert_eq!(Command::from_input(&cmd).is_none(), drives_the_daemon(&cmd), "{cmd:?}");
    }
}

#[test]
fn prev_and_next_pairs_map_to_opposite_offsets() {
    assert_eq!(Command::from_input(&I::StackFocusPrev), Some(Command::StackCycle(-1)));
    assert_eq!(Command::from_input(&I::StackFocusNext), Some(Command::StackCycle(1)));
    assert_eq!(Command::from_input(&I::StackMovePrev), Some(Command::StackMove(-1)));
    assert_eq!(Command::from_input(&I::StackMoveNext), Some(Command::StackMove(1)));
    assert_eq!(Command::from_input(&I::MoveTabPrev), Some(Command::MoveTab(-1)));
    assert_eq!(Command::from_input(&I::MoveTabNext), Some(Command::MoveTab(1)));
    assert_eq!(Command::from_input(&I::MoveTabMonitorPrev), Some(Command::MoveTabToScreen(-1)));
    assert_eq!(Command::from_input(&I::MoveTabMonitorNext), Some(Command::MoveTabToScreen(1)));
}

#[test]
fn focus_resize_and_move_share_the_direction_but_not_the_command() {
    assert_eq!(Command::from_input(&I::FocusLeft), Some(Command::Focus(Direction::Left)));
    assert_eq!(Command::from_input(&I::ResizeLeft), Some(Command::Resize(Direction::Left)));
    assert_eq!(Command::from_input(&I::MoveLeft), Some(Command::MoveWindow(Direction::Left)));
}

#[test]
fn the_two_splits_differ_by_axis() {
    assert_eq!(Command::from_input(&I::SplitH), Some(Command::Split(Dir::Horizontal)));
    assert_eq!(Command::from_input(&I::SplitV), Some(Command::Split(Dir::Vertical)));
}

#[test]
fn stack_next_prev_act_on_tabs_not_on_the_stack() {
    // cycle top-level tabs; the stack analogs are StackFocusNext/Prev.
    assert_eq!(Command::from_input(&I::StackNext), Some(Command::NextTab));
    assert_eq!(Command::from_input(&I::StackPrev), Some(Command::PrevTab));
}
