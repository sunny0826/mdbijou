//! macOS-native window chrome: extend the content view underneath the title bar,
//! make the title bar transparent and hide its title, so the egui toolbar sits
//! directly in the traffic-light (red/yellow/green) bar.
//!
//! Also owns the macOS "open documents" Apple-event hook (Finder double-click,
//! `open file.md`, Dock drop) that forwards file paths into the egui app.

use eframe::CreationContext;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Install the macOS file-open hook and forward received file paths through
/// `tx`. No-op on non-macOS platforms.
///
/// Must be called on the main thread before `eframe::run_native`.
pub fn install_open_file_handler(tx: Sender<PathBuf>) {
    #[cfg(target_os = "macos")]
    install_ae_open_handler(tx);

    #[cfg(not(target_os = "macos"))]
    drop(tx);
}

#[cfg(target_os = "macos")]
fn install_ae_open_handler(tx: Sender<PathBuf>) {
    use objc2::rc::Retained;
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
    use objc2_foundation::NSNotificationCenter;
    use open_files_hook::OpenFilesHook;

    *OPEN_TX.lock().unwrap() = Some(tx);

    // Observe NSApplicationWillFinishLaunching and inject
    // `application:openFiles:` into the app delegate class as soon as AppKit
    // starts launching — launch-time open requests (Finder double-click) are
    // delivered before `applicationDidFinishLaunching:`, which is the earliest
    // point eframe runs user code and would be too late.
    let hook: Retained<OpenFilesHook> = unsafe { msg_send![OpenFilesHook::class(), new] };
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: `hook` is a valid NSObject implementing the selector.
    unsafe {
        center.addObserver_selector_name_object(
            &hook,
            objc2::sel!(onWillFinishLaunching:),
            Some(NSApplicationWillFinishLaunchingNotification),
            None,
        );
    }
    // The observer lives for the whole app lifetime; leak it deliberately.
    std::mem::forget(hook);

    // Fallback for non-launch paths: try injecting immediately (succeeds when
    // the NSApplication delegate already exists).
    inject_open_files_method();
}

/// Forward a received file path to the app.
#[cfg(target_os = "macos")]
fn forward_path(path: PathBuf) {
    log::debug!("open-documents event: {}", path.display());
    if let Ok(guard) = OPEN_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(path);
        }
    }
}

/// Inject `application:openFiles:` into the app delegate's class.
///
/// winit's NSApplication delegate does not implement document opening, so
/// AppKit falls back to the "cannot open files" error panel. Adding the
/// method at runtime routes Finder/`open`/Dock file-open requests to us.
/// Idempotent: returns without effect when the method already exists.
#[cfg(target_os = "macos")]
fn inject_open_files_method() {
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSArray, NSString};

    unsafe extern "C" fn open_files(
        _this: &AnyObject,
        _sel: Sel,
        _app: &AnyObject,
        files: &NSArray<NSString>,
    ) {
        for path in files.iter() {
            forward_path(PathBuf::from(path.to_string()));
        }
    }

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else {
        return;
    };
    // The delegate's dynamic class (winit's application delegate).
    let class: &AnyClass = unsafe { objc2::msg_send![&delegate, class] };
    // SAFETY: `open_files` matches the `v@:@@` encoding; adding a new method
    // (winit's delegate does not define it) does not disturb existing state.
    unsafe {
        objc2::ffi::class_addMethod(
            (class as *const AnyClass).cast_mut(),
            objc2::sel!(application:openFiles:),
            std::mem::transmute::<unsafe extern "C" fn(_, _, _, _), Imp>(open_files),
            c"v@:@@".as_ptr(),
        );
    }
}

#[cfg(target_os = "macos")]
mod open_files_hook {
    use super::inject_open_files_method;
    use objc2::define_class;
    use objc2::runtime::NSObject;
    use objc2_foundation::NSNotification;

    // Observer that injects `application:openFiles:` as early as possible:
    // AppKit may deliver launch-time open requests *before*
    // `applicationDidFinishLaunching:`, which is the earliest point eframe
    // runs user code — too late.
    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "MdbijouOpenFilesHook"]
        pub struct OpenFilesHook;

        impl OpenFilesHook {
            #[unsafe(method(onWillFinishLaunching:))]
            fn on_will_finish_launching(&self, _notification: &NSNotification) {
                inject_open_files_method();
            }
        }
    );
}

/// Channel endpoint stashed for the injected delegate method.
#[cfg(target_os = "macos")]
static OPEN_TX: std::sync::Mutex<Option<Sender<PathBuf>>> = std::sync::Mutex::new(None);

/// Configure the macOS window so the app's own top bar replaces the native
/// title bar. No-op on non-macOS platforms.
pub fn configure_title_bar(cc: &CreationContext<'_>) {
    #[cfg(target_os = "macos")]
    apply_macos_chrome(cc);

    #[cfg(not(target_os = "macos"))]
    let _ = cc;
}

/// Width (in points) reserved on the left of the top bar for the native
/// traffic-light buttons, so our toolbar never overlaps them.
pub const fn traffic_light_pad() -> f32 {
    #[cfg(target_os = "macos")]
    {
        76.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        12.0
    }
}

/// Vertical center of the traffic-light buttons, in points from the top of
/// the window. Measured from AppKit at startup; falls back to the standard
/// 14pt (half of the 28pt title bar) when measurement is unavailable.
pub fn traffic_light_center() -> f32 {
    measured_light_center().unwrap_or(14.0)
}

#[cfg(target_os = "macos")]
fn measured_light_center() -> Option<f32> {
    LIGHT_CENTER.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(target_os = "macos"))]
fn measured_light_center() -> Option<f32> {
    None
}

#[cfg(target_os = "macos")]
static LIGHT_CENTER: LightCenter = LightCenter::new();

/// Measured traffic-light center, stored as f32 bits in an atomic.
#[cfg(target_os = "macos")]
struct LightCenter(std::sync::atomic::AtomicU32);

#[cfg(target_os = "macos")]
impl LightCenter {
    const fn new() -> Self {
        Self(std::sync::atomic::AtomicU32::new(0))
    }

    fn store(&self, value: f32) {
        self.0
            .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    fn load(&self, ordering: std::sync::atomic::Ordering) -> Option<f32> {
        let bits = self.0.load(ordering);
        if bits == 0 {
            return None;
        }
        let value = f32::from_bits(bits);
        // Sanity-clamp: a plausible traffic-light center sits within the
        // first ~40pt of the window.
        (value > 6.0 && value < 40.0).then_some(value)
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_chrome(cc: &CreationContext<'_>) {
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSView, NSWindowButton, NSWindowStyleMask, NSWindowTitleVisibility};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // We must run on the main thread to talk to AppKit; eframe calls us from the
    // app-creation path which is already on the main thread.
    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    // The raw handle exposes the window's content `NSView`; walk from it to the
    // owning `NSWindow`.
    // SAFETY: the pointer is valid for the window's lifetime and we are on the
    // main thread.
    let view_ptr = handle.ns_view.as_ptr();
    let Some(view) = (unsafe { Retained::<NSView>::retain(view_ptr.cast()) }) else {
        return;
    };
    let Some(window) = view.window() else {
        return;
    };

    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    let mut mask = window.styleMask();
    mask.insert(NSWindowStyleMask::FullSizeContentView);
    window.setStyleMask(mask);

    // Measure the traffic-light center so our toolbar can align with it and
    // leave equal space above and below the buttons.
    if let Some(button) = window.standardWindowButton(NSWindowButton::CloseButton) {
        // SAFETY: the button is owned by the window and we are on the main
        // thread, so its superview (the titlebar) is valid.
        let titlebar = unsafe { button.superview() };
        if let Some(titlebar) = titlebar {
            let frame = button.frame();
            let titlebar_h = titlebar.frame().size.height as f32;
            let center = titlebar_h - (frame.origin.y as f32 + frame.size.height as f32 / 2.0);
            if center.is_finite() {
                LIGHT_CENTER.store(center);
            }
        }
    }
}
