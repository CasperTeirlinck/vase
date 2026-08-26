//! Draws the tab bar and the switcher card over whatever is on screen, so a style can be looked at
//! without running the daemon.
//!
//! `cargo run -p vase-macos --example bar_preview [style]`, where style is `native` or `powerline`.

use objc2_app_kit::NSApplication;
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};
use vase_core::chrome::bar::{Bar, BarTab};
use vase_core::chrome::theme::{set_theme, Style, Theme};
use vase_core::chrome::{bar_height, ListAt, Painter, SwitchRow};
use vase_core::geometry::Rect;
use vase_macos::{nsapp_init, AppKitPainter};

fn main() {
    let style = match std::env::args().nth(1).as_deref() {
        Some("powerline") => Style::Powerline,
        _ => Style::Native,
    };
    set_theme(Theme { style, ..Theme::DEFAULT });

    let mtm = objc2::MainThreadMarker::new().unwrap();
    nsapp_init(mtm);
    let mut painter = AppKitPainter::new(mtm);

    let tabs = vec![
        tab(1, "Ghostty", "vase — zsh"),
        tab(2, "Safari", "Adopting Liquid Glass in your AppKit app"),
        BarTab { zoomed: true, ..tab(3, "Finder", "tessera") },
        BarTab { off_workspace: true, dim: true, ..tab(4, "Mail", "Inbox") },
        BarTab { hotkey: true, badges: vec![true], ..tab(5, "Slack", "dataminded") },
    ];
    // Apps running with no window of their own, drawn as bare icons after the last tab.
    let windowless: Vec<String> = ["Notes".into(), "Music".into(), "Preview".into()].into();
    for app in tabs.iter().flat_map(|t| &t.icons).chain(&windowless) {
        painter.prewarm_icon(app);
    }
    painter.bar(&Bar { rect: Rect::new(0.0, 300.0, 1200.0, bar_height()), tabs: &tabs, apps: &windowless, selected: 1, main: true, armed: false });

    let rows: Vec<SwitchRow> = tabs.iter().map(row).collect();
    // Centered below the bar, so the two surfaces can be looked at (or screenshot) together.
    painter.list(ListAt::Centered(Rect::new(0.0, 360.0, 1200.0, 600.0)), "switch to: liq", &rows, 1);
    if std::env::args().any(|a| a == "help") {
        painter.help(&vase_core::chrome::help::layout(Rect::new(0.0, 0.0, 1600.0, 1000.0)));
    }

    // Hold the panel on screen long enough to look at (or screenshot), servicing AppKit so it draws.
    let app = NSApplication::sharedApplication(mtm);
    let mode = unsafe { NSDefaultRunLoopMode };
    let until = NSDate::dateWithTimeIntervalSinceNow(20.0);
    while until.timeIntervalSinceNow() > 0.0 {
        let deadline = NSDate::dateWithTimeIntervalSinceNow(0.05);
        if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(objc2_app_kit::NSEventMask::Any, Some(&deadline), mode, true) {
            app.sendEvent(&event);
        }
    }
}

fn row(tab: &BarTab) -> SwitchRow {
    SwitchRow {
        number: tab.number,
        prefix: if tab.number > 2 { "  └ ".into() } else { String::new() },
        icons: tab.icons.clone(),
        label: tab.label.clone(),
        dim: tab.dim,
        off_workspace: tab.off_workspace,
        favorite: tab.hotkey,
        current: tab.number == 3,
    }
}

fn tab(number: usize, app: &str, label: &str) -> BarTab {
    BarTab { icons: vec![app.into()], badges: vec![false], label: label.into(), zoomed: false, number, dim: false, off_workspace: false, hotkey: false }
}
