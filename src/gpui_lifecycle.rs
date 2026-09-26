//! Process-owned window lifecycle for Dock and Finder requests.

use crate::{config, document::Document, gpui_app::GpuiMdbijouApp};
use futures::{channel::mpsc::UnboundedReceiver, StreamExt};
use gpui::{
    px, size, AnyWindowHandle, App, AppContext, Bounds, WindowBounds, WindowHandle, WindowOptions,
};
use gpui_component::{Root, TitleBar};
use std::path::PathBuf;

pub(crate) enum WindowRequest {
    Reopen,
    OpenFile(PathBuf),
}

pub(crate) struct WindowLifecycle {
    // Track the document window specifically: a Settings window is not a
    // replacement, and its view may keep the closed document entity alive.
    main_window: WindowHandle<Root>,
}

impl WindowLifecycle {
    pub(crate) fn new(
        document: Document,
        cfg: config::Config,
        cx: &mut App,
    ) -> Result<Self, String> {
        let main_window = Self::open_window(document, cfg, cx)?;
        cx.activate(true);
        Ok(Self { main_window })
    }

    fn open_window(
        document: Document,
        cfg: config::Config,
        cx: &mut App,
    ) -> Result<WindowHandle<Root>, String> {
        let bounds = Bounds::centered(None, size(px(1024.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| GpuiMdbijouApp::new(document, cfg, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .map_err(|error| error.to_string())
    }

    fn handle(&mut self, request: WindowRequest, cx: &mut App) -> Result<(), String> {
        if self.main_window.read(cx).is_err() {
            self.main_window = Self::open_window(Document::new(String::new()), config::load(), cx)?;
        }
        let view = self
            .main_window
            .read(cx)
            .map_err(|error| error.to_string())?
            .view()
            .clone()
            .downcast::<GpuiMdbijouApp>()
            .map_err(|_| "document window has an unexpected view".to_string())?;
        // Release the Root borrow before updating its document/editor view.
        AnyWindowHandle::from(self.main_window)
            .update(cx, |_, window, cx| {
                if let WindowRequest::OpenFile(path) = request {
                    // Use the existing unsaved-changes guard for an open view.
                    view.update(cx, |view, cx| view.request_open(path, window, cx));
                }
                window.activate_window();
                cx.activate(true);
            })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn listen(mut self, mut requests: UnboundedReceiver<WindowRequest>, cx: &mut App) {
        // The receiver outlives every window. The async channel wakes the UI
        // executor even with no windows or pending frames, and queues native
        // launch-time events until component initialization has completed.
        cx.spawn(async move |cx| {
            while let Some(request) = requests.next().await {
                match cx.update(|cx| self.handle(request, cx)) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!("could not open document window: {error}"),
                    Err(_) => break, // The application has exited.
                }
            }
        })
        .detach();
    }
}
