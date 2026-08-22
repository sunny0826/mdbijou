//! mdbijou — lightweight native Markdown and MDX reader + simple editor.
//!
//! Usage:
//!   mdbijou [options] [file.md|file.mdx]
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
mod file_types;
mod fonts;
mod highlight;
mod html;
mod images;
mod install;
mod macos;
mod mermaid;
mod render;
mod theme;
mod toc;

use config::View;

fn print_help() {
    let themes: Vec<String> = theme::ThemeRegistry::new()
        .themes
        .iter()
        .map(|t| t.id.clone())
        .collect();
    let theme_list = themes.join("|");
    println!(
        "mdbijou {} — lightweight Markdown and MDX reader + simple editor\n\
         \n\
         USAGE:\n\
         \x20 mdbijou [OPTIONS] [FILE.md|FILE.mdx]\n\
         \n\
         OPTIONS:\n\
         \x20 --edit            open directly in the editor view\n\
         \x20 --theme <id>      use a specific theme ({})\n\
         \x20 --list-themes     list builtin themes and exit\n\
         \x20 --help            show this help\n\
         \x20 --version         show version\n",
        env!("CARGO_PKG_VERSION"),
        theme_list
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

fn validate_cli_path(
    path: Option<std::path::PathBuf>,
) -> Result<Option<std::path::PathBuf>, String> {
    let Some(path) = path else {
        return Ok(None);
    };

    match path.try_exists() {
        Ok(true) => Ok(Some(path)),
        Ok(false) => Err(format!("file not found: {}", path.display())),
        Err(err) => Err(format!("cannot access '{}': {err}", path.display())),
    }
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

    let path = file
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from);
    let path = match validate_cli_path(path) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };

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

#[cfg(test)]
mod tests {
    use super::validate_cli_path;
    use std::path::PathBuf;

    #[test]
    fn rejects_missing_cli_file_before_app_launch() {
        let path = std::env::temp_dir().join(format!(
            "mdbijou-missing-cli-file-{}-{}",
            std::process::id(),
            env!("CARGO_PKG_VERSION")
        ));

        let err = validate_cli_path(Some(path.clone())).unwrap_err();

        assert_eq!(err, format!("file not found: {}", path.display()));
    }

    #[test]
    fn accepts_existing_cli_file() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert_eq!(validate_cli_path(Some(path.clone())), Ok(Some(path)));
    }
}
