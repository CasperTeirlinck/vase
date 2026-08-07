set shell := ["bash", "-uc"]

bin := "target/release/vase"
app := "dist/vase.app"
# App version = the vase-macos crate version.
version := `cd vase-macos && cargo read-manifest | jq -r .version`

default:
    @just --list

# --- build & checks ---

build:
    cargo build --release --bin vase

test:
    cargo test --workspace

check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check

# --- app bundle ---

# Assemble dist/vase.app around an already-built binary at `binpath`.
_bundle binpath ver:
    rm -rf "{{app}}"
    mkdir -p "{{app}}/Contents/MacOS" "{{app}}/Contents/Resources"
    cp "{{binpath}}" "{{app}}/Contents/MacOS/vase"
    chmod +x "{{app}}/Contents/MacOS/vase"
    cp assets/vase.icns "{{app}}/Contents/Resources/vase.icns"
    sed "s/__VERSION__/{{ver}}/g" assets/Info.plist > "{{app}}/Contents/Info.plist"

# Build for the host arch and bundle it. Try it locally: `just app && open dist/vase.app`
app ver=version: build
    @just _bundle {{bin}} {{ver}}
    @echo "built {{app}} (v{{ver}})"

# Universal (arm64 + x86_64) bundle, zipped for a GitHub release. Needs both rust
# targets (added here). This is what the release build runs.
release ver=version:
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    cargo build --release --bin vase --target aarch64-apple-darwin
    cargo build --release --bin vase --target x86_64-apple-darwin
    mkdir -p dist
    lipo -create -output dist/vase-universal \
        target/aarch64-apple-darwin/release/vase \
        target/x86_64-apple-darwin/release/vase
    @just _bundle dist/vase-universal {{ver}}
    rm -f dist/vase-universal
    ditto -c -k --keepParent "{{app}}" "dist/vase-v{{ver}}-macos.zip"
    @echo "packaged dist/vase-v{{ver}}-macos.zip"

# --- versioning (needs cargo-edit: `cargo install cargo-edit`) ---

bump-version-patch:
    @cargo set-version --workspace --bump patch

bump-version-minor:
    @cargo set-version --workspace --bump minor

bump-version-major:
    @cargo set-version --workspace --bump major

# --- changelog (needs git-cliff) ---

# Write the full CHANGELOG.md, treating HEAD as the given version.
changelog ver=version:
    git-cliff --tag v{{ver}} --tag-pattern 'v.*' --config cliff.toml --output CHANGELOG.md

# Print just this version's notes (for the GitHub release body).
changelog-latest ver=version:
    @git-cliff --unreleased --tag v{{ver}} --tag-pattern 'v.*' --strip all --config cliff.toml -o - | tail -n +2
