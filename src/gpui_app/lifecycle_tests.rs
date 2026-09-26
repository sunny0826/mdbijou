use super::*;
use crate::gpui_lifecycle::{WindowLifecycle, WindowRequest};
use futures::channel::mpsc::{self, UnboundedReceiver};
use gpui::{AnyWindowHandle, TestAppContext};

fn start(cx: &mut TestAppContext, requests: UnboundedReceiver<WindowRequest>) -> AnyWindowHandle {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_runtime::bind_app_keys(cx);
        let document = Document::with_path(PathBuf::from("before.md"), "# Before".into());
        let cfg = Config {
            auto_save: false,
            ..Config::default()
        };
        WindowLifecycle::new(document, cfg, cx)
            .expect("initial window")
            .listen(requests, cx);
        cx.windows()[0]
    })
}

fn view(cx: &TestAppContext, window: AnyWindowHandle) -> Entity<GpuiMdbijouApp> {
    cx.read(|cx| {
        window
            .downcast::<Root>()
            .expect("Root window")
            .read(cx)
            .expect("open window")
            .view()
            .clone()
            .downcast::<GpuiMdbijouApp>()
            .expect("document view")
    })
}

fn close(cx: &mut TestAppContext, window: AnyWindowHandle) {
    // GPUI 0.2.2's simulate_close only invokes an optional should-close
    // handler; remove_window is what its native on-close callback performs.
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

fn file_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("AGENTS.md")
}

#[gpui::test]
async fn changed_editor_text_is_still_marked_unsaved(cx: &mut TestAppContext) {
    let (_sender, requests) = mpsc::unbounded();
    let window = start(cx, requests);
    let original = view(cx, window);
    window
        .update(cx, |_, window, cx| {
            original.update(cx, |view, cx| {
                view.editor.update(cx, |editor, cx| {
                    editor.set_value("changed editor text", window, cx);
                });
            });
        })
        .unwrap();
    cx.run_until_parked();

    original.update(cx, |view, _| {
        assert_eq!(view.document.text, "changed editor text");
        assert!(view.document.dirty);
    });
}

#[gpui::test]
async fn dock_reopens_after_repeated_last_window_closes(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let mut window = start(cx, requests);
    cx.run_until_parked();

    for _ in 0..3 {
        close(cx, window);
        assert!(cx.windows().is_empty());
        sender.unbounded_send(WindowRequest::Reopen).unwrap();
        cx.run_until_parked();

        let windows = cx.windows();
        assert_eq!(windows.len(), 1);
        assert!(windows[0] != window, "the closed window must be replaced");
        window = windows[0];
        view(cx, window).update(cx, |view, _| {
            assert!(view.document.path.is_none());
            assert!(view.document.text.is_empty());
        });
    }
}

#[gpui::test]
async fn repeated_reopen_preserves_existing_document_and_window(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let window = start(cx, requests);
    let original = view(cx, window);
    original.update(cx, |view, _| {
        view.document.text = "unsaved work".into();
        view.document.dirty = true;
    });

    for _ in 0..3 {
        sender.unbounded_send(WindowRequest::Reopen).unwrap();
    }
    cx.run_until_parked();

    assert!(cx.windows() == vec![window]);
    original.update(cx, |view, _| {
        assert_eq!(view.document.text, "unsaved work");
        assert!(view.document.dirty);
    });
}

#[gpui::test]
async fn finder_opens_document_when_only_settings_window_remains(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let window = start(cx, requests);
    cx.run_until_parked();
    cx.simulate_keystrokes(window, "cmd-,");
    cx.run_until_parked();
    let settings = cx.windows().into_iter().find(|w| *w != window).unwrap();

    close(cx, window);
    assert!(cx.windows() == vec![settings]);
    let path = file_path();
    sender
        .unbounded_send(WindowRequest::OpenFile(path.clone()))
        .unwrap();
    cx.run_until_parked();

    let windows = cx.windows();
    assert_eq!(windows.len(), 2);
    assert!(windows.contains(&settings));
    let reopened = windows.into_iter().find(|w| *w != settings).unwrap();
    view(cx, reopened).update(cx, |view, _| {
        assert_eq!(view.document.path.as_ref(), Some(&path));
    });
}

#[gpui::test]
async fn finder_request_recreates_window_without_a_render_poll(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let window = start(cx, requests);
    close(cx, window);

    let path = file_path();
    sender
        .unbounded_send(WindowRequest::OpenFile(path.clone()))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(cx.windows().len(), 1);
    view(cx, cx.windows()[0]).update(cx, |view, _| {
        assert_eq!(view.document.path.as_ref(), Some(&path));
        assert_eq!(view.document.text, std::fs::read_to_string(&path).unwrap());
        assert!(
            !view.document.dirty,
            "opening a file must not mark it edited"
        );
    });
}

#[gpui::test]
async fn finder_request_queued_before_startup_opens_in_initial_window(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let path = file_path();
    sender
        .unbounded_send(WindowRequest::OpenFile(path.clone()))
        .unwrap();
    sender.unbounded_send(WindowRequest::Reopen).unwrap();
    let window = start(cx, requests);
    cx.run_until_parked();

    assert!(cx.windows() == vec![window]);
    view(cx, window).update(cx, |view, _| {
        assert_eq!(view.document.path.as_ref(), Some(&path));
        assert!(!view.document.dirty, "a launch-time file must start clean");
    });
}

#[gpui::test]
async fn finder_request_keeps_unsaved_changes_guard(cx: &mut TestAppContext) {
    let (sender, requests) = mpsc::unbounded();
    let window = start(cx, requests);
    let original = view(cx, window);
    original.update(cx, |view, _| {
        view.document.text = "unsaved work".into();
        view.document.dirty = true;
    });

    let path = file_path();
    sender
        .unbounded_send(WindowRequest::OpenFile(path.clone()))
        .unwrap();
    cx.run_until_parked();

    assert!(cx.windows() == vec![window]);
    original.update(cx, |view, _| {
        assert_eq!(view.document.text, "unsaved work");
        assert!(view.document.dirty);
        assert_eq!(view.pending_open.as_ref(), Some(&path));
    });
}
