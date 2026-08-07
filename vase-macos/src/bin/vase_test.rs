use super::*;
use vase_core::input::{Decision, KeyRouter};
use vase_macos::daemon::clean_title;

#[test]
fn prefix_cmd_arrow_routes_to_move_pane() {
    // The one layer not covered by core tests: prefix (Alt-a) then Cmd/Ctrl-
    // arrow must route to the Move* commands (which map to MoveWindow).
    let mut r = KeyRouter::new(Key::alt(VK_A), bindings());
    assert_eq!(r.key(Key::alt(VK_A)), Decision::Consume); // arm
    let cmd = Mods { cmd: true, ..Mods::default() };
    assert_eq!(
        r.key(Key { code: VK_LEFT, mods: cmd }),
        Decision::ConsumeAndRun(InputCommand::MoveLeft)
    );
    // Ctrl-arrow also moves (Karabiner remaps physical Cmd-arrow to Ctrl).
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    assert_eq!(r.key(Key::alt(VK_A)), Decision::Consume);
    assert_eq!(
        r.key(Key { code: VK_RIGHT, mods: ctrl }),
        Decision::ConsumeAndRun(InputCommand::MoveRight)
    );
    // Letters move too (arrow-exchange-proof): Cmd-H → MoveLeft.
    assert_eq!(r.key(Key::alt(VK_A)), Decision::Consume);
    assert_eq!(
        r.key(Key { code: VK_H, mods: cmd }),
        Decision::ConsumeAndRun(InputCommand::MoveLeft)
    );
}

#[test]
fn clean_title_strips_the_redundant_app_name() {
    // App-name prefix (Activity Monitor) → keep the meaningful remainder.
    assert_eq!(clean_title("Activity Monitor – All Processes", "Activity Monitor"), "All Processes");
    // App name mid/suffix (Chrome/Brave) → keep the part before it.
    assert_eq!(clean_title("Incidents - PagerDuty - Google Chrome – Person 1", "Google Chrome"), "Incidents - PagerDuty");
    // App's first word matches when the full name doesn't (Brave Browser → Brave).
    assert_eq!(clean_title("Workflow runs · checkup - Brave", "Brave Browser"), "Workflow runs · checkup");
    // Title that is only the app name → nothing left.
    assert_eq!(clean_title("Ghostty", "Ghostty"), "");
    // Unrelated title is left as-is.
    assert_eq!(clean_title("notes.md — draft", "Obsidian"), "notes.md — draft");
}
