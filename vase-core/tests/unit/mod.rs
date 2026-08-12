//! In-crate unit tests, mirroring the `src/` module structure. Wired into the
//! crate by a single `#[cfg(test)] mod tests;` in `lib.rs` so they keep
//! `pub(crate)` access while living outside `src/`.

mod backend;
mod chrome;
mod focus;
mod geometry;
mod input;
mod model;
mod registry;
mod state;
mod tree;
