//! macOS-native window chrome: extend the content view underneath the title bar,
//! make the title bar transparent and hide its title, so the egui toolbar sits
//! directly in the traffic-light (red/yellow/green) bar.

use eframe::CreationContext;

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
