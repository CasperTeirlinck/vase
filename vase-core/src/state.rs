//! Persist the layout as JSON, and rebuild it against the live windows at startup. Each window's
//! `(app, title)` is stored alongside so the layout can be re-matched after a reboot, when the OS
//! has reassigned window ids. The platform crate supplies the path.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::geometry::Rect;
use crate::model::Model;
use crate::tree::WindowId;
use serde::{Deserialize, Serialize};

/// One managed window's stable identity at save time.
pub type WindowIdentity = (WindowId, String, String); // (id, app, title)

/// A window that exists right now, and the screen its frame sits on.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveWindow {
    pub id: WindowId,
    pub app: String,
    pub title: String,
    pub screen: usize,
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    model: Model,
    windows: Vec<WindowIdentity>,
}

pub fn load(path: &Path) -> Option<(Model, Vec<WindowIdentity>)> {
    let data = std::fs::read_to_string(path).ok()?;
    let p: Persisted = serde_json::from_str(&data).ok()?;
    Some((p.model, p.windows))
}

/// Best effort: write errors are ignored.
pub fn save(path: &Path, model: &Model, windows: &[WindowIdentity]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let p = Persisted { model: model.clone(), windows: windows.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&p) {
        let _ = std::fs::write(path, json);
    }
}

/// The startup layout: a saved one re-matched onto `live`, or one fresh tab per live window.
pub fn restore(saved: Option<(Model, Vec<WindowIdentity>)>, live: &[LiveWindow], screens: &[Rect]) -> Model {
    let Some((mut model, identities)) = saved else {
        let pairs: Vec<(WindowId, usize)> = live.iter().map(|w| (w.id, w.screen)).collect();
        return Model::adopt(screens, &pairs);
    };
    model.reconfigure(screens);
    let map = match_windows(&model.all_windows(), &identities, live);
    model.remap_windows(&map);
    // Live windows no saved tab claimed become new tabs.
    let claimed: HashSet<WindowId> = map.values().copied().collect();
    for w in live {
        if !claimed.contains(&w.id) {
            model.add_window(w.id, w.screen);
        }
    }
    model
}

/// Pair each saved window id with the live window that inherits it.
///
/// Window ids are only stable within a login session, so matching runs in three passes, each claiming a live window exclusively: the saved id if it is still live (same session),
/// then the saved `(app, title)`, then the app alone (after a reboot, where the title has usually moved on). A saved window with no match is left out, and the caller prunes it.
fn match_windows(saved: &[WindowId], identities: &[WindowIdentity], live: &[LiveWindow]) -> HashMap<WindowId, WindowId> {
    let live_ids: HashSet<WindowId> = live.iter().map(|w| w.id).collect();
    let idmap: HashMap<WindowId, (&str, &str)> = identities.iter().map(|(id, a, t)| (*id, (a.as_str(), t.as_str()))).collect();

    let mut map = HashMap::new();
    let mut claimed: HashSet<WindowId> = HashSet::new();
    for old in saved {
        if live_ids.contains(old) {
            map.insert(*old, *old);
            claimed.insert(*old);
        }
    }
    for old in saved {
        if map.contains_key(old) {
            continue;
        }
        let Some((app, title)) = idmap.get(old) else { continue };
        let pick = live
            .iter()
            .find(|w| !claimed.contains(&w.id) && w.app.eq_ignore_ascii_case(app) && w.title == *title)
            .or_else(|| live.iter().find(|w| !claimed.contains(&w.id) && w.app.eq_ignore_ascii_case(app)))
            .map(|w| w.id);
        if let Some(new) = pick {
            map.insert(*old, new);
            claimed.insert(new);
        }
    }
    map
}
