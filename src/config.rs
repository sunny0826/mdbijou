//! Configuration: load/save `~/.config/mdbijou/config.toml`.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    /// Body font for the preview view (id from fonts::BODY_FONTS).
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub content_width: f32,
    pub follow_system_theme: bool,
    // editor (v0.2)
    pub default_view: View,
    pub editor_font_size: f32,
    pub show_line_numbers: bool,
    pub highlight: bool,
    pub tab_size: usize,
    pub auto_save: bool,
    pub show_status_bar: bool,
    /// Show the table-of-contents sidebar (wide windows) / drawer (narrow).
    pub show_toc: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Preview,
    Edit,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "github-light".into(),
            font_family: "default".into(),
            font_size: 16.0,
            line_height: 1.5,
            content_width: 720.0,
            follow_system_theme: false,
            default_view: View::Preview,
            editor_font_size: 15.0,
            show_line_numbers: true,
            highlight: true,
            tab_size: 4,
            auto_save: false,
            show_status_bar: true,
            show_toc: false,
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("dev", "mdbijou", "mdbijou")
        .map(|d| d.config_dir().to_path_buf())
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(|p| PathBuf::from(p).join("mdbijou")))
}

pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_file() else {
        return Config::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    toml::from_str(&raw).unwrap_or_default()
}

pub fn save(cfg: &Config) {
    let Some(path) = config_file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = toml::to_string_pretty(cfg) {
        let _ = atomic_write(&path, s.as_bytes());
    }
}

/// Write atomically (temp file + rename) to avoid partial writes / watch loops.
pub fn atomic_write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
    ));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
