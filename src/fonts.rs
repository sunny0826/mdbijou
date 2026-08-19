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
            match std::fs::read(path) {
                Ok(bytes) if bytes.len() >= 1000 => {
                    let data = std::sync::Arc::new(FontData::from_owned(bytes));
                    fonts.font_data.insert("body".to_string(), data);
                    fonts
                        .families
                        .entry(FontFamily::Proportional)
                        .or_default()
                        .insert(0, "body".to_string());
                }
                Ok(_) => log::warn!("body font {path:?} is too small to be a valid font"),
                Err(err) => log::warn!("failed to read body font {path:?}: {err}"),
            }
        }
    }

    match install_cjk_fonts(&mut fonts) {
        Some(()) => {
            // Belt-and-suspenders: the CJK fallback must be registered in
            // BOTH families (including Monospace, which the editor uses via
            // `FontId::monospace`), or full-width punctuation renders as tofu.
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                if !family_has_cjk(&fonts, &family) {
                    log::warn!(
                        "CJK font was loaded but not registered in {family:?}; \
                         Chinese text and full-width punctuation may not render"
                    );
                }
            }
        }
        None => {
            // Not fatal: the app still runs, but CJK text (including full-width
            // punctuation) will render as tofu. Surface it instead of silently
            // swallowing the failure so the user knows to install a CJK font.
            log::warn!(
                "no CJK font could be loaded from candidates {CJK_CANDIDATES:?}; \
                 install a CJK-capable system font (e.g. PingFang SC / STHeiti) \
                 so Chinese text and full-width punctuation render in the editor"
            );
        }
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts
}

/// Whether `family` in `defs` includes a loaded `cjk_*` fallback font.
fn family_has_cjk(defs: &FontDefinitions, family: &FontFamily) -> bool {
    defs.families
        .get(family)
        .map(|names| names.iter().any(|n| n.starts_with("cjk_")))
        .unwrap_or(false)
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
            // Append as LOWEST-priority fallback (after the default
            // monospace/proportional fonts). CJK must NOT go first: the CJK
            // system fonts carry proportional Latin glyphs, so inserting them
            // at the head of `Monospace` would break ASCII column alignment.
            // epaint walks the family list in order and falls through per
            // glyph (glyph_id 0 -> next font), so appending resolves
            // full-width punctuation without disturbing Latin/monospace text.
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.clone());
            }
            log::info!("CJK fallback font loaded from {path:?}");
            return Some(());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::FontId;

    #[test]
    fn install_cjk_fonts_adds_fallback_to_both_families() {
        let mut fonts = FontDefinitions::default();
        if install_cjk_fonts(&mut fonts).is_some() {
            assert!(family_has_cjk(&fonts, &FontFamily::Proportional));
            assert!(family_has_cjk(&fonts, &FontFamily::Monospace));
        }
    }

    #[test]
    fn build_fonts_monospace_retains_cjk_fallback() {
        // The editor renders via `FontId::monospace`, so the CJK fallback must
        // survive the full `build_fonts` pipeline (body font + phosphor icons).
        let mut probe = FontDefinitions::default();
        if install_cjk_fonts(&mut probe).is_some() {
            let defs = build_fonts("default");
            assert!(family_has_cjk(&defs, &FontFamily::Proportional));
            assert!(family_has_cjk(&defs, &FontFamily::Monospace));
        }
    }

    #[test]
    fn monospace_resolves_fullwidth_punctuation_glyphs() {
        let ctx = egui::Context::default();
        ctx.set_fonts(build_fonts("default"));
        let _ = ctx.run(egui::RawInput::default(), |_| {}); // init fonts backend
        let punct = "，。、；：！？（）【】《》“”‘’";
        let ok = ctx.fonts_mut(|f| f.has_glyphs(&FontId::monospace(14.0), punct));
        assert!(
            ok,
            "Monospace family must resolve full-width punctuation {punct:?}"
        );
    }
}
