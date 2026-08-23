//! Mermaid diagram rendering via `merman` (headless Mermaid.js-parity
//! renderer, pinned to mermaid@11.12.3) → SVG. The active UI backend owns
//! image presentation; this module only owns diagram semantics and styling.

use crate::color::Color;
use crate::theme::{Theme, ThemeKind};
use std::hash::{Hash, Hasher};

fn cache_key(src: &str, theme: &Theme, font_size: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    theme.id.hash(&mut h);
    font_size.to_bits().hash(&mut h);
    h.finish()
}

/// Render the source to a resvg-safe SVG string via merman. Returns `None`
/// when the source is not a recognized diagram or fails to parse.
/// Produce Mermaid SVG for a renderer that owns its own image backend.
pub fn render_svg(src: &str, theme: &Theme, font_size: f32) -> Option<String> {
    let cfg = theme_config(theme, font_size);
    let renderer = merman::render::HeadlessRenderer::new()
        .with_site_config(cfg)
        .with_diagram_id(&format!("mermaid-{:x}", cache_key(src, theme, font_size)));
    renderer
        .render_svg_resvg_safe_sync(src)
        .ok()
        .flatten()
        .map(|svg| strip_svg_background(&svg))
}

/// Rasterize a Mermaid diagram for image backends that cannot reliably paint
/// every SVG feature emitted by Mermaid (notably text labels and filters).
/// A transparent PNG keeps the card and document theme visible underneath.
pub fn render_png(src: &str, theme: &Theme, font_size: f32) -> Option<Vec<u8>> {
    let svg = render_svg(src, theme, font_size)?;
    let options = merman::render::raster::RasterOptions {
        // The document renderer can scale diagrams to the reading column, so
        // keep sufficient source detail for Retina displays and wide layouts.
        scale: 3.0,
        background: None,
        ..Default::default()
    };
    merman::render::raster::svg_to_png(&svg, &options).ok()
}

/// Mermaid hardcodes `background-color: white` on the SVG root (parity with
/// mermaid.js), ignoring the theme background. Make it transparent so the
/// surrounding card (`code_bg`) shows through with its rounded corners.
fn strip_svg_background(svg: &str) -> String {
    svg.replace("background-color:white", "background-color:transparent")
        .replace("background-color: white", "background-color: transparent")
}

/// Map the app `Theme` onto Mermaid `themeVariables` so diagrams adapt to the
/// current light/dark palette. The flowchart's hardcoded white background is
/// stripped to transparent later (see `strip_svg_background`) so the
/// surrounding card (`code_bg`) shows through with its rounded corners.
fn theme_config(theme: &Theme, font_size: f32) -> merman::MermaidConfig {
    let mut cfg = merman::MermaidConfig::empty_object();
    let dark = theme.kind == ThemeKind::Dark;
    cfg.set_value(
        "theme",
        serde_json::Value::String(if dark { "dark" } else { "default" }.into()),
    );
    let mut tv = |path: &str, c: Color| {
        cfg.set_value(
            &format!("themeVariables.{path}"),
            serde_json::Value::String(color32_to_css(c)),
        );
    };
    let node_fill = mix(theme.c.background, theme.c.link, 0.08);
    tv("background", theme.c.background);
    tv("primaryColor", node_fill);
    tv("primaryTextColor", theme.c.foreground);
    tv("primaryBorderColor", theme.c.link);
    // Flowchart nodes read these specific theme variables (not the generic ones).
    tv("mainBkg", node_fill);
    tv("nodeBorder", theme.c.link);
    tv("textColor", theme.c.foreground);
    tv("lineColor", theme.c.muted);
    tv("edgeLabelBackground", theme.c.code_bg);
    cfg.set_value(
        "themeVariables.fontFamily",
        serde_json::Value::String("sans-serif".into()),
    );
    cfg.set_value(
        "themeVariables.fontSize",
        serde_json::Value::String(format!("{font_size}px")),
    );
    cfg
}

fn color32_to_css(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Linear blend of two colors by `t` in [0, 1].
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t).round() as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t).round() as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    fn light() -> Theme {
        theme::builtin("github-light").unwrap()
    }

    fn dark() -> Theme {
        theme::builtin("github-dark").unwrap()
    }

    #[test]
    fn renders_flowchart_to_svg() {
        let svg = render_svg(
            "graph TD\nA[Start] --> B{Check}\nB -->|Yes| C[Done]",
            &light(),
            16.0,
        );
        let svg = svg.expect("flowchart should render");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Start"));
    }

    #[test]
    fn renders_lr_flowchart() {
        let svg = render_svg("flowchart LR\nA --> B --> C", &dark(), 16.0);
        assert!(svg.is_some());
    }

    #[test]
    fn renders_decision_flowchart_with_round_node() {
        let src = "graph TD\n  A[Start] --> B{Condition?}\n  B -->|Yes| C(Process)\n  B -->|No| D[End]\n  C --> D";
        let svg = render_svg(src, &light(), 16.0)
            .expect("decision flowchart with a round node should render");
        assert!(svg.contains("Condition?"));
        assert!(svg.contains("Process"));
    }

    #[test]
    fn rasterizes_decision_flowchart_for_native_image_backends() {
        let src = "graph TD\n  A[Start] --> B{Condition?}\n  B -->|Yes| C(Process)\n  B -->|No| D[End]\n  C --> D";
        let png =
            render_png(src, &light(), 16.0).expect("decision flowchart should rasterize for GPUI");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn rejects_non_mermaid() {
        assert!(render_svg("let x = 1;", &light(), 16.0).is_none());
        assert!(render_svg("", &light(), 16.0).is_none());
        assert!(render_svg("   \n  ", &light(), 16.0).is_none());
    }

    #[test]
    fn theme_config_adapts_to_dark() {
        let cfg = theme_config(&dark(), 16.0);
        assert_eq!(cfg.get_str("theme"), Some("dark"));
        assert_eq!(cfg.get_str("themeVariables.textColor"), Some("#c9d1d9"));
        assert_eq!(cfg.get_str("themeVariables.nodeBorder"), Some("#58a6ff"));
        assert_eq!(cfg.get_str("themeVariables.background"), Some("#0d1117"));
        assert_eq!(cfg.get_str("themeVariables.fontSize"), Some("16px"));
    }

    #[test]
    fn theme_config_adapts_to_light() {
        let cfg = theme_config(&light(), 14.0);
        assert_eq!(cfg.get_str("theme"), Some("default"));
        assert_eq!(cfg.get_str("themeVariables.textColor"), Some("#24292e"));
        assert_eq!(cfg.get_str("themeVariables.fontSize"), Some("14px"));
    }

    #[test]
    fn strips_hardcoded_svg_background() {
        assert_eq!(
            strip_svg_background(r#"<svg style="max-width:130px;background-color:white">"#),
            r#"<svg style="max-width:130px;background-color:transparent">"#
        );
        assert_eq!(
            strip_svg_background(r#"<svg style="background-color: white">"#),
            r#"<svg style="background-color: transparent">"#
        );
    }

    #[test]
    fn renders_cjk_flowchart() {
        let src = "graph TD\n  A[开始] --> B{判断}\n  B -->|是| C(处理)\n  C --> D[结束]";
        let svg = render_svg(src, &light(), 16.0).expect("flowchart should render");
        assert!(svg.contains("开始"));
    }
}
