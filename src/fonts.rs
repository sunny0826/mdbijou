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
