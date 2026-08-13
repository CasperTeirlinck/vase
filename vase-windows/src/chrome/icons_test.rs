use super::*;

#[test]
fn an_app_that_is_running_resolves_to_its_own_executable() {
    // The test binary is itself a process the scan has to find, under the same name the backend
    // would mint for a window of it.
    let me = std::env::current_exe().unwrap();
    assert_eq!(running_exe(me.file_stem().unwrap().to_str().unwrap()), Some(me));
}

#[test]
fn an_app_that_is_not_running_falls_through_to_the_start_menu() {
    assert_eq!(running_exe("no such process"), None);
}
