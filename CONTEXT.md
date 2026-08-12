# Vase — Ubiquitous Language

Vase is a keyboard-driven, tmux-philosophy window manager for native OS
windows. It manages windows it does not render: it can move, resize, raise,
hide, and focus them, and it draws its own overlay UI on top. It cannot embed
or reparent foreign app windows.

Canonical terms. "Window" always means a native OS window. Everything else is
named after its tmux analog, because the keybindings are, and a reader moving
between `CONTEXT.md` and the code should not have to translate.

| Term                   | Definition                                                                                                                                                                                                                             | In code                       | tmux analog                       |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | --------------------------------- |
| **Window**             | A native OS window drawn by some application. The atom vase manages.                                                                                                                                                                   | `WindowId`                    | _(none — tmux has no OS windows)_ |
| **Pane**               | A leaf slot in a layout tree holding one window, one stack, or nothing.                                                                                                                                                                | `Node::Leaf`, `PaneId`        | pane                              |
| **Split**              | An internal layout node dividing space horizontally or vertically into child nodes.                                                                                                                                                    | `Node::Split`                 | `⌥a \` / `⌥a -`                   |
| **Stack**              | A pane whose items share one rectangle; one is selected (visible), the rest are occluded, under a vase-drawn bar.                                                                                                                      | `Node::Stack`                 | inner-tmux tabbed pane            |
| **Stack item**         | One member of a stack. Selecting it raises its window over its siblings.                                                                                                                                                               | `Pane` inside `Node::Stack`   | a tmux _window_                   |
| **Tab**                | The top-level switchable container, owned by a screen. Holds one layout tree. **The tab bar is the user's bottom bar.** Switching tabs shows one set of windows and occludes another. There is no separate workspace or session level. | `Tab`, `Screen::tabs`         | a tmux _window_                   |
| **Layout**             | The tree of splits, stacks, and panes inside a tab.                                                                                                                                                                                    | `Node`                        | window layout                     |
| **Workspace**          | An OS-level desktop switch vase does not manage: a macOS Space, a Windows virtual desktop. A window on another one keeps its tab, marked `◇`.                                                                                          | `Backend::all_windows`        | _(none)_                          |
| **Screen**             | A physical monitor.                                                                                                                                                                                                                    | `Screen`                      | _(none)_                          |
| **Managed / floating** | A _managed_ window participates in a layout tree; a _floating_ window overlaps freely (dialogs, non-resizable/transient windows, rule-excluded apps).                                                                                  | `backend::manageable`         | —                                 |
| **Chrome**             | Everything vase paints on top of the windows: the tab bar, each stack's bar, empty-pane placeholders, the focus border, the switcher and picker lists.                                                                                 | `overlay::Overlays`           | tmux status line                  |
| **Prefix**             | The modal chord that arms vase to receive the next keystroke as a command. Two of them: `⌥a` for tabs and panes, `⌥e` redirecting the tab keys at the focused stack.                                                                   | `KeyRouter`, `keymap::router` | tmux prefix                       |
| **Registry**           | What vase knows about each adopted window outside the layout: its app, title, pre-adoption frame, minimized state, last placement.                                                                                                     | `registry::Registry`          | —                                 | ra  |

## Architecture

- **Deliberate placement + real stacks**: new windows become tabs in the
  current screen; the user splits/stacks/moves on purpose, _not_ automatic
  space-filling BSP.
- **Pure Accessibility**, no SIP disable, no private-API injection beyond the
  read-only `_AXUIElementGetWindow` lookup. Portability and resilience over
  maximal control.
- **Tab switching = re-render + raise.** Windows of the non-current tab are
  left where they are and occluded by raising the current tab's. Off-screen
  parking is specified in an ADR but is **not currently implemented**: nothing
  in either crate moves a window off-screen to hide it.
- **Platform boundary**: a `Backend` trait the core calls instead of any OS API.
  It covers everything vase asks of the OS, including the reads that used to
  bypass it (titles, minimize/fullscreen state, workspace membership, app
  launching), so the seam is real rather than nominal. Two adapters: macOS
  (Accessibility + CoreGraphics) and Windows (Win32). Linux is deferred, and
  Wayland is the known hard case.
- **Process model**: a single daemon owns the event tap, window management, and
  the chrome; it also **owns its keybindings** (a modal prefix needs in-process
  state).
- **Config**: declarative TOML, read from the platform's own support directory.
  Currently `[[app_focus]]` hotkeys, favorites, and the palette; the keymap lives
  in `input::keymap` and is not yet user-configurable.
- **Chrome**: transparent, non-activating, always-on-top surfaces, redrawn only
  through `Overlays::sync`. The _layout_ is shared: `chrome::bar` turns tabs into
  positioned shapes and runs, taking a text-measure closure as its only platform
  input, so the powerline bar has the same proportions everywhere. Painting is
  per-platform: AppKit on macOS, Direct2D on Windows.
- **New-window landing**: a new _managed_ window becomes its own tab, unless a
  launch into a focused empty pane is pending, in which case it fills that
  pane. Unmanaged windows are left alone.
- **Layout persistence**: the model is saved as JSON in the platform's support
  directory and re-matched against the live windows at startup, by id within a
  session and by `(app, title)` then app alone after a reboot.

- **Keys**: `KeyCode` is platform-neutral (a US-layout character, or a named
  non-text key). Each backend translates its own scancodes at the event hook, so
  one keymap and one set of modal grammars serve every platform.
