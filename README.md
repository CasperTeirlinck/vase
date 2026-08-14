<p align="center">
  <img src="docs/branding/vase-wordmark.svg" alt="vase" width="300">
</p>

<p align="center">A cross-platform manual tiling window manager.</p>

<p align="center">
  <a href="https://github.com/CasperTeirlinck/vase/releases/latest"><img src="https://img.shields.io/github/v/release/CasperTeirlinck/vase?style=flat-square&color=blue" alt="Latest release"></a>
  <a href="https://github.com/CasperTeirlinck/vase/releases"><img src="https://img.shields.io/github/downloads/CasperTeirlinck/vase/total?style=flat-square&color=blue" alt="Downloads"></a>
  <a href="https://github.com/CasperTeirlinck/homebrew-vase"><img src="https://img.shields.io/badge/homebrew-tap-fbb040?style=flat-square&logo=homebrew&logoColor=white" alt="Homebrew tap"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey?style=flat-square" alt="Platforms: macOS and Windows">
  <img src="https://img.shields.io/badge/rust-2021-b7410e?style=flat-square&logo=rust&logoColor=white" alt="Rust 2021 edition">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/CasperTeirlinck/vase?style=flat-square&color=blue" alt="License: GPL-3.0"></a>
</p>

---

**vase** is a cross-platform, keyboard-driven manual tiling window manager inspired by tmux.
It arranges your windows into **tabs** and **panes** and drives them from a single prefix key:
create windows, split into panes, and group panes into a tabbed **stack**, all from the keyboard.

The layout engine is platform-agnostic. **macOS and Windows** are available today, Linux is planned.
On macOS it runs on plain Accessibility (no SIP to disable, no injected code, no scripting additions);
on Windows, on the Win32 API. See [Platforms](#platforms) below.

## Platforms

The layout core (`vase-core`) is platform-agnostic; each OS gets its own thin
backend crate. macOS and Windows are available today; Linux is planned.

| Platform | Backend        | Status      |
| -------- | -------------- | ----------- |
| macOS    | `vase-macos`   | Available   |
| Windows  | `vase-windows` | Available   |
| Linux    | `vase-linux`   | Coming soon |

## Features

- **tmux model**: windows as tabs, splits into panes, and nested stacks
  (tabbed containers) inside a pane, up to two levels deep.
- **Multi-monitor**: tabs live per display; send a tab to another monitor in
  one keystroke. Layout survives quit/restart and monitor hotplug.
- **A powerline-style tab bar** no wasted space, with app icons, `prefix-N` numbers for quick switching, dimmed off-monitor
  tabs, and notification-badge dots.
- **Window switcher and pane picker**: searchable trees that mirror your layout hierarchy.

## Install

### macOS

Packaged builds ship for macOS; pick whichever fits you.

**Homebrew cask** (recommended):

```sh
brew install --cask CasperTeirlinck/vase/vase
```

**Homebrew formula**:

```sh
brew install CasperTeirlinck/vase/vase
```

**Install script** (downloads the latest `vase.app` into `/Applications`):

```sh
curl -fsSL https://raw.githubusercontent.com/CasperTeirlinck/vase/main/install.sh | bash
```

**Manual download**: grab `vase-vX.Y.Z-macos.zip` from the
[latest release](https://github.com/CasperTeirlinck/vase/releases/latest),
unzip it, and move `vase.app` to Applications.

**Build from source** (needs a Rust toolchain and [`just`](https://github.com/casey/just)): `just app` produces `dist/vase.app`,
or `cargo build --release` if you just want the binary.

On first launch, grant **Accessibility** and **Input Monitoring** in System
Settings → Privacy & Security. vase runs as a menu-bar accessory (no Dock icon),
adopts your open windows as tabs on start, and **restores every window to its
original frame on exit**.

### Windows

Build from source (needs a Rust toolchain):

```sh
cargo build --release -p vase-windows
```

Run `target\release\vase.exe`. No special permissions are required. vase runs in
the system tray.

## Keys

The prefix is a tmux-style `⌥a` (Option-A on macOS, **Alt+A** on Windows): press it,
release, then a command key. A dot at the right of the tab bar lights up in the accent
color while the prefix is armed. In the tables below `⌥`/`⌘`/`⌃`/`⇧` are
Option/Command/Control/Shift on macOS and Alt/Win/Ctrl/Shift on Windows.

### Tabs

| Key               | Action                                      |
| ----------------- | ------------------------------------------- |
| `⌥a c`            | new tab                                     |
| `⌥a .` / `⌥a ,`   | next / previous tab                         |
| `⌥a 1`-`9`        | jump to tab _n_                             |
| `⌥a t`            | rename the tab                              |
| `⌥a ⇧,` / `⌥a ⇧.` | move the tab left / right                   |
| `⌥a {` / `⌥a }`   | send the tab to the previous / next monitor |
| `⌥a w`            | window switcher                             |
| `⌥a l`            | jump to the last window                     |

### Panes

| Key                           | Action                                     |
| ----------------------------- | ------------------------------------------ |
| `⌥a \` / `⌥a -`               | split right / down (opens the pane picker) |
| `⌥a ← ↑ → ↓`                  | move focus                                 |
| `⌥a ⇧`+arrows / `⌥a ⇧HJKL`    | resize                                     |
| `⌥a ⌘/⌃/⌥`+`HJKL` (or arrows) | move the pane                              |
| `⌥a z`                        | zoom the focused pane                      |
| `⌥a x`                        | break the pane out into its own tab        |
| `⌥a m`                        | move the cursor to the focused pane        |

### Stacks

A stack is a tabbed container inside a pane: several windows sharing one slot,
one shown at a time, with a local powerline bar.

| Key               | Action                                  |
| ----------------- | --------------------------------------- |
| `⌥a s`            | make the pane a stack / add a tab to it |
| `⌥a [` / `⌥a ]`   | cycle the focused stack                 |
| `⌥e .` / `⌥e ,`   | cycle the focused stack                 |
| `⌥e 1`-`9`        | select stack item _n_                   |
| `⌥e t`            | rename the stack item                   |
| `⌥e ⇧,` / `⌥e ⇧.` | reorder the stack item                  |

### Command line

`⌥a :` opens a `:` prompt: `:q`, `:rename <name>`, `:close`, `:split`,
`:vsplit`, `:zoom`, `:tab <n>`.

### Pickers

When you open an empty pane or a new tab, a picker appears: type to search,
`j`/`k` or `1`-`9` to choose, `⏎` to move an existing window in or launch an app,
`Esc` to cancel.

## Config

vase reads a `config.toml`: `~/Library/Application Support/vase/config.toml` on
macOS, `%APPDATA%\vase\config.toml` on Windows. It holds the app-focus hotkeys:
global chords that toggle focus to an app. Press the chord to bring the app
forward, press it again to hide it.

```toml
[[app_focus]]
key = "ctrl+grave"
app = "Ghostty"
```

Chord syntax joins modifiers with `+`, for example `ctrl+grave`, `cmd+shift+k`,
or `alt+space`.

Edit it from the menu-bar / tray **Settings** item, then **Reload config** to
apply it without restarting.

## Limitations

- On macOS, with pure Accessibility a window can only be raised above another
  app's windows by fronting that app, so bringing a whole split forward can
  briefly flick focus across its panes. That's the trade for not shipping a
  scripting addition.
- A stack holds a flat list of windows; splits inside a stack aren't supported
  yet.
