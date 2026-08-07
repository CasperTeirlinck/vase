//! Persist the layout across restarts (and reboots) as JSON in Application
//! Support. Alongside the model we store each window's `(app, title)` so the
//! layout can be re-matched to live windows after a reboot, when the OS has
//! reassigned window ids.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use vase_core::model::Model;
use vase_core::tree::WindowId;

/// One managed window's stable identity at save time.
pub type WindowIdentity = (WindowId, String, String); // (id, app, title)

#[derive(Serialize, Deserialize)]
struct Persisted {
    model: Model,
    windows: Vec<WindowIdentity>,
}

/// `~/Library/Application Support/vase/state.json`.
fn state_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/vase/state.json"))
}

/// Load the saved model + window identities, or `None` if there's no (readable,
/// valid) state file.
pub fn load() -> Option<(Model, Vec<WindowIdentity>)> {
    let data = std::fs::read_to_string(state_path()?).ok()?;
    let p: Persisted = serde_json::from_str(&data).ok()?;
    Some((p.model, p.windows))
}

/// Write the model + window identities to disk (best effort; errors ignored).
pub fn save(model: &Model, windows: &[WindowIdentity]) {
    let Some(path) = state_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let p = Persisted { model: model.clone(), windows: windows.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&p) {
        let _ = std::fs::write(path, json);
    }
}
