use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
use objc2::{define_class, msg_send, sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAlert, NSApplication, NSColor, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{NSData, NSSize, NSString};

use crate::overlay::vase_mark_bezier;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and StatusHandler holds no Rust Drop state.
    #[unsafe(super(NSObject))]
    #[name = "VaseStatusHandler"]
    struct StatusHandler;

    impl StatusHandler {
        // Native About panel, with the version and tagline supplied so it is correct even when run unbundled.
        #[unsafe(method(about:))]
        fn about(&self, _sender: Option<&AnyObject>) {
            show_about();
        }

        #[unsafe(method(newTab:))]
        fn new_tab(&self, _sender: Option<&AnyObject>) {
            crate::request_new_tab();
        }

        #[unsafe(method(reloadConfig:))]
        fn reload_config(&self, _sender: Option<&AnyObject>) {
            crate::request_reload_config();
        }

        // Re-place every window onto its layout, recovering from a manual move or a monitor hotplug.
        #[unsafe(method(resync:))]
        fn resync(&self, _sender: Option<&AnyObject>) {
            crate::request_resync();
        }

        // Open the config file in the user's default text editor.
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            if let Some(path) = crate::paths::ensure_config() {
                let _ = std::process::Command::new("open").arg("-t").arg(path).spawn();
            }
        }

        // Ask the run loop to exit; the main loop then restores every window.
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            crate::request_quit();
        }
    }

    unsafe impl NSObjectProtocol for StatusHandler {}
);

impl StatusHandler {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Show an About dialog with the vase icon, version, and a one-line tagline.
fn show_about() {
    let mtm = MainThreadMarker::new().unwrap();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("vase"));
    alert.setInformativeText(&NSString::from_str(&format!("version {}\n\nA keyboard-driven manual tiling window manager.", env!("CARGO_PKG_VERSION"))));
    if let Some(icon) = vase_app_image() {
        unsafe { alert.setIcon(Some(&icon)) };
    }
    // Accessory apps aren't active, so the dialog would open behind other windows; bring vase forward first.
    NSApplication::sharedApplication(mtm).activate();
    alert.runModal();
}

/// The full-color vase app icon, embedded so it resolves whether or not the app runs from its bundle.
fn vase_app_image() -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(include_bytes!("../../assets/vase.icns"));
    NSImage::initWithData(NSImage::alloc(), &data)
}

/// The vase silhouette as a template `NSImage` (macOS tints it for the menu bar).
// lockFocus is deprecated for resolution independence, but it's fine for a tiny solid template icon; switch to imageWithSize:flipped:drawingHandler: (needs block2) if it reads soft on Retina.
#[allow(deprecated)]
fn vase_template_image() -> Retained<NSImage> {
    let h = 16.0;
    let w = h * (677.0 / 744.0);
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w, h));
    image.lockFocus();
    NSColor::blackColor().set(); // template: only the alpha channel matters
    vase_mark_bezier(0.0, 0.0, w, h).fill();
    image.unlockFocus();
    image.setTemplate(true);
    image
}

/// Add the vase status item to the menu bar; the returned `NSStatusItem` must be kept alive for the program's lifetime.
pub fn install(mtm: MainThreadMarker) -> Retained<NSStatusItem> {
    let handler = StatusHandler::new();
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        button.setImage(Some(&vase_template_image()));
    }

    let menu = NSMenu::new(mtm);
    // Don't let AppKit's menu-validation auto-disable our items (this app has no key window / responder chain to validate against); enable them explicitly.
    menu.setAutoenablesItems(false);
    let add = |title: &str, action: Sel, key: &str| {
        let it = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), Some(action), &NSString::from_str(key)) };
        unsafe { it.setTarget(Some(&*handler)) };
        it.setEnabled(true);
        menu.addItem(&it);
    };
    add("About vase", sel!(about:), "");
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add("New tab", sel!(newTab:), "");
    add("Resync windows", sel!(resync:), "");
    add("Reload config", sel!(reloadConfig:), "");
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add("Settings…", sel!(openSettings:), ",");
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add("Quit vase", sel!(quit:), "q");
    item.setMenu(Some(&menu));
    // The menu item's target is a weak reference, so keep the handler alive for the program's lifetime.
    std::mem::forget(handler);
    item
}
