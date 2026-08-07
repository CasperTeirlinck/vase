<p align="center">
  <img src="docs/branding/vase-wordmark.svg" alt="vase" width="300">
</p>

<p align="center">A cross-platform manual tiling window manager.</p>

---

vase arranges your windows into **tabs** and **panes** and drives them from a
single prefix key. It's **inspired by tmux**: the same tabs-and-panes model and
modal prefix, but for your desktop windows instead of terminals. Every window
starts as its own tab; split a tab into panes, or group panes into a tabbed
**stack**.

The layout engine is platform-agnostic. The macOS backend ships today and runs
on plain Accessibility: no SIP to disable, no injected code, no scripting
additions.

## Why

You place windows deliberately, the way you set up a tmux session; nothing
auto-rearranges behind your back. Every action is a keystroke away behind one
prefix, so your hands stay on the keyboard. And on macOS it manages windows
without weakening the system: it moves and resizes through the public
Accessibility API, adds one read-only SkyLight call for cross-display focus (the
technique yabai uses), and never slides a window off-screen (switching tabs just
restacks what's already there).

## Features

- **tmux model**: windows as tabs, splits into panes, and nested stacks
  (tabbed containers) inside a pane, up to two levels deep.
- **Two prefixes**: `⌥a` for tabs and panes, `⌥e` for the focused stack.
- **Multi-monitor**: tabs live per display; send a tab to another monitor in
  one keystroke. Layout survives quit/restart and monitor hotplug.
- **A powerline tab bar** with app icons, `prefix-N` numbers, dimmed off-monitor
  tabs, and notification-badge dots read live from the Dock.
- **Window switcher and pane picker**: searchable trees that mirror your
  layout, with index shortcuts and app launching.
- **A menu-bar item**: new tab, reload config, open settings, quit.
- **App-focus hotkeys**: a global chord that toggles focus to a chosen app.

## Install

The macOS backend is what ships today; it needs macOS (developed on 15) and a
Rust toolchain.

```sh
cargo build --release
```

Grant the binary (or the terminal you launch it from) **Accessibility** and
**Input Monitoring** in System Settings → Privacy & Security, then run it:

```sh
./target/release/vase
```

vase runs as a menu-bar accessory (no Dock icon). It adopts your open windows as
tabs on start and **restores every window to its original frame on exit**. Quit
with `⌥a q`, the menu-bar item, or the hard-release chord `Ctrl+Alt+Cmd+Esc`.

## Keys

The prefix is `⌥a` (Option-A): press it, release, then a command key, tmux-style.
A dot at the right of the tab bar turns green while the prefix is armed.

### Tabs

| Key | Action |
| --- | --- |
| `⌥a c` | new tab |
| `⌥a .` / `⌥a ,` | next / previous tab |
| `⌥a 1`-`9` | jump to tab *n* |
| `⌥a t` | rename the tab |
| `⌥a ⇧,` / `⌥a ⇧.` | move the tab left / right |
| `⌥a {` / `⌥a }` | send the tab to the previous / next monitor |
| `⌥a w` | window switcher |
| `⌥a l` | jump to the last window |

### Panes

| Key | Action |
| --- | --- |
| `⌥a \` / `⌥a -` | split right / down (opens the pane picker) |
| `⌥a ← ↑ → ↓` | move focus |
| `⌥a ⇧`+arrows / `⌥a ⇧HJKL` | resize |
| `⌥a ⌘/⌃/⌥`+`HJKL` (or arrows) | move the pane |
| `⌥a z` | zoom the focused pane |
| `⌥a x` | break the pane out into its own tab |

### Stacks

A stack is a tabbed container inside a pane: several windows sharing one slot,
one shown at a time, with a local powerline bar.

| Key | Action |
| --- | --- |
| `⌥a s` | make the pane a stack / add a tab to it |
| `⌥a [` / `⌥a ]` | cycle the focused stack |
| `⌥e .` / `⌥e ,` | cycle the focused stack |
| `⌥e 1`-`9` | select stack item *n* |
| `⌥e t` | rename the stack item |
| `⌥e ⇧,` / `⌥e ⇧.` | reorder the stack item |

### Command line

`⌥a :` opens a `:` prompt: `:q`, `:rename <name>`, `:close`, `:split`,
`:vsplit`, `:zoom`, `:tab <n>`.

### Pickers

When you open an empty pane or a new tab, a picker appears: type to search,
`j`/`k` or `1`-`9` to choose, `⏎` to move an existing window in or launch an app,
`Esc` to cancel.

## Config

`~/Library/Application Support/vase/config.json` (created on first run) holds the
app-focus hotkeys: global chords that toggle focus to an app.

```json
{
  "app_focus": [
    { "key": "ctrl+grave", "app": "Ghostty" }
  ]
}
```

Edit it from the menu bar (**Settings**), then **Reload config** to apply it
without restarting.

## How it works

Two crates:

- **`vase-core`**: the window-layout model and a pure
  `(state, command)` to `(state, effects)` reducer. No OS calls, fully
  unit-tested, and platform-agnostic, so a new platform is a new backend crate
  rather than a rewrite.
- **`vase-macos`**: the macOS backend. The Accessibility/AppKit layer, the
  overlays (tab bar, switcher, pickers, menu-bar item), the global event tap, and
  the daemon that connects them to the core.

Windows are co-located: each tab's windows tile the whole screen, and switching
tabs restacks them above the rest rather than parking anything off-screen. They
are placed through Accessibility and focused across displays with a read-only
SkyLight call. The layout is written to disk and re-matched to live windows
after a restart or a display change.

## Limitations

- Only the macOS backend exists so far. The core is portable, so other platforms
  are a backend away, but they aren't written yet.
- With pure Accessibility, a window can only be raised above another app's
  windows by fronting that app, so bringing a whole split forward can briefly
  flick focus across its panes. That's the trade for not shipping a scripting
  addition.
- A stack holds a flat list of windows; splits inside a stack aren't supported
  yet.
