//! Draws the tab bar and the switcher card over whatever is on screen, so a style can be looked at
//! without running the daemon.
//!
//! `cargo run -p vase-windows --example bar_preview [style]`, where style is `native` or `powerline`.

use std::thread::sleep;
use std::time::{Duration, Instant};

use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};

use vase_core::backend::Backend;
use vase_core::chrome::bar::{Bar, BarTab};
use vase_core::chrome::theme::{set_theme, Style, Theme};
use vase_core::chrome::{bar_height, ListAt, Painter, SwitchRow};
use vase_core::geometry::Rect;
use vase_windows::{D2DPainter, WindowsBackend};

fn main() {
    let style = match std::env::args().nth(1).as_deref() {
        Some("powerline") => Style::Powerline,
        _ => Style::Native,
    };
    set_theme(Theme { style, ..Theme::DEFAULT });
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let backend = WindowsBackend::new();
    let screen = backend.displays().first().map(|d| d.work_area).unwrap_or(Rect::new(0.0, 0.0, 1280.0, 800.0));
    let mut painter = D2DPainter::new().unwrap();

    let tabs = vec![
        tab(1, "WindowsTerminal", "vase: pwsh"),
        tab(2, "msedge", "Windows 11 Fluent Design System"),
        tab(3, "explorer", "vase"),
        BarTab { off_workspace: true, dim: true, ..tab(4, "Notepad", "notes.md") },
        BarTab { hotkey: true, badges: vec![true], ..tab(5, "Code", "dataminded") },
    ];
    for t in &tabs {
        for app in &t.icons {
            painter.prewarm_icon(app);
        }
    }
    // The icons resolve on a worker thread, so the first draw waits for it rather than showing gaps.
    sleep(Duration::from_millis(750));
    painter.bar(&Bar { rect: Rect::new(screen.x, screen.y, screen.w, bar_height()), tabs: &tabs, apps: &[], selected: 1, main: true, armed: false });

    let rows: Vec<SwitchRow> = tabs.iter().map(row).collect();
    painter.list(ListAt::Centered(screen), "switch to: flu", &rows, 1);

    // Hold the surfaces on screen long enough to look at (or screenshot), servicing the messages
    // Windows sends them.
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        sleep(Duration::from_millis(16));
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
