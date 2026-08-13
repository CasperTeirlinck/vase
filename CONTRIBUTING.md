# Contributing Guidelines

Thank you for considering contributing to [vase](https://github.com/CasperTeirlinck/vase)!

## Did you find a bug?

- Check and make sure the bug has not already been reported in the
  [GitHub issues](https://github.com/CasperTeirlinck/vase/issues).
- If you're unable to find an existing open issue addressing the problem,
  [open a new issue](https://github.com/CasperTeirlinck/vase/issues/new). Include your OS version, the vase version,
  and the steps that reproduce it.
- If you wrote a fix for a bug, consider
  [contributing it back to the project](#do-you-want-to-contribute-a-change-fix-or-feature).

## Do you want to contribute a change, fix, or feature?

Before contributing a change, please first communicate this via a
[GitHub issue](https://github.com/CasperTeirlinck/vase/issues).

### Development Setup

You need a [Rust toolchain](https://rustup.rs) and [`just`](https://github.com/casey/just).

| Command      | What it does                                                    |
| ------------ | --------------------------------------------------------------- |
| `just build` | Release build of the `vase` binary                              |
| `just test`  | `cargo test` for the core and this host's platform crate        |
| `just check` | `cargo clippy` with warnings denied, plus `cargo fmt --check`   |
| `just app`   | Bundles `dist/vase.app` for the host arch: `open dist/vase.app` |

Accessibility and Input Monitoring grants are keyed to the app's code signature, so a rebuilt bundle needs its grants
re-approved in System Settings → Privacy & Security.

[CONTEXT.md](CONTEXT.md) defines the vocabulary the code is named after (tab, pane, stack, chrome, prefix). Read it
before naming anything new.

### Creating a Pull Request

1. Create a pull request from your feature branch, and describe what changed and why.
   Make sure the Pull Request title follows our [conventional commit specification](#conventional-commits).
2. Make sure `just check` and `just test` pass.
3. Wait for review and approval from the project owners/maintainers.
4. Once approved, and all checks have passed, the Pull Request may be merged.

### Conventional Commits

The [conventional commit specification](https://www.conventionalcommits.org/en/v1.0.0) is required for titles of Pull
Requests, and for the individual commit messages: `CHANGELOG.md` is generated from the commit history with
[git-cliff](https://git-cliff.org), which drops commits that don't follow it.

**Types**:

- _feat_: A new feature
- _fix_: A bug fix
- _docs_: Documentation-only changes
- _style_: Changes that do not affect the meaning of the code (whitespace, formatting, etc.)
- _refactor_: A code change that neither fixes a bug nor adds a feature
- _perf_: A code change that improves performance
- _test_: Adding missing or correcting existing tests
- _ci_: Changes to the GitHub Actions workflows or release pipeline
- _chore_: Auxiliary changes to the build process, tooling, dependencies, or other
- _revert_: Reverts a previous commit

A breaking change is marked with a `!` before the colon, for example `feat(core)!: drop the legacy layout format`.

**Scopes**:
The (optional) scope indicates the area of the codebase that is affected by the change. More than one scope can be
included, separated by commas.

- _core_: the platform-agnostic layout and model crate (`vase-core`)
- _macos_: the macOS backend crate (`vase-macos`)
- _windows_: the Windows backend crate (`vase-windows`)
- _docs_: README, ADRs, and other documentation
- _packaging_: the app bundle, Homebrew formula/cask, and install script

### Contributing with an AI Coding Agent

Coding agents are welcome:

- **You own any code an agent generates.** Review the code before opening a pull request: the contribution is yours, and
  review comments come to you.

Claude Code picks these rules up from [CLAUDE.md](CLAUDE.md); other agents should be pointed at that file.
