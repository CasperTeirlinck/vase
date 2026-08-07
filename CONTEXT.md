# Vase — Ubiquitous Language

Vase is a keyboard-driven, tmux-philosophy window manager for native OS
windows. It manages windows it does not render: it can move, resize, raise,
hide, and focus them, and it draws its own overlay UI on top. It cannot embed
or reparent foreign app windows.

Canonical terms. "Window" always means a native OS window — never a tmux-style
tab. Keybindings mirror the user's tmux muscle memory, but the vocabulary below
is deliberately WM-native to avoid colliding with OS / Accessibility APIs.

| Term | Definition | tmux analog |
|---|---|---|
| **Window** | A native OS window drawn by some application. The atom vase manages. | *(none — tmux has no OS windows)* |
| **Tile** | A leaf slot in a layout tree holding exactly one window (or one stack). | pane |
| **Split** | An internal layout node dividing space horizontally or vertically into child nodes. | `C-a \` / `C-a -` |
| **Stack** | A node whose children share one rectangle; one is raised (visible), the rest hidden, with a vase-drawn overlay tab bar. | inner-tmux tabbed pane |
| **Tab** | One member of a stack. Selecting a tab raises its window(s) and hides the siblings. | a tmux *window* |
| **Workspace** | The top-level switchable container, owned by a screen. Holds one layout tree. The **workspace strip is the user's bottom bar** — one workspace ≈ one tmux *window*. Switching workspaces hides one set of windows and shows another. There is no separate session level. | a tmux *window* |
| **Managed / floating** | A *managed* window participates in the layout tree; a *floating* window overlaps freely (dialogs, non-resizable/transient windows, rule-excluded apps). | — |
| **Layout** | The tree of splits / stacks / tiles inside a workspace. | window layout |
| **Screen** | A physical monitor. | *(none)* |
| **Prefix** | The modal key that arms vase to receive the next keystroke as a command. Bound to the user's currently-free **physical Command key**; tmux keeps the **physical fn key** (→ Ctrl-a). Cmd = OS/window layer, Ctrl = terminal layer. Signal-routing (Karabiner line vs vase-owned event tap) deferred to build time. | tmux prefix |

## Architectural stance (decided)

- **Shape B + C**: deliberate placement (new windows become tabs in the current
  workspace; the user splits/stacks/moves on purpose), *not* automatic
  space-filling BSP. Plus real stacks with overlay tab bars.
- **Pure Accessibility**, no SIP disable, no private-API injection. Portability
  and resilience over maximal control.
- **Workspace switching = hide/show**, the same strategy needed on macOS and
  Windows (neither exposes reliably scriptable native virtual desktops).
- **Platform boundary**: a thin backend trait (~8 primitive operations); the
  core model calls no OS API directly. macOS first; Windows/Linux backends
  deferred (Wayland is the known hard case).
- **Hide mechanism**: off-screen parking (move hidden windows far outside any
  screen), the same primitive for workspace switches and non-selected stack
  tabs. Proven by AeroSpace on pure Accessibility; switch latency is validated
  in the real macOS backend, not a separate spike.
- **Process model**: a single daemon owns the event tap, window management, and
  overlay; it also **owns its keybindings** (a modal prefix needs in-process
  state). A thin **CLI/IPC** exposes a command vocabulary so scripts — and tmux
  edge-bindings — can drive it, completing the nvim → tmux → OS focus chain.
- **Config**: declarative **TOML**. Keybindings map to command strings drawn
  from the *same* verb set as the CLI. Modal sub-maps reproduce the prefix
  grammar; per-app rules replace tmux-style `if-shell` conditionals.
- **Overlay**: one transparent, non-activating, always-on-top window per screen
  into which vase paints all tab bars and the workspace strip. Rendered
  natively on macOS behind the backend trait; a shared cross-platform painter is
  deferred until Windows/Linux land.
- **New-window landing**: a new *managed* window joins the **focused node** as a
  stack tab (selected), leaving other tiles untouched. Overridable by
  app → workspace rules and a per-app "open floating" option. Unmanaged windows
  float.

## Known open decisions (not yet made)

- Persistence / restore of layouts across restart (the user relies on
  tmux-resurrect; GUI apps can only be re-placed or relaunched, not serialized).
- Reverse edge-forwarding (vase → a specific tmux pane on entry). Default for
  v1: vase just focuses the terminal window; tmux keeps its active pane. Full
  directional handoff is later polish.

The default command grammar is drafted in `docs/vase.example.toml` (single
Cmd prefix, distinct keys per axis, mapped from the user's tmux bindings).
