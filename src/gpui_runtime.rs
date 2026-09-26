//! Primary GPUI process entry point.

use crate::{
    assets::Assets,
    cli, config,
    document::Document,
    gpui_lifecycle::{WindowLifecycle, WindowRequest},
    theme,
};
use gpui::{App, Application, KeyBinding};
use std::{path::PathBuf, sync::Arc, time::Duration};

struct RemoteImageHttpClient {
    // GPUI runs asset loading on its own executor, not a Tokio runtime.
    // `reqwest::blocking` owns the runtime it needs, so requests remain off
    // the UI thread without panicking when an image first becomes visible.
    client: reqwest::blocking::Client,
    user_agent: gpui_http_client::http::HeaderValue,
}

impl RemoteImageHttpClient {
    fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .user_agent("mdbijou/0.0.2")
                // Markdown image hosts commonly redirect a stable URL to a
                // CDN object (for example picsum.photos -> fastly). Make this
                // behavior explicit instead of depending on reqwest defaults.
                .redirect(reqwest::redirect::Policy::limited(10))
                .timeout(Duration::from_secs(20))
                .build()
                .expect("build remote image HTTP client"),
            user_agent: gpui_http_client::http::HeaderValue::from_static("mdbijou/0.0.2"),
        }
    }
}

impl gpui_http_client::HttpClient for RemoteImageHttpClient {
    fn type_name(&self) -> &'static str {
        "mdbijou remote image client"
    }

    fn user_agent(&self) -> Option<&gpui_http_client::http::HeaderValue> {
        Some(&self.user_agent)
    }

    fn send(
        &self,
        request: gpui_http_client::Request<gpui_http_client::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        gpui_http_client::Result<gpui_http_client::Response<gpui_http_client::AsyncBody>>,
    > {
        let client = self.client.clone();
        Box::pin(async move {
            let mut outgoing = client.request(request.method().clone(), request.uri().to_string());
            // GPUI may attach headers such as Accept or Authorization. Dropping
            // them made this adapter differ from the requested HTTP operation
            // and caused some image CDNs to reject or alter their responses.
            for (name, value) in request.headers() {
                outgoing = outgoing.header(name.as_str(), value.as_bytes());
            }
            let response = outgoing.send()?.error_for_status()?;
            let status = response.status().as_u16();
            let body = response.bytes()?.to_vec();
            Ok(gpui_http_client::Response::builder()
                .status(status)
                .body(body.into())?)
        })
    }

    fn proxy(&self) -> Option<&gpui_http_client::Url> {
        None
    }
}

/// Embedded app artwork so the Dock icon is correct in dev runs (`just`),
/// where no .app bundle exists to supply one.
#[cfg(target_os = "macos")]
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/mdbijou-icon-1024.png");

#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(APP_ICON_PNG);
    let Some(image) = NSImage::initWithData(mtm.alloc(), &data) else {
        return;
    };
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
}

/// Keystroke map for the whole app. Registered once at startup and again in
/// tests, which bypass `run()`.
pub(crate) fn bind_app_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("cmd-e", crate::gpui_app::ToggleView, None),
        KeyBinding::new("cmd-s", crate::gpui_app::SaveDocument, None),
        KeyBinding::new("cmd-r", crate::gpui_app::ReloadDocument, None),
        KeyBinding::new("cmd-o", crate::gpui_app::OpenDocument, None),
        KeyBinding::new("cmd-,", crate::gpui_app::ToggleSettings, None),
        KeyBinding::new("escape", crate::gpui_app::CloseSettings, None),
        KeyBinding::new("cmd-t", crate::gpui_app::ToggleToc, None),
        KeyBinding::new("cmd-+", crate::gpui_app::IncreasePreviewFont, None),
        KeyBinding::new("cmd--", crate::gpui_app::DecreasePreviewFont, None),
    ]);
}

pub fn run() {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(cli::CliAction::Help) => return print_help(),
        Ok(cli::CliAction::Version) => {
            println!("mdbijou {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Ok(cli::CliAction::ListThemes) => {
            for (id, name, _) in theme::builtin_ids() {
                println!("{id:<14} {name}");
            }
            return;
        }
        Ok(cli::CliAction::Run(options)) => options,
        Err(error) => {
            eprintln!("{error}\nTry --help");
            return;
        }
    };
    let path = match cli::validate_path(options.path.clone()) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };
    let mut cfg = config::load();
    cli::apply_to_config(&options, &mut cfg);
    let document = document_from_path(path);
    let (request_tx, request_rx) = futures::channel::mpsc::unbounded();

    let application = Application::new()
        .with_assets(Assets)
        .with_http_client(Arc::new(RemoteImageHttpClient::new()));
    let reopen_tx = request_tx.clone();
    application.on_reopen(move |_| {
        let _ = reopen_tx.unbounded_send(WindowRequest::Reopen);
    });
    application.on_open_urls(move |urls| {
        for url in urls {
            let Ok(url) = url::Url::parse(&url) else {
                continue;
            };
            let Ok(path) = url.to_file_path() else {
                continue;
            };
            let _ = request_tx.unbounded_send(WindowRequest::OpenFile(path));
        }
    });
    application.run(move |cx: &mut App| {
        #[cfg(target_os = "macos")]
        set_dock_icon();
        gpui_component::init(cx);
        bind_app_keys(cx);
        match WindowLifecycle::new(document, cfg, cx) {
            Ok(lifecycle) => lifecycle.listen(request_rx, cx),
            Err(error) => {
                eprintln!("could not open document window: {error}");
                cx.quit();
            }
        }
    });
}

fn document_from_path(path: Option<PathBuf>) -> Document {
    let text = path
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    path.map(|path| Document::with_path(path, text.clone()))
        .unwrap_or_else(|| Document::new(text))
}

fn print_help() {
    let themes = theme::builtin_ids()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>()
        .join("|");
    println!(
        "mdbijou {} — native Markdown and MDX reader + editor\n\nUSAGE:\n  mdbijou [OPTIONS] [FILE.md|FILE.mdx]\n\nOPTIONS:\n  --edit\n  --theme <id> ({themes})\n  --list-themes\n  --help\n  --version",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::AsyncReadExt;
    use gpui_http_client::HttpClient;

    #[test]
    #[ignore = "requires external network access"]
    fn follows_redirecting_remote_image_urls() {
        let client = RemoteImageHttpClient::new();
        let mut response = futures::executor::block_on(client.get(
            "https://picsum.photos/400/200",
            ().into(),
            true,
        ))
        .expect("picsum response");
        let mut body = Vec::new();
        futures::executor::block_on(response.body_mut().read_to_end(&mut body))
            .expect("read image body");

        assert!(response.status().is_success());
        assert!(body.starts_with(&[0xff, 0xd8, 0xff]), "expected JPEG bytes");
    }
}
