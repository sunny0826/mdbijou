//! Font setup: load a CJK-capable system font (e.g. PingFang SC on macOS) so
//! Chinese text renders correctly in both preview and editor views.

use egui::{FontData, FontDefinitions, FontFamily};

/// Candidate system font files for CJK, in priority order.
const CJK_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Songti.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
];

/// A selectable body (proportional) font for the preview view.
pub struct BodyFont {
    pub id: &'static str,
    pub name: &'static str,
    /// System font file; None = egui builtin default.
    pub path: Option<&'static str>,
}

/// Body font choices shown in the settings page (macOS system fonts).
pub const BODY_FONTS: &[BodyFont] = &[
    BodyFont {
        id: "default",
        name: "默认",
        path: None,
    },
    BodyFont {
        id: "pingfang",
        name: "苹方 PingFang SC",
        path: Some("/System/Library/Fonts/PingFang.ttc"),
    },
    BodyFont {
        id: "hiragino",
        name: "冬青黑体 Hiragino Sans GB",
        path: Some("/System/Library/Fonts/Hiragino Sans GB.ttc"),
    },
    BodyFont {
        id: "songti",
        name: "宋体 Songti SC",
        path: Some("/System/Library/Fonts/Supplemental/Songti.ttc"),
    },
    BodyFont {
        id: "heiti",
        name: "黑体 STHeiti",
        path: Some("/System/Library/Fonts/STHeiti Light.ttc"),
    },
];

/// Build the full `FontDefinitions`: the chosen body font first in the
/// proportional family, then the CJK fallback, then the Phosphor icon font.
pub fn build_fonts(body_id: &str) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    if let Some(bf) = BODY_FONTS.iter().find(|f| f.id == body_id) {
        if let Some(path) = bf.path {
            if let Ok(bytes) = std::fs::read(path) {
                if bytes.len() >= 1000 {
                    let data = std::sync::Arc::new(FontData::from_owned(bytes));
                    fonts.font_data.insert("body".to_string(), data);
                    fonts
                        .families
                        .entry(FontFamily::Proportional)
                        .or_default()
                        .insert(0, "body".to_string());
                }
            }
        }
    }

    install_cjk_fonts(&mut fonts);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts
}

/// Build `FontDefinitions` with a CJK fallback layered under the default and
/// monospace families. Returns None when no CJK font is available.
pub fn install_cjk_fonts(fonts: &mut FontDefinitions) -> Option<()> {
    for path in CJK_CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            // egui parses lazily; guard against a partial/corrupt file.
            if bytes.len() < 1000 {
                continue;
            }
            let name = format!(
                "cjk_{}",
                std::path::Path::new(path).file_name()?.to_string_lossy()
            );
            let data = std::sync::Arc::new(FontData::from_owned(bytes));
            fonts.font_data.insert(name.clone(), data);
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.clone());
            }
            return Some(());
        }
    }
    None
}
