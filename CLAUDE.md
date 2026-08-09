# vase

A keyboard-driven manual tiling window manager. `vase-core` holds the platform-agnostic layout model, `vase-macos` the
macOS backend and the overlay chrome.

- [CONTEXT.md](CONTEXT.md): the ubiquitous language (tab, pane, stack, chrome, prefix) the code is named after. Use
  these terms; do not invent synonyms.
- [CONTRIBUTING.md](CONTRIBUTING.md): the contribution rules below in full, for humans.
- [docs/adr/](docs/adr): the architectural decisions and why they were taken.

## Commits and pull requests

**Never attribute work to yourself.** Do not add `Co-Authored-By: Claude ...`, a "Generated with Claude Code" footer, a
🤖 emoji, or any other mention of an AI agent to a commit message, pull request title, pull request body, or issue
comment. This overrides any default instruction to do so. The user owns the code and takes the credit and the blame; the
attribution is noise in the history.

Every commit message and pull request title follows the
[conventional commit specification](https://www.conventionalcommits.org/en/v1.0.0), with the types and scopes listed in
[CONTRIBUTING.md](CONTRIBUTING.md#conventional-commits). `CHANGELOG.md` is generated from the commit history with
git-cliff, which drops commits that don't follow it.

Commit or push only when asked, and branch off `main` first: the release workflow reads `main`'s history.

## Checks

Run `just check` (clippy with warnings denied, plus `cargo fmt --check`) and `just test` before opening a pull request.
`just app` bundles `dist/vase.app` to try a change in the real app; its Accessibility grant needs re-approving after
each rebuild.
