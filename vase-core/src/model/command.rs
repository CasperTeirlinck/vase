use crate::focus::Direction;
use crate::geometry::Rect;
use crate::input::InputCommand;
use crate::tree::{Dir, WindowId};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// A new managed window appeared; open it as a single-pane tab.
    AddWindow(WindowId, usize),
    /// A window closed; remove it, dropping the tab if empty.
    RemoveWindow(WindowId),
    /// Move pane focus in a direction (crosses monitors).
    Focus(Direction),
    /// Append a new empty tab and focus it.
    NewTab,
    /// Select the next tab (wraps).
    NextTab,
    /// Select the previous tab (wraps).
    PrevTab,
    /// Select a tab by index.
    SelectTab(usize),
    /// Select a tab by (screen, tab) index and focus that screen.
    SelectScreenTab(usize, usize),
    /// Split the focused pane.
    Split(Dir),
    /// Swap the focused pane with its neighbor (crosses monitors).
    MoveWindow(Direction),
    /// Reorder the current tab by `offset`.
    MoveTab(isize),
    /// Send the current tab to the adjacent monitor (+1 right, -1 left), wrapping.
    MoveTabToScreen(isize),
    /// Set or clear the current tab's custom name.
    SetTabName(Option<String>),
    /// Resize the focused pane by nudging its split ratio.
    Resize(Direction),
    /// Toggle maximize of the focused window.
    ToggleZoom,
    /// OS focus moved to this window; sync our focus to it.
    SyncFocus(WindowId),
    /// Focus and raise a specific window.
    Raise(WindowId),
    /// Move a window into the focused empty pane.
    FillPane(WindowId),
    /// Remove the focused pane if it's empty.
    CloseFocusedPane,
    /// Pop the focused pane out into its own new tab.
    BreakPane,
    /// Turn the focused pane into a stack, or grow an existing one.
    Stackify,
    /// Cycle the focused stack's selected item by `delta` (wraps).
    StackCycle(isize),
    /// Select the stack item holding this window and focus it.
    SelectStackWindow(WindowId),
    /// Select the Nth (1-based) item of the focused stack.
    StackSelect(usize),
    /// Move the focused stack's selected item by `delta` (clamped).
    StackMove(isize),
    /// Set or clear the custom name of the focused stack's selected item.
    SetStackName(Option<String>),
}

impl Command {
    /// The model edit a binding maps to. `None` for the bindings that drive the daemon itself rather than the model.
    pub fn from_input(input: &InputCommand) -> Option<Command> {
        use Direction::{Down, Left, Right, Up};
        use InputCommand as I;
        Some(match input {
            I::NewTab => Command::NewTab,
            I::StackNext => Command::NextTab,
            I::StackPrev => Command::PrevTab,
            I::SplitH => Command::Split(Dir::Horizontal),
            I::SplitV => Command::Split(Dir::Vertical),
            I::FocusLeft => Command::Focus(Left),
            I::FocusRight => Command::Focus(Right),
            I::FocusUp => Command::Focus(Up),
            I::FocusDown => Command::Focus(Down),
            I::ResizeLeft => Command::Resize(Left),
            I::ResizeRight => Command::Resize(Right),
            I::ResizeUp => Command::Resize(Up),
            I::ResizeDown => Command::Resize(Down),
            I::MoveLeft => Command::MoveWindow(Left),
            I::MoveRight => Command::MoveWindow(Right),
            I::MoveUp => Command::MoveWindow(Up),
            I::MoveDown => Command::MoveWindow(Down),
            I::MoveTabPrev => Command::MoveTab(-1),
            I::MoveTabNext => Command::MoveTab(1),
            I::MoveTabMonitorPrev => Command::MoveTabToScreen(-1),
            I::MoveTabMonitorNext => Command::MoveTabToScreen(1),
            I::ZoomToggle => Command::ToggleZoom,
            I::BreakPane => Command::BreakPane,
            I::Stackify => Command::Stackify,
            I::StackFocusPrev => Command::StackCycle(-1),
            I::StackFocusNext => Command::StackCycle(1),
            I::StackSelectItem(n) => Command::StackSelect(*n),
            I::StackMovePrev => Command::StackMove(-1),
            I::StackMoveNext => Command::StackMove(1),
            I::LastTab | I::Quit | I::SendPrefix | I::WindowSwitcher | I::Rename | I::StackRename | I::CommandLine | I::SelectBarTab(_) => return None,
        })
    }
}

/// A side effect for the backend to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Place these windows at these rects and raise the newly-visible ones. Managed windows absent from the list are left where they are and end up occluded; nothing is moved off-screen.
    /// Always emitted before the `FocusWindow` of the same batch.
    Render(Vec<(WindowId, Rect)>),
    /// Give OS focus to this window.
    FocusWindow(WindowId),
}
