//! mdbijou — lightweight native Markdown reader + simple editor.
//!
//! Usage:
//!   mdbijou [options] [file.md]
//! Options:
//!   --edit                 open directly in the editor view
//!   --theme <id>           start with a specific theme (github-light|github-dark|sepia)
//!   --list-themes          list builtin themes and exit
//!   --help                 show help
//!   --version              show version

mod app;
mod config;
mod document;
mod editor;
mod fonts;
mod highlight;
mod images;
mod install;
mod macos;
mod mermaid;
mod render;
mod theme;

use config::View;

fn print_help() {
    println!(
        "mdbijou {} — lightweight Markdown reader + simple editor\n\
         \n\
         USAGE:\n\
         \x20 mdbijou [OPTIONS] [FILE]\n\
         \n\
         OPTIONS:\n\
         \x20 --edit            open directly in the editor view\n\
         \x20 --theme <id>      use a specific theme (github-light|github-dark|sepia)\n\
         \x20 --list-themes     list builtin themes and exit\n\
         \x20 --help            show this help\n\
         \x20 --version         show version\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn list_themes() {
    let reg = theme::ThemeRegistry::new();
    println!("Built-in themes:");
    for t in &reg.themes {
        let kind = match t.kind {
            theme::ThemeKind::Light => "light",
            theme::ThemeKind::Dark => "dark",
        };
        println!("  {:<14} {:<14} {}", t.id, t.name, kind);
    }
}

/// Embedded app icon (macOS-style squircle composed from logo.png) used as
/// the window/Dock icon. Regenerate with `just icon`.
fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/mdbijou-icon-1024.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<String> = None;
    let mut theme_override: Option<String> = None;
    let mut edit = false;
    let mut print_theme_list = false;

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("mdbijou {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--list-themes" => {
                print_theme_list = true;
            }
            "--edit" => {
                edit = true;
            }
            "--theme" => {
                i += 1;
                if i < args.len() {
                    theme_override = Some(args[i].clone());
                }
            }
            _ => {
                if a.starts_with('-') {
                    eprintln!("unknown option: {a}\nTry --help");
                    return Ok(());
                }
                file = Some(a.clone());
            }
        }
        i += 1;
    }

    if print_theme_list {
        list_themes();
        return Ok(());
    }

    // Load config; apply CLI overrides.
    let mut cfg = config::load();
    if let Some(t) = theme_override {
        cfg.theme = t;
    }
    if edit {
        cfg.default_view = View::Edit;
    }

    let path = file.map(std::path::PathBuf::from);

    // Forward macOS "open documents" requests (Finder double-click, `open`,
    // Dock drop) into the app. Must be installed before `run_native`: AppKit
    // may deliver launch-time open requests before the first frame.
    let (open_tx, open_rx) = std::sync::mpsc::channel::<std::path::PathBuf>();
    macos::install_open_file_handler(open_tx);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("mdbijou")
        .with_inner_size([1024.0, 760.0])
        .with_min_inner_size([560.0, 420.0]);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let cfg2 = cfg.clone();
    eframe::run_native(
        "mdbijou",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(app::MdbijouApp::new(
                cc,
                cfg2,
                path.clone(),
                open_rx,
            )))
        }),
    )
}
